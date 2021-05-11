// Snowcap: Synthesizing Network-Wide Configuration Updates
// Copyright (C) 2021  Tibor Schneider
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program; if not, write to the Free Software Foundation, Inc.,
// 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.

//! Module defining an internal router with BGP functionality.

use crate::netsim::bgp::{BgpEvent, BgpRibEntry, BgpRoute, BgpSessionType};
use crate::netsim::route_map::RouteMap;
use crate::netsim::types::IgpNetwork;
use crate::netsim::{AsId, DeviceError, LinkWeight, Prefix, RouterId};
use crate::netsim::{Event, EventQueue};
use log::*;
use petgraph::algo::bellman_ford;
use std::collections::{hash_map::Iter, HashMap, HashSet};

/// Bgp Router
#[derive(Debug)]
pub struct Router {
    /// Name of the router
    name: String,
    /// ID of the router
    router_id: RouterId,
    /// AS Id of the router
    as_id: AsId,
    /// forwarding table for IGP messages
    pub(crate) igp_forwarding_table: HashMap<RouterId, Option<(RouterId, LinkWeight)>>,
    /// Static Routes for Prefixes
    pub(crate) static_routes: HashMap<Prefix, RouterId>,
    /// hashmap of all bgp sessions
    bgp_sessions: HashMap<RouterId, BgpSessionType>,
    /// Table containing all received entries. It is represented as a hashmap, mapping the prefixes
    /// to another hashmap, which maps the received router id to the entry. This way, we can store
    /// one entry for every prefix and every session.
    bgp_rib_in: HashMap<Prefix, HashMap<RouterId, BgpRibEntry>>,
    /// Table containing all selected best routes. It is represented as a hashmap, mapping the
    /// prefixes to the table entry
    bgp_rib: HashMap<Prefix, BgpRibEntry>,
    /// Table containing all exported routes, represented as a hashmap mapping the neighboring
    /// RouterId (of a BGP session) to the table entries.
    bgp_rib_out: HashMap<Prefix, HashMap<RouterId, BgpRibEntry>>,
    /// Set of known bgp prefixes
    bgp_known_prefixes: HashSet<Prefix>,
    /// BGP Route-Maps for Input
    bgp_route_maps_in: Vec<RouteMap>,
    /// BGP Route-Maps for Output
    bgp_route_maps_out: Vec<RouteMap>,
    /// Stack to undo action from event mesages. Each event processed will push a new vector onto
    /// the stack, containing all actions to perform in order to undo this event.
    undo_stack: Vec<Vec<UndoAction>>,
}

impl Clone for Router {
    fn clone(&self) -> Self {
        Router {
            name: self.name.clone(),
            router_id: self.router_id,
            as_id: self.as_id,
            igp_forwarding_table: self.igp_forwarding_table.clone(),
            static_routes: self.static_routes.clone(),
            bgp_sessions: self.bgp_sessions.clone(),
            bgp_rib_in: self.bgp_rib_in.clone(),
            bgp_rib: self.bgp_rib.clone(),
            bgp_rib_out: self.bgp_rib_out.clone(),
            bgp_known_prefixes: self.bgp_known_prefixes.clone(),
            bgp_route_maps_in: self.bgp_route_maps_in.clone(),
            bgp_route_maps_out: self.bgp_route_maps_out.clone(),
            undo_stack: Vec::new(),
        }
    }
}

impl Router {
    pub(crate) fn new(name: String, router_id: RouterId, as_id: AsId) -> Router {
        Router {
            name,
            router_id,
            as_id,
            igp_forwarding_table: HashMap::new(),
            static_routes: HashMap::new(),
            bgp_sessions: HashMap::new(),
            bgp_rib_in: HashMap::new(),
            bgp_rib: HashMap::new(),
            bgp_rib_out: HashMap::new(),
            bgp_known_prefixes: HashSet::new(),
            bgp_route_maps_in: Vec::new(),
            bgp_route_maps_out: Vec::new(),
            undo_stack: Vec::new(),
        }
    }

    /// Return the idx of the Router
    pub fn router_id(&self) -> RouterId {
        self.router_id
    }

    /// Return the name of the Router
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Return the AS ID of the Router
    pub fn as_id(&self) -> AsId {
        self.as_id
    }

    /// Returns the IGP Forwarding table. The table maps the ID of every router in the network to
    /// a tuple `(next_hop, cost)` of the next hop on the path and the cost to reach the
    /// destination.
    pub fn get_igp_fw_table(&self) -> &HashMap<RouterId, Option<(RouterId, LinkWeight)>> {
        &self.igp_forwarding_table
    }

