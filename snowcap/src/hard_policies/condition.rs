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

//! # Condition
//!
//! A condition is treated as one single boolean expression, which can evaluate to either true or
//! false.

use super::{prepare_loop_path, PolicyError};
use crate::netsim::{ForwardingState, Network, NetworkError, Prefix, RouterId};

use itertools::iproduct;
use std::fmt;

/// Condition that can be checked for either being true or false.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Condition that a router can reach a prefix, with optional conditions to the path that is
    /// taken.
    Reachable(RouterId, Prefix, Option<PathCondition>),
    /// Condition that the rotuer cannot reach the prefix, which means that there exists a black
    /// hole somewhere in between the path.
    NotReachable(RouterId, Prefix),
    /// Condition that the router has a route towards the prefix, even if every possible link in
    /// the network fails. Optionally, you can pass in a path condition, requiring the path when
    /// one of the links fail.
    Reliable(RouterId, Prefix, Option<PathCondition>),
    /// Condition on the path during transient state
    TransientPath(RouterId, Prefix, PathCondition),
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reachable(r, p, Some(c)) => {
                write!(f, "Reachability(r{}, prefix {}, condition {})", r.index(), p.0, c)
            }
            Self::Reachable(r, p, None) => {
                write!(f, "Reachability(r{}, prefix {})", r.index(), p.0)
            }
            Self::NotReachable(r, p) => write!(f, "Isolation(r{}, prefix {})", r.index(), p.0),
            Self::Reliable(r, p, Some(c)) => {
                write!(f, "Reliability(r{}, prefix {}, condition {})", r.index(), p.0, c)
            }
            Self::Reliable(r, p, None) => write!(f, "Reliability(r{}, prefix {})", r.index(), p.0),
            Self::TransientPath(r, p, c) => {
                write!(f, "Transient(r{}, prefix {}, condition {})", r.index(), p.0, c)
            }
        }
    }
}

impl Condition {
    /// Return the string representation of the condition, with router names inserted.
    pub fn repr_with_name(&self, net: &Network) -> String {
        match self {
            Self::Reachable(r, p, Some(c)) => format!(
                "Reachability({}, prefix {}, condition {})",
                net.get_router_name(*r).unwrap(),
                p.0,
                c.repr_with_name(net)
            ),
            Self::Reachable(r, p, None) => {
                format!("Reachability({}, prefix {})", net.get_router_name(*r).unwrap(), p.0)
            }
            Self::NotReachable(r, p) => {
                format!("Isolation({}, prefix {})", net.get_router_name(*r).unwrap(), p.0)
            }
            Self::Reliable(r, p, Some(c)) => format!(
                "Reliability({}, prefix {}, condition {})",
                net.get_router_name(*r).unwrap(),
                p.0,
                c.repr_with_name(net)
            ),
            Self::Reliable(r, p, None) => {
                format!("Reliability({}, prefix {})", net.get_router_name(*r).unwrap(), p.0)
            }
            Self::TransientPath(r, p, c) => format!(
                "Transient({}, prefix {}, condition {})",
                net.get_router_name(*r).unwrap(),
                p.0,
                c
            ),
        }
    }

    /// Check the the condition, returning a policy error if it is violated.
    ///
    /// **Warning**: reliability or transient condition is not checked here, but will just return
    /// `Ok`.
    pub fn check(&self, fw_state: &mut ForwardingState) -> Result<(), PolicyError> {
        match self {
            Self::Reachable(r, p, c) => match fw_state.get_route(*r, *p) {
                Ok(path) => match c {
                    None => Ok(()),
                    Some(c) => c.check(&path, *p),
                },
                Err(NetworkError::ForwardingLoop(path)) => {
                    Err(PolicyError::ForwardingLoop { path: prepare_loop_path(path), prefix: *p })
                }
                Err(NetworkError::ForwardingBlackHole(path)) => {
                    Err(PolicyError::BlackHole { router: *path.last().unwrap(), prefix: *p })
                }
                Err(e) => panic!("Unrecoverable error detected: {}", e),
            },
            Self::NotReachable(r, p) => match fw_state.get_route(*r, *p) {
                Err(NetworkError::ForwardingBlackHole(_)) => Ok(()),
                Err(NetworkError::ForwardingLoop(_)) => Ok(()),
                Err(e) => panic!("Unrecoverable error detected: {}", e),
                Ok(path) => Err(PolicyError::UnallowedPathExists { router: *r, prefix: *p, path }),
            },
            Self::Reliable(_, _, _) => Ok(()),
            Self::TransientPath(_, _, _) => Ok(()),
        }
    }

