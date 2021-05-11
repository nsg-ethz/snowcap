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

//! # Hard Policies
//!
//! This module contains all necessary structures and tools to generate hard policies as Linear
//! Temporal Logic.
//!
//! # Policy Language
//!
//! This module defines a languate with which to express complex policies. On the top-level, it is
//! based on [Linear Temporal Logic (LTL)](https://en.wikipedia.org/wiki/Linear_temporal_logic). In
//! LTL, there exists propositional variables, in the following called conditions, boolean operators
//! and temporal modal operators.
//!
//! ## Temporal Modal Operators
//!
//! For the LTL, we use a sequence of states, i.e., the sequence of converged states during
//! reconfiguration. At each of these states, every propositional variable can either be satsicied
//! or not. The following operators exist
//!
//! - $\phi$ (Now): $\phi$ has to hold in the current state
//! - $\mathbf{X}\ \phi$ (Next): $\phi$ has to hold in the next state
//! - $\mathbf{F}\ \phi$ (Finally): $\phi$ has to hold eventually, somewhere on the subsequent path
//! - $\mathbf{G}\ \phi$ (Globally): $\phi$ has to hold on the entire subsequent path, including the
//!   current state
//! - $\psi\ \mathbf{U}\ \phi$ (Until): $\psi$ has to hold at least until $\phi$ becomes true, which
//!   must hold at the current or any future position. Note, that $\psi$ and $phi$ don't necessarily
//!   need to hold at the same time.
//! - $\psi\ \mathbf{R}\ \phi$ (Release): $\phi$ has to be true until and including the point where
//!   $\psi$ first becomes true. If $\psi$ never becomes true, then $\phi$ must hold forever.
//! - $\psi\ \mathbf{W}\ \phi$ (Weak Until): $\psi$ has to hold at least until $\phi$ becomes true. If
//!   $\phi$ never becomes true, then $\psi$ must hold forever. Note, that $\psi$ and $phi$ don't
//!   necessarily need to hold at the same time.
//! - $\psi\ \mathbf{M}\ \phi$ (Strong Release): $\phi$ has to be true until and including the point where
//!   $\psi$ first becomes true. $\psi$ must hold eventually.
//!
//! ## Logical Operators
//!
//! The following logical operators are supported
//!
//! - $\neg \psi$ (Not)
//! - $\phi \land \psi$ (And)
//! - $\phi \lor \psi$ (Or)
//! - $\phi \oplus \psi$ (xor)
//! - $\phi \Rightarrow \psi$ (Implies)
//! - $\phi \iff \psi$ (If and only if)
//!
//! ## Propositional Variables (Conditions)
//!
//! Propositional variables are types of conditions which can be evaluated on the current state (or
//! when considering the current and last state) of the network (i.e., forwarding state). The
//! following conditions are possible:
//!
//! - $\mathbf{V}_{(r, p, c)}$ (Valid path / Reachability): Router $r$ is able to reach prefix $p$
//!   without encountering any black hole, or forwarding loop. Aitionally, the path condition $c$
//!   must hold, if it is provided.
//! - $\mathbf{I}_{(r, p)}$ (Isolation): Router $r$ is not able to reach prefix $p$, there exists
//!   a black hole on the path.
//! - $\mathbf{V}_{(r, p, c)}^+$ (Reliability): Router $r$ is able to reach prefix $p$ in the case
//!   where a single link fails. This condition is checked by simulating a link failure at every
//!   link in the network. The path condition $c$ (if given) must hold on every chosen path for all
//!   possible link failures.
//! - $\mathbf{T}_{(r, p, c)}$ (Transient behavior): During convergence to reach the current state,
//!   every possible path, that router $r$ might choose to reach $p$ does satisfy the path condition
//!   $c$. Note, that this condition cannot check, that during convergence, no forwarding loop or
//!   black hole may appear. Only the path can be checked.
//!
//! ## Path Condition
//!
//! The path condition is a condition on the path. This is an expression, which can contain boolean
//! operators $\land$ (and), $\lor$ (or) and $\neg$ (not). In addition, the expression may contain
//! router $r \in \mathcal{V}$, which needs to be reached in the path, an edge $e \in \mathcal{V}
//! \times \mathcal{V}$, or a positional condition. This positional constraint can be expressed as
//! a sequence of the alphabet $\lbrace \ast, ?\rbrace \cup \mathcal{V}$. Here, $?$ means any single
//! router, and $\ast$ means a sequence of any length (can be of length zero) of any router. This
//! can be used to express more complex conditions on the path. As an example, the positional
//! condition $[\ast, a, ?, b, c, \ast]$ means that the path must first reach $a$, then visit any
//! other node, then $b$ must be traversed, immediately followed by $c$. This always matches on the
//! entire path, and not just on a small part of it.
//!
//! # Transient Behavior
//!
//! For transient behavior, we cannot guarantee the absence of black holes or forwarding loops. In
//! fact, if we would be able, then we would be able to guarantee to the network operator, that the
//! network is in a more reliable state during reconfiguration, than it is during normal operation.
//! This obviously makes no sence. Nevertheless, we are able to guarantee that if there exists a
//! path, then this path will satisfy the specified conditions.
//!
//! ## Computation Complexity
//!
//! In the following, we use the notation $n = |\mathcal{V}|$ to be the number of routers in the
//! network. For this algorithm, the following things need to be computed:
//!
//! 1. First, we need to compute the route reachability graph $G_R(r)$ for all routes
//!    $r \in \mathcal{R}$. This graph $G_R$ can be computed using a DFS traversal on the BGP graph
//!    $G_{BGP} = (\mathcal{V}, E_{BGP})$ which takes $O(|\mathcal{V}| + |E_{BGP}|) = O(n^2)$. Hence,
//!    computing all route reachability graphs takes a total of $O(|\mathcal{R}| \cdot n^2)$ time.
//! 2. Then, we create the forwarding supergraph $G_{pcs} = (\mathcal{V}, E_{pcs})$ in
//!    $O(n \cdot |\mathcal{R}|)$ time.
//! 3. Finally, we need to perform a DFS traversal on $G_{pcs}$ for every node $v \in \mathcal{V}$.
//!    This takes a total of $O(|\mathcal{V}| + |E_{pcs}|)$ for every node $v \in \mathcal{V}$,
//!    which leads to a complexity $O(|\mathcal{V}| \cdot (|\mathcal{V}| + |E_{pcs}|)) = O(n^3)$.
//!
//! Collecting all these complexities yields the following total time complexity:
//!
//! $$O(n^2 (n + |\mathcal{R}|))$$
//!
//! However, if the path conditions cannot be checked using a single DFS traversal (i.e., if they
//! include positional conditions), then we need to enumerate over all possible paths in the
//! network (which is, on a DAG $\mathcal{O}(n^2)$), which leads to a total time complexity of:
//!
//! $$O(2^n + n^2 \mathcal{R})$$
//!
//! ## Algorithm Description
//!
//! The following algorithm is described on a per-prefix basis. We assume that no router is able
//! to change the prefix, and that prefixes canot overlap. Based on this assumption, we can safely
//! check the conditions for every prefix individually. Note, that the constraint on non-overlaping
//! prefixes can be satisfied by creating multiple prefixes for the different ranges.
//!
//! ### Definitions
//!
//! In the following, any variable annotated with $\square^-$ means that this is at the state
//! before the small delta reconfiguration, and $\square^+$ means after the reconfiguration.
//!
//! - $\mathcal{S}^\pm$: State of the network before or after the delta reconfiguration.
//! - $v \in \mathcal{V}$: All routers in the network, $\mathcal{V}_{ext} \subset \mathcal{V}$:
//!   external routers
//! - $r \in \mathcal{R}^\pm$: All routes in the network before and after the delta reconfiguration.
//!   Routes can be compared: $r_1 >_v r_2$ means that $r_1$ is preferred over $r_2$ at node $v$
//!   (the node is important here, because IGP cost is one of the criteria to choose a route).
//!   Additionally, we call $\mathcal{R} = \mathcal{R}^- \cup \mathcal{R}^+$ as the set of all
//!   routes before and after the reconfiguration combined.
//! - $rri(r)$: Route reachability of route $r$ is the set of nodes $rri(r) \subseteq \mathcal{V}$,
//!   which may be reached by this route. It is constructed by considering both states before and
//!   after the reconfiguration. The $rri(r)$ respects the rules of route dissemination and route
//!   maps.
//! - $nh(v, r)$: Next hop at node $v$, when router $v$ chooses route $r$.
//! - $pcr(v)$: possibly considered routes: Maps each router in the network to a set of possible
//!   routes, which might get activated and deactivated as transient behavior during convergence of
//!   delta reconfiguration.
//! - $G_{pcr} = (\mathcal{V}, E_{pcr})$: Forwarding supergraph that contains the forwarding state
//!   of every possible intermediate state (and the forwarding state of impossible intermediate
//!   states).
//!
//! ### Algorithm
//!
//! The algorithm can be split into several different parts. On a high level, we first extract the
//! route reachability information for each route (a set of routers which can theoretically learn
//! this route). Based on this, we generate a forwarding supergraph containing all possible
//! forwarding graphs. Finally, we perform a graph traversal on this grpah, to verify the path
//! constraints.
//!
//! As the first step, we need to prepare the BGP graph $G^\pm_{bgp} = (\mathcal{V}, E^\pm_{bgp},
//! L_{bgp})$. In this graph, every edge may be either labeled as up $U$, over $O$, or down $D$:
//! $e \in \lbrace U, O, D \rbrace$. Note, that we need two different graphs, one before and one
//! after the reconfiguration. both graphs may be different.
//!
//! As a next step, we compute the the route reachability information $rri(r)$ of every route
//! $r \in \mathcal{R}$. This information is the set of nodes that may be reached by route r based
//! on the BGP configuration. Notice, that we compute the reachability of $r \in \mathcal{R}$ on
//! both $G_{bgp}^-$ and $G_{bgp}^+$, and call the union of the reached nodes as
//! $rri(r) = reach(G_{bgp}^-) \cup reach(G_{bgp}^+)$. While traversing the bgp graph, if a route
//! map is encountered from node $u$ to $v$ (either an outgoing route map on node $u$ or an incoming
//! route map on node $v$, or both), we don't continue traversing node $v$, but we generate a new
//! route $r'$ and start traversing at node $v$. *(Implementation Detail: we generate $G_{bgp}$ and
//! compute $rri(r)$ both before and after the reconfiguration.)*
//!
//! Finally, we build the forwarding supergraph $G_{pcr}$, by looking at all possibly considered
//! routes $pcr(v) = \lbrace r \in \mathcal{R} \mid v \in rri^-(r) \cup rri^+(r) \rbrace$ and
//! looking at their next hop, both before and after the reconfiguration. The final graph
//! $G_{pcr} = (\mathcal{V}, E_{pcr})$ contains an edge from node $u$ to $v$, if and only if there
//! exists a route $r \in pcr(u)$, which might reach node $u$, and which would cause router $u$ to
//! send packets to node $v$. More formally,
//!
//! $$ E_{pcr} = \big\lbrace (u, v) \mid v \in \lbrace nh^-(u, r) \cup nh^+(u, r) \mid r \in
//! pcr(u) \rbrace \big\rbrace$$
//!
//! After having constructed the forwarding supergraph, we can check the path conditions of all
//! nodes in the network. If the condition contains a positional path expression, we have to
//! enumerate all possible paths, and check the condition on all these paths. This leads to a time
//! complexity of $\mathcal{O}(2^n)$ (due to finding all possible paths on a directed acyclic
//! graph). However, if there exists no such condition, we can check all conditions for a single
//! node in one single graph traversal (in a depth-first-search procedure). This then yields a time
//! complexity of only $\mathcal{O}(n^3)$.
//!
//! For the rest of the algorithm explenation, we look at a single node $v$, and assume that the
//! path conditions do not contain and positional expressions. We continue by transforming the
//! conditions into Conjunctive Normal Form (CNF). Such expressions are are written as product of
//! sums, or as an AND of ORs. Let the condition for node $v$ be:
//!
//! $$(a \lor b \lor \neg c) \land (d \lor e) \land \ldots = \phi_1 \land \phi_2 \land \ldots$$
//!
//! For each group of expressions, combined with an logical or (in the following called $\phi_i$),
//! we generate a gorup $g_i$, which we use to remember during DFS traversal if at least one of
//! these expressions in $g_i$ are satisfied, which means that $phi_i$ is satisfied. Finally, we can
//! determine, if the entire expression is satisfied, namely then, when all these groups are true.
//!
//! We perform the DFS traversal. If we encounter a black hole somewhere, we ignore this branch, and
//! go back without changing anything. *TODO: We might be able to do something more intelligent here
//! but we need to discuss this.*. If we encounter a loop, we similarly do nothing, and just ignore
//! this branch. However, if we reach the target, we update the groups in which this target is in.
//! Every time, we backtrack because every possible next hop has been explored, we combine the
//! information of the groups as follows:
//!
//! - If every branch satisfies the group, then the current node also satisfies the group.
//! - If at least one branch does not satisfy a group, then this group is also not satisfied for the
//!   current node.
//!
//! This information then propagates back to the root $v$. If all groups are still satisfied, we can
//! declare the condition as being satisfied. If not, then it is violated.
//!
//! ### IGP Link Weight Change
//!
//! If the IGP link weight changes, we apply the insight from the paper on [Disruption Free Topology
//! Reconfiguration in OSPF Networks](https://ieeexplore.ieee.org/document/4215601), in which the
//! authors prove that there always exists a sequence of link weights which guarantee no forwarding
//! loops in any transient state.
//!
//! So, our algorithm generates many different reconfiguraiton expressions to incrementally change
//! the link weight until we reach the desired value, treating all of these as individual changes.
//! While preparing the forwarding supergraph, we consider the next hop based on the IGP metric
//! before and after the reconfiguration.
//!
//! ### Why we cannot check for black holes
//!
//! Using this approach, it is impossible to check if black holes might appear, just by looking at
//! the state before and after the delta reconfiguration. The reason is that even though the chosen
//! route has not changed from before to after, it might still disappear temporarily. This is
//! highlighted by the following two examples:
//!
//! *Example 1*: Consier the following network and configuration:
//!
//! ![counter_example](https://n.ethz.ch/~sctibor/images/TransientStateCounterExample.svg)
//!
//! In this example, we apply one single modification, that the community of a route is changed from
//! 0 to 666 at router `r5`. Router `r3` only allows routes with community 666, while router `r4`
//! denies these routes. Finally, `r2` resets the community to 0, which hides everything that might
//! happen to the router `r1`. Depending on the order of messages, `r1` might not notice any change,
//! or `r1` might experience a transient black hole.
//!
//! This example can be extended to violate arbitrary conditions, such as the necessity of
//! traversing a specific link. Hence, our algorithm is not able to check for transient black holes,
//! and the algorithm must always consider all routes, that might be reached by a node.
//!
//! *Example 2*: Consier the following network and configuration:
//!
//! ![counter_example](https://n.ethz.ch/~sctibor/images/TransientStateCounterExample2.svg)
//!
//! In this example, we add a new route map to router `b2`. Now, let's consider router `rx`. First,
//! notice that the router `rx` can only receive the route propagated by `b1` with
//! `local_pref = 120`. This route is prefered by router `r1`, and hence, router `rx` will receive
//! this route. As soon as we introduce the modification, there are two orderings which we must
//! consider:
//!
//! 1. `r4` receives the new route with the community 666 before `r3`, and send its new route with
//!    `local_pref=150` towards `r2`. Then, `r2` will forward the route towards `r1`, which will
//!    then retract the route from `e1` for `rx`. Now, we have a black hole at `rx`.
//!
//! 2. `r3` receives the new route with the community 666 before `r4`, and sen its new route with
//!    `local_pref=200` towards `r2`. Since this route is advertised via peer session, `r2` will not
//!    notice `r1` about the change, and hence, `rx` will still know the same prefix.
//!
//! In the final state, no matter the ordering of messages, the router `r2` will prefer the route
//! form `r3`, and ignore the one from `r4`. Hence, router `r2` does not advertise a route towards
//! `r1`, and `rx` can reach the prefix via `e1`.
//!
//! ### Why we cannot improve the overapproximation
//!
//! Using this approach, it is impossible to improve the overapproximation of the forwarding
//! supergraph, just by looking at the state before and after the delta reconfiguration. The only
//! way, how the overapproximation can theoretically be improved, is by considering less routes at
//! any node $v$. However, the two examples above can be extended, such that a much wrose route is
//! selected temporarily, even if the same better route is selected before and after the delta
//! reconfiguration. This clearly shows that we cannot go fancy by only considering e.g., routes
//! that are better than the old known route (if this one is still known after reconfiguration).

