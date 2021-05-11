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

//! # Route Reachability Graph
//!
//! This module computes the route reachability graph of all routes in the network.
//!
//! TODO this module is currently not able to hanle route maps setting the IGP cost. Implement this

use crate::netsim::{
    bgp::{BgpRibEntry, BgpRoute},
    config::ConfigExpr,
    route_map::{RouteMap, RouteMapDirection::*},
    {
        BgpSessionType::{self, *},
        Network, NetworkDevice, RouterId,
    },
};

use maplit::hashmap;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::iter::repeat;

/// Returns a vector containiing all possible routes in the network along with the reachability
/// information. This operation takes $O(n^2 |R|)$
pub fn get_all_route_reachability(
    net: &Network,
    bgp_graph: &BgpGraph,
) -> HashMap<BgpRoute, RouteReachability> {
    // prepare result vector
    let mut result: HashMap<BgpRoute, RouteReachability> = HashMap::new();

    for ext_router_id in net.get_external_routers() {
        let ext_router = net.get_device(ext_router_id).unwrap_external();
        for route in ext_router.get_advertised_routes() {
            // initialize routes_todo
            let mut routes_todo: HashMap<BgpRoute, Vec<(RouterId, BgpEdge, Option<RouterId>)>> =
                hashmap!(route.clone() => vec![(ext_router_id, BgpEdge::UpExternal, None)]);

            while let Some((next_route, next_start_stack)) = pop_hashmap(&mut routes_todo) {
                let (rri, new_routes) =
                    RouteReachability::new(net, bgp_graph, next_route.clone(), next_start_stack);
                // push the information to the result. Check if the route was already created
                match result.entry(next_route) {
                    std::collections::hash_map::Entry::Occupied(mut e) => e.get_mut().union(rri),
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(rri);
                    }
                }

                // extend the routes with new information
                for (route, mut stack) in new_routes {
                    routes_todo.entry(route).or_default().append(&mut stack);
                }
            }
        }
    }

    result
}

fn pop_hashmap<K, V>(map: &mut HashMap<K, V>) -> Option<(K, V)>
where
    K: Eq + std::hash::Hash + Clone,
{
    let next_key = map.keys().next()?.clone();
    map.remove_entry(&next_key)
}

type RouteExploration = (BgpRoute, Vec<(RouterId, BgpEdge, Option<RouterId>)>);

/// Encodes the reachability information of a specific route
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteReachability(HashSet<RouterId>);