    /// Returns wether the condition is a reliability condition or not.
    pub fn is_reliability(&self) -> bool {
        matches!(self, Self::Reliable(_, _, _))
    }

    /// Returns wether the condition is a reliability condition or not.
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::TransientPath(_, _, _))
    }

    /// Returns the router id of the condition
    pub fn router_id(&self) -> RouterId {
        match self {
            Condition::Reachable(r, _, _) => *r,
            Condition::NotReachable(r, _) => *r,
            Condition::Reliable(r, _, _) => *r,
            Condition::TransientPath(r, _, _) => *r,
        }
    }

    /// Returns the prefix of the condition
    pub fn prefix(&self) -> Prefix {
        match self {
            Condition::Reachable(_, p, _) => *p,
            Condition::NotReachable(_, p) => *p,
            Condition::Reliable(_, p, _) => *p,
            Condition::TransientPath(_, p, _) => *p,
        }
    }
}

/// Condition on the path, which may be either to require that the path passes through a specirif
/// node, or that the path traverses a specific edge.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PathCondition {
    /// Condition that a specific node must be traversed by the path
    Node(RouterId),
    /// Condition that a specific edge must be traversed by the path
    Edge(RouterId, RouterId),
    /// Set of conditions, combined with a logical and
    And(Vec<PathCondition>),
    /// Set of conditions, combined with a logical or
    Or(Vec<PathCondition>),
    /// inverted condition.
    Not(Box<PathCondition>),
    /// Condition for expressing positional waypointing. The vector represents a sequence of
    /// waypoints, including placeholders. It is not possible to express logical OR or AND inside
    /// this positional expression. However, by combining multiple positional expressions, a similar
    /// expressiveness can be achieved.
    Positional(Vec<Waypoint>),
}

impl fmt::Display for PathCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Node(r) => write!(f, "r{}", r.index()),
            Self::Edge(a, b) => write!(f, "[r{} -> r{}]", a.index(), b.index()),
            Self::And(v) => {
                write!(f, "(")?;
                let mut i = v.iter();
                if let Some(c) = i.next() {
                    write!(f, "{}", c)?;
                } else {
                    write!(f, "true")?;
                }
                for c in i {
                    write!(f, " && {}", c)?;
                }
                write!(f, ")")
            }
            Self::Or(v) => {
                write!(f, "(")?;
                let mut i = v.iter();
                if let Some(c) = i.next() {
                    write!(f, "{}", c)?;
                } else {
                    write!(f, "false")?;
                }
                for c in i {
                    write!(f, " || {}", c)?;
                }
                write!(f, ")")
            }
            Self::Not(c) => write!(f, "!{}", c),
            Self::Positional(v) => {
                write!(f, "[")?;
                let mut i = v.iter();
                if let Some(w) = i.next() {
                    write!(f, "{}", w)?;
                }
                for w in i {
                    write!(f, " -> {}", w)?;
                }
                write!(f, "]")
            }
        }
    }
}