    /// handle an `Event`, and enqueue several resulting events. Returns Ok(true) if the forwarding
    /// state changes. Returns Ok(false) if the forwarding state is unchanged
    pub(crate) fn handle_event(
        &mut self,
        event: Event,
        queue: &mut EventQueue,
        parent_event_id: usize,
    ) -> Result<bool, DeviceError> {
        // since we need to handle an event, we must push a new empty element to the undo stack
        self.undo_stack.push(Vec::new());
        match event {
            Event::Bgp(from, to, bgp_event) if to == self.router_id => {
                // first, check if the event was received from a bgp peer
                if !self.bgp_sessions.contains_key(&from) {
                    debug!("Received a bgp event form a non-neighbor! Ignore event!");
                    return Ok(false);
                }
                // phase 1 of BGP protocol
                let prefix = match bgp_event {
                    BgpEvent::Update(route) => self.insert_bgp_route(route, from)?,
                    BgpEvent::Withdraw(prefix) => self.remove_bgp_route(prefix, from),
                };
                if self.bgp_known_prefixes.insert(prefix) {
                    // value was not present. Add to the stack
                    self.undo_stack.last_mut().unwrap().push(UndoAction::RemoveKnownPrefix(prefix));
                };
                // phase 2
                let previous_next_hop = self.get_next_hop(prefix);
                self.run_bgp_decision_process_for_prefix(prefix)?;
                let new_next_hop = self.get_next_hop(prefix);
                // phase 3
                self.run_bgp_route_dissemination_for_prefix(prefix, queue, parent_event_id)?;
                // return wether the forwarding state has changed
                Ok(previous_next_hop != new_next_hop)
                // alternative version: check if the last undo stack frame is not empty
                // Ok(!self.undo_stack.last().unwrap().is_empty())
            }
            _ => Ok(false),
        }
    }

    /// Check if something would happen when the event would be processed by this device
    #[allow(dead_code)]
    pub(crate) fn peek_event(&self, event: &Event) -> Result<bool, DeviceError> {
        match event {
            Event::Bgp(from, to, BgpEvent::Update(route))
                if *to == self.router_id && self.bgp_sessions.contains_key(from) =>
            {
                // would receive an update
                // process the new route and the eventual old route with the routemap
                let new_entry: Option<BgpRibEntry> =
                    self.process_bgp_rib_in_route(BgpRibEntry {
                        route: route.clone(),
                        from_type: *self.bgp_sessions.get(from).unwrap(),
                        from_id: *from,
                        to_id: None,
                        igp_cost: None,
                    })?;
                let old_entry: Option<BgpRibEntry> = self
                    .bgp_rib_in
                    // get the correct table for the prefix
                    .get(&route.prefix)
                    // if the table exists, get the entry
                    .and_then(|table| table.get(from))
                    // if the table and the entry exists, process the event. output type:
                    // Option<Result<Option<_>, _>>
                    .map(|entry| self.process_bgp_rib_in_route(entry.clone()))
                    // then, we transpose, to get form Option<Result<Option<_>, _>> to Result<Option<Option<_>>, _>
                    .transpose()?
                    .flatten();

                // check all possible cases of the current best route, and the route before and after
                match (new_entry, old_entry, self.bgp_rib.get(&route.prefix)) {
                    // best route must be present if the old entry is also present
                    (_, Some(_), None) => {
                        unreachable!();
                    }
                    // no new route exported to forwarding table
                    (None, None, _) => Ok(false),
                    // if old is the same as the best, but new is None, then something will change
                    (None, Some(o), Some(b)) if o.from_id == b.from_id => Ok(true),
                    // if old is different as the best, and new is None, then nothing will change
                    (None, Some(_), Some(_)) => Ok(false),
                    // if new is some, and best is none, then we will have a new route
                    (Some(_), _, None) => Ok(true),
                    // if new is some, and old is some, and they are equal, then nothing will change
                    (Some(n), Some(o), _) if n == o => Ok(false),
                    // if new and old are both some, but different, and old is selected, then
                    // something will change
                    (Some(_), Some(o), Some(b)) if o.from_id == b.from_id => Ok(true),
                    // If the new route is better than the best route (and everything above does not
                    // hold), then something will change
                    (Some(n), _, Some(b)) if &n > b => Ok(true),
                    // in the final case, nothing will change
                    _ => Ok(false),
                }
            }
            Event::Bgp(from, to, BgpEvent::Withdraw(prefix))
                if *to == self.router_id && self.bgp_sessions.contains_key(from) =>
            {
                // would receive a withdraw
                let old_entry: Option<BgpRibEntry> = self
                    .bgp_rib_in
                    // get the correct table for the prefix
                    .get(&prefix)
                    // if the table exists, get the entry
                    .and_then(|table| table.get(from))
                    // if the table and the entry exists, process the event. output type:
                    // Option<Result<Option<_>, _>>
                    .map(|entry| self.process_bgp_rib_in_route(entry.clone()))
                    // then, we transpose, to get form Option<Result<Option<_>, _>> to Result<Option<Option<_>>, _>
                    .transpose()?
                    .flatten();
                match (old_entry, self.bgp_rib.get(&prefix)) {
                    // best route must be present if the old entry is also present
                    (Some(_), None) => {
                        unreachable!();
                    }
                    // if old is the same as the best, but new is None, then something will change
                    (Some(o), Some(b)) if o.from_id == b.from_id => Ok(true),
                    // in all other cases, nothing will change
                    _ => Ok(false),
                }
            }
            _ => Ok(false),
        }
    }

