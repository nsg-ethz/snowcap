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

//! # Top-level Network module
//!
//! This module represents the network topology, applies the configuration, and simulates the
//! network.

#[cfg(feature = "transient-violation")]
use crate::hard_policies::{Condition, PolicyError};
use crate::netsim::bgp::{BgpEvent, BgpSessionType};
use crate::netsim::config::{Config, ConfigExpr, ConfigModifier, ConfigPatch};
use crate::netsim::event::{Event, EventQueue};
use crate::netsim::external_router::ExternalRouter;
use crate::netsim::printer;
use crate::netsim::route_map::RouteMapDirection;
use crate::netsim::router::Router;
use crate::netsim::types::{IgpNetwork, NetworkDevice};
use crate::netsim::{
    AsId, ConfigError, ForwardingState, LinkWeight, NetworkError, Prefix, RouterId,
};

use log::*;
use petgraph::algo::FloatMeasure;
#[cfg(feature = "transient-violation")]
use rand::prelude::*;
use std::collections::{HashMap, HashSet};

static DEFAULT_STOP_AFTER: usize = 10_000;
static MAXIMUM_ALLOWED_LOOP_LEN: usize = 500;

#[derive(Debug)]
/// # Network struct
/// The struct contains all information about the underlying physical network (Links), a manages
/// all (both internal and external) routers, and handles all events between them. Configuration is
/// applied on the network itself, treated as network-wide configuration.
///
/// ## Undo Funcitonality
///
/// The Undo funcitonality is implemented by two interacting parts. First, the network keeps track
/// of all past events (in the `event_history`). Second, every network device keeps a local stack,
/// in which all information is present to undo an event.
///
/// The network is responsible for calling undo on the correct network device, since the network
/// device does not check if the event matches the one which caused the modifications in the first
/// place. This is trivial for all BGP messages exchanged, but not for configuration modifications
/// and other user interactions with the network. For changes in external advertised routes, we need
/// not to do something, except changing the external routers. For the configuraiton changes, we
/// reverse the modifier (delete becomes insert, and modify swaps the old and the new expression),
/// and call the same function as appyling a normal modifier, but set the `undo` flag to true. Then,
/// instead of updating the bgp tables on the routers, the function will call `undo_last_event` on
/// the same router as the action before has updated the BGP tables.
///
/// Along with each event, the network stores the parent event ID in the `event history`. This
/// allows us to recreate the exact event queue when undoing a single event. When we undo an event,
/// we check the queue if any of them have as parent ID the ID of the event that was restored. If
/// so, they will be removed. When undoing an entire action (like applying a config modification),
/// we do not keep track of the queue, but just delete it in the end, since we require the queue to
/// be empty when applying a new modifier.
///
/// ## Transient State
///
/// **NOTE** This part of the code is currently commented out due to legacy hard policy use.
/// Additionally, we have figured out that for some networks, this approach may lead to an infinite
/// loop of messages. This approach does not work, and therefore, it should not be used.
///
/// By calling `apply_modifier_check_constraint`, we explore the entire space of event orderings.
/// This is done by checking if two events do commute or not. The exact algorithm works as follows:
///
/// 1. First, we apply the modifier as normal, and check that it does converge. Then, we compute the
///    set of prefixes, which were affected by the modification. We then undo all events as before,
///    and reapply the modifier without executing the queue.
///
/// 2. The following sequence is then repeated for each prefix that affected by the modification:
///    1. We create a stack that keeps track of all event reordering at every step, that still needs
///       to be explored.
///    2. Then, we continue as follows:
///       1. If the current branch is not yet entered, we take the next event from the queue, and
///          build a set of non-commuting events from the events that are still inside the queue
///          (see [Commutativity](#commutativity)). If the event does not
///          correspond to the chosen prefix, we ignore all non-commuting events and use an empty
///          set. Then, We push this set onto the stack.
///       2. If the current branch is already entered, take out one event form the stack and
///          continue with this event.
///    3. Next, we execute the the chosen event (from 2.2.1 or 2.2.2). If the event changed the
///       forwarding state of the network, we check the hard hard_policy (but only for the chosen
///       prefix). If they fail, we return the from the procedure with an error.
///    4. Finally, there are three different ways in which we continue:
///       1. If the queue is not empty, we continue with step 2.2.1.
///       2. If the queue is empty, and there are no branches left to explore, we call this prefix
///          ok and continue with the next prefix on step 2.
///       3. If the queue is empty, and there are still branches left to explore, we undo all events
///          on the network until we reach the most recent point where there are still branches to
///          explore. At the same time, we pop the stack to keep it consistent with the network
///          state. Then, continue with step 2.2.2.
///
/// ### Notes and Details
///
/// - *TCP Streams*: BGP is a Distance-Vector protocol that exchanges messages via TCP. This means,
///   that two BGP routers with an active BGP session also have an active TCP session open. Since
///   TCP is a reliable transport protocol, we can be sure that all BGP messages are received by the
///   destination router, and that the messages of one BGP session are always received in the same
///   order as they are sent. This means, that two BGP messages that both have the same source and
///   destination cannot be reordere (at least if they are caused at different time instances).
///   Hence, in step 2.2.1, two things need to happen:
///
///   1. When taking the next event from the queue, we check if the TCP transmission order is
///      validated. If so, then choose the event which must be handled first by the destination
///      router.
///   2. When preparing the set of events that may not commute with the cosen one, we need also to
///      consider the event ordering. We are not allowed to add events to the set if it does not
///      comply with the TCP order. This is achieved by checking every event if it must be freezed.
///
///   However, something is very important: while computing the non-commutive events for event
///   $e_A$, and when event $e_B$ is freezed due to TCP ordering with event $e_C$, then events $e_A$
///   and $C$ do not commute, since applying $e_C$ might cause $e_B$ to be applied, which in terms
///   might commute with $e_A$.
///
/// - *Decoupling Prefixes* In step 2 of the algorithm, we iterate over all prefixes that are
///   updated during convergence. First, we are allowed to do this, because all events talking about
///   two different prefixes do commute. Also, it reduces the complexity of the algorithm
///   dramatically. Assume events $e_{11}$, $e_{12}$ and $e_{13}$ are for prefix 1 and all are not
///   commutative, and $e_{21}$, $e_{22}$, and $e_{23}$ are for prefix 2, which also do not commute.
///   When not decoupling the prefixes, we need to check $3! \cdot 3! = 36$ orderings. However, when
///   decoupling the two prefixes, we only need to check $3! + 3! = 12$ orderings.
pub struct Network {
    net: IgpNetwork,
    links: Vec<(RouterId, RouterId)>,
    routers: HashMap<RouterId, Router>,
    external_routers: HashMap<RouterId, ExternalRouter>,
    known_prefixes: HashSet<Prefix>,
    stop_after: Option<usize>,
    config: Config,
    queue: EventQueue,
    event_history: Vec<(Event, Option<usize>)>,
    skip_queue: bool,
}

impl Clone for Network {
    /// Cloning the network does not clone the event history, and any of the undo traces.
    fn clone(&self) -> Self {
        // for the new queue, remove the history of all enqueued events
        Self {
            net: self.net.clone(),
            links: self.links.clone(),
            routers: self.routers.clone(),
            external_routers: self.external_routers.clone(),
            known_prefixes: self.known_prefixes.clone(),
            stop_after: self.stop_after,
            config: self.config.clone(),
            queue: self.queue.clone(),
            event_history: Vec::new(),
            skip_queue: false,
        }
    }
}

impl Default for Network {
    fn default() -> Self {
        Self::new()
    }
}

impl Network {
    /// Generate an empty Network
    pub fn new() -> Self {
        Self {
            net: IgpNetwork::new(),
            links: Vec::new(),
            routers: HashMap::new(),
            known_prefixes: HashSet::new(),
            external_routers: HashMap::new(),
            stop_after: Some(DEFAULT_STOP_AFTER),
            config: Config::new(),
            queue: EventQueue::new(),
            event_history: Vec::new(),
            skip_queue: false,
        }
    }