impl RouteReachability {
    /// Create a new route reachability information, based on the BGP graph for a given route, which
    /// starts at the chosen node in the chosen state. The initial stack contains all routers (with
    /// the respective state) where the route starts propagating. The function returns the route
    /// reachability information, along with a vector of additional routes to build.
    ///
    /// Important: The returned structure may contain duplicate routes. This needs to be checked
    /// afterwards!
    ///
    /// The DFS algorithm goes through the BGP Graph. When traversing each link, it applies all
    /// potential outgoing route maps of the source and all ingoing route maps of the destination.
    ///
    /// TODO this structure is not able to hanle route maps setting the IGP cost.
    fn new(
        net: &Network,
        bgp: &BgpGraph,
        route: BgpRoute,
        initial_stack: Vec<(RouterId, BgpEdge, Option<RouterId>)>,
    ) -> (Self, Vec<RouteExploration>) {
        let mut reachability: HashSet<RouterId> = HashSet::new();
        let mut dfs_stack: Vec<Vec<(RouterId, BgpEdge, Option<RouterId>)>> = vec![initial_stack];
        let mut dfs_seen: Vec<BgpEdge> = repeat(BgpEdge::None).take(bgp.n).collect();
        let mut new_routes: Vec<(BgpRoute, RouterId, BgpEdge, Option<RouterId>)> = Vec::new();

        // DFS algorithm
        while !dfs_stack.is_empty() {
            if let Some((node, state, node_before)) = dfs_stack.last_mut().unwrap().pop() {
                // update the reachability
                reachability.insert(node);
                // create the new stack frame
                let mut new_stack_frame: Vec<(RouterId, BgpEdge, Option<RouterId>)> = Vec::new();
                // continue algorithm, go through every neighbor
                for (neighbor, new_state) in bgp.neighbors(node) {
                    // if the neighbor is the same as the node_before, then do nothing and continue
                    // with the next neighbor
                    if Some(neighbor) == node_before {
                        continue;
                    }
                    // check if the neighbor can still be reached in the current state. If the state
                    // less than the edge state, go to the next neighbor
                    if state < new_state {
                        continue;
                    }
                    // neighbor can be reached. Do something!
                    // check if there is a route map that matches the entry
                    match bgp.apply_route_maps(net, node_before, node, neighbor, &route) {
                        RouteMapResult::NoMatch => {
                            // continue as normal, route is not altered
                            // check if the node was already seen with a lower type.
                            if dfs_seen[neighbor.index()] <= new_state {
                                // add the neighbor and the new state to the stack
                                new_stack_frame.push((
                                    neighbor,
                                    new_state.over_to_down(),
                                    Some(node),
                                ));
                                // set the dfs_seen tag
                                dfs_seen[neighbor.index()] = new_state.over_to_down();
                            } else {
                                // we have already seen this noe in this pass, nothing to do!
                            }
                        }
                        RouteMapResult::Deny => {} // nothing to do, route is dropped here!
                        RouteMapResult::Allow(new_route) => {
                            // Seems like the route was changed.
                            new_routes.push((
                                new_route,
                                neighbor,
                                new_state.over_to_down(),
                                Some(node),
                            ));
                        }
                    }
                }
                // push the new stack frame if it is not empty
                if !new_stack_frame.is_empty() {
                    dfs_stack.push(new_stack_frame);
                }
            } else {
                // last element is empty. pop from stack and continue
                dfs_stack.pop();
            }
        }

        // build new_routes by combining the same routes
        let mut new_routes_map: HashMap<BgpRoute, Vec<(RouterId, BgpEdge, Option<RouterId>)>> =
            HashMap::new();
        for (route, node, state, previous_node) in new_routes {
            new_routes_map.entry(route).or_default().push((node, state, previous_node));
        }

        (Self(reachability), new_routes_map.into_iter().collect())
    }

    #[allow(dead_code)]
    pub fn contains(&self, router_id: RouterId) -> bool {
        self.0.contains(&router_id)
    }

    /// Consumes the other route reachability information and updates the current one by creating
    /// the union.
    pub(crate) fn union(&mut self, other: Self) {
        for node in other.0.into_iter() {
            self.0.insert(node);
        }
    }

    /// Returns an iterator over all routers that this route may reach
    pub fn iter(&self) -> std::collections::hash_set::Iter<'_, RouterId> {
        self.0.iter()
    }
}

/// BGP Graph
#[derive(Debug, Clone, PartialEq)]
pub struct BgpGraph {
    pub g: Vec<BgpEdge>,
    pub rm_inc: Vec<Vec<RouteMap>>,
    pub rm_out: Vec<Vec<RouteMap>>,
    pub n: usize,
}