mod condition;
mod ltl;
mod transient_behavior;

pub use condition::{Condition, PathCondition, Waypoint};
pub use ltl::{HardPolicy, LTLBoolean, LTLModal, LTLOperator, WatchErrors};
use transient_behavior::TransientStateAnalyzer;

use crate::netsim::{Network, Prefix, RouterId};
//use crate::transient_behavior::TransientError;

use std::collections::VecDeque;
use thiserror::Error;

/// # Hard Policy Error
/// This indicates which policy resulted in the policy failing.
#[derive(Debug, Error, PartialEq, Eq, Hash, Clone)]
pub enum PolicyError {
    /// Forwarding Black Hole occured
    #[error("Black Hole at router {router:?} for {prefix:?}")]
    BlackHole {
        /// The router where the black hole exists.
        router: RouterId,
        /// The prefix for which the black hole exists.
        prefix: Prefix,
    },

    /// Forwarding Loop occured
    #[error("Forwarding Loop {path:?} for {prefix:?}")]
    ForwardingLoop {
        /// The loop, only containing the relevant routers.
        path: Vec<RouterId>,
        /// The prefix for which the forwarding loop exists.
        prefix: Prefix,
    },

    /// PathRequirement was not satisfied
    #[error("Invalid Path for {prefix:?}: path: {path:?} condition: {condition}")]
    PathCondition {
        /// The actual path taken in the network
        path: Vec<RouterId>,
        /// The expected path
        condition: PathCondition,
        /// the prefix for which the wrong path exists.
        prefix: Prefix,
    },

