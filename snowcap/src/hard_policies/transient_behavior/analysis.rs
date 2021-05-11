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

//! # Transient State Analysis
//!
//! This module contains the algorithms to analyze transient state.

use super::super::condition::{Condition, PathCondition, PathConditionCNF};
use super::{
    forwarding_supergraph::ForwardingSupergraph, get_all_route_reachability, BgpGraph,
    RouteReachability,
};

use crate::netsim::{bgp::BgpRoute, Network, Prefix, RouterId};
use std::collections::{HashMap, HashSet};

/// Structure to manage and check transient state. This module supports functions to push and pop
/// from a stack, in order to maintain history (similar to `HardPolicy`).
#[derive(Debug, Clone)]
pub struct TransientStateAnalyzer {
    /// All analyzers for each prefix
    analyzers: Vec<PrefixAnalyzer>,
}

impl TransientStateAnalyzer {
    /// Generate a new TransientStateAnalyzer, not yet initialized!
    pub fn new(prefixes: &HashSet<Prefix>, conditions: &[Condition]) -> Self {
        Self { analyzers: prefixes.iter().map(|p| PrefixAnalyzer::new(*p, conditions)).collect() }
    }

    /// Perform a step, preparing the analyzer to perform the analysis
    pub fn step(&mut self, net: &Network) {
        let bgp_graph = BgpGraph::new(net);
        let rri = get_all_route_reachability(net, &bgp_graph);
        self.analyzers.iter_mut().for_each(|a| a.step(net, &rri));
    }

    /// Undo a step, going back in the history
    pub fn undo(&mut self) {
        self.analyzers.iter_mut().for_each(|a| a.undo());
    }

    /// reset the analyzer, clearing all information
    pub fn reset(&mut self) {
        self.analyzers.iter_mut().for_each(|a| a.reset());
    }

    /// Perform the check of all transient conditions, returning a vector containing all checked
    /// conditions, with their id and wether they succeeded
    pub fn check(&self) -> Vec<(usize, bool)> {
        self.analyzers.iter().map(|a| a.check().into_iter()).flatten().collect()
    }

    /// Represent the transient state analyzer as a string
    pub fn repr_with_name(&self, net: &Network) -> String {
        self.analyzers.iter().map(|a| a.repr_with_name(net)).collect::<Vec<_>>().join("\n")
    }
}

/// Structure to manage an check the transient state of a single prefix in the network.
#[derive(Debug, Clone)]
struct PrefixAnalyzer {
    /// Prefix which is checked
    prefix: Prefix,
    /// Forwarding Supergraph at every position
    fwsg: Vec<ForwardingSupergraph>,
    /// Conditions for this prefix
    conds: Vec<TransientCondition>,
}

impl PrefixAnalyzer {
    fn new(prefix: Prefix, conditions: &[Condition]) -> Self {
        Self {
            prefix,
            fwsg: Vec::new(),
            conds: conditions
                .iter()
                .enumerate()
                .filter_map(|(i, c)| match c {
                    Condition::TransientPath(r, p, c) if *p == prefix => {
                        Some(TransientCondition::new(*r, i, c))
                    }
                    _ => None,
                })
                .collect(),
        }
    }

    /// Prepare the data by performing a single step
    fn step(&mut self, net: &Network, rri: &HashMap<BgpRoute, RouteReachability>) {
        self.fwsg.push(ForwardingSupergraph::new(net, self.prefix, rri));
    }

    /// Undo the last call to step
    fn undo(&mut self) {
        self.fwsg.pop();
    }

    /// reset the prefix analyzer
    fn reset(&mut self) {
        self.fwsg.clear();
    }

    /// check all conditions
    fn check(&self) -> Vec<(usize, bool)> {
        let mut result = Vec::new();
        if self.fwsg.len() >= 2 {
            let cur = self.fwsg.last().unwrap();
            let old = self.fwsg.get(self.fwsg.len() - 2).unwrap();
            for cond in self.conds.iter() {
                result.push((cond.cond_id(), cur.check_condition(old, cond)));
            }
        }
        result
    }