impl BgpGraph {
    /// Create a new BGP graph based on the network
    pub fn new(net: &Network) -> Self {
        let n = net.num_devices();
        let mut g: Vec<BgpEdge> = repeat(BgpEdge::None).take(n * n).collect();
        let mut rm_inc: Vec<Vec<RouteMap>> = repeat(Vec::new()).take(n).collect();
        let mut rm_out: Vec<Vec<RouteMap>> = repeat(Vec::new()).take(n).collect();

        for expr in net.current_config().iter() {
            match expr {
                ConfigExpr::BgpSession { source, target, session_type: EBgp } => {
                    if net.get_device(*source).is_external() {
                        g[edge_idx(*source, *target, n)] = BgpEdge::UpExternal;
                        g[edge_idx(*target, *source, n)] = BgpEdge::DownExternal;
                    } else {
                        g[edge_idx(*source, *target, n)] = BgpEdge::DownExternal;
                        g[edge_idx(*target, *source, n)] = BgpEdge::UpExternal;
                    }
                }
                ConfigExpr::BgpSession { source, target, session_type: IBgpPeer } => {
                    g[edge_idx(*source, *target, n)] = BgpEdge::Over;
                    g[edge_idx(*target, *source, n)] = BgpEdge::Over;
                }
                ConfigExpr::BgpSession { source, target, session_type: IBgpClient } => {
                    g[edge_idx(*source, *target, n)] = BgpEdge::Down;
                    g[edge_idx(*target, *source, n)] = BgpEdge::Up;
                }
                ConfigExpr::BgpRouteMap { router, direction: Incoming, map } => {
                    rm_inc[router.index()].push(map.clone());
                }
                ConfigExpr::BgpRouteMap { router, direction: Outgoing, map } => {
                    rm_out[router.index()].push(map.clone());
                }
                _ => {}
            }
        }

        // sort the route maps according to their order
        for rm_list in rm_inc.iter_mut() {
            rm_list.sort_by(|a, b| a.order().cmp(&b.order()));
        }
        for rm_list in rm_out.iter_mut() {
            rm_list.sort_by(|a, b| a.order().cmp(&b.order()));
        }

        Self { g, n, rm_inc, rm_out }
    }

    /// Return an iterator over all neighbors of the chosen router
    pub fn neighbors(&self, router: RouterId) -> BgpGraphNeighborIterator<'_> {
        let start_idx = router.index() * self.n;
        let end_idx = start_idx + self.n;
        BgpGraphNeighborIterator { data: &self.g[start_idx..end_idx], pos: 0 }
    }

    /// Applies the route advertisement from `current` to `target`, with a route that `current`
    /// received from the `source`. Source can be None, which means that the outgoing route maps of
    /// `current` are not applied.
    ///
    /// This function panics if there exists no BGP session between `source` and `current`, or
    /// between `current` and `target`.
    pub(super) fn apply_route_maps(
        &self,
        net: &Network,
        source: Option<RouterId>,
        current: RouterId,
        target: RouterId,
        route: &BgpRoute,
    ) -> RouteMapResult {
        let next_hop = route.next_hop;
        let mut rm_applied: bool = false;
        // apply the outgoing route map (if all checks are ok)
        let new_route = if let Some(source) = source {
            if let NetworkDevice::InternalRouter(current_device) = net.get_device(current) {
                // create the entry at the current router for the outgoing route map
                let mut entry = BgpRibEntry {
                    route: route.clone(),
                    from_type: self.g[self.edge_idx(current, source)].to_session_type().unwrap(),
                    from_id: source,
                    to_id: Some(target),
                    igp_cost: current_device
                        .igp_forwarding_table
                        .get(&next_hop)
                        .unwrap()
                        .map(|(_, w)| w),
                };

                // apply all outgoing route maps of the source
                for rm in self.rm_out[current.index()].iter() {
                    match rm.apply(entry) {
                        (false, Some(e)) => {
                            entry = e;
                        }
                        (true, Some(e)) => {
                            entry = e;
                            rm_applied = true;
                            break;
                        }
                        (true, None) => {
                            return RouteMapResult::Deny;
                        }
                        _ => unreachable!(),
                    }
                }

                entry.route
            } else {
                route.clone()
            }
        } else {
            route.clone()
        };

        // check if the target is an internal router.
        let new_route = if let NetworkDevice::InternalRouter(target_device) = net.get_device(target)
        {
            // create entry at the target of the incoming route
            let mut entry = BgpRibEntry {
                route: new_route,
                from_type: self.g[self.edge_idx(target, current)].to_session_type().unwrap(),
                from_id: current,
                to_id: None,
                igp_cost: target_device
                    .igp_forwarding_table
                    .get(&next_hop)
                    .unwrap()
                    .map(|(_, w)| w),
            };

            // apply all outgoing route maps of the source
            for rm in self.rm_inc[target.index()].iter() {
                match rm.apply(entry) {
                    (false, Some(e)) => {
                        entry = e;
                    }
                    (true, Some(e)) => {
                        entry = e;
                        rm_applied = true;
                        break;
                    }
                    (true, None) => {
                        return RouteMapResult::Deny;
                    }
                    _ => unreachable!(),
                }
            }

            entry.route
        } else {
            new_route
        };

        if rm_applied {
            if &new_route == route {
                // seems like the route has not changed! Hence, we can treat it as if no route map
                // would have been applied.
                // NOTE we ignore the igp cost on route maps. Hence, this is ok
                RouteMapResult::NoMatch
            } else {
                RouteMapResult::Allow(new_route)
            }
        } else {
            RouteMapResult::NoMatch
        }
    }

    /// Get the index of an edge
    fn edge_idx(&self, from: RouterId, to: RouterId) -> usize {
        edge_idx(from, to, self.n)
    }
}