    /// A route is present, where it should be dropped somewhere
    #[error("Router {router:?} should not be able to reach {prefix:?} but the following path is valid: {path:?}")]
    UnallowedPathExists {
        /// The router who should not be able to reach the prefix
        router: RouterId,
        /// The prefix which should not be reached
        prefix: Prefix,
        /// the path with which the router can reach the prefix
        path: Vec<RouterId>,
    },

    /// Reliability Constraint is not satisfied
    #[error("Router {router:?} has no backup path for {prefix:?} when link ({link_a:?} -> {link_b:?}) fails.")]
    NotReliable {
        /// Router for thich the reliability is violated
        router: RouterId,
        /// Prefix for which the reliability is violated
        prefix: Prefix,
        /// Critical link, where a link failure causes the unreliability (router a)
        link_a: RouterId,
        /// Critical link, where a link failure causes the unreliability (router b)
        link_b: RouterId,
    },

    /// Condition during reliability check is not satisfied
    #[error("Backup path for {prefix:?} when link ({link_a:?} -> {link_b:?}) fails is not satisfied: path: {path:?}, codition: {condition}")]
    ReliabilityCondition {
        /// Router for which the reliability condition is violated
        path: Vec<RouterId>,
        /// Condition which is violated during reliability check, when the given link fails.
        condition: PathCondition,
        /// Prefix for which the reliability condition is violated
        prefix: Prefix,
        /// Critical link, where a link failure causes the reliability constraint to fail (router a)
        link_a: RouterId,
        /// Critical link, where a link failure causes the reliability constraint to fail (router b)
        link_b: RouterId,
    },