    /// Represent the prefix analyzer as a string, with the router names inserted
    fn repr_with_name(&self, net: &Network) -> String {
        format!(
            "\nPrefix {}\nconds:\n{}\nold graph:\n{}\nnew graph:\n{}\n",
            self.prefix.0,
            self.conds.iter().map(|c| c.repr_with_name(net)).collect::<Vec<_>>().join("\n"),
            self.fwsg
                .iter()
                .rev()
                .skip(1)
                .next()
                .map(|fwsg| fwsg.repr_with_name(net))
                .unwrap_or_else(|| "----".to_string()),
            self.fwsg
                .last()
                .map(|fwsg| fwsg.repr_with_name(net))
                .unwrap_or_else(|| "----".to_string()),
        )
    }
}

#[derive(Debug, Clone)]
pub enum TransientCondition {
    #[allow(dead_code)]
    FastMode {
        router_id: RouterId,
        cond_id: usize,
        groups_pos: Vec<HashSet<NodeEdge>>,
        groups_neg: Vec<HashSet<NodeEdge>>,
    },
    SlowMode {
        router_id: RouterId,
        cond_id: usize,
        condition: PathCondition,
    },
}

impl TransientCondition {
    fn repr_with_name(&self, net: &Network) -> String {
        match self {
            TransientCondition::FastMode { router_id, groups_pos, groups_neg, .. } => {
                let condition = PathConditionCNF {
                    e: (0..groups_pos.len())
                        .map(|i| {
                            (
                                groups_pos[i].iter().cloned().map(|c| c.into()).collect(),
                                groups_neg[i].iter().cloned().map(|c| c.into()).collect(),
                            )
                        })
                        .collect(),
                    is_cnf: true,
                };
                format!(
                    "{}: {}",
                    net.get_router_name(*router_id).unwrap(),
                    condition.repr_with_name(net)
                )
            }
            TransientCondition::SlowMode { router_id, condition, .. } => {
                format!(
                    "{}: {}",
                    net.get_router_name(*router_id).unwrap(),
                    condition.repr_with_name(net)
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeEdge {
    Node(RouterId),
    Edge(RouterId, RouterId),
}

impl Into<PathCondition> for NodeEdge {
    fn into(self) -> PathCondition {
        match self {
            Self::Node(v) => PathCondition::Node(v),
            Self::Edge(a, b) => PathCondition::Edge(a, b),
        }
    }
}

impl NodeEdge {
    /// generate a NodeEdge from a path condition. This function panics if the path condition is not
    /// either a node or an edge
    #[allow(dead_code)]
    fn from(p: &PathCondition) -> Self {
        match p {
            PathCondition::Node(v) => Self::Node(*v),
            PathCondition::Edge(a, b) => Self::Edge(*a, *b),
            _ => panic!("PathCondition is neither a node nor an edge"),
        }
    }
}

impl TransientCondition {
    fn new(router_id: RouterId, cond_id: usize, cond: &PathCondition) -> Self {
        Self::SlowMode { router_id, cond_id, condition: cond.clone() }
        // TODO The fast mode is deactivated, since it is implemented wrongly! The algorithm
        // needs to be fixed, before the code below can be commented in!
        /*
        let cnf: PathConditionCNF = cond.clone().into();
        if cnf.is_cnf() {
            Self::FastMode {
                router_id,
                cond_id,
                groups_pos: cnf
                    .e
                    .iter()
                    .map(|(v, _)| {
                        v.iter().map(|c| NodeEdge::from(c)).collect::<HashSet<NodeEdge>>()
                    })
                    .collect(),
                groups_neg: cnf
                    .e
                    .iter()
                    .map(|(_, v)| {
                        v.iter().map(|c| NodeEdge::from(c)).collect::<HashSet<NodeEdge>>()
                    })
                    .collect(),
            }
        } else {
            Self::SlowMode { router_id, cond_id, condition: cond.clone() }
        }
        */
    }

    fn cond_id(&self) -> usize {
        match self {
            Self::FastMode { cond_id, .. } => *cond_id,
            Self::SlowMode { cond_id, .. } => *cond_id,
        }
    }
}