pub enum RouteMapResult {
    NoMatch,
    Deny,
    Allow(BgpRoute),
}

pub struct BgpGraphNeighborIterator<'a> {
    data: &'a [BgpEdge],
    pos: usize,
}

impl<'a> Iterator for BgpGraphNeighborIterator<'a> {
    type Item = (RouterId, BgpEdge);
    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.data.len() {
            if let Some(e) = self.data[self.pos].to_option() {
                let router_id: RouterId = (self.pos as u32).into();
                self.pos += 1;
                return Some((router_id, e));
            } else {
                self.pos += 1;
            }
        }
        None
    }
}

/// Get the index of the edge in the 2-dim table
fn edge_idx(from: RouterId, to: RouterId, n: usize) -> usize {
    from.index() * n + to.index()
}

/// Type of an edge (BGP Session), including optional route maps
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BgpEdge {
    Up,
    UpExternal,
    Over,
    Down,
    DownExternal,
    None,
}

impl BgpEdge {
    /// Returns self as an option.
    pub fn to_option(&self) -> Option<Self> {
        match self {
            Self::Up => Some(Self::Up),
            Self::UpExternal => Some(Self::UpExternal),
            Self::Over => Some(Self::Over),
            Self::Down => Some(Self::Down),
            Self::DownExternal => Some(Self::DownExternal),
            Self::None => None,
        }
    }

    /// Returns the edge interpreted as a session type. As an example, assume an UP session from
    /// node u to node v. This means, that node u is the client, but node u will have configured
    /// node v as a peer, and hence, the type will be peer.
    pub fn to_session_type(&self) -> Option<BgpSessionType> {
        match self {
            Self::Up => Some(IBgpPeer),
            Self::UpExternal => Some(EBgp),
            Self::Over => Some(IBgpPeer),
            Self::Down => Some(IBgpClient),
            Self::DownExternal => Some(EBgp),
            Self::None => None,
        }
    }

    /// Returns self, but if self is `Over`, it will be changed to `Down`
    pub fn over_to_down(self) -> Self {
        match self {
            Self::Over => Self::Down,
            s => s,
        }
    }

    /// Convert to an u32 for ordering.
    fn to_u32(&self) -> u32 {
        match self {
            BgpEdge::Up => 3,
            BgpEdge::UpExternal => 3,
            BgpEdge::Over => 2,
            BgpEdge::Down => 1,
            BgpEdge::DownExternal => 1,
            BgpEdge::None => 0,
        }
    }
}

impl Ord for BgpEdge {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_u32().cmp(&other.to_u32())
    }
}