impl PathCondition {
    /// Return the string representation of the path condition, with router names inserted.
    pub fn repr_with_name(&self, net: &Network) -> String {
        match self {
            Self::Node(r) => net.get_router_name(*r).unwrap().to_string(),
            Self::Edge(a, b) => format!(
                "[{} -> {}]",
                net.get_router_name(*a).unwrap(),
                net.get_router_name(*b).unwrap()
            ),
            Self::And(v) => {
                let mut result = String::from("(");
                let mut i = v.iter();
                if let Some(c) = i.next() {
                    result.push_str(&c.repr_with_name(net));
                } else {
                    result.push_str("true");
                }
                for c in i {
                    result.push_str(" && ");
                    result.push_str(&c.repr_with_name(net));
                }
                result.push(')');
                result
            }
            Self::Or(v) => {
                let mut result = String::from("(");
                let mut i = v.iter();
                if let Some(c) = i.next() {
                    result.push_str(&c.repr_with_name(net));
                } else {
                    result.push_str("false");
                }
                for c in i {
                    result.push_str(" || ");
                    result.push_str(&c.repr_with_name(net));
                }
                result.push(')');
                result
            }
            Self::Not(c) => format!("!{}", c.repr_with_name(net)),
            Self::Positional(v) => {
                let mut result = String::from("[");
                let mut i = v.iter();
                if let Some(w) = i.next() {
                    result.push_str(&w.repr_with_name(net));
                }
                for w in i {
                    result.push_str(" -> ");
                    result.push_str(&w.repr_with_name(net));
                }
                result.push(']');
                result
            }
        }
    }

    /// Returns wether the path condition is satisfied
    pub fn check(&self, path: &[RouterId], prefix: Prefix) -> Result<(), PolicyError> {
        if match self {
            Self::And(v) => v.iter().all(|c| c.check(path, prefix).is_ok()),
            Self::Or(v) => v.iter().any(|c| c.check(path, prefix).is_ok()),
            Self::Not(c) => c.check(path, prefix).is_err(),
            Self::Node(v) => path.iter().any(|x| x == v),
            Self::Edge(x, y) => {
                let mut iter_path = path.iter().peekable();
                let mut found = false;
                while let (Some(a), Some(b)) = (iter_path.next(), iter_path.peek()) {
                    if x == a && y == *b {
                        found = true;
                    }
                }
                found
            }
            Self::Positional(v) => {
                // algorithm to check if the positional condition matches the path
                let mut p = path.iter();
                let mut v = v.iter();
                'alg: loop {
                    match v.next() {
                        Some(Waypoint::Any) => {
                            // ? operator. Advance the p iterator, and check that it is not none
                            if p.next().is_none() {
                                break 'alg false;
                            }
                        }
                        Some(Waypoint::Fix(n)) => {
                            // The current node must be correct.
                            if p.next() != Some(n) {
                                break 'alg false;
                            }
                        }
                        Some(Waypoint::Star) => {
                            // The star operator is dependent on what comes next. Hence, we match
                            // again on the following waypoint
                            'star: loop {
                                match v.next() {
                                    Some(Waypoint::Any) => {
                                        // again, do the same thing as in the main 'alg loop. But we
                                        // remain in the star search. Notice, that `*?` = `?*`
                                        if p.next().is_none() {
                                            break 'alg false;
                                        }
                                    }
                                    Some(Waypoint::Star) => {
                                        // do nothing, because `**` = `*`
                                    }
                                    Some(Waypoint::Fix(n)) => {
                                        // advance the path until we reach the node. If we reach the
                                        // node, then break out of the star loop. If we don't reach
                                        // the node, then break out of the alg loop with false!
                                        while let Some(u) = p.next() {
                                            if u == n {
                                                break 'star;
                                            }
                                        }
                                        // node was not found!
                                        break 'alg false;
                                    }
                                    None => {
                                        // No next waypoint found. This means, that the remaining
                                        // path does not matter. Break out with true
                                        break 'alg true;
                                    }
                                }
                            }
                        }
                        None => {
                            // If there is no other waypoint, then the path must be empty!
                            break 'alg p.next().is_none();
                        }
                    }
                }
            }
        } {
            // check was successful
            Ok(())
        } else {
            // check unsuccessful
            Err(PolicyError::PathCondition {
                path: path.to_owned(),
                condition: self.clone(),
                prefix,
            })
        }
    }

    /// Private function for doing the recursive cnf conversion. The return has the following form:
    /// The first array represents the expressions combined with a logical AND. each of these
    /// elements represent a logical OR. The first array are regular elements, and the second array
    /// contains the negated elements.
    fn into_cnf_recursive(self) -> Vec<(Vec<Self>, Vec<Self>)> {
        match self {
            Self::Node(a) => vec![(vec![Self::Node(a)], vec![])],
            Self::Edge(a, b) => vec![(vec![Self::Edge(a, b)], vec![])],
            Self::Positional(v) => vec![(vec![Self::Positional(v)], vec![])],
            Self::And(v) => {
                // convert all elements in v, and then combine the outer AND expression into one
                // large AND expression
                v.into_iter().map(|e| e.into_cnf_recursive().into_iter()).flatten().collect()
            }
            Self::Or(v) => {
                // convert all elements in v. Then, combine them by generating the product of all
                // possible combinations of elements in the AND, and or them together into a bigger
                // AND (generates a huge amount of elements!)
                // This is done all in pairs
                let mut v_iter = v.into_iter();
                // If the vector is empty, we prepare a vector with one empty OR expression
                let mut x = v_iter
                    .next()
                    .map(|e| e.into_cnf_recursive())
                    .unwrap_or_else(|| vec![(vec![], vec![])]);
                // then, iterate over all remaining elements, and generate the combination
                for e in v_iter {
                    // generate cnf of e
                    let e = e.into_cnf_recursive();
                    // combine x and e into x
                    x = iproduct!(x.into_iter(), e.into_iter())
                        .map(|((mut xt, mut xf), (mut et, mut ef))| {
                            xt.append(&mut et);
                            xf.append(&mut ef);
                            (xt, xf)
                        })
                        .collect()
                }
                x
            }
            Self::Not(e) => match *e {
                Self::Node(a) => vec![(vec![], vec![Self::Node(a)])],
                Self::Edge(a, b) => vec![(vec![], vec![Self::Edge(a, b)])],
                Self::Positional(v) => vec![(vec![], vec![Self::Positional(v)])],
                // Doube negation
                Self::Not(e) => e.into_cnf_recursive(),
                // Morgan's Law: !(x & y) = !x | !y
                Self::And(v) => Self::Or(v.into_iter().map(|e| Self::Not(Box::new(e))).collect())
                    .into_cnf_recursive(),
                // Morgan's Law: !(x | y) = !x & !y
                Self::Or(v) => Self::And(v.into_iter().map(|e| Self::Not(Box::new(e))).collect())
                    .into_cnf_recursive(),
            },
        }
    }
}

