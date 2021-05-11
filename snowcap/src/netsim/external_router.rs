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

//! # External Router
//!
//! The external router representa a router located in a different AS, not controlled by the network
//! operators.

use crate::netsim::bgp::{BgpEvent, BgpRoute};
use crate::netsim::event::{Event, EventQueue};
use crate::netsim::{AsId, DeviceError, Prefix, RouterId};
use std::collections::HashSet;

/// Struct representing an external router
/// NOTE: We use vectors, for both the neighbors and active routes. The reason is the following:
/// - `neighbors`: it is to be expected that there are only very few neighbors to an external
///   router (usually 1). Hence, searching through the vector will be faster than using a `HashSet`.
///   Also, cloning the External router is faster this way.
/// - `active_routes`: The main usecase of netsim is to be used in snowcap. There, we never
///   advertise new routes or withdraw them during the main iteration. Thus, this operation can be
///   a bit more expensive. However, it is to be expected that neighbors are added and removed more
///   often. In this case, we need to iterate over the `active_routes`, which is faster than using a
///   `HashMap`. Also, cloning the External Router is faster when we have a vector.
#[derive(Debug)]
pub struct ExternalRouter {
    name: String,
    router_id: RouterId,
    as_id: AsId,
    neighbors: Vec<RouterId>,
    active_routes: Vec<BgpRoute>,
    undo_stack: Vec<UndoAction>,
}

impl Clone for ExternalRouter {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            router_id: self.router_id,
            as_id: self.as_id,
            neighbors: self.neighbors.clone(),
            active_routes: self.active_routes.clone(),
            undo_stack: Vec::new(),
        }
    }
}

impl ExternalRouter {
    /// Create a new NetworkDevice instance
    pub(crate) fn new(name: String, router_id: RouterId, as_id: AsId) -> Self {
        Self {
            name,
            router_id,
            as_id,
            neighbors: Vec::new(),
            active_routes: Vec::new(),
            undo_stack: Vec::new(),
        }
    }

    /// Handle an `Event` and produce the necessary result. Always returns Ok(false), to tell that
    /// the forwarding state has not changed.
    pub(crate) fn handle_event(
        &mut self,
        _event: Event,
        _queue: &mut EventQueue,
        _parent_event_id: usize,
    ) -> Result<bool, DeviceError> {
        self.undo_stack.push(UndoAction::None);
        Ok(false)
    }

    /// Check if something would happen when the event would be processed by this device
    #[allow(dead_code)]
    pub(crate) fn peek_event(&self, _event: Event) -> Result<bool, DeviceError> {
        Ok(false)
    }

    /// Undo the last event, without triggering any events.
    pub(crate) fn undo_last_event(&mut self) -> Result<(), DeviceError> {
        match self.undo_stack.pop() {
            Some(UndoAction::AddActiveRoute(route)) => {
                if self.active_routes.iter().any(|x| x.prefix == route.prefix) {
                    return Err(DeviceError::UndoStackError(
                        "Cannot add route, it is already present!",
                    ));
                }
                self.active_routes.push(route);
            }
            Some(UndoAction::UpdateActiveRoute(route)) => {
                if let Some(pos) = self.active_routes.iter().position(|x| x.prefix == route.prefix)
                {
                    self.active_routes[pos] = route;
                } else {
                    return Err(DeviceError::UndoStackError(
                        "Cannot modify route to advertise, it is not present!",
                    ));
                }
            }
            Some(UndoAction::RemoveActiveRoute(prefix)) => {
                let pos = self.active_routes.iter().position(|x| x.prefix == prefix).ok_or(
                    DeviceError::UndoStackError(
                        "Router does not advertise any route for the prefix!",
                    ),
                )?;
                self.active_routes.remove(pos);
            }
            Some(UndoAction::None) => {}
            None => {
                println!("external router error");
                return Err(DeviceError::UndoStackEmpty);
            }
        }
        Ok(())
    }

    // Clears the undo stack
    pub(crate) fn clear_undo_stack(&mut self) {
        self.undo_stack.clear();
    }

    /// Return the ID of the network device
    pub fn router_id(&self) -> RouterId {
        self.router_id
    }

    /// Return the AS of the network device
    pub fn as_id(&self) -> AsId {
        self.as_id
    }

    /// Return the name of the network device
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Return a set of routes which are advertised
    pub fn advertised_prefixes(&self) -> HashSet<Prefix> {
        self.active_routes.iter().map(|r| r.prefix).collect()
    }

