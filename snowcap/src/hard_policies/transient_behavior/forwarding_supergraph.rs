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

//! # Forwarding Supergraph
//!
//! This module contains the implementation of the forwarding supergraph. This is only the graph
//! corresponding with a single instance. All algorithms on it require also a reference to the old
//! graph.

use super::analysis::{NodeEdge, TransientCondition};
use super::RouteReachability;

use crate::netsim::{bgp::BgpRoute, Network, NetworkDevice, Prefix, RouterId};
use std::collections::{HashMap, HashSet};
use std::iter::repeat;

/// Forwarding Supergraph
#[derive(Clone, Debug)]
pub struct ForwardingSupergraph {
    neighbors: Vec<Vec<RouterId>>,
    external: Vec<bool>,
}

impl ForwardingSupergraph {
    /// Build the forwarding supergraph for a single prefix. The routes are filtered in this
    /// funciton (i.e., only the routes with the given prefix are considered).
    pub fn new(net: &Network, prefix: Prefix, rri: &HashMap<BgpRoute, RouteReachability>) -> Self {
        // build the graph
        let mut g: Vec<Vec<RouterId>> = repeat(Vec::new()).take(net.num_devices()).collect();

        // go through each route and update the graph accordingly
        for (route, routers) in rri {
            // skip all routes that are not of the given prefix
            if route.prefix != prefix {
                continue;
            }
            let target = route.next_hop;
            for r_id in routers.iter() {
                // compute the next hop for this potential route
                if let NetworkDevice::InternalRouter(r) = net.get_device(*r_id) {
                    if let Some(nh) =
                        r.igp_forwarding_table.get(&target).cloned().flatten().map(|(nh, _)| nh)
                    {
                        // check if this next hop is already stored in the graph
                        if !g[r_id.index()].contains(&nh) {
                            g[r_id.index()].push(nh);
                        }
                    }
                }
            }
        }

        // insert all static routes
        for r_id in net.get_routers().iter() {
            let r = net.get_device(*r_id).unwrap_internal();
            if let Some(nh) = r.static_routes.get(&prefix) {
                g[r_id.index()] = vec![*nh];
            }
        }

        // build the external routers vector
        let mut external: Vec<bool> = repeat(false).take(net.num_devices()).collect();
        for r in net.get_external_routers() {
            external[r.index()] = true;
        }

        Self { neighbors: g, external }
    }

    /// Check the transient condition on the current supergraph, including the old supergraph
    pub fn check_condition<'n, 'o>(&'n self, old: &'o Self, cond: &TransientCondition) -> bool {
        match cond {
            TransientCondition::FastMode { router_id, groups_pos, groups_neg, .. } => {
                self.cond_algorithm(old, *router_id, groups_pos, groups_neg)
            }
            TransientCondition::SlowMode { router_id, condition, .. } => {
                for path in self.simple_paths(*router_id, old) {
                    if condition.check(&path, Prefix(0)).is_err() {
                        return false;
                    }
                }
                true
            }
        }
    }

    /// Returns an iterator over all simple paths starting from the given router.
    pub fn simple_paths<'n, 'o>(
        &'n self,
        start: RouterId,
        old: &'o Self,
    ) -> ForwardingSupergraphPaths<'n, 'o> {
        ForwardingSupergraphPaths {
            old: &old.neighbors,
            new: &self.neighbors,
            external: &self.external,
            options_stack: vec![vec![start]],
            path_stack: Vec::new(),
        }
    }