impl PartialOrd for BgpEdge {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::example_networks::*;
    use crate::netsim::config::ConfigModifier;
    use crate::netsim::route_map::{RouteMapBuilder, RouteMapDirection};
    use crate::netsim::{AsId, Prefix};
    use maplit::{hashmap, hashset};

    #[test]
    fn bgp_tree_simplenet() {
        let net = SimpleNet::net(0);
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e4 = net.get_router_id("e4").unwrap();
        let bgp = BgpGraph::new(&net);
        assert_eq!(
            bgp.neighbors(e1).collect::<HashMap<_, _>>(),
            hashmap! {r1 => BgpEdge::UpExternal}
        );
        assert_eq!(
            bgp.neighbors(r1).collect::<HashMap<_, _>>(),
            hashmap! {
                e1 => BgpEdge::DownExternal,
                r2 => BgpEdge::Over,
                r3 => BgpEdge::Over,
            }
        );
        assert_eq!(bgp.neighbors(r2).collect::<HashMap<_, _>>(), hashmap! {r1 => BgpEdge::Over});
        assert_eq!(bgp.neighbors(r3).collect::<HashMap<_, _>>(), hashmap! {r1 => BgpEdge::Over});
        assert_eq!(
            bgp.neighbors(r4).collect::<HashMap<_, _>>(),
            hashmap! {e4 => BgpEdge::DownExternal}
        );
        assert_eq!(
            bgp.neighbors(e4).collect::<HashMap<_, _>>(),
            hashmap! {r4 => BgpEdge::UpExternal}
        );
    }

    #[test]
    fn bgp_tree_smallnet() {
        let mut net = SmallNet::net(0);
        let c = SmallNet::final_config(&net, 0);
        net.set_config(&c).unwrap();
        let rr = net.get_router_id("rr").unwrap();
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let b1 = net.get_router_id("b1").unwrap();
        let b2 = net.get_router_id("b2").unwrap();
        let b3 = net.get_router_id("b3").unwrap();
        let b4 = net.get_router_id("b4").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e2 = net.get_router_id("e2").unwrap();
        let e3 = net.get_router_id("e3").unwrap();
        let e4 = net.get_router_id("e4").unwrap();
        let bgp = BgpGraph::new(&net);

        assert_eq!(
            bgp.neighbors(rr).collect::<HashMap<_, _>>(),
            hashmap! {
                r1 => BgpEdge::Down,
                r2 => BgpEdge::Down,
                b4 => BgpEdge::Down,
            }
        );
        assert_eq!(
            bgp.neighbors(r1).collect::<HashMap<_, _>>(),
            hashmap! {
                rr => BgpEdge::Up,
                b1 => BgpEdge::Down,
                b2 => BgpEdge::Down,
            }
        );
        assert_eq!(
            bgp.neighbors(r2).collect::<HashMap<_, _>>(),
            hashmap! {
                rr => BgpEdge::Up,
                b3 => BgpEdge::Down,
                b4 => BgpEdge::Down,
            }
        );
        assert_eq!(
            bgp.neighbors(b1).collect::<HashMap<_, _>>(),
            hashmap! {
                r1 => BgpEdge::Up,
                e1 => BgpEdge::DownExternal,
            }
        );
        assert_eq!(
            bgp.neighbors(b2).collect::<HashMap<_, _>>(),
            hashmap! {
                r1 => BgpEdge::Up,
                e2 => BgpEdge::DownExternal,
            }
        );
        assert_eq!(
            bgp.neighbors(b3).collect::<HashMap<_, _>>(),
            hashmap! {
                r2 => BgpEdge::Up,
                e3 => BgpEdge::DownExternal,
            }
        );
        assert_eq!(
            bgp.neighbors(b4).collect::<HashMap<_, _>>(),
            hashmap! {
                rr => BgpEdge::Up,
                r2 => BgpEdge::Up,
                e4 => BgpEdge::DownExternal,
            }
        );
        assert_eq!(
            bgp.neighbors(e1).collect::<HashMap<_, _>>(),
            hashmap! {
                b1 => BgpEdge::UpExternal,
            }
        );
        assert_eq!(
            bgp.neighbors(e2).collect::<HashMap<_, _>>(),
            hashmap! {
                b2 => BgpEdge::UpExternal,
            }
        );
        assert_eq!(
            bgp.neighbors(e3).collect::<HashMap<_, _>>(),
            hashmap! {
                b3 => BgpEdge::UpExternal,
            }
        );
        assert_eq!(
            bgp.neighbors(e4).collect::<HashMap<_, _>>(),
            hashmap! {
                b4 => BgpEdge::UpExternal,
            }
        );
    }