    /// No Convergence
    #[error("Network did not converge")]
    NoConvergence,

    /// Transient Behavior Violation
    #[error("Transient behavior on router {router:?} for {prefix:?} may be violated: {condition}")]
    TransientBehavior {
        /// Router for which transient behavior might be violated
        router: RouterId,
        /// Prefix for which the router might violate transient behavior
        prefix: Prefix,
        /// Path condition which may be violated in transient behavior
        condition: PathCondition,
    },
}

impl PolicyError {
    /// Get a string representing the policy error, where all router names are inserted.
    pub fn repr_with_name(&self, net: &Network) -> String {
        match self {
            PolicyError::BlackHole { router, prefix } => format!(
                "Black hole for prefix {} at router {}",
                prefix.0,
                net.get_router_name(*router).unwrap(),
            ),
            PolicyError::ForwardingLoop { path, prefix } => format!(
                "Forwarding loop for prefix {}: {} -> {}",
                prefix.0,
                path.iter()
                    .map(|r| net.get_router_name(*r).unwrap())
                    .collect::<Vec<&str>>()
                    .join(" -> "),
                net.get_router_name(*path.first().unwrap()).unwrap(),
            ),
            PolicyError::PathCondition { path, condition, prefix } => format!(
                "Path condition invalidated for prefix {}: path: {}, condition: {}",
                prefix.0,
                path.iter()
                    .map(|r| net.get_router_name(*r).unwrap())
                    .collect::<Vec<&str>>()
                    .join(" -> "),
                condition.repr_with_name(net)
            ),
            PolicyError::UnallowedPathExists { router, prefix, path } => format!(
                "Router {} can reach unallowed prefix {} via path [{}]",
                net.get_router_name(*router).unwrap(),
                prefix.0,
                path.iter()
                    .map(|r| net.get_router_name(*r).unwrap())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            PolicyError::NotReliable { router, prefix, link_a, link_b} => format!(
                "Router {} cannot reach prefix {} when link [{} -> {}] fails",
                net.get_router_name(*router).unwrap(),
                prefix.0,
                net.get_router_name(*link_a).unwrap(),
                net.get_router_name(*link_b).unwrap(),
            ),
            PolicyError::ReliabilityCondition { path, condition, prefix, link_a, link_b} => format!(
                "Reliability condition {} violated for prefix {} with path {} when link [{} -> {}] fails",
                condition.repr_with_name(net),
                prefix.0,
                path.iter()
                    .map(|r| net.get_router_name(*r).unwrap())
                    .collect::<Vec<&str>>()
                    .join(" -> "),
                net.get_router_name(*link_a).unwrap(),
                net.get_router_name(*link_b).unwrap(),
            ),
            PolicyError::NoConvergence => String::from("No Convergence"),
            PolicyError::TransientBehavior {router, prefix, condition} => format!(
                "Transient behavior of router {} for prefix {} may be violated! condition: {}",
                net.get_router_name(*router).unwrap(),
                prefix.0,
                condition.repr_with_name(net),
            )
        }
    }
}

/// Extracts only the loop from the path.
/// The last node in the path must already exist previously in the path. If no loop exists in the
/// path, then an unrecoverable error occurs.
///
/// TODO: this is inefficient. We should not collect into a VecDeque, rotate and collect back, but
/// we should push only the elements that are needed in the correct order, without allocating a
/// VecDeque.
fn prepare_loop_path(path: Vec<RouterId>) -> Vec<RouterId> {
    let len = path.len();
    let loop_router = path[len - 1];
    let mut first_loop_router: Option<usize> = None;
    for (i, r) in path.iter().enumerate().take(len - 1) {
        if *r == loop_router {
            first_loop_router = Some(i);
            break;
        }
    }
    let first_loop_router =
        first_loop_router.unwrap_or_else(|| panic!("Loop-Free path given: {:?}", path));
    let mut loop_unordered: VecDeque<RouterId> =
        path.into_iter().skip(first_loop_router + 1).collect();

    // order the loop, such that the smallest router ID starts the loop
    let lowest_pos = loop_unordered
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.cmp(b.1))
        .map(|(i, _)| i)
        .expect("Loop is empty");