    /// Undo the last call to `self.handle_event`
    pub(crate) fn undo_last_event(&mut self) -> Result<(), DeviceError> {
        for action in self.undo_stack.pop().ok_or(DeviceError::UndoStackEmpty)? {
            match action {
                UndoAction::UpdateBgpRibIn(prefix, neighbor, entry) => {
                    match self.bgp_rib_in.get_mut(&prefix) {
                        Some(table) => table,
                        None => {
                            self.bgp_rib_in.insert(prefix, HashMap::new());
                            self.bgp_rib_in.get_mut(&prefix).unwrap()
                        }
                    }
                    .insert(neighbor, entry);
                }
                UndoAction::RemoveBgpRibIn(prefix, neighbor) => {
                    self.bgp_rib_in
                        .get_mut(&prefix)
                        .ok_or(DeviceError::UndoStackError("Prefix not existing in BGP RIB IN"))?
                        .remove(&neighbor)
                        .ok_or(DeviceError::UndoStackError("Entry in BGP RIB IN does not exist"))?;
                }
                UndoAction::UpdateBgpRib(prefix, entry) => {
                    self.bgp_rib.insert(prefix, entry);
                }
                UndoAction::RemoveBgpRib(prefix) => {
                    self.bgp_rib
                        .remove(&prefix)
                        .ok_or(DeviceError::UndoStackError("Entry in BGP RIB does not exist"))?;
                }
                UndoAction::UpdateBgpRibOut(prefix, neighbor, entry) => {
                    match self.bgp_rib_out.get_mut(&prefix) {
                        Some(table) => table,
                        None => {
                            self.bgp_rib_out.insert(prefix, HashMap::new());
                            self.bgp_rib_out.get_mut(&prefix).unwrap()
                        }
                    }
                    .insert(neighbor, entry);
                }
                UndoAction::RemoveBgpRibOut(prefix, neighbor) => {
                    self.bgp_rib_out
                        .get_mut(&prefix)
                        .ok_or(DeviceError::UndoStackError("Prefix not existing in BGP RIB OUT"))?
                        .remove(&neighbor)
                        .ok_or(DeviceError::UndoStackError(
                            "Entry in BGP RIB OUT does not exist",
                        ))?;
                }
                UndoAction::RemoveKnownPrefix(prefix) => {
                    if !self.bgp_known_prefixes.remove(&prefix) {
                        return Err(DeviceError::UndoStackError(
                            "Prefix was not known previously!",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    // Clears the undo stack
    pub(crate) fn clear_undo_stack(&mut self) {
        self.undo_stack.clear();
    }

    /// Get the IGP next hop for a prefix
    pub fn get_next_hop(&self, prefix: Prefix) -> Option<RouterId> {
        // first, check the static routes
        if let Some(target) = self.static_routes.get(&prefix) {
            return Some(*target);
        };
        // then, check the bgp table
        match self.bgp_rib.get(&prefix) {
            Some(entry) => {
                self.igp_forwarding_table.get(&entry.route.next_hop).unwrap().map(|e| e.0)
            }
            None => None,
        }
    }

    /// Return a list of all known bgp routes for a given origin
    pub fn get_known_bgp_routes(&self, prefix: Prefix) -> Result<Vec<BgpRibEntry>, DeviceError> {
        let mut entries: Vec<BgpRibEntry> = Vec::new();
        if let Some(table) = self.bgp_rib_in.get(&prefix) {
            for e in table.values() {
                if let Some(new_entry) = self.process_bgp_rib_in_route(e.clone())? {
                    entries.push(new_entry);
                }
            }
        }
        Ok(entries)
    }

    /// Returns the selected bgp route for the prefix, or returns None
    pub fn get_selected_bgp_route(&self, prefix: Prefix) -> Option<BgpRibEntry> {
        self.bgp_rib.get(&prefix).cloned()
    }

    /// Add a static route. Note that the router must be a neighbor. This is not checked in this
    /// funciton.
    pub(crate) fn add_static_route(
        &mut self,
        prefix: Prefix,
        target: RouterId,
    ) -> Result<(), DeviceError> {
        match self.static_routes.insert(prefix, target) {
            None => Ok(()),
            Some(_) => Err(DeviceError::StaticRouteAlreadyExists(prefix)),
        }
    }

    /// Remove an existing static route
    pub(crate) fn remove_static_route(&mut self, prefix: Prefix) -> Result<(), DeviceError> {
        match self.static_routes.remove(&prefix) {
            Some(_) => Ok(()),
            None => Err(DeviceError::NoStaticRoute(prefix)),
        }
    }

    /// Modify a static route
    pub(crate) fn modify_static_route(
        &mut self,
        prefix: Prefix,
        target: RouterId,
    ) -> Result<(), DeviceError> {
        match self.static_routes.insert(prefix, target) {
            Some(_) => Ok(()),
            None => Err(DeviceError::NoStaticRoute(prefix)),
        }
    }

    /// establish a bgp session with a peer
    /// `session_type` tells that `target` is in relation to `self`. If `session_type` is
    /// `BgpSessionType::IbgpClient`, then the `target` is added as client to `self`. Update the
    /// bgp tables if undo is not set. If `undo` is set, undo from the undo_stack instead of
    /// updating the bgp tables.
    pub(crate) fn establish_bgp_session(
        &mut self,
        target: RouterId,
        session_type: BgpSessionType,
        queue: &mut EventQueue,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), DeviceError> {
        if self.bgp_sessions.contains_key(&target) {
            return Err(DeviceError::SessionAlreadyExists(target));
        }

        self.bgp_sessions.insert(target, session_type);

        // udpate the tables
        if undo {
            self.undo_last_event()
        } else {
            self.update_bgp_tables(queue, parent_event_id)
        }
    }

    /// Change the BGP session type and update the BGP tables. If `undo` is set, undo from the
    /// undo_stack instead of updating the bgp tables.
    pub(crate) fn modify_bgp_session(
        &mut self,
        target: RouterId,
        session_type: BgpSessionType,
        queue: &mut EventQueue,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), DeviceError> {
        match self.bgp_sessions.get_mut(&target) {
            Some(t) => {
                *t = session_type;
            }
            None => return Err(DeviceError::NoBgpSession(target)),
        }

        // udpate the tables
        if undo {
            self.undo_last_event()
        } else {
            self.update_bgp_tables(queue, parent_event_id)
        }
    }

    /// remove a bgp session and update the BGP tables. If `undo` is set, undo from the undo_stack
    /// instead of updating the bgp tables.
    pub(crate) fn close_bgp_session(
        &mut self,
        target: RouterId,
        queue: &mut EventQueue,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), DeviceError> {
        match self.bgp_sessions.remove(&target) {
            Some(_) => Ok(()),
            None => Err(DeviceError::NoBgpSession(target)),
        }?;

        if undo {
            // In case of undo, we just need to undo the last event. because any previous esssion
            // establishment did already add all the new sessions to the undo stack. and therefore,
            // we can just remove them by calling undo_last_event.
            self.undo_last_event()
        } else {
            // temporary stack to push it to the undo stack after updating
            let mut stack: Vec<UndoAction> = Vec::new();
            for prefix in self.bgp_known_prefixes.iter() {
                // remove the entry in the rib tables, and add it to the stack
                if let Some(entry) =
                    self.bgp_rib_in.get_mut(&prefix).and_then(|rib| rib.remove(&target))
                {
                    stack.push(UndoAction::UpdateBgpRibIn(*prefix, target, entry));
                }
                if let Some(entry) =
                    self.bgp_rib_out.get_mut(&prefix).and_then(|rib| rib.remove(&target))
                {
                    stack.push(UndoAction::UpdateBgpRibOut(*prefix, target, entry));
                }
            }

            self.update_bgp_tables(queue, parent_event_id)?;
            self.undo_stack.last_mut().unwrap().append(&mut stack);
            Ok(())
        }
    }

    /// Returns an interator over all BGP sessions
    pub fn get_bgp_sessions(&self) -> Iter<'_, RouterId, BgpSessionType> {
        self.bgp_sessions.iter()
    }

    /// Returns the bgp session type.
    #[allow(dead_code)]
    pub(crate) fn get_bgp_session_type(&self, neighbor: RouterId) -> Option<BgpSessionType> {
        self.bgp_sessions.get(&neighbor).copied()
    }

    /// Add a route-map for the input and update the BGP tables. If `undo` is set, undo from the
    /// undo_stack instead of updating the bgp tables.
    pub(crate) fn add_bgp_route_map_in(
        &mut self,
        map: RouteMap,
        queue: &mut EventQueue,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), DeviceError> {
        // check that the order doesn't yet exist
        match self.bgp_route_maps_in.binary_search_by(|probe| probe.order.cmp(&map.order)) {
            Ok(_) => return Err(DeviceError::BgpRouteMapAlreadyExists(map.order)),
            Err(pos) => {
                self.bgp_route_maps_in.insert(pos, map);
            }
        }
        if undo {
            self.undo_last_event()
        } else {
            self.update_bgp_tables(queue, parent_event_id)
        }
    }

    /// Remove a route-map for the input and update the BGP tables. If `undo` is set, undo from the
    /// undo_stack instead of updating the bgp tables.
    pub(crate) fn remove_bgp_route_map_in(
        &mut self,
        order: usize,
        queue: &mut EventQueue,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), DeviceError> {
        match self.bgp_route_maps_in.binary_search_by(|probe| probe.order.cmp(&order)) {
            Ok(pos) => {
                self.bgp_route_maps_in.remove(pos);
            }
            Err(_) => return Err(DeviceError::NoBgpRouteMap(order)),
        }
        if undo {
            self.undo_last_event()
        } else {
            self.update_bgp_tables(queue, parent_event_id)
        }
    }

    /// Modify an existing route-map for the input and update the BGP tables. If `undo` is set, undo
    /// from the undo_stack instead of updating the bgp tables.
    pub(crate) fn modify_bgp_route_map_in(
        &mut self,
        order: usize,
        map: RouteMap,
        queue: &mut EventQueue,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), DeviceError> {
        match self.bgp_route_maps_in.binary_search_by(|probe| probe.order.cmp(&order)) {
            Ok(pos) if order == map.order => {
                self.bgp_route_maps_in[pos] = map;
            }
            Ok(pos) => {
                self.bgp_route_maps_in.remove(pos);
                // add the route map at the correct position
                match self.bgp_route_maps_in.binary_search_by(|probe| probe.order.cmp(&map.order)) {
                    Ok(_) => return Err(DeviceError::BgpRouteMapAlreadyExists(map.order)),
                    Err(pos) => {
                        self.bgp_route_maps_in.insert(pos, map);
                    }
                }
            }
            Err(_) => return Err(DeviceError::NoBgpRouteMap(order)),
        }
        if undo {
            self.undo_last_event()
        } else {
            self.update_bgp_tables(queue, parent_event_id)
        }
    }

    /// Add a route-map for the output and update the BGP tables. If `undo` is set, undo from the
    /// undo_stack instead of updating the bgp tables.
    pub(crate) fn add_bgp_route_map_out(
        &mut self,
        map: RouteMap,
        queue: &mut EventQueue,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), DeviceError> {
        // check that the order doesn't yet exist
        match self.bgp_route_maps_out.binary_search_by(|probe| probe.order.cmp(&map.order)) {
            Ok(_) => return Err(DeviceError::BgpRouteMapAlreadyExists(map.order)),
            Err(pos) => {
                self.bgp_route_maps_out.insert(pos, map);
            }
        }
        if undo {
            self.undo_last_event()
        } else {
            self.update_bgp_tables(queue, parent_event_id)
        }
    }

    /// Remove a route-map for the output and update the BGP tables. If `undo` is set, undo from the
    /// undo_stack instead of updating the bgp tables.
    pub(crate) fn remove_bgp_route_map_out(
        &mut self,
        order: usize,
        queue: &mut EventQueue,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), DeviceError> {
        match self.bgp_route_maps_out.binary_search_by(|probe| probe.order.cmp(&order)) {
            Ok(pos) => {
                self.bgp_route_maps_out.remove(pos);
            }
            Err(_) => return Err(DeviceError::NoBgpRouteMap(order)),
        }
        if undo {
            self.undo_last_event()
        } else {
            self.update_bgp_tables(queue, parent_event_id)
        }
    }

    /// Modify an existing route-map for the output, and update the BGP tables. If `undo` is set,
    /// undo from the undo_stack instead of updating the bgp tables.
    pub(crate) fn modify_bgp_route_map_out(
        &mut self,
        order: usize,
        map: RouteMap,
        queue: &mut EventQueue,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), DeviceError> {
        match self.bgp_route_maps_out.binary_search_by(|probe| probe.order.cmp(&order)) {
            Ok(pos) if order == map.order => {
                self.bgp_route_maps_out[pos] = map;
            }
            Ok(pos) => {
                self.bgp_route_maps_out.remove(pos);
                match self.bgp_route_maps_out.binary_search_by(|probe| probe.order.cmp(&map.order))
                {
                    Ok(_) => return Err(DeviceError::BgpRouteMapAlreadyExists(map.order)),
                    Err(pos) => {
                        self.bgp_route_maps_out.insert(pos, map);
                    }
                }
            }
            Err(_) => return Err(DeviceError::NoBgpRouteMap(order)),
        }
        if undo {
            self.undo_last_event()
        } else {
            self.update_bgp_tables(queue, parent_event_id)
        }
    }

    /// write forawrding table based on graph
    /// This function requres that all RouterIds are set to the GraphId, and update the BGP tables
    pub(crate) fn write_igp_forwarding_table(
        &mut self,
        graph: &IgpNetwork,
        queue: &mut EventQueue,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), DeviceError> {
        // clear the forwarding table
        self.igp_forwarding_table = HashMap::new();
        // compute shortest path to all other nodes in the graph
        let (path_weights, predecessors) = bellman_ford(graph, self.router_id).unwrap();
        let mut paths: Vec<(RouterId, LinkWeight, Option<RouterId>)> = path_weights
            .into_iter()
            .zip(predecessors.into_iter())
            .enumerate()
            .map(|(i, (w, p))| ((i as u32).into(), w, p))
            .collect();
        paths.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        for (router, cost, predecessor) in paths {
            if cost.is_infinite() {
                self.igp_forwarding_table.insert(router, None);
                continue;
            }
            let next_hop = if let Some(predecessor) = predecessor {
                // the predecessor must already be inserted into the forwarding table, because we sorted the table
                if predecessor == self.router_id {
                    router
                } else {
                    self.igp_forwarding_table
                        .get(&predecessor)
                        .unwrap() // first unwrap for get, which returns an option
                        .unwrap() // second unwrap to unwrap wether the route exists (it must!)
                        .0
                }
            } else {
                router
            };
            self.igp_forwarding_table.insert(router, Some((next_hop, cost)));
        }
        if undo {
            self.undo_last_event()
        } else {
            self.update_bgp_tables(queue, parent_event_id)
        }
    }

    /// Update the bgp tables only, This funciton also causes the undo stack to be created.
    fn update_bgp_tables(
        &mut self,
        queue: &mut EventQueue,
        parent_event_id: usize,
    ) -> Result<(), DeviceError> {
        // first, push an element to the stack
        self.undo_stack.push(Vec::new());
        // run the decision process
        for prefix in self.bgp_known_prefixes.clone() {
            self.run_bgp_decision_process_for_prefix(prefix)?
        }
        // run the route dissemination
        for prefix in self.bgp_known_prefixes.clone() {
            self.run_bgp_route_dissemination_for_prefix(prefix, queue, parent_event_id)?
        }
        Ok(())
    }

    /// This function checks if all BGP tables are the same for all prefixes
    pub(crate) fn compare_bgp_table(&self, other: &Self) -> bool {
        if self.bgp_rib != other.bgp_rib {
            return false;
        }
        for prefix in self.bgp_known_prefixes.union(&other.bgp_known_prefixes) {
            match (self.bgp_rib_in.get(prefix), other.bgp_rib_in.get(prefix)) {
                (Some(x), None) if !x.is_empty() => return false,
                (None, Some(x)) if !x.is_empty() => return false,
                (Some(a), Some(b)) if a != b => return false,
                _ => {}
            }
            match (self.bgp_rib_out.get(prefix), other.bgp_rib_out.get(prefix)) {
                (Some(x), None) if !x.is_empty() => return false,
                (None, Some(x)) if !x.is_empty() => return false,
                (Some(a), Some(b)) if a != b => return false,
                _ => {}
            }
        }
        true
    }

    // -----------------
    // Private Functions
    // -----------------

    /// only run bgp decision process (phase 2)
    fn run_bgp_decision_process_for_prefix(&mut self, prefix: Prefix) -> Result<(), DeviceError> {
        // search the best route and compare
        let old_entry = self.bgp_rib.get(&prefix);
        let mut new_entry = None;

        // find the new best route
        if let Some(rib_in) = self.bgp_rib_in.get(&prefix) {
            for entry_unprocessed in rib_in.values() {
                let entry = match self.process_bgp_rib_in_route(entry_unprocessed.clone())? {
                    Some(e) => e,
                    None => continue,
                };
                let mut better = true;
                if let Some(current_best) = new_entry.as_ref() {
                    better = &entry > current_best;
                }
                if better {
                    new_entry = Some(entry)
                }
            }
        }

        // check if the entry will get changed
        if new_entry.as_ref() != old_entry {
            // replace the entry
            if let Some(new_entry) = new_entry {
                // insert the new entry, and add the change to the undo stack
                match self.bgp_rib.insert(prefix, new_entry) {
                    Some(old_entry) => self
                        .undo_stack
                        .last_mut()
                        .unwrap()
                        .push(UndoAction::UpdateBgpRib(prefix, old_entry)),
                    None => {
                        self.undo_stack.last_mut().unwrap().push(UndoAction::RemoveBgpRib(prefix))
                    }
                };
            } else if let Some(old_entry) = self.bgp_rib.remove(&prefix) {
                self.undo_stack
                    .last_mut()
                    .unwrap()
                    .push(UndoAction::UpdateBgpRib(prefix, old_entry));
            }
        }
        Ok(())
    }

    /// only run bgp route dissemination (phase 3)
    fn run_bgp_route_dissemination_for_prefix(
        &mut self,
        prefix: Prefix,
        queue: &mut EventQueue,
        parent_event_id: usize,
    ) -> Result<(), DeviceError> {
        self.bgp_rib_out.entry(prefix).or_default();

        for (peer, peer_type) in self.bgp_sessions.iter() {
            // apply the route for the specific peer
            let best_route: Option<BgpRibEntry> = self
                .bgp_rib
                .get(&prefix)
                .map(|e| self.process_bgp_rib_out_route(e.clone(), *peer))
                .transpose()?
                .flatten();
            // check if the current information is the same
            let current_route: Option<&BgpRibEntry> =
                self.bgp_rib_out.get_mut(&prefix).and_then(|rib| rib.get(peer));
            let event = match (best_route, current_route) {
                (Some(best_r), Some(current_r)) if best_r.route == current_r.route => {
                    // Nothing to do, no new route received
                    None
                }
                (Some(best_r), Some(_)) => {
                    // Route information was changed
                    if self.should_export_route(best_r.from_id, *peer, *peer_type)? {
                        // update the route
                        let old_entry = self
                            .bgp_rib_out
                            .get_mut(&prefix)
                            .and_then(|rib| rib.insert(*peer, best_r.clone()))
                            .unwrap();
                        // push the update to the undo stack
                        self.undo_stack
                            .last_mut()
                            .unwrap()
                            .push(UndoAction::UpdateBgpRibOut(prefix, *peer, old_entry));
                        Some(BgpEvent::Update(best_r.route))
                    } else {
                        // send a withdraw of the old route.
                        let old_entry = self
                            .bgp_rib_out
                            .get_mut(&prefix)
                            .and_then(|rib| rib.remove(&peer))
                            .unwrap();
                        // push the update to the undo stack
                        self.undo_stack
                            .last_mut()
                            .unwrap()
                            .push(UndoAction::UpdateBgpRibOut(prefix, *peer, old_entry));
                        Some(BgpEvent::Withdraw(prefix))
                    }
                }
                (Some(best_r), None) => {
                    // New route information received
                    if self.should_export_route(best_r.from_id, *peer, *peer_type)? {
                        // send the route, but update the undo stack accordingly
                        self.bgp_rib_out
                            .get_mut(&prefix)
                            .and_then(|rib| rib.insert(*peer, best_r.clone()));
                        // push the remove action to the stack, because there was no route before.
                        self.undo_stack
                            .last_mut()
                            .unwrap()
                            .push(UndoAction::RemoveBgpRibOut(prefix, *peer));
                        Some(BgpEvent::Update(best_r.route))
                    } else {
                        None
                    }
                }
                (None, Some(_)) => {
                    // Current route must be WITHDRAWN, since we do no longer know any route
                    let old_entry = self
                        .bgp_rib_out
                        .get_mut(&prefix)
                        .and_then(|rib| rib.remove(&peer))
                        .unwrap();
                    // push the update action to the undo stack
                    self.undo_stack
                        .last_mut()
                        .unwrap()
                        .push(UndoAction::UpdateBgpRibOut(prefix, *peer, old_entry));
                    Some(BgpEvent::Withdraw(prefix))
                }
                (None, None) => {
                    // Nothing to do
                    None
                }
            };
            // add the event to the queue
            if let Some(event) = event {
                queue.push_back((Event::Bgp(self.router_id, *peer, event), parent_event_id));
            }
        }

        Ok(())
    }

    /// Tries to insert the route into the bgp_rib_in table. If the same route already exists in the table,
    /// replace the route. It returns the prefix for which the route was inserted
    fn insert_bgp_route(&mut self, route: BgpRoute, from: RouterId) -> Result<Prefix, DeviceError> {
        let from_type = *self.bgp_sessions.get(&from).ok_or(DeviceError::NoBgpSession(from))?;

        // the incoming bgp routes should not be processed here!
        // This is because when configuration chagnes, the routes should also change without needing
        // to receive them again.
        // Also, we don't yet compute the igp cost.
        let new_entry =
            BgpRibEntry { route, from_type, from_id: from, to_id: None, igp_cost: None };

        let prefix = new_entry.route.prefix;

        let rib_in = if self.bgp_rib_in.contains_key(&prefix) {
            self.bgp_rib_in.get_mut(&prefix).unwrap()
        } else {
            self.bgp_rib_in.insert(new_entry.route.prefix, HashMap::new());
            self.bgp_rib_in.get_mut(&prefix).unwrap()
        };

        // insert the new route. Also, update the undo action to be able to go back.
        match rib_in.insert(from, new_entry) {
            Some(old_entry) => self
                .undo_stack
                .last_mut()
                .unwrap()
                .push(UndoAction::UpdateBgpRibIn(prefix, from, old_entry)),
            None => {
                self.undo_stack.last_mut().unwrap().push(UndoAction::RemoveBgpRibIn(prefix, from))
            }
        }

        Ok(prefix)
    }

    /// remove an existing bgp route in bgp_rib_in and returns the prefix for which the route was
    /// inserted.
    fn remove_bgp_route(&mut self, prefix: Prefix, from: RouterId) -> Prefix {
        // check if the prefix does exist in the table. if there was an entry, then also add it to
        // the undo action.
        if let Some(old_entry) = self.bgp_rib_in.get_mut(&prefix).and_then(|rib| rib.remove(&from))
        {
            self.undo_stack
                .last_mut()
                .unwrap()
                .push(UndoAction::UpdateBgpRibIn(prefix, from, old_entry));
        }
        prefix
    }

    /// process incoming routes from bgp_rib_in
    fn process_bgp_rib_in_route(
        &self,
        mut entry: BgpRibEntry,
    ) -> Result<Option<BgpRibEntry>, DeviceError> {
        // apply bgp_route_map_in
        let mut maps = self.bgp_route_maps_in.iter();
        let mut entry = loop {
            match maps.next() {
                Some(map) => {
                    entry = match map.apply(entry) {
                        (true, Some(e)) => break e,
                        (true, None) => return Ok(None),
                        (false, Some(e)) => e,
                        (false, None) => unreachable!(),
                    }
                }
                None => break entry,
            }
        };

        // compute the igp cost
        entry.igp_cost = Some(
            entry.igp_cost.unwrap_or(
                match self
                    .igp_forwarding_table
                    .get(&entry.route.next_hop)
                    .ok_or(DeviceError::RouterNotFound(entry.route.next_hop))?
                {
                    Some((_, cost)) => *cost,
                    None => return Ok(None),
                },
            ),
        );

        // set the next hop to the egress from router if the message came from externally
        if entry.from_type.is_ebgp() {
            entry.route.next_hop = entry.from_id;
        }

        // set the default values
        entry.route.apply_default();

        // set the to_id to None
        entry.to_id = None;

        Ok(Some(entry))
    }

    /// Process a route from bgp_rib for sending it to bgp peers, and storing it into bgp_rib_out.
    /// The entry is cloned and modified
    fn process_bgp_rib_out_route(
        &self,
        mut entry: BgpRibEntry,
        target_peer: RouterId,
    ) -> Result<Option<BgpRibEntry>, DeviceError> {
        // set the to_id to the target peer
        entry.to_id = Some(target_peer);

        // apply bgp_route_map_out
        let mut maps = self.bgp_route_maps_out.iter();
        let mut entry = loop {
            match maps.next() {
                Some(map) => {
                    entry = match map.apply(entry) {
                        (true, Some(e)) => break e,
                        (true, None) => return Ok(None),
                        (false, Some(e)) => e,
                        (false, None) => unreachable!(),
                    }
                }
                None => break entry,
            }
        };

        // get the peer type
        entry.from_type =
            *self.bgp_sessions.get(&target_peer).ok_or(DeviceError::NoBgpSession(target_peer))?;

        // if the peer type is external, overwrite values of the route accordingly.
        if entry.from_type.is_ebgp() {
            entry.route.next_hop = self.router_id;
            entry.route.local_pref = None;
        }

        Ok(Some(entry))
    }

    /// returns a bool which tells to export the route to the target, which was advertised by the
    /// source.
    fn should_export_route(
        &self,
        from: RouterId,
        to: RouterId,
        to_type: BgpSessionType,
    ) -> Result<bool, DeviceError> {
        // never advertise a route to the receiver
        if from == to {
            return Ok(false);
        }
        // check the types
        let from_type = self.bgp_sessions.get(&from).ok_or(DeviceError::NoBgpSession(from))?;

        Ok(match (from_type, to_type) {
            (BgpSessionType::EBgp, _) => true,
            (BgpSessionType::IBgpClient, _) => true,
            (_, BgpSessionType::EBgp) => true,
            (_, BgpSessionType::IBgpClient) => true,
            _ => false,
        })
    }
}

#[derive(Debug)]
enum UndoAction {
    /// Undo by updating (or inserting) a BGP RIB entry in the BGP RIB IN table
    UpdateBgpRibIn(Prefix, RouterId, BgpRibEntry),
    /// Undo by removing an entry from the BGP RIB IN table
    RemoveBgpRibIn(Prefix, RouterId),
    /// Undo by updating the BGP RIB entry in the BGP RIB table
    UpdateBgpRib(Prefix, BgpRibEntry),
    /// Undo by removing an entry in the BGP RIB table
    RemoveBgpRib(Prefix),
    /// Undo by updating (or inserting) a BGP RIB entry in the BGP RIB OUT table
    UpdateBgpRibOut(Prefix, RouterId, BgpRibEntry),
    /// Undo by removing an entry from the BGP RIB OUT table
    RemoveBgpRibOut(Prefix, RouterId),
    /// Remove a known prefix, if it was not previously there.
    RemoveKnownPrefix(Prefix),
}