impl Into<PathConditionCNF> for PathCondition {
    fn into(self) -> PathConditionCNF {
        PathConditionCNF::new(self.into_cnf_recursive())
    }
}

/// Part of the positional waypointing argument
#[derive(Clone, Debug, PartialEq, Eq, Hash, Copy)]
pub enum Waypoint {
    /// The next node is always allowed, no matter what it is. This is equivalent to the regular
    /// expression `.` (UNIX style)
    Any,
    /// A sequence of undefined length is allowed (including length 0). This is equivalent to the
    /// regular expression `.*` (UNIX style)
    Star,
    /// At the current position, the path must contain the given node.
    Fix(RouterId),
}

impl fmt::Display for Waypoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => write!(f, "?"),
            Self::Star => write!(f, "..."),
            Self::Fix(r) => write!(f, "r{}", r.index()),
        }
    }
}

impl Waypoint {
    /// Return a string representation of the waypoint, where the router name is inserted.
    pub fn repr_with_name(&self, net: &Network) -> String {
        String::from(match self {
            Self::Any => "?",
            Self::Star => "...",
            Self::Fix(r) => net.get_router_name(*r).unwrap(),
        })
    }
}

/// Path Condition, expressed in Conjunctive Normal Form (CNF), which is a product of sums, or in
/// other words, an AND of ORs.
/// There might be cases, where the PathCondition cannot fully be expressed as a CNF. This is the
/// case if positional requirements are used (like requiring the path * A * B *). In this case,
/// is_cnf is set to false.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PathConditionCNF {
    /// Expression in the CNF form. The first vector contains all groups, which are finally combined
    /// with a logical AND. Every group consists of two vectors, the first containing the non-
    /// negated parts, and the second contains the negated parts, which are finally OR-ed together.
    pub e: Vec<(Vec<PathCondition>, Vec<PathCondition>)>,
    pub(super) is_cnf: bool,
}