    #[test]
    fn route_reachability_no_rr() {
        let net = SimpleNet::net(0);
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e4 = net.get_router_id("e4").unwrap();
        let bgp = BgpGraph::new(&net);

        let route1 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65101), AsId(65200)],
            next_hop: e1,
            local_pref: None,
            med: None,
            community: None,
        };
        let route4 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65104), AsId(65200)],
            next_hop: e4,
            local_pref: None,
            med: None,
            community: None,
        };

        let expected = hashmap![
            route1 => RouteReachability(hashset![e1, r1, r2, r3]),
            route4 => RouteReachability(hashset![e4, r4]),
        ];

        let rri = get_all_route_reachability(&net, &bgp);

        assert_eq!(rri, expected);
    }

    #[test]
    fn route_reachability_with_rr_1() {
        let mut net = SimpleNet::net(0);
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e4 = net.get_router_id("e4").unwrap();

        // set incoming route map on r1
        net.apply_modifier(&ConfigModifier::Insert(ConfigExpr::BgpRouteMap {
            router: r1,
            direction: RouteMapDirection::Incoming,
            map: RouteMapBuilder::new()
                .order(10)
                .allow()
                .match_neighbor(e1)
                .set_local_pref(200)
                .build(),
        }))
        .unwrap();

        let bgp = BgpGraph::new(&net);

        let route1 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65101), AsId(65200)],
            next_hop: e1,
            local_pref: None,
            med: None,
            community: None,
        };
        let route2 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65101), AsId(65200)],
            next_hop: e1,
            local_pref: Some(200),
            med: None,
            community: None,
        };
        let route4 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65104), AsId(65200)],
            next_hop: e4,
            local_pref: None,
            med: None,
            community: None,
        };

        let expected = hashmap![
            route1 => RouteReachability(hashset![e1]),
            route2 => RouteReachability(hashset![r1, r2, r3]),
            route4 => RouteReachability(hashset![e4, r4]),
        ];

        let rri = get_all_route_reachability(&net, &bgp);

        assert_eq!(rri, expected);
    }

    #[test]
    fn route_reachability_with_rr_2() {
        let mut net = SimpleNet::net(0);
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e4 = net.get_router_id("e4").unwrap();

        // set incoming route map on r1
        net.apply_modifier(&ConfigModifier::Insert(ConfigExpr::BgpRouteMap {
            router: r2,
            direction: RouteMapDirection::Incoming,
            map: RouteMapBuilder::new()
                .order(10)
                .allow()
                .match_neighbor(r1)
                .set_local_pref(200)
                .build(),
        }))
        .unwrap();

        let bgp = BgpGraph::new(&net);

        let route1 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65101), AsId(65200)],
            next_hop: e1,
            local_pref: None,
            med: None,
            community: None,
        };
        let route2 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65101), AsId(65200)],
            next_hop: e1,
            local_pref: Some(200),
            med: None,
            community: None,
        };
        let route4 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65104), AsId(65200)],
            next_hop: e4,
            local_pref: None,
            med: None,
            community: None,
        };

        let expected = hashmap![
            route1 => RouteReachability(hashset![e1, r1, r3]),
            route2 => RouteReachability(hashset![r2]),
            route4 => RouteReachability(hashset![e4, r4]),
        ];

        let rri = get_all_route_reachability(&net, &bgp);

        assert_eq!(rri, expected);
    }

    #[test]
    fn route_reachability_with_rr_3() {
        let mut net = SimpleNet::net(0);
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e4 = net.get_router_id("e4").unwrap();

        // set incoming route map on r1
        net.apply_modifier(&ConfigModifier::Insert(ConfigExpr::BgpRouteMap {
            router: r1,
            direction: RouteMapDirection::Outgoing,
            map: RouteMapBuilder::new()
                .order(10)
                .allow()
                .match_prefix(Prefix(0))
                .set_local_pref(200)
                .build(),
        }))
        .unwrap();
        net.apply_modifier(&ConfigModifier::Insert(ConfigExpr::BgpRouteMap {
            router: r2,
            direction: RouteMapDirection::Incoming,
            map: RouteMapBuilder::new()
                .order(10)
                .allow()
                .match_neighbor(r1)
                .set_local_pref(300)
                .build(),
        }))
        .unwrap();

        let bgp = BgpGraph::new(&net);

        let route1 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65101), AsId(65200)],
            next_hop: e1,
            local_pref: None,
            med: None,
            community: None,
        };
        let route2 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65101), AsId(65200)],
            next_hop: e1,
            local_pref: Some(200),
            med: None,
            community: None,
        };
        let route3 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65101), AsId(65200)],
            next_hop: e1,
            local_pref: Some(300),
            med: None,
            community: None,
        };
        let route4 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65104), AsId(65200)],
            next_hop: e4,
            local_pref: None,
            med: None,
            community: None,
        };

        let expected = hashmap![
            route1 => RouteReachability(hashset![e1, r1]),
            route2 => RouteReachability(hashset![r3]),
            route3 => RouteReachability(hashset![r2]),
            route4 => RouteReachability(hashset![e4, r4]),
        ];

        let rri = get_all_route_reachability(&net, &bgp);

        assert_eq!(rri, expected);
    }

    #[test]
    fn route_reachability_with_rr_4() {
        let mut net = SimpleNet::net(0);
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e4 = net.get_router_id("e4").unwrap();

        // set incoming route map on r1
        net.apply_modifier(&ConfigModifier::Insert(ConfigExpr::BgpRouteMap {
            router: r1,
            direction: RouteMapDirection::Outgoing,
            map: RouteMapBuilder::new()
                .order(10)
                .allow()
                .match_prefix(Prefix(0))
                .set_local_pref(200)
                .build(),
        }))
        .unwrap();
        net.apply_modifier(&ConfigModifier::Insert(ConfigExpr::BgpRouteMap {
            router: r2,
            direction: RouteMapDirection::Incoming,
            map: RouteMapBuilder::new().order(10).deny().match_neighbor(r1).build(),
        }))
        .unwrap();

        let bgp = BgpGraph::new(&net);

        let route1 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65101), AsId(65200)],
            next_hop: e1,
            local_pref: None,
            med: None,
            community: None,
        };
        let route2 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65101), AsId(65200)],
            next_hop: e1,
            local_pref: Some(200),
            med: None,
            community: None,
        };
        let route4 = BgpRoute {
            prefix: Prefix(0),
            as_path: vec![AsId(65104), AsId(65200)],
            next_hop: e4,
            local_pref: None,
            med: None,
            community: None,
        };

        let expected = hashmap![
            route1 => RouteReachability(hashset![e1, r1]),
            route2 => RouteReachability(hashset![r3]),
            route4 => RouteReachability(hashset![e4, r4]),
        ];

        let rri = get_all_route_reachability(&net, &bgp);

        assert_eq!(rri, expected);
    }
}