    loop_unordered.rotate_left(lowest_pos);
    loop_unordered.into_iter().collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::example_networks::*;
    use crate::netsim::{
        config::{ConfigExpr::*, ConfigModifier::*},
        BgpSessionType::*,
        Prefix,
    };

    #[test]
    fn static_policy_reachability() {
        let mut net = SimpleNet::net(2);
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let mut policy =
            HardPolicy::reachability(vec![r1, r2, r3, r4].iter(), vec![Prefix(0)].iter());

        let mut fw_state = net.get_forwarding_state();
        policy.step(&mut net, &mut fw_state).unwrap();
        assert!(policy.check_overwrite_finish(false));

        net.apply_modifier(&Remove(BgpSession { source: r1, target: r2, session_type: IBgpPeer }))
            .unwrap();

        let mut fw_state = net.get_forwarding_state();
        policy.step(&mut net, &mut fw_state).unwrap();
        assert!(policy.check_overwrite_finish(false));

        net.apply_modifier(&Remove(BgpSession { source: r2, target: r4, session_type: IBgpPeer }))
            .unwrap();

        let mut fw_state = net.get_forwarding_state();
        policy.step(&mut net, &mut fw_state).unwrap();
        assert!(!policy.check_overwrite_finish(true));
    }

    #[test]
    fn static_policy_link_required() {
        let mut net = SimpleNet::net(2);
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e4 = net.get_router_id("e4").unwrap();

        let p = Prefix(0);

        let mut policy = HardPolicy::globally(vec![
            Condition::Reachable(r1, p, Some(PathCondition::Edge(r1, e1))),
            Condition::Reachable(r2, p, Some(PathCondition::Edge(r2, r1))),
            Condition::Reachable(r3, p, Some(PathCondition::Edge(r2, r1))),
            Condition::Reachable(r4, p, Some(PathCondition::Edge(r2, r1))),
        ]);

        net.apply_modifier(&Remove(BgpSession { source: r4, target: e4, session_type: EBgp }))
            .unwrap();

        net.apply_modifier(&Update {
            from: IgpLinkWeight { source: r1, target: r2, weight: 1.0 },
            to: IgpLinkWeight { source: r1, target: r2, weight: 0.1 },
        })
        .unwrap();

        net.apply_modifier(&Update {
            from: IgpLinkWeight { source: r2, target: r1, weight: 1.0 },
            to: IgpLinkWeight { source: r2, target: r1, weight: 0.1 },
        })
        .unwrap();

        net.apply_modifier(&Update {
            from: IgpLinkWeight { source: r3, target: r1, weight: 1.0 },
            to: IgpLinkWeight { source: r3, target: r1, weight: 2.0 },
        })
        .unwrap();

        net.apply_modifier(&Update {
            from: IgpLinkWeight { source: r1, target: r3, weight: 1.0 },
            to: IgpLinkWeight { source: r1, target: r3, weight: 2.0 },
        })
        .unwrap();

        let mut fw_state = net.get_forwarding_state();
        policy.step(&mut net, &mut fw_state).unwrap();
        assert!(policy.check_overwrite_finish(false));

        net.apply_modifier(&Update {
            from: IgpLinkWeight { source: r1, target: r2, weight: 0.1 },
            to: IgpLinkWeight { source: r1, target: r2, weight: 1.0 },
        })
        .unwrap();

        let mut fw_state = net.get_forwarding_state();
        policy.step(&mut net, &mut fw_state).unwrap();
        assert!(policy.check_overwrite_finish(false));

        net.apply_modifier(&Update {
            from: IgpLinkWeight { source: r2, target: r1, weight: 0.1 },
            to: IgpLinkWeight { source: r2, target: r1, weight: 1.0 },
        })
        .unwrap();

        let mut fw_state = net.get_forwarding_state();
        policy.step(&mut net, &mut fw_state).unwrap();
        assert!(!policy.check_overwrite_finish(true));
    }