    /// Fancy algorithm for checking the condition using a single DFS traversal in $O(n^2)$ time,
    /// instead of enumerating all paths in $O(2^n)$ time.
    ///
    /// # TODO
    /// This algorithm seems to be invalid! Sometimes, it does trigger the condition to be satisfied
    /// even though it actually is not! This does not happen with the naive algorithm of enumerating
    /// all paths! This function must be fixed!
    ///
    /// The following is the counterexample for generating the problem:
    /// ```
    /// # use snowcap::topology_zoo::*;
    /// # use snowcap::hard_policies::*;
    /// # use snowcap::netsim::*;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // generate the network
    /// let switch_l3_gml = format!("{}/test_files/switch.gml", env!("CARGO_MANIFEST_DIR"));
    /// let mut topo = ZooTopology::new(switch_l3_gml, 12)?;
    /// let net = topo.get_net();
    /// let (mut net, final_config, _) = topo.apply_transient_condition_scenario(
    ///     net,
    ///     100,
    ///     true,
    ///     Some(("BelWue", "TIX", "GEANT2")),
    /// )?;
    /// let command = &net.current_config().get_diff(&final_config).modifiers[0];
    ///
    /// // extract the routers for the condition
    /// let p = Prefix(0);
    /// let epfl = net.get_router_id("Lausanne_(EPFL)")?;
    /// let cern = net.get_router_id("CERN_1")?;
    /// let neuchatel = net.get_router_id("Neuchatel")?;
    ///
    /// // build the path condition
    /// let path_cond = PathCondition::Or(vec![
    ///     PathCondition::Edge(epfl, cern),
    ///     PathCondition::Edge(epfl, neuchatel),
    /// ]);
    ///
    /// // build the transient condition and the hard policy
    /// let transient_conds = [Condition::Reachable(epfl, p, Some(path_cond.clone()))];
    /// let mut policy = HardPolicy::globally(vec![Condition::TransientPath(epfl, p, path_cond)]);
    /// policy.set_num_mods_if_none(2);
    /// let mut fw_state = net.get_forwarding_state();
    /// policy.step(&mut net, &mut fw_state)?;
    ///
    /// // Apply the modifier while performing the transient check (simulation, ground thruth)
    /// let num_correct = net.apply_modifier_check_transient(command, &transient_conds, 1000)?;
    /// let ground_truth = num_correct == 1000;
    ///
    /// // Use the transient condition to check if everything is ok
    /// let mut fw_state = net.get_forwarding_state();
    /// policy.step(&mut net, &mut fw_state)?;
    /// let prediction = policy.check();
    ///
    /// assert!(!ground_truth);
    /// assert!(!prediction); // This assertion is not valid!
    /// # Ok(())
    /// # }
    /// ```
    fn cond_algorithm(
        &self,
        old: &Self,
        router: RouterId,
        groups_pos: &[HashSet<NodeEdge>],
        groups_neg: &[HashSet<NodeEdge>],
    ) -> bool {
        // prepare the dfs traversal stack
        // `dfs_seen`: this structure stores the nodes that we have already seen, along with the
        // set of nodes, which every downward path sees.
        let mut dfs_seen: HashMap<RouterId, Vec<bool>> = HashMap::new();
        // `dfs_path`: this is a vector storing the current path, which is used to determine loops.
        let mut dfs_path: Vec<RouterId> = Vec::new();
        // `dfs_stack`: this is the stack storing the choices of yet unexplored neighbors.
        let mut dfs_stack: Vec<Vec<RouterId>> = vec![vec![router]];

        // start the main loop
        while !dfs_stack.is_empty() {
            // get the next router to be considered
            if let Some(node) = dfs_stack.last_mut().unwrap().pop() {
                // check if we have already seen this node
                if dfs_seen.contains_key(&node) {
                    // nothing to do, we have already seen this node.
                    continue;
                }

                // check for possible forwarding loops
                if dfs_path.contains(&node) {
                    // loop detected, ignore the loop and go to the next element in the dfs stack.
                    continue;
                }

                // check if we have reached an external node
                if self.external[node.index()] {
                    // upate the dfs_seen stuff for all groups
                    let groups_state: Vec<bool> = groups_pos
                        .iter()
                        .zip(groups_neg.iter())
                        .map(|(pos, neg)| group_matches_leaf(pos, neg, node))
                        .collect();
                    // 1. get the default true or false value (if there exists an negated condition)
                    dfs_seen.insert(node, groups_state);
                    // go to the next neighbor
                    continue;
                }

                // if we have neither seen this node already, nor the node is external, continue inside
                let mut next_options = self.neighbors[node.index()]
                    .iter()
                    .chain(old.neighbors[node.index()].iter())
                    .cloned()
                    .collect::<Vec<_>>();
                next_options.sort();
                next_options.dedup();
                dfs_stack.push(next_options);
                dfs_path.push(node);
            } else {
                // no next router to consier. update the dfs_seen and go back
                dfs_stack.pop();
                if let Some(last_node) = dfs_path.pop() {
                    // all of its neighbors must be already seen. Hence, we combine the group states
                    // of all neighbors in an AND gate. This means, that a group is only satisfied,
                    // if al possible paths from this node also satisfy this group. If there are no
                    // neighbors, then we just assume that all groups are satisfied.
                    let mut neighbors: Vec<RouterId> = self.neighbors[last_node.index()]
                        .iter()
                        .chain(old.neighbors[last_node.index()].iter())
                        .cloned()
                        .collect::<Vec<_>>();
                    neighbors.sort();
                    neighbors.dedup();
                    // compute the gorups state by iterating over all groups
                    let groups_state = groups_pos
                        .iter()
                        .zip(groups_neg.iter())
                        .enumerate()
                        .map(|(i, (pos, neg))| {
                            // first, combine the group information of all previous values (using
                            // `all`, which returns true if there are no neighbors.)
                            let combined_value = neighbors
                                .iter()
                                .filter_map(|n| {
                                    if let Some(other_state) = dfs_seen.get(n) {
                                        Some(group_matches_edge(
                                            pos,
                                            neg,
                                            last_node,
                                            *n,
                                            other_state[i],
                                        ))
                                    } else {
                                        None
                                    }
                                })
                                .all(|x| x);
                            // then, compute the resulting value by also taking the curren node into
                            // consideration
                            group_matches_node(pos, neg, last_node, combined_value)
                        })
                        .collect();
                    dfs_seen.insert(last_node, groups_state);
                }
            }
        }

        // now, finally, the state of the router itself must be true for all groups!
        dfs_seen.get(&router).unwrap().iter().all(|g| *g)
    }