    /// Start advertizing a specific route. All neighbors (including future neighbors) will get an
    /// update message with the route.
    pub(crate) fn advertise_prefix(
        &mut self,
        prefix: Prefix,
        as_path: Vec<AsId>,
        med: Option<u32>,
        community: Option<u32>,
        queue: &mut EventQueue,
        parent_event_id: usize,
    ) -> BgpRoute {
        let route = BgpRoute {
            prefix,
            as_path,
            next_hop: self.router_id,
            local_pref: None,
            med,
            community,
        };

        let mut new_route: bool = true;
        // check wether there was already a route present with the same prefix
        for existing_route in self.active_routes.iter_mut() {
            if existing_route.prefix == route.prefix {
                new_route = false;
                self.undo_stack.push(UndoAction::UpdateActiveRoute(existing_route.clone()));
                *existing_route = route.clone();
                break;
            }
        }
        if new_route {
            self.active_routes.push(route.clone());
            self.undo_stack.push(UndoAction::RemoveActiveRoute(prefix));
        }

        // send an UPDATE to all neighbors
        let bgp_event = BgpEvent::Update(route.clone());
        for neighbor in self.neighbors.iter() {
            queue.push_back((
                Event::Bgp(self.router_id, *neighbor, bgp_event.clone()),
                parent_event_id,
            ));
        }

        route
    }

    /// Send a BGP WITHDRAW to all neighbors for the given prefix
    pub(crate) fn widthdraw_prefix(
        &mut self,
        prefix: Prefix,
        queue: &mut EventQueue,
        parent_event_id: usize,
    ) {
        // Find the position of the prefix in the vector
        // NOTE: We know that a prefix can only be in the list once, because the field
        // `active_routes` is private, and the only way to change it is through the exposed
        // methods (`advertise_prefix` and `withdraw_prefix`). In `advertise_prefix`, a new
        // route is added only when the prefix does not yet exist. Thus, routes are unique
        // in the list in terms of prefixes
        if let Some(pos) = self.active_routes.iter().position(|x| x.prefix == prefix) {
            // remove the prefix from the vector
            let old_route = self.active_routes.remove(pos);
            self.undo_stack.push(UndoAction::AddActiveRoute(old_route));

            // only send the withdraw if the route actually did exist
            for neighbor in self.neighbors.iter() {
                queue.push_back((
                    Event::Bgp(self.router_id, *neighbor, BgpEvent::Withdraw(prefix)),
                    parent_event_id,
                ));
            }
        }
    }

    /// Add an ebgp session with an internal router. Generate all events if undo is not set!
    pub(crate) fn establish_ebgp_session(
        &mut self,
        router: RouterId,
        queue: &mut EventQueue,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), DeviceError> {
        // check if the neighbor is already in the list
        if self.neighbors.contains(&router) {
            return Err(DeviceError::SessionAlreadyExists(router));
        }

        // if the session does not yet exist, push the new router into the list
        self.neighbors.push(router);

        // send all prefixes to this router
        if !undo {
            for route in self.active_routes.iter() {
                queue.push_back((
                    Event::Bgp(self.router_id, router, BgpEvent::Update(route.clone())),
                    parent_event_id,
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn close_ebgp_session(&mut self, router: RouterId) -> Result<(), DeviceError> {
        // NOTE: Since `self.neighbors` is private, and the only way to add neighbors is by calling
        // `establish_ebgp_session`, which only inserts neighbors if it isn't yet present in the
        // list, we know that the router will not be in the list more than once.
        if let Some(pos) = self.neighbors.iter().position(|&x| x == router) {
            self.neighbors.remove(pos);
            Ok(())
        } else {
            Err(DeviceError::NoBgpSession(router))
        }
    }

    /// Checks if both routers advertise the same routes.
    pub(crate) fn advertises_same_routes(&self, other: &Self) -> bool {
        self.active_routes.iter().collect::<HashSet<_>>()
            == other.active_routes.iter().collect::<HashSet<_>>()
    }

    /// Checks if the router advertises the given prefix
    pub(crate) fn has_active_route(&self, prefix: Prefix) -> bool {
        self.active_routes.iter().any(|r| r.prefix == prefix)
    }

    /// Returns a reference to all advertised routes of this router
    pub fn get_advertised_routes(&self) -> &Vec<BgpRoute> {
        &self.active_routes
    }
}

#[derive(Debug, Clone)]
enum UndoAction {
    RemoveActiveRoute(Prefix),
    AddActiveRoute(BgpRoute),
    UpdateActiveRoute(BgpRoute),
    None,
}