    #[test]
    fn static_policy_reliability() {
        let mut net = SimpleNet::net(2);
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let e4 = net.get_router_id("e4").unwrap();

        let p = Prefix(0);

        let mut policy = HardPolicy::globally(vec![
            Condition::Reachable(r1, p, None),
            Condition::Reachable(r2, p, None),
            Condition::Reachable(r3, p, None),
            Condition::Reachable(r4, p, None),
            Condition::Reliable(r1, p, None),
            Condition::Reliable(r2, p, None),
            Condition::Reliable(r3, p, None),
            Condition::Reliable(r4, p, None),
        ]);

        let mut fw_state = net.get_forwarding_state();
        policy.step(&mut net, &mut fw_state).unwrap();
        assert!(policy.check_overwrite_finish(false));

        net.apply_modifier(&Remove(BgpSession { source: r2, target: r4, session_type: IBgpPeer }))
            .unwrap();

        let mut fw_state = net.get_forwarding_state();
        policy.step(&mut net, &mut fw_state).unwrap();
        assert!(!policy.check_overwrite_finish(true));

        net.apply_modifier(&Remove(BgpSession { source: r4, target: e4, session_type: EBgp }))
            .unwrap();

        let mut fw_state = net.get_forwarding_state();
        policy.step(&mut net, &mut fw_state).unwrap();
        assert!(!policy.check_overwrite_finish(true));
    }