    /// Add a new router to the topology. Note, that the AS id is always set to `AsId(65001)`. This
    /// function returns the ID of the router, which can be used to reference it while confiugring
    /// the network.
    pub fn add_router<S: Into<String>>(&mut self, name: S) -> RouterId {
        let new_router = Router::new(name.into(), self.net.add_node(()), AsId(65001));
        let router_id = new_router.router_id();
        self.routers.insert(router_id, new_router);
        router_id
    }

    /// Add a new external router to the topology. An external router does not process any BGP
    /// messages, it just advertises routes from outside of the network. This function returns
    /// the ID of the router, which can be used to reference it while configuring the network.
    pub fn add_external_router<S: Into<String>>(&mut self, name: S, as_id: AsId) -> RouterId {
        let new_router = ExternalRouter::new(name.into(), self.net.add_node(()), as_id);
        let router_id = new_router.router_id();
        self.external_routers.insert(router_id, new_router);
        router_id
    }

    /// This function creates an link in the network The link will have infinite weight for both
    /// directions. The network needs to be configured such that routers can use the link, since
    /// a link with infinte weight is treated as not connected.
    ///
    /// ```rust
    /// # use snowcap::netsim::{Network, config::ConfigModifier, config::ConfigExpr};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut net = Network::new();
    /// let r1 = net.add_router("r1");
    /// let r2 = net.add_router("r2");
    /// net.add_link(r1, r2);
    /// net.apply_modifier(&ConfigModifier::Insert(ConfigExpr::IgpLinkWeight {
    ///     source: r1,
    ///     target: r2,
    ///     weight: 5.0,
    /// }))?;
    /// net.apply_modifier(&ConfigModifier::Insert(ConfigExpr::IgpLinkWeight {
    ///     source: r2,
    ///     target: r1,
    ///     weight: 4.0,
    /// }))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_link(&mut self, source: RouterId, target: RouterId) {
        self.links.push((source, target));
        self.net.add_edge(source, target, LinkWeight::infinite());
        self.net.add_edge(target, source, LinkWeight::infinite());
    }

    /// Set the provided network-wide configuration. The network first computes the patch from the
    /// current configuration to the next one, and applies the patch. If the patch cannot be
    /// applied, then an error is returned. Note, that this function may apply a large number of
    /// modifications in an order which cannot be determined beforehand. If the process fails, then
    /// the network is in an undefined state.
    pub fn set_config(&mut self, config: &Config) -> Result<(), NetworkError> {
        let patch = self.config.get_diff(config);
        self.apply_patch(&patch)
    }

    /// Apply a configuration patch. The modifications of the patch are applied to the network in
    /// the order in which they appear in `patch.modifiers`. After each modifier is applied, the
    /// network will process all necessary messages to let the network converge. The process may
    /// fail if the modifiers cannot be applied to the current config, or if there was a problem
    /// while applying a modifier and letting the network converge. If the process fails, the
    /// network is in an undefined state.
    pub fn apply_patch(&mut self, patch: &ConfigPatch) -> Result<(), NetworkError> {
        // apply every modifier in order
        self.skip_queue = true;
        for modifier in patch.modifiers.iter() {
            self.apply_modifier(modifier)?;
        }
        self.skip_queue = false;
        self.do_queue()
    }

    /// Apply a single configuration modification. The modification must be applicable to the
    /// current configuration. All messages are exchanged. The process fails, then the network is
    /// in an undefined state, and it should be rebuilt.
    pub fn apply_modifier(&mut self, modifier: &ConfigModifier) -> Result<(), NetworkError> {
        debug!("Applying modifier: {}", printer::config_modifier(self, modifier)?);

        // add the event to the history
        let parent_event_id = self.event_history.len();
        self.event_history.push((Event::Config(modifier.clone()), None));

        // execute the event
        self.apply_or_undo_modifier(modifier, false, parent_event_id)
    }

    /// # Transient condition verification
    ///
    /// *This method is only available if the `"transient-violation"` feature is enabled!*
    ///
    /// This function applies the modifier `n_iter` times, each time in a different ordering. At
    /// every intermediate state, it checks the condition, which must hold at each and every state.
    /// Notice, that for the condition, it only allows [`Condition::Reachable`] and
    /// [`Condition::NotReachable`]. This function then returns the number of convergence series, in
    /// which every condition is satisfied at all intermediate states of this convergence process.
    ///
    /// It does this by shuffling the queue each ane very time before performing the next step. If
    /// there exists no message to be reordered, this function returns
    /// [`NetworkError::NoEventsToReorder`].
    #[cfg(feature = "transient-violation")]
    pub fn apply_modifier_check_transient(
        &mut self,
        modifier: &ConfigModifier,
        policy: &[Condition],
        n_iter: usize,
    ) -> Result<usize, NetworkError> {
        debug!("Starting transient state condition");

        let mut rng = thread_rng();
        let mut num_success: usize = 0;

        for _ in 0..n_iter {
            debug!("Restarting Transient Condition Checker...");

            // prohibit the network from executing the queue right away!
            self.skip_queue = true;
            self.apply_modifier(modifier)?;
            self.skip_queue = false;

            // initially, everything is ok
            let mut process_correct: bool = true;
            let mut has_reordered: bool = false;

            // do the step
            while !self.queue.is_empty() {
                // check if we have reordered something
                if self.queue.len() > 1 {
                    has_reordered = true;
                }

                // shuffle the first element of the queue
                let mut pos = (rng.next_u64() as usize) % self.queue.len();
                // get the source and target from the selected message and set pos to the first
                // message from this source to this target. This guarantees TCP message ordering to
                // be considered.
                if let Event::Bgp(from, to, _) = self.queue.get(pos).unwrap().0 {
                    pos = self
                        .queue
                        .iter()
                        .take(pos + 1)
                        .filter_map(|m| match m.0 {
                            Event::Bgp(a, b, _) => Some((a, b)),
                            _ => None,
                        })
                        .position(|(a, b)| a == from && b == to)
                        .unwrap_or(pos);
                }
                self.queue.swap(0, pos);
                // perform the step
                self.do_queue_step()?;
                // check the transient state
                let mut fw_state = self.get_forwarding_state();

                // the policies we check must be reachability policies.
                // The only error, we consider, is a violation of the path condition
                if policy.iter().filter(|c| matches!(c, Condition::Reachable(_, _, _))).any(|c| {
                    matches!(c.check(&mut fw_state), Err(PolicyError::PathCondition { .. }))
                }) {
                    process_correct = false;
                    debug!("Error: invalid path for packets!");
                }
            }

            if !has_reordered {
                return Err(NetworkError::NoEventsToReorder);
            }

            if process_correct {
                num_success += 1;
            }

            // undo the change
            self.undo_action()?;
        }

        self.apply_modifier(modifier)?;

        Ok(num_success)
    }

    /*
     * The following part is legacy code for executing the queue qhile checking hard policies. This
     * however does not work due to several reasons. Also, the hard policies are legacy code, and
     * it must be rewritten!
     */

    /*
    /// Apply a single configuration modification. Then, check that the hard_policy are satisfied
    /// for all possible valid orderings of messages. If there was an error, the modifier is not
    /// applied, and the network state is not changed!
    ///
    /// This function goes through all possible and interesting orderings. This is done by
    /// reordering all non-commuting messages. Two messages are non-commuting, if the following
    /// conditions hold:
    /// 1. Both messages talk about the same prefix
    /// 2. One of the following holds:
    ///    a. The two messages are targeted towards the same router,
    ///    b. Both messages cause a change in the BGP_RIB table of both routers
    ///
    /// Additionally, it checks that the messages can actually be reordered. As an example, if
    /// message A triggeres message B, they cannot be reordered. Additionally, we force the TCP
    /// streams from each router to another to be valid, which means that all messages sent from one
    /// router to another must be ordered correctly.
    ///
    /// In fact, the implemented algorithm does multiple passes through the event sequence, one for
    /// each affected prefix. For each of these passes, it does explore all possible and interesting
    /// reorderings of events that talk about the chosen prefix. This reduces the complexity alot.
    pub fn apply_modifier_check_hard_policy(
        &mut self,
        modifier: &ConfigModifier,
        hard_policy: &HardPolicy,
    ) -> Result<(), NetworkError> {
        debug!("Applying modifier: {}", printer::config_modifier(self, modifier)?);

        // add the event to the history
        let parent_event_id = self.event_history.len();
        self.event_history.push((Event::Config(modifier.clone()), None));

        // apply the modifier and check that it is ok!
        self.apply_or_undo_modifier(modifier, false, parent_event_id)?;
        // in this case, it is ok, and there will be no convergence issues. In this case, we can
        // continue with undoing the last modification and applying the queue with hard_policy.
        // But before we undo the last event, we check which prefixes will be updated, in order to
        // speed up the computation.
        let prefixes_to_check = self
            .event_history
            .iter()
            .rev()
            .take_while(|(e, _)| e.is_bgp_event())
            .map(|(e, _)| e.prefix().unwrap())
            .collect::<HashSet<Prefix>>();

        // we exit if no prefix was affected
        if prefixes_to_check.is_empty() {
            hard_policy.check(&mut self.get_forwarding_state())?;
            return Ok(());
        }

        self.undo_action()?;

        // apply the modifier without executing do_queue.
        self.event_history.push((Event::Config(modifier.clone()), None));
        self.skip_queue = true;
        self.apply_or_undo_modifier(modifier, false, parent_event_id)?;
        self.skip_queue = false;

        // do queue while checking the hard_policy
        self.do_queue_check_hard_policy(hard_policy, prefixes_to_check)
    }
    */

    /// Advertise an external route and let the network converge, The source must be a `RouterId`
    /// of an `ExternalRouter`. If not, an error is returned. When advertising a route, all
    /// eBGP neighbors will receive an update with the new route. If a neighbor is added later
    /// (after `advertise_external_route` is called), then this new neighbor will receive an update
    /// as well.
    pub fn advertise_external_route(
        &mut self,
        source: RouterId,
        prefix: Prefix,
        as_path: Vec<AsId>,
        med: Option<u32>,
        community: Option<u32>,
    ) -> Result<(), NetworkError> {
        debug!("Advertise prefix {} on {}", prefix.0, self.get_router_name(source)?);
        // insert the prefix into the hashset
        self.known_prefixes.insert(prefix);
        // get the event id this event will get
        let parent_event_id = self.event_history.len();

        // initiate the advertisement
        let route = self
            .external_routers
            .get_mut(&source)
            .ok_or(NetworkError::DeviceNotFound(source))?
            .advertise_prefix(prefix, as_path, med, community, &mut self.queue, parent_event_id);

        // add the event to the history
        self.event_history.push((Event::AdvertiseExternalRoute(source, route), None));

        self.do_queue()
    }

    /// Retract an external route and let the network converge. The source must be a `RouterId` of
    /// an `ExternalRouter`. All current eBGP neighbors will receive a withdraw message.
    pub fn retract_external_route(
        &mut self,
        source: RouterId,
        prefix: Prefix,
    ) -> Result<(), NetworkError> {
        debug!("Retract prefix {} on {}", prefix.0, self.get_router_name(source)?);
        // initiate the advertisement
        let parent_event_id = self.event_history.len();

        self.external_routers
            .get_mut(&source)
            .ok_or(NetworkError::DeviceNotFound(source))?
            .widthdraw_prefix(prefix, &mut self.queue, parent_event_id);

        self.event_history.push((Event::WithdrawExternalRoute(source, prefix), None));

        // run the queue
        self.do_queue()
    }

    /// Undo the last action of the network, causing the network to be in the earlier state. If
    /// there was no action to be undone, then Ok(false) is returned. If something has changed,
    /// then Ok(true) is returned.
    ///
    /// The following are considered actions:
    /// - `apply_modifier`
    /// - `advertise_external_route`
    /// - `retract_external_route`
    ///
    /// After undo, the event queue will be empty.
    ///
    /// # Warning
    ///
    /// Once the network is cloned, the copy will not contain the information to undo!
    pub fn undo_action(&mut self) -> Result<bool, NetworkError> {
        if self.event_history.is_empty() {
            Ok(false)
        } else {
            while self.undo_queue_step(false)? {}
            self.queue.clear();
            Ok(true)
        }
    }

    /// Compute and return the current forwarding state.
    pub fn get_forwarding_state(&self) -> ForwardingState {
        ForwardingState::from_net(self)
    }

    // ********************
    // * Helper Functions *
    // ********************

    /// Returns a reference to the network topology (PetGraph struct)
    pub fn get_topology(&self) -> &IgpNetwork {
        &self.net
    }

    /// Returns the number of devices in the topology
    pub fn num_devices(&self) -> usize {
        self.routers.len() + self.external_routers.len()
    }

    /// Returns a reference to the network device.
    pub fn get_device(&self, id: RouterId) -> NetworkDevice<'_> {
        match self.routers.get(&id) {
            Some(r) => NetworkDevice::InternalRouter(r),
            None => match self.external_routers.get(&id) {
                Some(r) => NetworkDevice::ExternalRouter(r),
                None => NetworkDevice::None,
            },
        }
    }

    /// Returns a list of all internal router IDs in the network
    pub fn get_routers(&self) -> Vec<RouterId> {
        self.routers.keys().cloned().collect()
    }

    /// Returns a list of all external router IDs in the network
    pub fn get_external_routers(&self) -> Vec<RouterId> {
        self.external_routers.keys().cloned().collect()
    }

    /// Get the RouterID with the given name. If multiple routers have the same name, then the first
    /// occurence of this name is returned. If the name was not found, an error is returned.
    pub fn get_router_id(&self, name: impl AsRef<str>) -> Result<RouterId, NetworkError> {
        if let Some(id) = self
            .routers
            .values()
            .filter(|r| r.name() == name.as_ref())
            .map(|r| r.router_id())
            .next()
        {
            Ok(id)
        } else if let Some(id) = self
            .external_routers
            .values()
            .filter(|r| r.name() == name.as_ref())
            .map(|r| r.router_id())
            .next()
        {
            Ok(id)
        } else {
            Err(NetworkError::DeviceNameNotFound(name.as_ref().to_string()))
        }
    }

    /// Returns a hashset of all known prefixes
    pub fn get_known_prefixes(&self) -> &HashSet<Prefix> {
        &self.known_prefixes
    }

    /// Return a reference to the current config.
    pub fn current_config(&self) -> &Config {
        &self.config
    }

    /// Returns an iterator over all (undirected) links in the network.
    pub fn links_symmetric(&self) -> std::slice::Iter<'_, (RouterId, RouterId)> {
        self.links.iter()
    }

    /// Configure the topology to pause the queue and return after a certain number of queue have
    /// been executed. The job queue will remain active. If set to None, the queue will continue
    /// running until converged.
    pub fn set_msg_limit(&mut self, stop_after: Option<usize>) {
        self.stop_after = stop_after;
    }

    /// Returns the name of the router, if the ID was found.
    pub fn get_router_name(&self, router_id: RouterId) -> Result<&str, NetworkError> {
        if let Some(r) = self.routers.get(&router_id) {
            Ok(r.name())
        } else if let Some(r) = self.external_routers.get(&router_id) {
            Ok(r.name())
        } else {
            Err(NetworkError::DeviceNotFound(router_id))
        }
    }

    /// Returns the number of messages exchanged in the last operation. Such an operation might be
    /// `set_config`, `apply_modifier`, `apply_patch`, `advertise_external_route`, .... If the
    /// network has been cloned, then the number of msg exchanged is reset to zero.
    pub fn num_msg_exchanged(&self) -> usize {
        self.event_history.len()
    }

    /// Clear the undo stack of all routers, and reset the event history. This does not change
    /// anything on the state of the network itself.
    pub fn clear_undo_stack(&mut self) {
        self.event_history.clear();
        for r in self.routers.values_mut() {
            r.clear_undo_stack();
        }
        for r in self.external_routers.values_mut() {
            r.clear_undo_stack();
        }
    }

    // *******************
    // * Print Functions *
    // *******************

    /// Return the route for the given prefix, starting at the source router, as a list of
    /// `RouterIds,` starting at the source, and ending at the (probably external) router ID that
    /// originated the prefix. The Router ID must be the ID of an internal router.
    pub fn get_route(
        &self,
        source: RouterId,
        prefix: Prefix,
    ) -> Result<Vec<RouterId>, NetworkError> {
        // check if we are already at an external router
        if self.external_routers.get(&source).is_some() {
            return Err(NetworkError::DeviceIsExternalRouter(source));
        }
        let mut visited_routers: HashSet<RouterId> = HashSet::new();
        let mut result: Vec<RouterId> = Vec::new();
        let mut current_node = source;
        loop {
            if !(self.routers.contains_key(&current_node)
                || self.external_routers.contains_key(&current_node))
            {
                return Err(NetworkError::DeviceNotFound(current_node));
            }
            result.push(current_node);
            // insert the current node into the visited routes
            if let Some(r) = self.routers.get(&current_node) {
                // we are still inside our network
                if !visited_routers.insert(current_node) {
                    debug!(
                        "Forwarding Loop detected: {:?}",
                        result
                            .iter()
                            .map(|r| self.get_router_name(*r).unwrap())
                            .collect::<Vec<&str>>()
                    );
                    return Err(NetworkError::ForwardingLoop(result));
                }
                current_node = match r.get_next_hop(prefix) {
                    Some(router_id) => router_id,
                    None => {
                        return {
                            debug!(
                                "Black hole detected: {:?}",
                                result
                                    .iter()
                                    .map(|r| self.get_router_name(*r).unwrap())
                                    .collect::<Vec<&str>>()
                            );
                            Err(NetworkError::ForwardingBlackHole(result))
                        }
                    }
                };
            } else {
                break;
            }
        }
        Ok(result)
    }

    /// Print the route of a routerID to the destination. This is a helper function, wrapping
    /// `self.get_route(source, prefix)` inside some print statements. The router ID must he the ID
    /// of an internal router
    pub fn print_route(&self, source: RouterId, prefix: Prefix) -> Result<(), NetworkError> {
        match self.get_route(source, prefix) {
            Ok(path) => println!(
                "{}",
                path.iter()
                    .map(|r| self.get_router_name(*r))
                    .collect::<Result<Vec<&str>, NetworkError>>()?
                    .join(" => ")
            ),
            Err(NetworkError::ForwardingLoop(path)) => {
                println!(
                    "{} FORWARDING LOOP!",
                    path.iter()
                        .map(|r| self.get_router_name(*r))
                        .collect::<Result<Vec<&str>, NetworkError>>()?
                        .join(" => ")
                );
            }
            Err(NetworkError::ForwardingBlackHole(path)) => {
                println!(
                    "{} BLACK HOLE!",
                    path.iter()
                        .map(|r| self.get_router_name(*r))
                        .collect::<Result<Vec<&str>, NetworkError>>()?
                        .join(" => ")
                );
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// Print the igp forwarding table for a specific router.
    pub fn print_igp_fw_table(&self, router_id: RouterId) -> Result<(), NetworkError> {
        let r = self.routers.get(&router_id).ok_or(NetworkError::DeviceNotFound(router_id))?;
        println!("Forwarding table for {}", r.name());
        let routers_set = self
            .routers
            .keys()
            .cloned()
            .collect::<HashSet<RouterId>>()
            .union(&self.external_routers.keys().cloned().collect::<HashSet<RouterId>>())
            .cloned()
            .collect::<HashSet<RouterId>>();
        for target in routers_set {
            if let Some(Some((next_hop, cost))) = r.get_igp_fw_table().get(&target) {
                println!(
                    "  {} via {} (IGP cost: {})",
                    self.get_router_name(target)?,
                    self.get_router_name(*next_hop)?,
                    cost
                );
            } else {
                println!("  {} unreachable!", self.get_router_name(target)?);
            }
        }
        println!();
        Ok(())
    }

    /// Checks for weak equivalence, by only comparing the BGP tables. This funciton assumes that
    /// both networks have identical routers, identical topologies, identical configuration and that
    /// the same routes are advertised by the same external routers.
    pub fn weak_eq(&self, other: &Self) -> bool {
        // check if the queue is the same. Notice that the length of the queue will be checked
        // before every element is compared!
        if self.queue != other.queue {
            return false;
        }

        // check if the forwarding state is the same
        if self.get_forwarding_state() != other.get_forwarding_state() {
            return false;
        }

        // if we have passed all those tests, it is time to check if the BGP tables on the routers
        // are the same.
        for router in self.routers.keys() {
            if !self.routers[router].compare_bgp_table(other.routers.get(router).unwrap()) {
                return false;
            }
        }

        true
    }

    // *******************
    // * Local Functions *
    // *******************

    /// Apply or undo a single modifier. In the undo case, make sure that the modifier reversed!
    fn apply_or_undo_modifier(
        &mut self,
        modifier: &ConfigModifier,
        undo: bool,
        parent_event_id: usize,
    ) -> Result<(), NetworkError> {
        // check that the modifier can be applied on the config
        self.config.apply_modifier(modifier)?;

        // If the modifier can be applied, then everything is ok and we can do the actual change.
        match modifier {
            ConfigModifier::Insert(expr) => match expr {
                ConfigExpr::IgpLinkWeight { source, target, weight } => {
                    // check if router has a link to target
                    if !self.net.contains_edge(*source, *target) {
                        return Err(NetworkError::RoutersNotConnected(*source, *target));
                    }
                    self.net.update_edge(*source, *target, *weight);
                    self.write_igp_fw_tables(parent_event_id, undo)
                }
                ConfigExpr::BgpSession { source, target, session_type } => {
                    self.add_bgp_session(*source, *target, *session_type, parent_event_id, undo)
                }
                ConfigExpr::BgpRouteMap { router, direction, map } => {
                    match direction {
                        RouteMapDirection::Incoming => {
                            self.routers
                                .get_mut(router)
                                .ok_or(NetworkError::DeviceNotFound(*router))?
                                .add_bgp_route_map_in(
                                    map.clone(),
                                    &mut self.queue,
                                    parent_event_id,
                                    undo,
                                )?;
                        }
                        RouteMapDirection::Outgoing => {
                            self.routers
                                .get_mut(router)
                                .ok_or(NetworkError::DeviceNotFound(*router))?
                                .add_bgp_route_map_out(
                                    map.clone(),
                                    &mut self.queue,
                                    parent_event_id,
                                    undo,
                                )?;
                        }
                    }
                    if undo {
                        Ok(())
                    } else {
                        self.do_queue()
                    }
                }
                ConfigExpr::StaticRoute { router, prefix, target } => {
                    // check if router has a link to target
                    if !self.net.contains_edge(*router, *target) {
                        return Err(NetworkError::RoutersNotConnected(*router, *target));
                    }
                    self.routers
                        .get_mut(router)
                        .ok_or(NetworkError::DeviceNotFound(*router))?
                        .add_static_route(*prefix, *target)?;
                    Ok(())
                }
            },
            ConfigModifier::Remove(expr) => match expr {
                ConfigExpr::IgpLinkWeight { source, target, weight: _ } => {
                    // check if router has a link to target
                    if !self.net.contains_edge(*source, *target) {
                        return Err(NetworkError::RoutersNotConnected(*source, *target));
                    }
                    self.net.update_edge(*source, *target, LinkWeight::infinite());
                    self.write_igp_fw_tables(parent_event_id, undo)
                }
                ConfigExpr::BgpSession { source, target, session_type: _ } => {
                    self.remove_bgp_session(*source, *target, parent_event_id, undo)
                }
                ConfigExpr::BgpRouteMap { router, direction, map } => {
                    match direction {
                        RouteMapDirection::Incoming => {
                            self.routers
                                .get_mut(router)
                                .ok_or(NetworkError::DeviceNotFound(*router))?
                                .remove_bgp_route_map_in(
                                    map.order,
                                    &mut self.queue,
                                    parent_event_id,
                                    undo,
                                )?;
                        }
                        RouteMapDirection::Outgoing => {
                            self.routers
                                .get_mut(router)
                                .ok_or(NetworkError::DeviceNotFound(*router))?
                                .remove_bgp_route_map_out(
                                    map.order,
                                    &mut self.queue,
                                    parent_event_id,
                                    undo,
                                )?;
                        }
                    }
                    if undo {
                        Ok(())
                    } else {
                        self.do_queue()
                    }
                }

                ConfigExpr::StaticRoute { router, prefix, target } => {
                    // check if router has a link to target
                    if !self.net.contains_edge(*router, *target) {
                        return Err(NetworkError::RoutersNotConnected(*router, *target));
                    }
                    self.routers
                        .get_mut(router)
                        .ok_or(NetworkError::DeviceNotFound(*router))?
                        .remove_static_route(*prefix)?;
                    Ok(())
                }
            },
            ConfigModifier::Update { from, to } => match (from, to) {
                (
                    ConfigExpr::IgpLinkWeight { source: s1, target: t1, weight: _ },
                    ConfigExpr::IgpLinkWeight { source: s2, target: t2, weight: w },
                ) if s1 == s2 && t1 == t2 => {
                    // check if router has a link to target
                    if !self.net.contains_edge(*s1, *t1) {
                        return Err(NetworkError::RoutersNotConnected(*s1, *t1));
                    }
                    self.net.update_edge(*s1, *t1, *w);
                    self.write_igp_fw_tables(parent_event_id, undo)
                }
                (
                    ConfigExpr::BgpSession { source: s1, target: t1, session_type: _ },
                    ConfigExpr::BgpSession { source: s2, target: t2, session_type: x },
                ) if (s1 == s2 && t1 == t2) || (s1 == t2 && t1 == s2) => {
                    self.modify_bgp_session(*s2, *t2, *x, parent_event_id, undo)
                }
                (
                    ConfigExpr::BgpRouteMap { router: r1, direction: d1, map: m1 },
                    ConfigExpr::BgpRouteMap { router: r2, direction: d2, map: m2 },
                ) if r1 == r2 && d1 == d2 => {
                    match d1 {
                        RouteMapDirection::Incoming => {
                            self.routers
                                .get_mut(r1)
                                .ok_or(NetworkError::DeviceNotFound(*r1))?
                                .modify_bgp_route_map_in(
                                    m1.order,
                                    m2.clone(),
                                    &mut self.queue,
                                    parent_event_id,
                                    undo,
                                )?;
                        }
                        RouteMapDirection::Outgoing => {
                            self.routers
                                .get_mut(r1)
                                .ok_or(NetworkError::DeviceNotFound(*r1))?
                                .modify_bgp_route_map_out(
                                    m1.order,
                                    m2.clone(),
                                    &mut self.queue,
                                    parent_event_id,
                                    undo,
                                )?;
                        }
                    }
                    if undo {
                        Ok(())
                    } else {
                        self.do_queue()
                    }
                }
                (
                    ConfigExpr::StaticRoute { router: r1, prefix: p1, target: _ },
                    ConfigExpr::StaticRoute { router: r2, prefix: p2, target: t },
                ) if r1 == r2 && p1 == p2 => {
                    // check if router has a link to target
                    if !self.net.contains_edge(*r1, *t) {
                        return Err(NetworkError::RoutersNotConnected(*r1, *t));
                    }
                    self.routers
                        .get_mut(r1)
                        .ok_or(NetworkError::DeviceNotFound(*r1))?
                        .modify_static_route(*p1, *t)?;
                    Ok(())
                }
                _ => Err(NetworkError::ConfigError(ConfigError::ConfigModifierError(
                    modifier.clone(),
                ))),
            },
        }
    }

    /// # Add an BGP session
    ///
    /// Adds an BGP session between source and target. If the session type is set to IBGpClient,
    /// then the target is considered client of the source.
    ///
    /// If the `undo` flag is set, then the routers are only reconfigured, but no update will be
    /// triggered!
    fn add_bgp_session(
        &mut self,
        source: RouterId,
        target: RouterId,
        session_type: BgpSessionType,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), NetworkError> {
        let is_source_external = self.external_routers.contains_key(&source);
        let is_target_external = self.external_routers.contains_key(&target);
        let (source_type, target_type) = match session_type {
            BgpSessionType::IBgpPeer => {
                if is_source_external || is_target_external {
                    Err(NetworkError::InvalidBgpSessionType(source, target, session_type))
                } else {
                    Ok((BgpSessionType::IBgpPeer, BgpSessionType::IBgpPeer))
                }
            }
            BgpSessionType::IBgpClient => {
                if is_source_external || is_target_external {
                    Err(NetworkError::InvalidBgpSessionType(source, target, session_type))
                } else {
                    Ok((BgpSessionType::IBgpClient, BgpSessionType::IBgpPeer))
                }
            }
            BgpSessionType::EBgp => {
                if !(is_source_external || is_target_external) {
                    Err(NetworkError::InvalidBgpSessionType(source, target, session_type))
                } else {
                    Ok((BgpSessionType::EBgp, BgpSessionType::EBgp))
                }
            }
        }?;

        // configure source
        if is_source_external {
            self.external_routers
                .get_mut(&source)
                .ok_or(NetworkError::DeviceNotFound(source))?
                .establish_ebgp_session(target, &mut self.queue, parent_event_id, undo)?;
        } else {
            self.routers
                .get_mut(&source)
                .ok_or(NetworkError::DeviceNotFound(source))?
                .establish_bgp_session(
                    target,
                    source_type,
                    &mut self.queue,
                    parent_event_id,
                    undo,
                )?;
        }
        // configure target
        if is_target_external {
            self.external_routers
                .get_mut(&target)
                .ok_or(NetworkError::DeviceNotFound(target))?
                .establish_ebgp_session(source, &mut self.queue, parent_event_id, undo)?;
        } else {
            self.routers
                .get_mut(&target)
                .ok_or(NetworkError::DeviceNotFound(target))?
                .establish_bgp_session(
                    source,
                    target_type,
                    &mut self.queue,
                    parent_event_id,
                    undo,
                )?;
        }
        if undo {
            Ok(())
        } else {
            self.do_queue()
        }
    }

    /// # Modify an BGP session type
    ///
    /// Modifies an BGP session type between source and target. If the session type is set to
    /// IBGpClient, then the target is considered client of the source.
    fn modify_bgp_session(
        &mut self,
        source: RouterId,
        target: RouterId,
        session_type: BgpSessionType,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), NetworkError> {
        let is_source_external = self.external_routers.contains_key(&source);
        let is_target_external = self.external_routers.contains_key(&target);
        // you can only change a session between two internal routers, because it is not possible
        // to use a bgp session type different from eBGP with an external router. Therefore, we can
        // safely return here if the type of the session is eBGP, and return an error if the type is
        // not eBGP
        if is_source_external || is_target_external {
            return if session_type.is_ebgp() {
                Ok(())
            } else {
                Err(NetworkError::InvalidBgpSessionType(source, target, session_type))
            };
        }

        let (source_type, target_type) = match session_type {
            BgpSessionType::IBgpPeer => (BgpSessionType::IBgpPeer, BgpSessionType::IBgpPeer),
            BgpSessionType::IBgpClient => (BgpSessionType::IBgpClient, BgpSessionType::IBgpPeer),
            BgpSessionType::EBgp => {
                // in this case, we can return an error, since an ebgp session is only allowed to be
                // established between an internal and an external router. But we have already
                // checked that both routers are internal.
                return Err(NetworkError::InvalidBgpSessionType(source, target, session_type));
            }
        };

        self.routers
            .get_mut(&source)
            .ok_or(NetworkError::DeviceNotFound(source))?
            .modify_bgp_session(target, source_type, &mut self.queue, parent_event_id, undo)?;
        self.routers
            .get_mut(&target)
            .ok_or(NetworkError::DeviceNotFound(target))?
            .modify_bgp_session(source, target_type, &mut self.queue, parent_event_id, undo)?;
        if undo {
            Ok(())
        } else {
            self.do_queue()
        }
    }

    /// Remove an iBGP session
    fn remove_bgp_session(
        &mut self,
        source: RouterId,
        target: RouterId,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), NetworkError> {
        let is_source_external = self.external_routers.contains_key(&source);
        let is_target_external = self.external_routers.contains_key(&target);

        if is_source_external {
            self.external_routers
                .get_mut(&source)
                .ok_or(NetworkError::DeviceNotFound(source))?
                .close_ebgp_session(target)?;
        } else {
            self.routers
                .get_mut(&source)
                .ok_or(NetworkError::DeviceNotFound(source))?
                .close_bgp_session(target, &mut self.queue, parent_event_id, undo)?;
        }

        if is_target_external {
            self.external_routers
                .get_mut(&target)
                .ok_or(NetworkError::DeviceNotFound(target))?
                .close_ebgp_session(source)?;
        } else {
            self.routers
                .get_mut(&target)
                .ok_or(NetworkError::DeviceNotFound(target))?
                .close_bgp_session(source, &mut self.queue, parent_event_id, undo)?;
        }
        if undo {
            Ok(())
        } else {
            self.do_queue()
        }
    }

    /// Write the igp forwarding tables for all internal routers. As soon as this is done, recompute
    /// the BGP table. and run the algorithm. This will happen all at once, in a very unpredictable
    /// manner. If you want to do this more predictable, use `write_ibgp_fw_table`.
    ///
    /// The function returns Ok(true) if all events caused by the igp fw table write are handled
    /// correctly. Returns Ok(false) if the max number of iterations is exceeded, and returns an
    /// error if an event was not handled correctly.
    fn write_igp_fw_tables(
        &mut self,
        parent_event_id: usize,
        undo: bool,
    ) -> Result<(), NetworkError> {
        // update igp table
        for r in self.routers.values_mut() {
            r.write_igp_forwarding_table(&self.net, &mut self.queue, parent_event_id, undo)?;
        }
        if undo {
            Ok(())
        } else {
            self.do_queue()
        }
    }

    /*
     * The following part is legacy code for executing the queue qhile checking hard policies. This
     * however does not work due to several reasons. Also, the hard policies are legacy code, and
     * it must be rewritten!
     */

    /*
    /// Execute the queue, but check that the hard_policy are satisfied in every possible ordering
    /// of the messages. No convergence issues should occurr!
    ///
    /// This function makes sure that messages from a router to another must be in order, since BGP
    /// is based on TCP.
    fn do_queue_check_hard_policy(
        &mut self,
        hard_policy: &HardPolicy,
        prefixes_to_check: HashSet<Prefix>,
    ) -> Result<(), NetworkError> {
        // skip if necessary
        if self.skip_queue {
            return Ok(());
        }
        let mut hard_policy = hard_policy.clone();
        // before we start, we need to check the state
        if let Err(policy_error) = hard_policy.check(&mut self.get_forwarding_state()) {
            // undo all changes
            self.undo_action()?;
            // return the error
            return Err(NetworkError::UnsatisfiedHard_Policy(policy_error));
        }

        // we need to repeat the same thing for all different prefixes
        for prefix in prefixes_to_check {
            let mut num_branches: usize = 0;
            let mut open_branches: usize = 0;
            let mut new_branch: bool = true;
            let mut options_stack: Vec<Vec<(Event, usize)>> = Vec::new();

            while !self.queue.is_empty() {
                // get the next event to execute
                let (next_event, parent_event_id) = if new_branch {
                    // reorder the top most event, such that it always complies with TCP sequence
                    // ordering
                    self.reorder_events_for_tcp();
                    // get the next event
                    let (next_event, parent_event_id) = self.queue.pop_front().unwrap();
                    // get the set of other events that may interfere with this one, and push it to the
                    // options stack. However, check that the two prefixes match.
                    let non_commuting_events = if next_event.prefix() == Some(prefix) {
                        self.get_non_commuting_events(&next_event, parent_event_id)?
                    } else {
                        // If the event is not about the given prefix, then ignore this event of
                        // building all possible orderings.
                        Vec::new()
                    };

                    if !non_commuting_events.is_empty() {
                        open_branches += 1;
                    }
                    options_stack.push(non_commuting_events);
                    (next_event, parent_event_id)
                } else {
                    let next_event = options_stack.last_mut().unwrap().pop().unwrap();
                    if options_stack.last().unwrap().is_empty() {
                        open_branches -= 1;
                    }
                    while self.queue.front().unwrap() != &next_event {
                        self.queue.rotate_left(1);
                    }
                    self.queue.pop_front().unwrap()
                };

                // perform the event
                let event_id = self.event_history.len();
                self.event_history.push((next_event.clone(), Some(parent_event_id)));
                let fw_state_change = match next_event {
                    Event::Bgp(from, to, bgp_event) => {
                        //self.bgp_race_checker(to, &bgp_event, &history);
                        if let Some(r) = self.routers.get_mut(&to) {
                            r.handle_event(
                                Event::Bgp(from, to, bgp_event),
                                &mut self.queue,
                                event_id,
                            )
                            .map_err(|e| NetworkError::DeviceError(e))
                        } else if let Some(r) = self.external_routers.get_mut(&to) {
                            r.handle_event(
                                Event::Bgp(from, to, bgp_event),
                                &mut self.queue,
                                event_id,
                            )
                            .map_err(|e| NetworkError::DeviceError(e))
                        } else {
                            Err(NetworkError::DeviceNotFound(to))
                        }
                    }
                    e => Err(NetworkError::InvalidEvent(e)),
                }?;

                if fw_state_change {
                    // something changed in the forwarding state. Recompute hard hard_policy
                    // TODO make the hard_policy checker to check only a single prefix
                    if let Err(policy_error) =
                        hard_policy.check_prefix(&mut self.get_forwarding_state(), prefix)
                    {
                        num_branches += 1;
                        debug!(
                            "Error with event sequence after checking {} branches:\n{}",
                            num_branches,
                            self.event_history
                                .iter()
                                .rev()
                                .take_while(|(e, _)| e.is_bgp_event())
                                .map(|(e, _)| printer::event(&self, e).unwrap())
                                .collect::<Vec<_>>()
                                .into_iter()
                                .rev()
                                .collect::<Vec<_>>()
                                .join("\n")
                        );
                        debug!("error: {}", policy_error.repr_with_name(&self));
                        // undo all changes
                        self.undo_action()?;
                        // return the error
                        return Err(NetworkError::UnsatisfiedHard_Policy(policy_error));
                    }
                }

                // if we reach this point, then the hard_policy are ok.
                if self.queue.is_empty() && open_branches == 0 {
                    // queue is empty, and there exists no open branches! Everything is ok, netwokr has
                    // converged without causing any problems in any ordering
                    num_branches += 1;
                    debug!("Queue OK, checked {} branches", num_branches);
                    return Ok(());
                } else if self.queue.is_empty() {
                    // queue is empty, but there exists some open branch. Undo until we find this branch
                    // first, undo one step, for which we have not created any options in th estack
                    self.undo_queue_step(true)?;
                    'pop_loop: loop {
                        options_stack.pop();
                        self.undo_queue_step(true)?;
                        if !options_stack.last().unwrap().is_empty() {
                            break 'pop_loop;
                        }
                    }
                    new_branch = false;
                    num_branches += 1;

                    if num_branches % 10000 == 0 && num_branches > 0 {
                        warn!(
                            "many options: {} {:?}",
                            num_branches,
                            options_stack
                                .iter()
                                .map(|v| v.len())
                                .filter(|c| *c > 0)
                                .collect::<Vec<_>>(),
                        );
                    }
                } else {
                    // queue is not empty! go into new branch
                    new_branch = true;
                }
            }
        }
        Ok(())
    }
    */

    /// Execute the queue
    fn do_queue(&mut self) -> Result<(), NetworkError> {
        if self.skip_queue {
            return Ok(());
        }
        let mut remaining_iter = self.stop_after;
        while !self.queue.is_empty() {
            if let Some(rem) = remaining_iter {
                if rem == 0 {
                    debug!("Network cannot converge! try to extract the loop");
                    return Err(match self.no_convergence_repetition_checker() {
                        Ok(None) => {
                            debug!("No loop could be detected!");
                            NetworkError::NoConvergence
                        }
                        Ok(Some((events, nets))) => {
                            debug!("Convergence loop detected!");
                            NetworkError::ConvergenceLoop(events, nets)
                        }
                        Err(e) => {
                            warn!("Error during convergence check: {}", e);
                            NetworkError::NoConvergence
                        }
                    });
                }
                remaining_iter = Some(rem - 1);
            }
            self.do_queue_step()?;
        }

        Ok(())
    }

    /// Executes one single step. If the result is Ok(true), then a step is successfully executed.
    /// If the result is Ok(false), then there was no event present in the queue.
    fn do_queue_step(&mut self) -> Result<bool, NetworkError> {
        if let Some((event, parent_event_id)) = self.queue.pop_front() {
            // log the job
            self.log_event(&event)?;
            // execute the event
            let event_id = self.event_history.len();
            let _fw_state_change = match event.clone() {
                Event::Bgp(from, to, bgp_event) => {
                    //self.bgp_race_checker(to, &bgp_event, &history);
                    if let Some(r) = self.routers.get_mut(&to) {
                        r.handle_event(Event::Bgp(from, to, bgp_event), &mut self.queue, event_id)?
                    } else if let Some(r) = self.external_routers.get_mut(&to) {
                        r.handle_event(Event::Bgp(from, to, bgp_event), &mut self.queue, event_id)?
                    } else {
                        return Err(NetworkError::DeviceNotFound(to));
                    }
                }
                e => return Err(NetworkError::InvalidEvent(e)),
            };
            self.event_history.push((event, Some(parent_event_id)));
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Undo the last action of a router
    fn undo_router(&mut self, router: RouterId) -> Result<(), NetworkError> {
        if let Some(r) = self.routers.get_mut(&router) {
            r.undo_last_event()?;
        } else if let Some(r) = self.external_routers.get_mut(&router) {
            r.undo_last_event()?;
        } else {
            return Err(NetworkError::DeviceNotFound(router));
        }
        Ok(())
    }

    /// Undo the last event in the queue. Returns Ok(true) if there an event was undone. Returns
    /// Ok(false) if the event history is empty, or if the last event was a config modification
    /// that changed. The event history will be changed. If `update_queue` is enabled, then all
    /// events, that have been created by the undone event will be removed from the queue, and the
    /// removed event will be added back to the queue.
    fn undo_queue_step(&mut self, update_queue: bool) -> Result<bool, NetworkError> {
        match self.event_history.pop() {
            Some((Event::Bgp(from, to, bgp_event), Some(parent_event_id))) => {
                // Undo the event and push it back to the queue
                // the event was executed on the to router.
                self.undo_router(to)?;
                if update_queue {
                    // get the event id
                    let event_id = self.event_history.len();
                    // remove all events that have been caused by this event
                    self.queue.retain(|(_, parent)| *parent != event_id);
                    self.queue.push_front((Event::Bgp(from, to, bgp_event), parent_event_id));
                }
                Ok(true)
            }
            Some((Event::Config(modifier), None)) => {
                // Undo the modifier
                self.apply_or_undo_modifier(&modifier.reverse(), true, 0)?;
                Ok(false)
            }
            Some((Event::AdvertiseExternalRoute(router, route), None)) => {
                self.undo_router(router)?;
                // fix known prefixes
                if !self.external_routers.values().any(|r| r.has_active_route(route.prefix)) {
                    self.known_prefixes.remove(&route.prefix);
                }
                Ok(false)
            }
            Some((Event::WithdrawExternalRoute(router, prefix), None)) => {
                self.undo_router(router)?;
                self.known_prefixes.insert(prefix);
                Ok(false)
            }
            Some(_) => Err(NetworkError::HistoryError("Parent event id is invalid!")),
            None => Ok(false),
        }
    }

    /*
     * The following part is legacy code for executing the queue qhile checking hard policies. This
     * however does not work due to several reasons. Also, the hard policies are legacy code, and
     * it must be rewritten!
     */

    /*
    /// This funciton checks the queue if there exists an event which does not commute with the
    /// provided event. They commute only if the two events can be reordered without changing the
    /// outcome. The following conditions are checked (in the given order):
    ///
    /// 1. If two messages do not talk about the same prefix, they do commute
    /// 2. If two messages have the same source and target and different parent event ids, they
    ///    cannot be reordered (due to TCP sequence), and hence, they cannot count as non-commuting.
    /// 3. If two messages have the same target, then they do not commute.
    /// 4. If both messages cause a change in the BGP_RIB table of the target routers, then they do
    ///    not commute.
    fn get_non_commuting_events(
        &self,
        event: &Event,
        parent_event_id: usize,
    ) -> Result<Vec<(Event, usize)>, NetworkError> {
        if let Event::Bgp(from, to, bgp_event) = event {
            let mut result: Vec<(Event, usize)> = Vec::new();
            let prefix = bgp_event.prefix();
            let event_has_effect =
                self.routers.get(to).map(|r| r.peek_event(event)).unwrap_or(Ok(false))?;

            // build the freeze vector, which tells us if either an event is frozen, or an event
            // would unfreeze another if applied.
            #[derive(Clone)]
            enum FreezeState {
                None,
                Freezed,
                Blocking,
            }

            let mut freeze_state =
                std::iter::repeat(FreezeState::None).take(self.queue.len()).collect::<Vec<_>>();
            for pos_a in 0..self.queue.len() {
                for pos_b in (pos_a + 1)..self.queue.len() {
                    match (self.queue.get(pos_a).unwrap(), self.queue.get(pos_b).unwrap()) {
                        (
                            (Event::Bgp(from_a, to_a, event_a), parent_a),
                            (Event::Bgp(from_b, to_b, event_b), parent_b),
                        ) if event_a.prefix() == prefix && event_b.prefix() == prefix => {
                            if from_a == from_b && to_a == to_b {
                                if parent_a > parent_b {
                                    freeze_state[pos_a] = FreezeState::Freezed;
                                    if let FreezeState::None = freeze_state[pos_b] {
                                        freeze_state[pos_b] = FreezeState::Blocking;
                                    }
                                }
                                if parent_a < parent_b {
                                    freeze_state[pos_b] = FreezeState::Freezed;
                                    if let FreezeState::None = freeze_state[pos_a] {
                                        freeze_state[pos_a] = FreezeState::Blocking;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            // now, check for commutativity
            'future_events_loop: for (future_event, freeze) in
                self.queue.iter().zip(freeze_state.into_iter())
            {
                if let (Event::Bgp(future_from, future_to, future_bgp_event), future_parent) =
                    future_event
                {
                    // the prefix must be the same
                    if future_bgp_event.prefix() != prefix {
                        // the two events commute
                        continue 'future_events_loop;
                    }
                    // check for TCP order condition
                    if from == future_from && to == future_to && parent_event_id != *future_parent {
                        // reordering both messages would violate TCP ordering!
                        continue 'future_events_loop;
                    }
                    if let FreezeState::Freezed = freeze {
                        // Executing the future event now would violate TCP ordering!
                        continue 'future_events_loop;
                    }
                    if let FreezeState::Blocking = freeze {
                        // Execute the future event will unlock other events which might be
                        // commuting!
                        result.push(future_event.clone());
                    } else if to == future_to {
                        // check if both events either target the same router
                        result.push(future_event.clone());
                    } else if event_has_effect {
                        // check if both events have an effect
                        if self
                            .routers
                            .get(future_to)
                            .map(|r| r.peek_event(&future_event.0))
                            .unwrap_or(Ok(false))?
                        {
                            result.push(future_event.clone());
                        }
                    }
                }
            }
            Ok(result)
        } else {
            Ok(Vec::new())
        }
    }

    /// This function reorderes the event queue, such that the next event triggered does comply with
    /// the TCP ordering. If the next event cannot be executed before another event later in the
    /// queue, then the two events are swapped in the queue. The condition for this is, that both
    /// messages must have the same source and target, and that one has an earlier parent event id
    /// than the other.
    fn reorder_events_for_tcp(&mut self) {
        let mut queue_iter = self.queue.iter();
        let mut swap_pos = None;
        if let Some((Event::Bgp(from, to, _), parent)) = queue_iter.next() {
            let mut pos = 1;
            let mut lowest_parent_id = parent;
            // the next event might need to be reordered. Check with every other event in the queue
            while let Some(other_event) = queue_iter.next() {
                if let (Event::Bgp(other_from, other_to, _), other_parent) = other_event {
                    if from == other_from && to == other_to && lowest_parent_id > other_parent {
                        swap_pos = Some(pos);
                        lowest_parent_id = other_parent;
                    }
                }
                pos += 1;
            }
        }

        if let Some(pos) = swap_pos {
            self.queue.swap(0, pos);
        }
    }
    */

    fn log_event(&self, event: &Event) -> Result<(), NetworkError> {
        match event {
            Event::Bgp(from, to, BgpEvent::Update(route)) => trace!(
                "{} -> {}: BGP Update prefix {}",
                self.get_router_name(*from)?,
                self.get_router_name(*to)?,
                route.prefix.0
            ),
            Event::Bgp(from, to, BgpEvent::Withdraw(prefix)) => trace!(
                "{} -> {}: BGP withdraw prefix {}",
                self.get_router_name(*from)?,
                self.get_router_name(*to)?,
                prefix.0
            ),
            Event::Config(modifier) => trace!("{}", printer::config_modifier(self, modifier)?),
            Event::AdvertiseExternalRoute(source, route) => trace!(
                "Router {} advertises [{}]",
                self.get_router_name(*source)?,
                printer::bgp_route(self, route)?
            ),
            Event::WithdrawExternalRoute(source, prefix) => trace!(
                "Router {} withdraws advertisement for prefix {}",
                self.get_router_name(*source)?,
                prefix.0
            ),
        }
        Ok(())
    }

    /// This function checks if there exists a repetition in the stored history. If there is, then
    /// this function will return a vector containing the events which forms an event loop, and all
    /// possible states (including the queue of messages). None is returned if the algorithm could
    /// not identify any loop. The loop must have been executed for at least two full cycles.
    ///
    /// TODO To keep the order consistent over multiple runs, use a linked hashset instead of a
    /// regular hashset!
    fn no_convergence_repetition_checker(
        &mut self,
    ) -> Result<Option<ConvergenceRepetition>, NetworkError> {
        let mut max_loop_len = self.event_history.len() / 2 - 1;
        if max_loop_len > MAXIMUM_ALLOWED_LOOP_LEN {
            max_loop_len = MAXIMUM_ALLOWED_LOOP_LEN
        }
        'conjecture_loop: for conjectured_loop_dur in 1..(max_loop_len) {
            // check if this conjectured loop duration is a loop by checking every event, if they
            // are equal.
            let mut running_pos_back = self.event_history.len() - 1 - 2 * conjectured_loop_dur;
            let mut running_pos_front = self.event_history.len() - 1 - conjectured_loop_dur;
            // we have maybe found a correct loop. Check every other possition
            for _ in 0..conjectured_loop_dur {
                if self.event_history.get(running_pos_back).map(|(e, _)| e)
                    != self.event_history.get(running_pos_front).map(|(e, _)| e)
                {
                    // no luck! no loop detected!
                    continue 'conjecture_loop;
                }
                running_pos_back += 1;
                running_pos_front += 1;
            }

            // if we reach this position, then we have found a loop! As a next step, we need to
            // extract all networks, and return them.
            let loop_dur = conjectured_loop_dur;

            let mut loop_events: Vec<Event> = Vec::with_capacity(loop_dur);
            let mut loop_networks: Vec<Network> = Vec::with_capacity(loop_dur);

            // increase the running_pos_back, because it should be the same as the one currently
            // enqueued.
            running_pos_back += 1;

            // we now execute for the enitrety of the loop, one step at a time, and clone the
            // network after every event, in order to get all the possible events.
            for _ in 0..loop_dur {
                // first, make sure that the event enqueued matches the one we expect in the loop
                if self.queue.is_empty() {
                    return Err(NetworkError::UnexpectedEventConvergenceLoop);
                }
                if self.event_history.get(running_pos_back).unwrap().0
                    != self.queue.front().unwrap().0
                {
                    return Err(NetworkError::UnexpectedEventConvergenceLoop);
                }
                // then, execute one single step of the loop
                self.do_queue_step()?;
                // now, clone the network and insert into the loop netowrks
                loop_events.push(self.event_history.get(running_pos_back).unwrap().0.clone());
                loop_networks.push(self.clone());
                // finally, go to the next position in the history
                running_pos_back += 1;
            }

            // found a loop. return it
            return Ok(Some((loop_events, loop_networks)));
        }

        Ok(None)
    }
}

type ConvergenceRepetition = (Vec<Event>, Vec<Network>);

/// The `PartialEq` implementation checks if two networks are identica. The implementation first
/// checks "simple" conditions, like the configuration, before checking the state of each individual
/// router. Use the `Network::weak_eq` function to skip some checks, which can be known beforehand.
/// This implementation will check the configuration, advertised prefixes and all routers.
impl PartialEq for Network {
    fn eq(&self, other: &Self) -> bool {
        // first, check if the same number of internal and external routers exists
        if self.routers.len() != other.routers.len()
            || self.external_routers.len() != other.external_routers.len()
        {
            return false;
        }

        // check if the known prefixes are the same
        if self.known_prefixes != other.known_prefixes {
            return false;
        }

        // check if the configuration is the same
        if self.config != other.config {
            return false;
        }

        // check if the external routers advertise the same prefix
        let external_routers_same_prefixes = self.external_routers.keys().all(|rid| {
            self.external_routers
                .get(rid)
                .unwrap()
                .advertises_same_routes(other.external_routers.get(rid).unwrap())
        });
        if !external_routers_same_prefixes {
            return false;
        }

        self.weak_eq(other)
    }
}