    /// Returns a representation string of the forwarding supergraph, where the router names have
    /// been inserted
    pub fn repr_with_name(&self, net: &Network) -> String {
        (0..self.neighbors.len())
            .filter(|idx| !self.external[*idx])
            .collect::<Vec<_>>()
            .into_iter()
            .map(|idx| {
                format!(
                    "{} -> [{}]",
                    net.get_router_name((idx as u32).into()).unwrap(),
                    self.neighbors[idx]
                        .iter()
                        .map(|r| net.get_router_name(*r).unwrap())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// This is the logic for checking if a leaf makes a group to become true or not.
///
/// - if pos matches, no matter neg, we are true
/// - if pos does not match, but neg does, we are false
/// - if nothing matches, but neg is non-empty, then we are true
/// - if nothing matches, but neg is empty, then we are false.
fn group_matches_leaf(pos: &HashSet<NodeEdge>, neg: &HashSet<NodeEdge>, node: RouterId) -> bool {
    if pos.contains(&NodeEdge::Node(node)) {
        true
    } else if neg.is_empty() {
        false
    } else {
        !neg.contains(&NodeEdge::Node(node))
    }
}

/// This is the logic for checking if an edge to an already checked node is true
///
/// - if pos matches the edge, no matter neg, we are true
/// - if pos does not match the edge, but neg does, we are false
/// - if nothing matches, take the value from the target.
fn group_matches_edge(
    pos: &HashSet<NodeEdge>,
    neg: &HashSet<NodeEdge>,
    a: RouterId,
    b: RouterId,
    old_value: bool,
) -> bool {
    if pos.contains(&NodeEdge::Edge(a, b)) {
        true
    } else if neg.contains(&NodeEdge::Edge(a, b)) {
        false
    } else {
        old_value
    }
}

/// This is the logic, responsible for determining if a group matches, based on the current node,
/// the parent and the group definition, based on pos and neg.
///
/// - if pos matches, no matter neg, we are true
/// - if pos does not match, but neg does, we are false
/// - if nothing matches, then the old value will remain
fn group_matches_node(
    pos: &HashSet<NodeEdge>,
    neg: &HashSet<NodeEdge>,
    node: RouterId,
    old_value: bool,
) -> bool {
    if pos.contains(&NodeEdge::Node(node)) {
        true
    } else if neg.contains(&NodeEdge::Node(node)) {
        false
    } else {
        old_value
    }
}

pub struct ForwardingSupergraphPaths<'n, 'o> {
    old: &'o Vec<Vec<RouterId>>,
    new: &'n Vec<Vec<RouterId>>,
    external: &'n Vec<bool>,
    options_stack: Vec<Vec<RouterId>>,
    path_stack: Vec<RouterId>,
}

impl<'n, 'o> Iterator for ForwardingSupergraphPaths<'n, 'o> {
    type Item = Vec<RouterId>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.options_stack.is_empty() {
                break None;
            } else {
                let frame = self.options_stack.last_mut().unwrap();
                let mut pop_stack = false;
                let mut new_frame: Option<Vec<RouterId>> = None;
                if let Some(next_option) = frame.pop() {
                    if self.external[next_option.index()] {
                        // external router reached
                        let mut result = self.path_stack.clone();
                        result.push(next_option);
                        break Some(result);
                    } else if self.path_stack.contains(&next_option) {
                        // loop detected, next option is invalid! do nothing
                    } else {
                        // go deeper into the tree
                        self.path_stack.push(next_option);
                        let mut new_options = self.old[next_option.index()]
                            .iter()
                            .chain(self.new[next_option.index()].iter())
                            .cloned()
                            .collect::<Vec<_>>();
                        new_options.sort();
                        new_options.dedup();
                        new_frame = Some(new_options);
                    }
                } else {
                    pop_stack = true;
                }

                // pop the frame if necessary
                if pop_stack {
                    self.options_stack.pop();
                    self.path_stack.pop();
                }

                if let Some(new_frame) = new_frame {
                    self.options_stack.push(new_frame);
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::super::super::condition::{PathCondition, Waypoint};
    use super::*;
    use maplit::hashset;

    #[test]
    fn simple_paths() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 3.into()],
            vec![4.into(), 5.into()],
            vec![4.into()],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![vec![], vec![], vec![], vec![], vec![], vec![]];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };
        let mut paths = fwsg.simple_paths(0.into(), &old);

        assert_eq!(paths.next(), Some(vec![0.into(), 3.into()]));
        assert_eq!(paths.next(), Some(vec![0.into(), 2.into(), 4.into(), 5.into()]));
        assert_eq!(paths.next(), Some(vec![0.into(), 1.into(), 5.into()]));
        assert_eq!(paths.next(), Some(vec![0.into(), 1.into(), 4.into(), 5.into()]));
        assert_eq!(paths.next(), None);
    }

    #[test]
    fn simple_paths_with_loops() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 3.into()],
            vec![4.into(), 5.into(), 1.into()],
            vec![4.into(), 0.into()],
            vec![],
            vec![5.into(), 4.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![vec![], vec![], vec![], vec![], vec![], vec![]];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };
        let mut paths = fwsg.simple_paths(0.into(), &old);

        assert_eq!(paths.next(), Some(vec![0.into(), 3.into()]));
        assert_eq!(paths.next(), Some(vec![0.into(), 2.into(), 4.into(), 5.into()]));
        assert_eq!(paths.next(), Some(vec![0.into(), 1.into(), 5.into()]));
        assert_eq!(paths.next(), Some(vec![0.into(), 1.into(), 4.into(), 5.into()]));
        assert_eq!(paths.next(), None);
    }

    #[test]
    fn simple_paths_union() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 3.into()],
            vec![4.into()],
            vec![],
            vec![],
            vec![5.into(), 4.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![
            vec![2.into(), 3.into()],
            vec![5.into(), 1.into()],
            vec![4.into(), 0.into()],
            vec![],
            vec![],
            vec![],
        ];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };
        let paths = fwsg.simple_paths(0.into(), &old);

        let expected: HashSet<Vec<RouterId>> = hashset![
            vec![0.into(), 3.into()],
            vec![0.into(), 1.into(), 4.into(), 5.into()],
            vec![0.into(), 1.into(), 5.into()],
            vec![0.into(), 2.into(), 4.into(), 5.into()],
        ];

        assert_eq!(paths.collect::<HashSet<Vec<RouterId>>>(), expected);
    }