impl PathConditionCNF {
    /// Generate a new PathCondition in Conjunctive Normal Form (CNF).
    pub fn new(e: Vec<(Vec<PathCondition>, Vec<PathCondition>)>) -> Self {
        let is_cnf = e
            .iter()
            .map(|(t, f)| t.iter().chain(f.iter()))
            .flatten()
            .all(|c| matches!(c, PathCondition::Node(_) | PathCondition::Edge(_, _)));
        Self { e, is_cnf }
    }

    /// Returns true if the path condition is a valid cnf, and does not contain any positional path
    /// requirements
    pub fn is_cnf(&self) -> bool {
        self.is_cnf
    }

    /// Return the string representation of the path condition, with router names inserted.
    pub fn repr_with_name(&self, net: &Network) -> String {
        let cond: PathCondition = self.clone().into();
        cond.repr_with_name(net)
    }

    /// Returns wether the path condition is satisfied
    pub fn check(&self, path: &[RouterId], prefix: Prefix) -> Result<(), PolicyError> {
        // define the function for checking each ANDed element of the CNF formula
        fn cnf_or(
            vt: &[PathCondition],
            vf: &[PathCondition],
            path: &[RouterId],
            prefix: Prefix,
        ) -> bool {
            vt.iter().any(|c| c.check(path, prefix).is_ok())
                || vf.iter().any(|c| c.check(path, prefix).is_err())
        }

        if self.e.iter().all(|(vt, vf)| cnf_or(vt, vf, path, prefix)) {
            Ok(())
        } else {
            // check unsuccessful
            Err(PolicyError::PathCondition {
                path: path.to_owned(),
                condition: self.clone().into(),
                prefix,
            })
        }
    }
}

impl Into<PathCondition> for PathConditionCNF {
    fn into(self) -> PathCondition {
        PathCondition::And(
            self.e
                .into_iter()
                .map(|(vt, vf)| {
                    PathCondition::Or(
                        // first, convert the vf vector into a vector of Not(...)
                        // Then, chain the vt vector onto it, and generate a large OR expression
                        vf.into_iter()
                            .map(|e| PathCondition::Not(Box::new(e)))
                            .chain(vt.into_iter())
                            .collect(),
                    )
                })
                .collect(),
        )
    }
}

impl fmt::Display for PathConditionCNF {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c: PathCondition = self.clone().into();
        c.fmt(f)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use rand::prelude::*;

    use super::PathCondition::*;
    use super::Waypoint::*;