    #[test]
    fn dynamic_policy_reachability_new_firewall() {
        let mut net = SimpleNet::net(2);
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e4 = net.get_router_id("e4").unwrap();

        let p = Prefix(0);

        // the policy requires that all routers can always reach the prefix. After the
        // reconfiguration, we would like for all traffic to go via route r2 --> r1 --> e1.
        let mut policy = HardPolicy::new(
            vec![
                Condition::Reachable(r1, p, None),
                Condition::Reachable(r2, p, None),
                Condition::Reachable(r3, p, None),
                Condition::Reachable(r4, p, None),
                Condition::Reachable(r1, p, Some(PathCondition::Edge(r1, e1))),
                Condition::Reachable(r2, p, Some(PathCondition::Edge(r2, r1))),
                Condition::Reachable(r3, p, Some(PathCondition::Edge(r2, r1))),
                Condition::Reachable(r4, p, Some(PathCondition::Edge(r2, r1))),
            ],
            LTLModal::Now(Box::new(LTLBoolean::And(vec![
                Box::new(LTLModal::Globally(Box::new(LTLBoolean::And(vec![
                    Box::new(0),
                    Box::new(1),
                    Box::new(2),
                    Box::new(3),
                ])))),
                Box::new(LTLModal::Finally(Box::new(LTLModal::Globally(Box::new(
                    LTLBoolean::And(vec![Box::new(4), Box::new(5), Box::new(6), Box::new(7)]),
                ))))),
            ]))),
        );

        net.apply_modifier(&Remove(BgpSession { source: r4, target: e4, session_type: EBgp }))
            .unwrap();

        let mut fw_state = net.get_forwarding_state();
        policy.step(&mut net, &mut fw_state).unwrap();
        eprintln!("{}", policy.repr_with_name(&net));
        assert!(policy.check_overwrite_finish(false));
        assert!(!policy.check_overwrite_finish(true));

        net.apply_modifier(&Update {
            from: IgpLinkWeight { source: r3, target: r1, weight: 1.0 },
            to: IgpLinkWeight { source: r3, target: r1, weight: 2.0 },
        })
        .unwrap();

        net.apply_modifier(&Update {
            from: IgpLinkWeight { source: r1, target: r3, weight: 1.0 },
            to: IgpLinkWeight { source: r1, target: r3, weight: 2.0 },
        })
        .unwrap();

        let mut fw_state = net.get_forwarding_state();
        policy.step(&mut net, &mut fw_state).unwrap();
        assert!(policy.check_overwrite_finish(false));
        assert!(!policy.check_overwrite_finish(true));

        net.apply_modifier(&Update {
            from: IgpLinkWeight { source: r1, target: r2, weight: 1.0 },
            to: IgpLinkWeight { source: r1, target: r2, weight: 0.1 },
        })
        .unwrap();

        net.apply_modifier(&Update {
            from: IgpLinkWeight { source: r2, target: r1, weight: 1.0 },
            to: IgpLinkWeight { source: r2, target: r1, weight: 0.1 },
        })
        .unwrap();

        let mut fw_state = net.get_forwarding_state();
        policy.step(&mut net, &mut fw_state).unwrap();
        assert!(policy.check_overwrite_finish(false));
        assert!(policy.check_overwrite_finish(true));
    }
}