    #[test]
    fn algorithm_simple() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 4.into()],
            vec![4.into(), 5.into()],
            vec![4.into()],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![vec![], vec![], vec![], vec![], vec![], vec![]];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::FastMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    groups_pos: vec![hashset![NodeEdge::Node(5.into())]],
                    groups_neg: vec![hashset![]]
                }
            ),
            true
        );

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::SlowMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    condition: PathCondition::Positional(vec![
                        Waypoint::Star,
                        Waypoint::Fix(5.into()),
                        Waypoint::Star
                    ])
                }
            ),
            true
        );
    }

    #[test]
    fn algorithm_simple_loop() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 4.into()],
            vec![4.into(), 5.into()],
            vec![4.into(), 1.into()],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![vec![], vec![], vec![], vec![], vec![], vec![]];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::FastMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    groups_pos: vec![hashset![NodeEdge::Node(5.into())]],
                    groups_neg: vec![hashset![]]
                }
            ),
            true
        );

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::SlowMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    condition: PathCondition::Positional(vec![
                        Waypoint::Star,
                        Waypoint::Fix(5.into()),
                        Waypoint::Star
                    ])
                }
            ),
            true
        );
    }

    #[test]
    fn algorithm_simple_black_hole() {
        let external: Vec<bool> = vec![false, false, false, false, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 3.into()],
            vec![4.into(), 5.into()],
            vec![4.into()],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![vec![], vec![], vec![], vec![], vec![], vec![]];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::FastMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    groups_pos: vec![hashset![NodeEdge::Node(5.into())]],
                    groups_neg: vec![hashset![]]
                }
            ),
            true
        );

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::SlowMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    condition: PathCondition::Positional(vec![
                        Waypoint::Star,
                        Waypoint::Fix(5.into()),
                        Waypoint::Star
                    ])
                }
            ),
            true
        );
    }

    #[test]
    fn algorithm_simple_union() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 4.into()],
            vec![],
            vec![4.into()],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 4.into()],
            vec![4.into(), 5.into()],
            vec![],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::FastMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    groups_pos: vec![hashset![NodeEdge::Node(5.into())]],
                    groups_neg: vec![hashset![]]
                }
            ),
            true
        );

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::SlowMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    condition: PathCondition::Positional(vec![
                        Waypoint::Star,
                        Waypoint::Fix(5.into()),
                        Waypoint::Star
                    ])
                }
            ),
            true
        );
    }

    #[test]
    fn algorithm_simple_invalid() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 3.into()],
            vec![4.into(), 5.into()],
            vec![4.into()],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![vec![], vec![], vec![], vec![], vec![], vec![]];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::FastMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    groups_pos: vec![hashset![NodeEdge::Node(5.into())]],
                    groups_neg: vec![hashset![]]
                }
            ),
            false
        );

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::SlowMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    condition: PathCondition::Positional(vec![
                        Waypoint::Star,
                        Waypoint::Fix(5.into()),
                        Waypoint::Star
                    ])
                }
            ),
            false
        );
    }

    #[test]
    fn algorithm_simple_invalid_union() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> =
            vec![vec![], vec![4.into(), 5.into()], vec![], vec![], vec![5.into()], vec![]];
        let old_g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 3.into()],
            vec![],
            vec![4.into()],
            vec![],
            vec![],
            vec![],
        ];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::FastMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    groups_pos: vec![hashset![NodeEdge::Node(5.into())]],
                    groups_neg: vec![hashset![]]
                }
            ),
            false
        );

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::SlowMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    condition: PathCondition::Positional(vec![
                        Waypoint::Star,
                        Waypoint::Fix(5.into()),
                        Waypoint::Star
                    ])
                }
            ),
            false
        );
    }

    #[test]
    fn algorithm_not() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 4.into()],
            vec![4.into(), 5.into()],
            vec![4.into()],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![vec![], vec![], vec![], vec![], vec![], vec![]];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::FastMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    groups_pos: vec![hashset![]],
                    groups_neg: vec![hashset![NodeEdge::Node(3.into())]]
                }
            ),
            true
        );

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::SlowMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    condition: PathCondition::Not(Box::new(PathCondition::Positional(vec![
                        Waypoint::Star,
                        Waypoint::Fix(3.into()),
                        Waypoint::Star
                    ])))
                }
            ),
            true
        );
    }

    #[test]
    fn algorithm_not_invalid() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 3.into()],
            vec![4.into(), 5.into()],
            vec![4.into()],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![vec![], vec![], vec![], vec![], vec![], vec![]];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::FastMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    groups_pos: vec![hashset![]],
                    groups_neg: vec![hashset![NodeEdge::Node(3.into())]]
                }
            ),
            false
        );

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::SlowMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    condition: PathCondition::Not(Box::new(PathCondition::Positional(vec![
                        Waypoint::Star,
                        Waypoint::Fix(3.into()),
                        Waypoint::Star
                    ])))
                }
            ),
            false
        );
    }

    #[test]
    fn algorithm_edge() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 4.into()],
            vec![4.into()],
            vec![4.into()],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![vec![], vec![], vec![], vec![], vec![], vec![]];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::FastMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    groups_pos: vec![hashset![NodeEdge::Edge(4.into(), 5.into())]],
                    groups_neg: vec![hashset![]]
                }
            ),
            true
        );

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::SlowMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    condition: PathCondition::Positional(vec![
                        Waypoint::Star,
                        Waypoint::Fix(4.into()),
                        Waypoint::Fix(5.into()),
                        Waypoint::Star
                    ])
                }
            ),
            true
        );
    }

    #[test]
    fn algorithm_edge_invalid() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 4.into()],
            vec![4.into(), 5.into()],
            vec![4.into()],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![vec![], vec![], vec![], vec![], vec![], vec![]];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::FastMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    groups_pos: vec![hashset![NodeEdge::Edge(4.into(), 5.into())]],
                    groups_neg: vec![hashset![]]
                }
            ),
            false
        );

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::SlowMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    condition: PathCondition::Positional(vec![
                        Waypoint::Star,
                        Waypoint::Fix(4.into()),
                        Waypoint::Fix(5.into()),
                        Waypoint::Star
                    ])
                }
            ),
            false
        );
    }

    #[test]
    fn algorithm_either_two_nodes() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 3.into()],
            vec![4.into(), 5.into()],
            vec![4.into()],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![vec![], vec![], vec![], vec![], vec![], vec![]];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::FastMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    groups_pos: vec![hashset![NodeEdge::Node(5.into()), NodeEdge::Node(3.into())]],
                    groups_neg: vec![hashset![]]
                }
            ),
            true
        );

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::SlowMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    condition: PathCondition::Or(vec![
                        PathCondition::Positional(vec![
                            Waypoint::Star,
                            Waypoint::Fix(3.into()),
                            Waypoint::Star
                        ]),
                        PathCondition::Positional(vec![
                            Waypoint::Star,
                            Waypoint::Fix(5.into()),
                            Waypoint::Star
                        ]),
                    ])
                }
            ),
            true
        );
    }

    #[test]
    fn algorithm_either_two_nodes_invalid() {
        let external: Vec<bool> = vec![false, false, false, true, true, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 3.into()],
            vec![4.into(), 5.into()],
            vec![4.into()],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![vec![], vec![], vec![], vec![], vec![], vec![]];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::FastMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    groups_pos: vec![hashset![NodeEdge::Node(5.into()), NodeEdge::Node(3.into())]],
                    groups_neg: vec![hashset![]]
                }
            ),
            false
        );

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::SlowMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    condition: PathCondition::Or(vec![
                        PathCondition::Positional(vec![
                            Waypoint::Star,
                            Waypoint::Fix(3.into()),
                            Waypoint::Star
                        ]),
                        PathCondition::Positional(vec![
                            Waypoint::Star,
                            Waypoint::Fix(5.into()),
                            Waypoint::Star
                        ]),
                    ])
                }
            ),
            false
        );
    }

    #[test]
    fn algorithm_both_two_nodes() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 4.into()],
            vec![4.into()],
            vec![4.into()],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![vec![], vec![], vec![], vec![], vec![], vec![]];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::FastMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    groups_pos: vec![
                        hashset![NodeEdge::Node(4.into())],
                        hashset![NodeEdge::Node(5.into())]
                    ],
                    groups_neg: vec![hashset![], hashset![]]
                }
            ),
            true
        );

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::SlowMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    condition: PathCondition::And(vec![
                        PathCondition::Positional(vec![
                            Waypoint::Star,
                            Waypoint::Fix(4.into()),
                            Waypoint::Star
                        ]),
                        PathCondition::Positional(vec![
                            Waypoint::Star,
                            Waypoint::Fix(5.into()),
                            Waypoint::Star
                        ]),
                    ])
                }
            ),
            true
        );
    }

    #[test]
    fn algorithm_both_two_nodes_invalid() {
        let external: Vec<bool> = vec![false, false, false, true, false, true];
        let g: Vec<Vec<RouterId>> = vec![
            vec![1.into(), 2.into(), 4.into()],
            vec![4.into(), 5.into()],
            vec![4.into()],
            vec![],
            vec![5.into()],
            vec![],
        ];
        let old_g: Vec<Vec<RouterId>> = vec![vec![], vec![], vec![], vec![], vec![], vec![]];
        let fwsg = ForwardingSupergraph { neighbors: g, external: external.clone() };
        let old = ForwardingSupergraph { neighbors: old_g, external: external.clone() };

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::FastMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    groups_pos: vec![
                        hashset![NodeEdge::Node(4.into())],
                        hashset![NodeEdge::Node(5.into())]
                    ],
                    groups_neg: vec![hashset![], hashset![]]
                }
            ),
            false
        );

        assert_eq!(
            fwsg.check_condition(
                &old,
                &TransientCondition::SlowMode {
                    router_id: 0.into(),
                    cond_id: 0,
                    condition: PathCondition::And(vec![
                        PathCondition::Positional(vec![
                            Waypoint::Star,
                            Waypoint::Fix(3.into()),
                            Waypoint::Star
                        ]),
                        PathCondition::Positional(vec![
                            Waypoint::Star,
                            Waypoint::Fix(5.into()),
                            Waypoint::Star
                        ]),
                    ])
                }
            ),
            false
        );
    }
}