    #[test]
    fn path_condition_node() {
        let c = Node(0.into());
        assert!(c.check(&vec![1.into(), 0.into(), 2.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![2.into(), 1.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![], Prefix(0)).is_err());
    }

    #[test]
    fn path_condition_edge() {
        let c = Edge(0.into(), 1.into());
        assert!(c.check(&vec![2.into(), 0.into(), 1.into(), 3.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![1.into(), 0.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into(), 2.into(), 1.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![1.into()], Prefix(0)).is_err());
    }

    #[test]
    fn path_condition_not() {
        let c = Not(Box::new(Node(0.into())));
        assert!(c.check(&vec![1.into(), 0.into(), 2.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![2.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![], Prefix(0)).is_ok());
    }

    #[test]
    fn path_condition_or() {
        let c = Or(vec![Node(0.into()), Node(1.into())]);
        assert!(c.check(&vec![0.into(), 2.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![2.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 2.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![3.into(), 2.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![], Prefix(0)).is_err());
        let c = Or(vec![]);
        assert!(c.check(&vec![0.into(), 2.into(), 1.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![], Prefix(0)).is_err());
    }

    #[test]
    fn path_condition_and() {
        let c = And(vec![Node(0.into()), Node(1.into())]);
        assert!(c.check(&vec![0.into(), 2.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![2.into(), 1.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into(), 2.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![3.into(), 2.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![], Prefix(0)).is_err());
        let c = And(vec![]);
        assert!(c.check(&vec![0.into(), 2.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![], Prefix(0)).is_ok());
    }

    fn test_cnf_equivalence(c: PathCondition, n: usize, num_devices: usize) {
        let c_cnf: PathConditionCNF = c.clone().into();
        let c_rev: PathCondition = c_cnf.clone().into();
        let mut rng = rand::thread_rng();
        for _ in 0..n {
            let mut path: Vec<RouterId> = (0..num_devices).map(|x| (x as u32).into()).collect();
            path.shuffle(&mut rng);
            let path: Vec<RouterId> = path.into_iter().take(rng.next_u32() as usize).collect();
            assert_eq!(c.check(&path, Prefix(0)).is_ok(), c_cnf.check(&path, Prefix(0)).is_ok());
            assert_eq!(c.check(&path, Prefix(0)).is_ok(), c_rev.check(&path, Prefix(0)).is_ok());
        }
    }

    #[test]
    fn path_condition_to_cnf_simple() {
        let r0: RouterId = 0.into();
        let r1: RouterId = 1.into();
        test_cnf_equivalence(Node(r0), 1000, 10);
        test_cnf_equivalence(Edge(r0, r1), 1000, 10);
        test_cnf_equivalence(Not(Box::new(Node(r0))), 1000, 10);
        test_cnf_equivalence(And(vec![Node(r0), Node(r1)]), 1000, 10);
        test_cnf_equivalence(Or(vec![Node(r0), Node(r1)]), 1000, 10);
    }

    #[test]
    fn path_condition_to_cnf_complex() {
        let r0: RouterId = 0.into();
        let r1: RouterId = 1.into();
        let r2: RouterId = 2.into();
        test_cnf_equivalence(And(vec![Not(Box::new(Node(r0))), Not(Box::new(Node(r1)))]), 1000, 10);
        test_cnf_equivalence(Or(vec![Not(Box::new(Node(r0))), Not(Box::new(Node(r1)))]), 1000, 10);
        test_cnf_equivalence(
            Or(vec![
                And(vec![Node(r0), Node(r1)]),
                And(vec![Edge(r0, r1), Node(r2)]),
                Not(Box::new(Node(r2))),
            ]),
            1000,
            10,
        );
        test_cnf_equivalence(
            Or(vec![
                And(vec![Node(r0), Node(r1)]),
                And(vec![Not(Box::new(Edge(r0, r1))), Node(r2)]),
                Not(Box::new(Node(r2))),
            ]),
            1000,
            10,
        );
        test_cnf_equivalence(
            Or(vec![
                And(vec![Node(r0), Or(vec![Node(r2), Not(Box::new(Edge(r0, r1)))])]),
                And(vec![Not(Box::new(Edge(r0, r1))), Node(r2)]),
                Not(Box::new(Node(r2))),
            ]),
            1000,
            10,
        );
        test_cnf_equivalence(
            Not(Box::new(Or(vec![
                And(vec![Node(r0), Or(vec![Node(r2), Not(Box::new(Edge(r0, r1)))])]),
                And(vec![Not(Box::new(Edge(r0, r1))), Node(r2)]),
                Not(Box::new(Node(r2))),
            ]))),
            1000,
            10,
        );
    }

    #[test]
    fn path_positional_single_any() {
        let c = Positional(vec![Any]);
        assert!(c.check(&vec![0.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into(), 1.into()], Prefix(0)).is_err());
    }

    #[test]
    fn path_positional_single_star() {
        let c = Positional(vec![Star]);
        assert!(c.check(&vec![], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into(), 2.into()], Prefix(0)).is_ok());
    }

    #[test]
    fn path_positional_single_fix() {
        let c = Positional(vec![Fix(0.into())]);
        assert!(c.check(&vec![0.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![1.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into(), 1.into()], Prefix(0)).is_err());
    }

    #[test]
    fn path_positional_star_any() {
        let c = Positional(vec![Star, Any]);
        assert!(c.check(&vec![], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into(), 2.into()], Prefix(0)).is_ok());
        let c = Positional(vec![Any, Star]);
        assert!(c.check(&vec![], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into(), 2.into()], Prefix(0)).is_ok());
    }

    #[test]
    fn path_positional_star_star() {
        let c = Positional(vec![Star, Star]);
        assert!(c.check(&vec![], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into(), 2.into()], Prefix(0)).is_ok());
    }

    #[test]
    fn path_positional_any_any() {
        let c = Positional(vec![Any, Any]);
        assert!(c.check(&vec![], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into(), 2.into()], Prefix(0)).is_err());
    }

    #[test]
    fn path_positional_star_fix() {
        let c = Positional(vec![Star, Fix(0.into())]);
        assert!(c.check(&vec![], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![1.into(), 0.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![2.into(), 1.into(), 0.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![2.into(), 1.into(), 0.into(), 3.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![2.into(), 1.into(), 3.into()], Prefix(0)).is_err());
    }

    #[test]
    fn path_positional_fix_star() {
        let c = Positional(vec![Fix(0.into()), Star]);
        assert!(c.check(&vec![], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into(), 2.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![3.into(), 0.into(), 1.into(), 2.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![3.into(), 1.into(), 2.into()], Prefix(0)).is_err());
    }

    #[test]
    fn path_positional_star_fix_star() {
        let c = Positional(vec![Star, Fix(0.into()), Star]);
        assert!(c.check(&vec![], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into(), 2.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![3.into(), 0.into(), 1.into(), 2.into()], Prefix(0)).is_ok());
        assert!(c
            .check(&vec![3.into(), 4.into(), 0.into(), 1.into(), 2.into()], Prefix(0))
            .is_ok());
        assert!(c.check(&vec![3.into(), 1.into(), 2.into()], Prefix(0)).is_err());
    }

    #[test]
    fn path_positional_star_fix_fix_star() {
        let c = Positional(vec![Star, Fix(0.into()), Fix(1.into()), Star]);
        assert!(c.check(&vec![], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into(), 2.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![3.into(), 0.into(), 1.into(), 2.into()], Prefix(0)).is_ok());
        assert!(c
            .check(&vec![3.into(), 4.into(), 0.into(), 1.into(), 2.into()], Prefix(0))
            .is_ok());
        assert!(c.check(&vec![3.into(), 1.into(), 2.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![3.into(), 0.into(), 2.into(), 1.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![3.into(), 2.into(), 1.into()], Prefix(0)).is_err());
    }

    #[test]
    fn path_positional_star_fix_any_fix_star() {
        let c = Positional(vec![Star, Fix(0.into()), Any, Fix(1.into()), Star]);
        assert!(c.check(&vec![], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into(), 1.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into(), 1.into(), 2.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![3.into(), 0.into(), 1.into(), 2.into()], Prefix(0)).is_err());
        assert!(c
            .check(&vec![3.into(), 4.into(), 0.into(), 1.into(), 2.into()], Prefix(0))
            .is_err());
        assert!(c.check(&vec![3.into(), 1.into(), 2.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![3.into(), 0.into(), 2.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c
            .check(&vec![3.into(), 0.into(), 2.into(), 1.into(), 3.into()], Prefix(0))
            .is_ok());
        assert!(c
            .check(&vec![3.into(), 0.into(), 2.into(), 3.into(), 1.into()], Prefix(0))
            .is_err());
        assert!(c.check(&vec![3.into(), 2.into(), 1.into()], Prefix(0)).is_err());
    }

    #[test]
    fn path_positional_star_fix_star_fix_star() {
        let c = Positional(vec![Star, Fix(0.into()), Star, Fix(1.into()), Star]);
        assert!(c.check(&vec![], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![0.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![0.into(), 1.into(), 2.into()], Prefix(0)).is_ok());
        assert!(c.check(&vec![3.into(), 0.into(), 1.into(), 2.into()], Prefix(0)).is_ok());
        assert!(c
            .check(&vec![3.into(), 4.into(), 0.into(), 1.into(), 2.into()], Prefix(0))
            .is_ok());
        assert!(c.check(&vec![3.into(), 1.into(), 2.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![3.into(), 0.into(), 2.into(), 1.into()], Prefix(0)).is_ok());
        assert!(c
            .check(&vec![3.into(), 0.into(), 2.into(), 1.into(), 3.into()], Prefix(0))
            .is_ok());
        assert!(c
            .check(&vec![3.into(), 0.into(), 2.into(), 3.into(), 1.into()], Prefix(0))
            .is_ok());
        assert!(c.check(&vec![3.into(), 2.into(), 1.into()], Prefix(0)).is_err());
        assert!(c.check(&vec![3.into(), 2.into(), 1.into(), 0.into()], Prefix(0)).is_err());
    }
}
