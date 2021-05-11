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

//! # Linear Temporal Logic

use super::condition::Condition;
use super::{PolicyError, TransientStateAnalyzer};
use crate::netsim::{
    config::{ConfigExpr, ConfigModifier},
    ForwardingState, Network, NetworkError, Prefix, RouterId,
};

use itertools::iproduct;
use std::boxed::Box;
use std::collections::HashSet;
use std::fmt;
use std::ops::{BitAnd, BitOr, BitXor, Not};

/// Type alias for comfortable handling of the watch errors
pub type WatchErrors = (Vec<usize>, Vec<Option<PolicyError>>);

/// # Linear Temporal Logic
///
/// This structure holds the entire LTL expression. It is stored as a vector of propositional
/// a history of which constraints were satisfied, and an expression which can check the property
/// based on the history.
#[derive(Debug, Clone)]
pub struct HardPolicy {
    /// Conditional variables of the hard poicy
    pub prop_vars: Vec<Condition>,
    reliability: Vec<usize>,
    history: Vec<Vec<bool>>,
    error_history: Vec<Vec<Option<PolicyError>>>,
    /// LTL Expression
    pub expr: LTLModal,
    num_mods: Option<usize>,
    tsa: Option<TransientStateAnalyzer>,
}

impl HardPolicy {
    /// Helper function to generate the reachability policy
    pub fn reachability<'r, 'p, R, P>(routers: R, prefixes: P) -> Self
    where
        R: Iterator<Item = &'r RouterId> + Clone,
        P: Iterator<Item = &'p Prefix> + Clone,
    {
        let prop_vars: Vec<Condition> =
            iproduct!(routers, prefixes).map(|(r, p)| Condition::Reachable(*r, *p, None)).collect();
        Self::globally(prop_vars)
    }

    /// Create a new Linear Temporal Logic Hard Policy, where all conditions supplied need to be
    /// satisfied all the time.
    pub fn globally(prop_vars: Vec<Condition>) -> Self {
        let expr = LTLModal::Globally(Box::new(LTLBoolean::And(
            (0..prop_vars.len()).map(|i| Box::new(i) as Box<dyn LTLOperator>).collect(),
        )));
        Self::new(prop_vars, expr)
    }

    /// Create a new Linear Temporal Logic Hard Policy. The LTL expression will have the following
    /// form, where $\phi_i$ are all propositional variables in the second argument, and $\psi_i$
    /// are all from the third argument.
    ///
    /// $$ \Big( \bigwedge \phi_i \Big) \ \mathbf{U}\ \mathbf{G}\ \Big( \bigwedge \psi_i \Big) $$
    ///
    pub fn until_globally(prop_vars: Vec<Condition>, phi: &[usize], psi: &[usize]) -> Self {
        let expr = LTLModal::Until(
            Box::new(LTLBoolean::And(
                phi.iter().map(|i| Box::new(*i) as Box<dyn LTLOperator>).collect(),
            )),
            Box::new(LTLModal::Globally(Box::new(LTLBoolean::And(
                psi.iter().map(|i| Box::new(*i) as Box<dyn LTLOperator>).collect(),
            )))),
        );
        Self::new(prop_vars, expr)
    }

    /// Create a new Linear Temporal Logic Hard Policy
    pub fn new(prop_vars: Vec<Condition>, expr: LTLModal) -> Self {
        let reliability = prop_vars
            .iter()
            .enumerate()
            .filter(|(_, v)| v.is_reliability())
            .map(|(i, _)| i)
            .collect();
        let prefixes = prop_vars.iter().map(|c| c.prefix()).collect();
        let tsa = if prop_vars.iter().any(|c| c.is_transient()) {
            Some(TransientStateAnalyzer::new(&prefixes, &prop_vars))
        } else {
            None
        };
        Self {
            prop_vars,
            reliability,
            history: Vec::new(),
            error_history: Vec::new(),
            expr,
            num_mods: None,
            tsa,
        }
    }

    /// Sets the total number of modifiers, if it was not yet set before. If it is already set, then
    /// nothing will change. This function returns `true` if there was no previous value.
    pub fn set_num_mods_if_none(&mut self, num_mods: usize) -> bool {
        if self.num_mods.is_none() {
            self.num_mods = Some(num_mods);
            true
        } else {
            false
        }
    }

    /// Applies a next step to the LTL model
    pub fn step(
        &mut self,
        net: &mut Network,
        state: &mut ForwardingState,
    ) -> Result<(), NetworkError> {
        // prepare new state
        let mut new_state = Vec::with_capacity(self.prop_vars.len());
        let mut new_error: Vec<Option<PolicyError>> = Vec::with_capacity(self.prop_vars.len());

        // check all prop_vars
        for v in self.prop_vars.iter() {
            match v.check(state) {
                Ok(()) => {
                    new_state.push(true);
                    new_error.push(None);
                }
                Err(e) => {
                    new_state.push(false);
                    new_error.push(Some(e));
                }
            }
        }

        // Next, we need to check the reliability
        if !self.reliability.is_empty() {
            // iterate over all links in the network, deactivating them ony by one
            for (a, b) in net.links_symmetric().cloned().collect::<Vec<_>>() {
                // let link a -- b fail
                let mut num_undo = 0;
                match net.apply_modifier(&ConfigModifier::Remove(ConfigExpr::IgpLinkWeight {
                    source: a,
                    target: b,
                    weight: 1.0,
                })) {
                    Ok(_) => num_undo += 1,
                    Err(NetworkError::NoConvergence) | Err(NetworkError::ConvergenceLoop(_, _)) => {
                        num_undo += 1
                    }
                    Err(NetworkError::ConfigError(_)) => {}
                    Err(e) => return Err(e),
                }
                match net.apply_modifier(&ConfigModifier::Remove(ConfigExpr::IgpLinkWeight {
                    source: b,
                    target: a,
                    weight: 1.0,
                })) {
                    Ok(_) => num_undo += 1,
                    Err(NetworkError::NoConvergence) | Err(NetworkError::ConvergenceLoop(_, _)) => {
                        num_undo += 1
                    }
                    Err(NetworkError::ConfigError(_)) => {}
                    Err(e) => return Err(e),
                }

                // perform the check
                let mut fw_state = net.get_forwarding_state();
                for c_id in self.reliability.iter() {
                    let check_result = if let Some(Condition::Reliable(r, p, c)) =
                        self.prop_vars.get(*c_id)
                    {
                        match fw_state.get_route(*r, *p) {
                            Ok(path) => match c {
                                None => Ok(()),
                                Some(c) => match c.check(&path, *p) {
                                    Ok(()) => Ok(()),
                                    Err(PolicyError::PathCondition { path, condition, prefix }) => {
                                        Err(PolicyError::ReliabilityCondition {
                                            path,
                                            condition,
                                            prefix,
                                            link_a: a,
                                            link_b: b,
                                        })
                                    }
                                    // Condition::check can only return either Ok or Err(PolicyError::PathCondition)
                                    Err(_) => unreachable!(),
                                },
                            },
                            Err(NetworkError::ForwardingLoop(_))
                            | Err(NetworkError::ForwardingBlackHole(_)) => {
                                Err(PolicyError::NotReliable {
                                    router: *r,
                                    prefix: *p,
                                    link_a: a,
                                    link_b: b,
                                })
                            }
                            Err(e) => panic!("Unrecoverable error detected: {}", e),
                        }
                    } else {
                        // this is the else statements from getting the prop_var. This obviously is
                        // not reachable, becaues we prepare the reliability array internally, and
                        // don't expose it to the outside.
                        unreachable!();
                    };
                    match check_result {
                        Ok(()) => {}
                        Err(e) => {
                            new_state[*c_id] = false;
                            new_error[*c_id] = Some(e);
                        }
                    }
                }

                // undo the action
                for _ in 0..num_undo {
                    net.undo_action()?;
                }
            }
        }

        // then, perform the step on the transient state analyzer, and do the check
        if self.tsa.is_some() {
            let tsa = self.tsa.as_mut().unwrap();
            tsa.step(net);
            for (c_id, result) in tsa.check() {
                if result {
                    // behavior is OK, nothing to do
                } else {
                    new_state[c_id] = false;
                    new_error[c_id] = match self.prop_vars.get(c_id) {
                        Some(Condition::TransientPath(r, p, c)) => {
                            Some(PolicyError::TransientBehavior {
                                router: *r,
                                prefix: *p,
                                condition: c.clone(),
                            })
                        }
                        _ => unreachable!(),
                    };
                }
            }
        }

        // finally, push the changes to the stack
        self.history.push(new_state);
        self.error_history.push(new_error);

        Ok(())
    }

    /// Undoes the last call to step
    pub fn undo(&mut self) {
        self.history.pop();
        self.error_history.pop();
        if self.tsa.is_some() {
            self.tsa.as_mut().unwrap().undo();
        }
    }

    /// Reset the strucutre, such that no state exists.
    pub fn reset(&mut self) {
        self.history.clear();
        self.error_history.clear();
        if self.tsa.is_some() {
            self.tsa.as_mut().unwrap().reset();
        }
    }

    /// Computes the condition using Linear Temporal Logic, and returns if the expression holds. The
    /// finish flag will be determined automatically if the number of calls to `step` subtracted by
    /// the number of calls to `undo` (i.e., the length of the history) is equal to the number of
    /// modifiers plus one (since the initial state will also be part of the history.) Finish will
    /// be set to `false` if the number of modifiers was not set yet.
    pub fn check(&self) -> bool {
        let finish = self.num_mods.map(|m| m + 1 == self.history.len()).unwrap_or(false);
        if finish {
            self.expr.check(&self.history[..])
        } else {
            !self.expr.partial(&self.history[..]).is_false()
        }
    }

    /// Computes the condition using Linear Temporal Logic, and returns if the expression holds. The
    /// provided `finish` flag will be used to determine the method (`check` vs `partial`).
    pub fn check_overwrite_finish(&self, finish: bool) -> bool {
        if finish {
            self.expr.check(&self.history[..])
        } else {
            !self.expr.partial(&self.history[..]).is_false()
        }
    }

    /// Compute the set of propositional variables that need to be watched in order to change the
    /// outcome of the current state of the checker. This function should only be used when the
    /// result is either false or undefined.
    pub fn get_watch(&self) -> Vec<usize> {
        let finish = self.num_mods.map(|m| m + 1 == self.history.len()).unwrap_or(false);
        self.get_watch_overwrite_finish(finish)
    }

    /// Compute the watch and get the last errors of the respective watch. This is a helpful utility
    /// for getting the errors, and comparing it later.
    pub fn get_watch_errors(&self) -> WatchErrors {
        let watch = self.get_watch();
        let errors = self.last_errors_of_watch(&watch);
        (watch, errors)
    }

    /// Compute the set of propositional variables that need to be watched in order to change the
    /// outcome of the current state of the checker. This function should only be used when the
    /// result is either false or undefined. The `finish` flagg will be used to determine the method
    /// (`watch` or `watch_partial`).
    pub fn get_watch_overwrite_finish(&self, finish: bool) -> Vec<usize> {
        let mut watch = if finish {
            self.expr.watch(&self.history[..])
        } else {
            self.expr.watch_partial(&self.history[..])
        };
        watch.sort();
        watch.dedup();
        watch
    }

    /// Returns the set of all errors of all propositional variables. It might be the case that
    /// these errors don't necessarily contribute to the result of the last call to check.
    pub fn last_errors(&self) -> HashSet<PolicyError> {
        self.error_history
            .last()
            .map(|v| v.iter().filter_map(|e| e.clone()).collect())
            .unwrap_or_default()
    }

    /// Get the error associated with the watch provided to this method. The watch is an array
    /// containing the indices of all propositional variables which need to be considered. The
    /// resulting vector contains the error of the propositional variable, in the same order as
    /// provided. If a propositional variable is true, the error corresponding to this variable
    /// is None.
    pub fn last_errors_of_watch(&self, watch: &[usize]) -> Vec<Option<PolicyError>> {
        if let Some(last_e) = self.error_history.last() {
            watch.iter().map(|&i| last_e[i].clone()).collect()
        } else {
            Vec::new()
        }
    }

    /// This method compares the current state of the checker with a previous state, which was
    /// extracted using the method `get_watch_errors`. This funciton returns `true` if the errors
    /// are the same, and `false` if the errors are different. Only the errors from the watch are
    /// compared.
    pub fn compare_watch_errors(&self, watch_errors: &WatchErrors) -> bool {
        let watch = &watch_errors.0;
        let errors = &watch_errors.1;

        let new_errors = self.last_errors_of_watch(watch);
        errors == &new_errors
    }

    /// Represent the LTL condition by a multiline string
    pub fn repr_with_name(&self, net: &Network) -> String {
        format!(
            "LTL: {}\nVars:\n    {}\nHistory:\n    {}\ntransient state:\n{}",
            self.expr.repr(),
            self.prop_vars
                .iter()
                .enumerate()
                .map(|(i, c)| format!("{}: {}", i.repr(), c.repr_with_name(net)))
                .collect::<Vec<_>>()
                .join("\n    "),
            (0..self.prop_vars.len())
                .map(|i| format!(
                    "{}: {}",
                    i.repr(),
                    self.history
                        .iter()
                        .map(|v| if v[i] { "T" } else { "F" })
                        .collect::<Vec<_>>()
                        .join(" ")
                ))
                .collect::<Vec<_>>()
                .join("\n    "),
            self.tsa
                .as_ref()
                .map(|tsa| tsa.repr_with_name(net))
                .unwrap_or_else(|| "----".to_string())
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LTLResult {
    T,
    F,
    U,
}

impl LTLResult {
    /// Returns true if self is `LTLResult::True`
    pub fn is_true(&self) -> bool {
        *self == LTLResult::T
    }

    /// Returns true if self is `LTLResult::False`
    pub fn is_false(&self) -> bool {
        *self == LTLResult::F
    }

    /// Returns true if self is `LTLResult::Undef`
    pub fn is_undef(&self) -> bool {
        *self == LTLResult::U
    }
}

impl From<bool> for LTLResult {
    fn from(x: bool) -> Self {
        if x {
            Self::T
        } else {
            Self::F
        }
    }
}

impl From<&bool> for LTLResult {
    fn from(x: &bool) -> Self {
        if *x {
            Self::T
        } else {
            Self::F
        }
    }
}

impl From<Option<bool>> for LTLResult {
    fn from(x: Option<bool>) -> Self {
        if let Some(b) = x {
            Self::from(b)
        } else {
            Self::U
        }
    }
}

impl BitAnd for LTLResult {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::U, _) => Self::U,
            (_, Self::U) => Self::U,
            (Self::T, Self::T) => Self::T,
            _ => Self::F,
        }
    }
}

impl BitOr for LTLResult {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::T, _) => Self::T,
            (_, Self::T) => Self::T,
            (Self::U, _) => Self::U,
            (_, Self::U) => Self::U,
            _ => Self::F,
        }
    }
}

impl BitXor for LTLResult {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::U, _) => Self::U,
            (_, Self::U) => Self::U,
            (Self::F, Self::F) => Self::F,
            (Self::F, Self::T) => Self::T,
            (Self::T, Self::F) => Self::T,
            (Self::T, Self::T) => Self::F,
        }
    }
}

impl Not for LTLResult {
    type Output = Self;
    fn not(self) -> Self::Output {
        match self {
            Self::U => Self::U,
            Self::T => Self::F,
            Self::F => Self::T,
        }
    }
}

pub trait LTLBoxClone {
    fn box_clone(&self) -> Box<dyn LTLOperator>;
}

impl<T> LTLBoxClone for T
where
    T: 'static + LTLOperator + Clone,
{
    fn box_clone(&self) -> Box<dyn LTLOperator> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn LTLOperator> {
    fn clone(&self) -> Box<dyn LTLOperator> {
        self.box_clone()
    }
}

/// # Operators of LTL
///
/// An operator may either be a simple propositional variable, a boolean or a temporal modal
/// operator.
pub trait LTLOperator:
    fmt::Debug + LTLBoxClone + Send + std::panic::UnwindSafe + std::panic::RefUnwindSafe
{
    /// Checks if the operator holds for the given history. For this, it is assumed that the
    /// sequence is finished!
    fn check(&self, history: &[Vec<bool>]) -> bool;

    /// Checks if the operator holds for the given history, assuming that we have only a partial
    /// sequence.
    fn partial(&self, history: &[Vec<bool>]) -> LTLResult;

    /// Extract the set of propositional variables, that need to change in order for the result of
    /// the operator to change. Note, that not all variables in the returned set need to change,
    /// but only a subset. Some examples:
    ///
    /// ```
    /// use snowcap::hard_policies::*;
    /// use snowcap_ltl_parser::ltl;
    ///
    /// assert_eq!(ltl!(And(0, 1, 2)).watch(&vec![vec![true, true, true]]), vec![0, 1, 2]);
    /// assert_eq!(ltl!(And(0, 1, 2)).watch(&vec![vec![true, true, false]]), vec![2]);
    /// assert_eq!(ltl!(And(0, 1, 2)).watch(&vec![vec![true, false, false]]), vec![1, 2]);
    /// assert_eq!(ltl!(And(0, 1, 2)).watch(&vec![vec![false, false, false]]), vec![0, 1, 2]);
    /// ```
    fn watch(&self, history: &[Vec<bool>]) -> Vec<usize>;

    /// Extract the set of propositional variables, that need to change in order for the result of
    /// the operator to change. Note, that not all variables in the returned set need to change,
    /// but only a subset. For this funciton, the history is not yet complete. Some examples:
    ///
    /// ```
    /// use snowcap::hard_policies::*;
    /// use snowcap_ltl_parser::ltl;
    ///
    /// assert_eq!(ltl!(And(0, 1, 2)).watch(&vec![vec![true, true, true]]), vec![0, 1, 2]);
    /// assert_eq!(ltl!(And(0, 1, 2)).watch(&vec![vec![true, true, false]]), vec![2]);
    /// assert_eq!(ltl!(And(0, 1, 2)).watch(&vec![vec![true, false, false]]), vec![1, 2]);
    /// assert_eq!(ltl!(And(0, 1, 2)).watch(&vec![vec![false, false, false]]), vec![0, 1, 2]);
    /// ```
    fn watch_partial(&self, history: &[Vec<bool>]) -> Vec<usize>;

    /// represent the operator as a string
    fn repr(&self) -> String;
}

impl LTLOperator for bool {
    fn check(&self, _: &[Vec<bool>]) -> bool {
        *self
    }

    fn partial(&self, _: &[Vec<bool>]) -> LTLResult {
        self.into()
    }

    fn watch(&self, _history: &[Vec<bool>]) -> Vec<usize> {
        Vec::new()
    }

    fn watch_partial(&self, _history: &[Vec<bool>]) -> Vec<usize> {
        Vec::new()
    }

    fn repr(&self) -> String {
        if *self {
            String::from("true")
        } else {
            String::from("false")
        }
    }
}

impl LTLOperator for usize {
    fn check(&self, history: &[Vec<bool>]) -> bool {
        history[0][*self]
    }

    fn partial(&self, history: &[Vec<bool>]) -> LTLResult {
        history[0][*self].into()
    }

    fn watch(&self, _history: &[Vec<bool>]) -> Vec<usize> {
        vec![*self]
    }

    fn watch_partial(&self, _history: &[Vec<bool>]) -> Vec<usize> {
        vec![*self]
    }

    fn repr(&self) -> String {
        format!("x{:02}", self)
    }
}

fn partial_any<I, F>(iter: I, mut f: F) -> LTLResult
where
    I: Iterator,
    F: FnMut(I::Item) -> LTLResult,
{
    let mut partial: LTLResult = LTLResult::F;
    for elem in iter {
        match f(elem) {
            LTLResult::T => return LTLResult::T,
            LTLResult::U => partial = LTLResult::U,
            LTLResult::F => {}
        }
    }
    partial
}

fn partial_all<I, F>(iter: I, mut f: F) -> LTLResult
where
    I: Iterator,
    F: FnMut(I::Item) -> LTLResult,
{
    for elem in iter {
        match f(elem) {
            LTLResult::T => {}
            LTLResult::U => return LTLResult::U,
            LTLResult::F => return LTLResult::F,
        }
    }
    LTLResult::T
}

/// # Boolean operator of LTL
#[derive(Debug, Clone)]
pub enum LTLBoolean {
    /// Not: $\neg \phi$
    Not(Box<dyn LTLOperator>),
    /// Or: $\phi \lor \psi$
    Or(Vec<Box<dyn LTLOperator>>),
    /// And: $\phi \land \psi$
    And(Vec<Box<dyn LTLOperator>>),
    /// Xor: $\phi \oplus \psi = (\phi \land \neg \psi) \lor (\neg \phi \land \psi)$
    Xor(Box<dyn LTLOperator>, Box<dyn LTLOperator>),
    /// Implies: $\phi \rightarrow \psi = \neg \phi \lor \psi$
    Implies(Box<dyn LTLOperator>, Box<dyn LTLOperator>),
    /// Iff: $\phi \iff \psi = \neg (\phi \oplus \psi)$
    Iff(Box<dyn LTLOperator>, Box<dyn LTLOperator>),
}

impl LTLOperator for LTLBoolean {
    fn check(&self, history: &[Vec<bool>]) -> bool {
        match self {
            Self::Not(a) => !a.check(history),
            Self::Or(v) => v.iter().any(|x| x.check(history)),
            Self::And(v) => v.iter().all(|x| x.check(history)),
            Self::Xor(a, b) => a.check(history) ^ b.check(history),
            Self::Implies(a, b) => (!a.check(history)) | b.check(history),
            Self::Iff(a, b) => !(a.check(history) ^ b.check(history)),
        }
    }

    fn partial(&self, history: &[Vec<bool>]) -> LTLResult {
        match self {
            Self::Not(a) => !a.partial(history),
            Self::Or(v) => partial_any(v.iter(), |x| x.partial(history)),
            Self::And(v) => partial_all(v.iter(), |x| x.partial(history)),
            Self::Xor(a, b) => a.partial(history) ^ b.partial(history),
            Self::Implies(a, b) => (!a.partial(history)) | b.partial(history),
            Self::Iff(a, b) => !(a.partial(history) ^ b.partial(history)),
        }
    }

    /// Generate the watch of the operator at the current state. All operations and their behavior
    /// is explained:
    ///
    /// - *Not*: Simply generate the watch of the argument
    /// - *Or*: If the expression evaluates to true, then create the union of the watch for all sub-
    ///   expressions that evaluate to true (since changing these would result in the expression
    ///   being false). If the expression evaluates to false, then create the union of the watch of
    ///   all sub-expressions, since any of them can become true for the entire expression to turn
    ///   true.
    /// - *And*: If the expression evaluates to true, then create the union of the watch for all
    ///   sub-expressions, since any of them can become false in order for the entire expression to
    ///   turn false. If it evaluates to false, then create the union of the watch for all sub-
    ///   expressions that evaluate to false, since if all of them turn, then the expression becomes
    ///   true.
    /// - *Xor* and *Iff*: Since any change in input will change the output, always return the union
    ///   of both watches of the sub-expressions
    /// - *Implies*: Transorm $\phi \Rightarrow \psi$ into $\neg phi \vee \psi$ and recursively call
    ///   watch no them.
    fn watch(&self, history: &[Vec<bool>]) -> Vec<usize> {
        match self {
            LTLBoolean::Not(a) => a.watch(history),
            LTLBoolean::Or(v) => {
                if self.check(history) {
                    // result is true. to make it false, all of the true operands need to become
                    // false
                    v.iter()
                        .filter(|x| x.check(history))
                        .map(|x| x.watch(history).into_iter())
                        .flatten()
                        .collect()
                } else {
                    // result is false, to make true, at least one of the operands need to become
                    // true, and all are currenty false. Add all elements ot the watch
                    v.iter().map(|x| x.watch(history).into_iter()).flatten().collect()
                }
            }
            LTLBoolean::And(v) => {
                if self.check(history) {
                    // result is true. to make false, at least one of the operands need to become
                    // false, all of them are currently true. Add all elements to the watch
                    v.iter().map(|x| x.watch(history).into_iter()).flatten().collect()
                } else {
                    // result is false. To make true, all of the operands that now are false must
                    // become true.
                    v.iter()
                        .filter(|x| !x.check(history))
                        .map(|x| x.watch(history).into_iter())
                        .flatten()
                        .collect()
                }
            }
            LTLBoolean::Xor(a, b) => {
                let mut a_watch = a.watch(history);
                let mut b_watch = b.watch(history);
                a_watch.append(&mut b_watch);
                a_watch
            }
            LTLBoolean::Implies(a, b) => {
                // TODO implement this inplace, without constructing the operation
                LTLBoolean::Or(vec![Box::new(LTLBoolean::Not(a.clone())), b.clone()]).watch(history)
            }
            LTLBoolean::Iff(a, b) => {
                let mut a_watch = a.watch(history);
                let mut b_watch = b.watch(history);
                a_watch.append(&mut b_watch);
                a_watch
            }
        }
    }

    /// Here, we basically do the same as for the non-partial case. However, if the result is
    /// undefined, we return an empty set. Some Remarks
    ///
    /// - *Or*: If the result is true, then we want all that evaluate to true to become false. Hence
    ///   we can ignore all sub-expressions evaluating to false or are undefined. If the expression
    ///   is false, then we want any of the currently false or undefined values to become true, and
    ///   hence, just return the union of all of them.
    /// - *And*: If the result is false, then we want all that evaluate to false to become true.
    ///   This is anaolgeous to Or, using the transformation $\neg (x_1 \wedge x_2) = \neg x_1 \vee
    ///   \neg x_2$
    ///
    /// Notice, that undefined is always forwarded, if necessary. So if a part of the expression is
    /// undefined, then the result is only defined if the undefined part does not contribute tho the
    /// result.
    fn watch_partial(&self, history: &[Vec<bool>]) -> Vec<usize> {
        match self {
            LTLBoolean::Not(a) => a.watch_partial(history),
            LTLBoolean::Or(v) => {
                match self.partial(history) {
                    LTLResult::T => {
                        // if the partial result is true, then we need to watch all the elements
                        // that are true now.
                        v.iter()
                            .filter(|x| x.partial(history).is_true())
                            .map(|x| x.watch_partial(history).into_iter())
                            .flatten()
                            .collect()
                    }
                    LTLResult::F => {
                        // if the partial result is false, then every element is false, none of them
                        // is neither true nor undefined. We need to watch every element
                        v.iter().map(|x| x.watch_partial(history).into_iter()).flatten().collect()
                    }
                    LTLResult::U => {
                        // If the partial result is undefined, then we can return nothing as a watch
                        // because undefined will manifest itself only later.
                        Vec::new()
                    }
                }
            }
            LTLBoolean::And(v) => {
                match self.partial(history) {
                    LTLResult::T => {
                        // If the result is true, then every element is true, (and not false or
                        // unerined). Hence, we need to watch every element.
                        v.iter().map(|x| x.watch_partial(history).into_iter()).flatten().collect()
                    }
                    LTLResult::F => {
                        // If the partial result is false, then we need to watch every element, that
                        // is currently false, to be come true.
                        v.iter()
                            .filter(|x| x.partial(history).is_false())
                            .map(|x| x.watch_partial(history).into_iter())
                            .flatten()
                            .collect()
                    }
                    LTLResult::U => {
                        // If the partial result is undefined, then we can return nothing as a watch
                        // because undefined will manifest itself only later.
                        Vec::new()
                    }
                }
            }
            LTLBoolean::Xor(a, b) => {
                if a.partial(history).is_undef() || b.partial(history).is_undef() {
                    // If at least one of them is undefined, then nothing must be watched!
                    Vec::new()
                } else {
                    // here, both of them need to be watched
                    let mut a_watch = a.watch_partial(history);
                    let mut b_watch = b.watch_partial(history);
                    a_watch.append(&mut b_watch);
                    a_watch
                }
            }
            LTLBoolean::Implies(a, b) => {
                // TODO implement this inplace, without constructing the operation
                LTLBoolean::Or(vec![Box::new(LTLBoolean::Not(a.clone())), b.clone()])
                    .watch_partial(history)
            }
            LTLBoolean::Iff(a, b) => {
                if a.partial(history).is_undef() || b.partial(history).is_undef() {
                    // If at least one of them is undefined, then nothing must be watched!
                    Vec::new()
                } else {
                    // here, both of them need to be watched
                    let mut a_watch = a.watch_partial(history);
                    let mut b_watch = b.watch_partial(history);
                    a_watch.append(&mut b_watch);
                    a_watch
                }
            }
        }
    }

    fn repr(&self) -> String {
        match self {
            Self::Not(a) => format!("!{}", a.repr()),
            Self::Or(v) => {
                format!("({})", v.iter().map(|x| x.repr()).collect::<Vec<_>>().join(" || "))
            }
            Self::And(v) => {
                format!("({})", v.iter().map(|x| x.repr()).collect::<Vec<_>>().join(" && "))
            }
            Self::Xor(a, b) => format!("({} ^^ {})", a.repr(), b.repr()),
            Self::Implies(a, b) => format!("({} => {})", a.repr(), b.repr()),
            Self::Iff(a, b) => format!("({} <=> {})", a.repr(), b.repr()),
        }
    }
}

/// Temporal modal operators of LTL. For reconfiguration purpose, in the last state, we assume that
/// nothing changes anymore, and every propositional variable does not change its state. See
/// [here](https://en.wikipedia.org/wiki/Linear_temporal_logic#Weak_until_and_strong_release)
///
/// As an example, let $\phi_1$ be the policy that needs to be satisfied initially, and $\phi_2$ the
/// policy that needs to be satisfied at the end. Additionally, we would like that we switch once
/// from $\phi_1$ to $\phi_2$. This would be the following expression:
///
/// $$\phi_1\ \mathbf{U}\ \mathbf{G}\ \phi_2$$
#[derive(Debug, Clone)]
pub enum LTLModal {
    /// $\phi$: $\phi$ holds at the current state.
    Now(Box<dyn LTLOperator>),
    /// $\mathbf{X}\ \phi$: $\phi$ holds at the next state. If the sequence is finished, then
    /// $\mathbf{X} \phi \iff \phi$ is identical to stating that $\phi$ holds now.
    Next(Box<dyn LTLOperator>),
    /// $\mathbf{F}\ \phi$: $\phi$ needs to hold eventually (once)
    Finally(Box<dyn LTLOperator>),
    /// $\mathbf{G}\ \phi$: $\phi$ needs to hold in the current and every future state
    Globally(Box<dyn LTLOperator>),
    /// $\psi\ \mathbf{U}\ \phi$: $\psi$ has to hold *at least* until (but not including the state
    /// where) $\phi$ becomes true. $\phi$ can hold at the current or a future position (once).
    /// Until then, $psi$ must be true. $\phi$ must hold eventually!
    Until(Box<dyn LTLOperator>, Box<dyn LTLOperator>),
    /// $\psi\ \mathbf{R}\ \phi$: $\phi$ has to hold until *and including* the point where $\psi$
    /// first holds. If $\psi$ never holds, then $\phi$ must hold forever.
    Release(Box<dyn LTLOperator>, Box<dyn LTLOperator>),
    /// $\psi\ \mathbf{W}\ \phi$: $\psi$ has to hold at least *at least* util (but not including the
    /// state where) $\phi$ becomes true. If $\phi$ never holds, $\psi$ must hold forever.
    WeakUntil(Box<dyn LTLOperator>, Box<dyn LTLOperator>),
    /// $\psi\ \mathbf{M}\ \phi$: $\phi$ has to hold until *and including* the point where $\psi$
    /// first holds. $\psi$ can hold now or at any future state, but $\psi$ must hold eventually!
    StrongRelease(Box<dyn LTLOperator>, Box<dyn LTLOperator>),
}

impl LTLOperator for LTLModal {
    fn check(&self, history: &[Vec<bool>]) -> bool {
        match self {
            Self::Now(phi) => phi.check(history),
            Self::Next(phi) => {
                if history.len() >= 2 {
                    phi.check(&history[1..])
                } else {
                    phi.check(history)
                }
            }
            Self::Finally(phi) => {
                for i in 0..history.len() {
                    if phi.check(&history[i..]) {
                        return true;
                    }
                }
                false
            }
            Self::Globally(phi) => {
                for i in 0..history.len() {
                    if !phi.check(&history[i..]) {
                        return false;
                    }
                }
                true
            }
            Self::Until(psi, phi) => {
                for i in 0..history.len() {
                    if phi.check(&history[i..]) {
                        return true;
                    } else if !psi.check(&history[i..]) {
                        return false;
                    }
                }
                // If we have reached this position, phi has not become true! This is false.
                false
            }
            Self::Release(psi, phi) => {
                for i in 0..history.len() {
                    if phi.check(&history[i..]) {
                        if psi.check(&history[i..]) {
                            return true;
                        }
                    } else {
                        return false;
                    }
                }
                // if we reach this position, it means that phi has always been true. This is ok!
                true
            }
            Self::WeakUntil(psi, phi) => {
                for i in 0..history.len() {
                    if phi.check(&history[i..]) {
                        return true;
                    } else if !psi.check(&history[i..]) {
                        return false;
                    }
                }
                // if we reach this position, it means that psi does always hold. This is ok!
                true
            }
            Self::StrongRelease(psi, phi) => {
                for i in 0..history.len() {
                    if phi.check(&history[i..]) {
                        if psi.check(&history[i..]) {
                            return true;
                        }
                    } else {
                        return false;
                    }
                }
                // If we have reached this position, psi has not become true! This is false.
                false
            }
        }
    }

    fn partial(&self, history: &[Vec<bool>]) -> LTLResult {
        match self {
            Self::Now(phi) => phi.partial(history),
            Self::Next(phi) => {
                if history.len() >= 2 {
                    phi.partial(&history[1..])
                } else {
                    LTLResult::U
                }
            }
            Self::Finally(phi) => {
                for i in 0..history.len() {
                    if phi.partial(&history[i..]).is_true() {
                        return LTLResult::T;
                    }
                }
                LTLResult::U
            }
            Self::Globally(phi) => {
                for i in 0..history.len() {
                    if phi.partial(&history[i..]).is_false() {
                        return LTLResult::F;
                    }
                }
                LTLResult::U
            }
            Self::Until(psi, phi) | Self::WeakUntil(psi, phi) => {
                for i in 0..history.len() {
                    match phi.partial(&history[i..]) {
                        LTLResult::T => return LTLResult::T,
                        LTLResult::U => return LTLResult::U,
                        LTLResult::F => {}
                    }
                    match psi.partial(&history[i..]) {
                        LTLResult::F => return LTLResult::F,
                        LTLResult::U => return LTLResult::U,
                        LTLResult::T => {}
                    }
                }
                // If we have reached this position, psi was always true, but phi has not yet
                // become true. Hence, it is undefined.
                LTLResult::U
            }
            Self::Release(psi, phi) | Self::StrongRelease(psi, phi) => {
                for i in 0..history.len() {
                    match phi.partial(&history[i..]) {
                        LTLResult::U => return LTLResult::U,
                        LTLResult::F => return LTLResult::F,
                        LTLResult::T => {
                            if psi.partial(&history[i..]).is_true() {
                                return LTLResult::T;
                            }
                        }
                    }
                }
                // if we reach this position, it means that phi was always true, and psi was always
                // either false or undefined. Hence, it is undefined
                LTLResult::U
            }
        }
    }

    /// Generate the watch of the operator at the current state. All operations and their behavior
    /// is explained:
    ///
    /// - *Now*: Simply generate the watch of the current history
    /// - *Next*: Generate the watch of the next step. if there exists no next step, then generate
    ///   the watch of the current state.
    /// - *Finally*: Tis is anaolg to the *Or* boolean operation for computing the watch
    /// - *Globally*: This is analog to the *And* boolean operation for computing the watch.
    /// - *Unitl*($psi$, $phi$): If the expression is True, then create the union of all ways in
    ///   which this expression can become false:
    ///   1. Watch $psi$ at every state before (not including where) $phi$ first holds.
    ///   2. Watch $phi$ at every point where $phi$ holds, while (but not including where) $psi$ is
    ///   true
    ///
    ///   If the expression is false, try all possible ways in which it can turn true. For this, we
    ///   need to iterate over all possible cases how it might become true. This means, iterating
    ///   over every position and trying to make it true. Since we build the union of this, this is
    ///   the same as building the union over all watches of $psi$ and $phi$, where $psi$ or $phi$
    ///   are false.
    /// - *Release*($psi$, $phi$): If the expression is True, then create the union of all ways in
    ///   which this expression can become false:
    ///   1. Watch $phi$ at every state before (and including where) $psi$ first holds.
    ///   2. If $phi$ does not hold forever, watch $psi$ at every point where $psi$ holds, while
    ///      (and including where) $phi$ is true
    ///
    ///   If the expression is false, try all possible ways in which it can turn true. For this, we
    ///   need to iterate over all possible cases how it might become true. This means, iterating
    ///   over every position and trying to make it true. Since we build the union of this, this is
    ///   the same as building the union over all watches of $psi$ and $phi$, where $psi$ or $phi$
    ///   are false.
    /// - *WeakUntil*($psi$, $phi$): Here, we do exactly the same as for *Until*.
    /// - *StrongRelease*($psi$, $phi$): Here, we do exactly the same as for *Release*.
    fn watch(&self, history: &[Vec<bool>]) -> Vec<usize> {
        match self {
            LTLModal::Now(phi) => phi.watch(history),
            LTLModal::Next(phi) => {
                if history.len() >= 2 {
                    phi.watch(&history[1..])
                } else {
                    phi.watch(history)
                }
            }
            LTLModal::Finally(phi) => {
                if self.check(history) {
                    // result is true. to make it false, all of the true operands need to become
                    // false
                    (0..history.len())
                        .filter(|&i| phi.check(&history[i..]))
                        .map(|i| phi.watch(&history[i..]).into_iter())
                        .flatten()
                        .collect()
                } else {
                    // result is false, to make true, at least one of the operands need to become
                    // true, and all are currenty false. Add all elements ot the watch
                    (0..history.len())
                        .map(|i| phi.watch(&history[i..]).into_iter())
                        .flatten()
                        .collect()
                }
            }
            LTLModal::Globally(phi) => {
                if self.check(history) {
                    // result is true. To make false, any of the states need to become false
                    (0..history.len())
                        .map(|i| phi.watch(&history[i..]).into_iter())
                        .flatten()
                        .collect()
                } else {
                    // result is false. To make true, the ones that are false need to become true
                    (0..history.len())
                        .filter(|&i| !phi.check(&history[i..]))
                        .map(|i| phi.watch(&history[i..]).into_iter())
                        .flatten()
                        .collect()
                }
            }
            LTLModal::Until(psi, phi) | LTLModal::WeakUntil(psi, phi) => {
                let second_psi_false = (0..history.len())
                    .position(|i| !psi.check(&history[i..]))
                    .map(|x| x + 1)
                    .unwrap_or_else(|| history.len());
                if self.check(history) {
                    // the check is successful. To make false, two things can happen: Either psi
                    // becomes false before phi becomes true, or phi becomes false while psi is
                    // true. (always excluding)
                    let psi_watch = (0..history.len())
                        .take_while(|&i| !phi.check(&history[i..]))
                        .map(|i| psi.watch(&history[i..]).into_iter());
                    let phi_watch = (0..second_psi_false)
                        .filter(|&i| phi.check(&history[i..]))
                        .map(|i| phi.watch(&history[i..]).into_iter());
                    psi_watch.chain(phi_watch).flatten().collect()
                } else {
                    // If the expression is false, try all possible ways in which it can turn true.
                    // For this, we need to iterate over all possible cases how it might become
                    // true. This means, iterating over every position and trying to make it true.
                    // Since we build the union of this, this is the same as building the union over
                    // all watches of psi and phi, where psi or phi are false.
                    let psi_watch = (0..history.len())
                        .filter(|&i| !psi.check(&history[i..]))
                        .map(|i| psi.watch(&history[i..]).into_iter());
                    let phi_watch = (0..history.len())
                        .filter(|&i| !phi.check(&history[i..]))
                        .map(|i| phi.watch(&history[i..]).into_iter());
                    psi_watch.chain(phi_watch).flatten().collect()
                }
            }
            LTLModal::Release(psi, phi) | LTLModal::StrongRelease(psi, phi) => {
                let first_phi_false = (0..history.len())
                    .position(|i| !phi.check(&history[i..]))
                    .unwrap_or_else(|| history.len());
                let second_psi_true = (0..history.len())
                    .position(|i| psi.check(&history[i..]))
                    .map(|x| x + 1)
                    .unwrap_or_else(|| history.len());
                if self.check(history) {
                    // the check is successful. To make false, two things can happen: Either phi
                    // becomes false before psi becomes true, or psi becomes false while phi is
                    // true. (always including)
                    let phi_watch =
                        (0..second_psi_true).map(|i| phi.watch(&history[i..]).into_iter());
                    let psi_watch = (0..first_phi_false)
                        .filter(|&i| psi.check(&history[i..]))
                        .map(|i| psi.watch(&history[i..]).into_iter());
                    psi_watch.chain(phi_watch).flatten().collect()
                } else {
                    // If the expression is false, try all possible ways in which it can turn true.
                    // For this, we need to iterate over all possible cases how it might become
                    // true. This means, iterating over every position and trying to make it true.
                    // Since we build the union of this, this is the same as building the union over
                    // all watches of psi and phi, where psi or phi are false.
                    let psi_watch = (0..history.len())
                        .filter(|&i| !psi.check(&history[i..]))
                        .map(|i| psi.watch(&history[i..]).into_iter());
                    let phi_watch = (0..history.len())
                        .filter(|&i| !phi.check(&history[i..]))
                        .map(|i| phi.watch(&history[i..]).into_iter());
                    psi_watch.chain(phi_watch).flatten().collect()
                }
            }
        }
    }

    /// Here, we do the exact samething as for `watch`. However, if the result is undefined, then
    /// return an empty watch list.
    fn watch_partial(&self, history: &[Vec<bool>]) -> Vec<usize> {
        match self {
            LTLModal::Now(phi) => phi.watch_partial(history),
            LTLModal::Next(phi) => {
                if history.len() >= 2 {
                    phi.watch_partial(&history[1..])
                } else {
                    Vec::new()
                }
            }
            LTLModal::Finally(phi) => {
                match self.partial(history) {
                    LTLResult::U => Vec::new(),
                    LTLResult::T => {
                        // result is true. to make it false, all of the true operands need to become
                        // false
                        (0..history.len())
                            .filter(|&i| phi.partial(&history[i..]).is_true())
                            .map(|i| phi.watch_partial(&history[i..]).into_iter())
                            .flatten()
                            .collect()
                    }
                    LTLResult::F => {
                        // result is false, to make true, at least one of the operands need to become
                        // true, and all are currenty false. Add all elements ot the watch
                        (0..history.len())
                            .map(|i| phi.watch_partial(&history[i..]).into_iter())
                            .flatten()
                            .collect()
                    }
                }
            }
            LTLModal::Globally(phi) => {
                match self.partial(history) {
                    LTLResult::U => Vec::new(),
                    LTLResult::T => {
                        // result is true. To make false, any of the states need to become false
                        (0..history.len())
                            .map(|i| phi.watch_partial(&history[i..]).into_iter())
                            .flatten()
                            .collect()
                    }
                    LTLResult::F => {
                        // Result is false. To make it true, every state that is currently false
                        // needs to change.
                        (0..history.len())
                            .filter(|&i| phi.partial(&history[i..]).is_false())
                            .map(|i| phi.watch_partial(&history[i..]).into_iter())
                            .flatten()
                            .collect()
                    }
                }
            }
            LTLModal::Until(psi, phi) | LTLModal::WeakUntil(psi, phi) => {
                match self.partial(history) {
                    LTLResult::U => Vec::new(),
                    LTLResult::T => {
                        // the check is successful. To make false, two things can happen: Either psi
                        // becomes false before phi becomes true, or phi becomes false while psi is
                        // true. (always excluding)
                        let second_psi_false = (0..history.len())
                            .position(|i| psi.partial(&history[i..]).is_false())
                            .map(|x| x + 1)
                            .unwrap_or_else(|| history.len());
                        let psi_watch = (0..history.len())
                            .take_while(|&i| phi.partial(&history[i..]).is_false())
                            .map(|i| psi.watch_partial(&history[i..]).into_iter());
                        let phi_watch = (0..second_psi_false)
                            .filter(|&i| phi.partial(&history[i..]).is_true())
                            .map(|i| phi.watch_partial(&history[i..]).into_iter());
                        psi_watch.chain(phi_watch).flatten().collect()
                    }
                    LTLResult::F => {
                        // If the expression is false, try all possible ways in which it can turn
                        // true. For this, we need to iterate over all possible cases how it might
                        // become true. This means, iterating over every position and trying to make
                        // it true. Since we build the union of this, this is the same as building
                        // the union over all watches of psi and phi, where psi or phi are false.
                        let psi_watch = (0..history.len())
                            .filter(|&i| psi.partial(&history[i..]).is_false())
                            .map(|i| psi.watch_partial(&history[i..]).into_iter());
                        let phi_watch = (0..history.len())
                            .filter(|&i| phi.partial(&history[i..]).is_false())
                            .map(|i| phi.watch_partial(&history[i..]).into_iter());
                        psi_watch.chain(phi_watch).flatten().collect()
                    }
                }
            }
            LTLModal::Release(psi, phi) | LTLModal::StrongRelease(psi, phi) => {
                match self.partial(history) {
                    LTLResult::U => Vec::new(),
                    LTLResult::T => {
                        // the check is successful. To make false, two things can happen: Either phi
                        // becomes false before psi becomes true, or psi becomes false while phi is
                        // true. (always including)
                        let first_phi_false = (0..history.len())
                            .position(|i| phi.partial(&history[i..]).is_false())
                            .unwrap_or_else(|| history.len());
                        let second_psi_true = (0..history.len())
                            .position(|i| psi.partial(&history[i..]).is_true())
                            .map(|x| x + 1)
                            .unwrap_or_else(|| history.len());
                        let phi_watch = (0..second_psi_true)
                            .map(|i| phi.watch_partial(&history[i..]).into_iter());
                        let psi_watch = (0..first_phi_false)
                            .filter(|&i| psi.partial(&history[i..]).is_true())
                            .map(|i| psi.watch_partial(&history[i..]).into_iter());
                        psi_watch.chain(phi_watch).flatten().collect()
                    }
                    LTLResult::F => {
                        // If the expression is false, try all possible ways in which it can turn
                        // true. For this, we need to iterate over all possible cases how it might
                        // become true. This means, iterating over every position and trying to make
                        // it true. Since we build the union of this, this is the same as building
                        // the union over all watches of psi and phi, where psi or phi are false.
                        let psi_watch = (0..history.len())
                            .filter(|&i| psi.partial(&history[i..]).is_false())
                            .map(|i| psi.watch_partial(&history[i..]).into_iter());
                        let phi_watch = (0..history.len())
                            .filter(|&i| phi.partial(&history[i..]).is_false())
                            .map(|i| phi.watch_partial(&history[i..]).into_iter());
                        psi_watch.chain(phi_watch).flatten().collect()
                    }
                }
            }
        }
    }

    fn repr(&self) -> String {
        match self {
            LTLModal::Now(a) => a.repr(),
            LTLModal::Next(a) => format!("(N {})", a.repr()),
            LTLModal::Finally(a) => format!("(F {})", a.repr()),
            LTLModal::Globally(a) => format!("(G {})", a.repr()),
            LTLModal::Until(a, b) => format!("({} U {})", a.repr(), b.repr()),
            LTLModal::Release(a, b) => format!("({} R {})", a.repr(), b.repr()),
            LTLModal::WeakUntil(a, b) => format!("({} W {})", a.repr(), b.repr()),
            LTLModal::StrongRelease(a, b) => format!("({} M {})", a.repr(), b.repr()),
        }
    }
}

#[cfg(test)]
#[rustfmt::skip]
mod test {
    use super::*;

    use crate as snowcap;
    use snowcap_ltl_parser::ltl;

    const T: bool = true;
    const F: bool = false;
    const LT: LTLResult = LTLResult::T;
    const LF: LTLResult = LTLResult::F;
    const LU: LTLResult = LTLResult::U;

    #[test]
    fn modal_now() {
        let x = LTLModal::Now(Box::new(0));
        assert_eq!(F, x.check(&vec![vec![F], vec![T], vec![F]]));
        assert_eq!(T, x.check(&vec![vec![T], vec![T], vec![T]]));
        assert_eq!(T, x.check(&vec![vec![T], vec![F], vec![T]]));
        assert_eq!(T, x.check(&vec![vec![T], vec![F], vec![F]]));
    }

    #[test]
    fn modal_now_partial() {
        let x = LTLModal::Now(Box::new(0));
        assert_eq!(LF, x.partial(&vec![vec![F], vec![T], vec![F]]));
        assert_eq!(LT, x.partial(&vec![vec![T], vec![T], vec![T]]));
        assert_eq!(LT, x.partial(&vec![vec![T], vec![F], vec![T]]));
        assert_eq!(LT, x.partial(&vec![vec![T], vec![F], vec![F]]));
    }

    #[test]
    fn modal_next() {
        let x = LTLModal::Next(Box::new(0));
        assert_eq!(F, x.check(&vec![vec![T], vec![F], vec![F]]));
        assert_eq!(F, x.check(&vec![vec![F], vec![F], vec![F]]));
        assert_eq!(F, x.check(&vec![vec![T], vec![F], vec![T]]));
        assert_eq!(T, x.check(&vec![vec![T], vec![T], vec![F]]));
        assert_eq!(T, x.check(&vec![vec![F], vec![T], vec![F]]));
        assert_eq!(T, x.check(&vec![vec![T], vec![T], vec![T]]));
        assert_eq!(T, x.check(&vec![vec![T]]));
        assert_eq!(F, x.check(&vec![vec![F]]));
    }

    #[test]
    fn modal_next_partial() {
        let x = LTLModal::Next(Box::new(0));
        assert_eq!(LF, x.partial(&vec![vec![T], vec![F], vec![F]]));
        assert_eq!(LF, x.partial(&vec![vec![F], vec![F], vec![F]]));
        assert_eq!(LF, x.partial(&vec![vec![T], vec![F], vec![T]]));
        assert_eq!(LT, x.partial(&vec![vec![T], vec![T], vec![F]]));
        assert_eq!(LT, x.partial(&vec![vec![F], vec![T], vec![F]]));
        assert_eq!(LT, x.partial(&vec![vec![T], vec![T], vec![T]]));
        assert_eq!(LU, x.partial(&vec![vec![T]]));
        assert_eq!(LU, x.partial(&vec![vec![F]]));
    }

    #[test]
    fn modal_twice_next() {
        let x = LTLModal::Next(Box::new(LTLModal::Next(Box::new(0))));
        assert_eq!(F, x.check(&vec![vec![T], vec![F], vec![F]]));
        assert_eq!(F, x.check(&vec![vec![F], vec![F], vec![F]]));
        assert_eq!(F, x.check(&vec![vec![T], vec![T], vec![F]]));
        assert_eq!(T, x.check(&vec![vec![T], vec![F], vec![T]]));
        assert_eq!(T, x.check(&vec![vec![F], vec![F], vec![T]]));
        assert_eq!(T, x.check(&vec![vec![T], vec![T], vec![T]]));
        assert_eq!(F, x.check(&vec![vec![F], vec![F]]));
        assert_eq!(T, x.check(&vec![vec![F], vec![T]]));
        assert_eq!(F, x.check(&vec![vec![T], vec![F]]));
        assert_eq!(T, x.check(&vec![vec![T], vec![T]]));
        assert_eq!(T, x.check(&vec![vec![T]]));
        assert_eq!(F, x.check(&vec![vec![F]]));
    }

    #[test]
    fn modal_twice_next_partial() {
        let x = LTLModal::Next(Box::new(LTLModal::Next(Box::new(0))));
        assert_eq!(LF, x.partial(&vec![vec![T], vec![F], vec![F]]));
        assert_eq!(LF, x.partial(&vec![vec![F], vec![F], vec![F]]));
        assert_eq!(LF, x.partial(&vec![vec![T], vec![T], vec![F]]));
        assert_eq!(LT, x.partial(&vec![vec![T], vec![F], vec![T]]));
        assert_eq!(LT, x.partial(&vec![vec![F], vec![F], vec![T]]));
        assert_eq!(LT, x.partial(&vec![vec![T], vec![T], vec![T]]));
        assert_eq!(LU, x.partial(&vec![vec![F], vec![F]]));
        assert_eq!(LU, x.partial(&vec![vec![F], vec![T]]));
        assert_eq!(LU, x.partial(&vec![vec![T], vec![F]]));
        assert_eq!(LU, x.partial(&vec![vec![T], vec![T]]));
        assert_eq!(LU, x.partial(&vec![vec![T]]));
        assert_eq!(LU, x.partial(&vec![vec![F]]));
    }

    #[test]
    fn modal_finally() {
        let x = LTLModal::Finally(Box::new(0));
        assert_eq!(T, x.check(&vec![vec![T], vec![F], vec![F]]));
        assert_eq!(T, x.check(&vec![vec![F], vec![T], vec![F]]));
        assert_eq!(T, x.check(&vec![vec![F], vec![F], vec![T]]));
        assert_eq!(T, x.check(&vec![vec![T], vec![T], vec![T]]));
        assert_eq!(F, x.check(&vec![vec![F], vec![F], vec![F]]));
        assert_eq!(T, x.check(&vec![vec![T]]));
        assert_eq!(F, x.check(&vec![vec![F]]));
        assert_eq!(F, x.check(&vec![]));
    }

    #[test]
    fn modal_finally_partial() {
        let x = LTLModal::Finally(Box::new(0));
        assert_eq!(LT, x.partial(&vec![vec![T], vec![F], vec![F]]));
        assert_eq!(LT, x.partial(&vec![vec![F], vec![T], vec![F]]));
        assert_eq!(LT, x.partial(&vec![vec![F], vec![F], vec![T]]));
        assert_eq!(LT, x.partial(&vec![vec![T], vec![T], vec![T]]));
        assert_eq!(LU, x.partial(&vec![vec![F], vec![F], vec![F]]));
        assert_eq!(LT, x.partial(&vec![vec![T]]));
        assert_eq!(LU, x.partial(&vec![vec![F]]));
        assert_eq!(LU, x.partial(&vec![]));
    }

    #[test]
    fn modal_globally() {
        let x = LTLModal::Globally(Box::new(0));
        assert_eq!(T, x.check(&vec![vec![T], vec![T], vec![T]]));
        assert_eq!(F, x.check(&vec![vec![T], vec![F], vec![F]]));
        assert_eq!(F, x.check(&vec![vec![T], vec![F], vec![T]]));
        assert_eq!(F, x.check(&vec![vec![F], vec![T], vec![F]]));
        assert_eq!(F, x.check(&vec![vec![F], vec![F], vec![T]]));
        assert_eq!(F, x.check(&vec![vec![F], vec![F], vec![F]]));
        assert_eq!(T, x.check(&vec![vec![T]]));
        assert_eq!(F, x.check(&vec![vec![F]]));
        assert_eq!(T, x.check(&vec![]));
    }

    #[test]
    fn modal_globally_partial() {
        let x = LTLModal::Globally(Box::new(0));
        assert_eq!(LU, x.partial(&vec![vec![T], vec![T], vec![T]]));
        assert_eq!(LF, x.partial(&vec![vec![T], vec![F], vec![F]]));
        assert_eq!(LF, x.partial(&vec![vec![T], vec![F], vec![T]]));
        assert_eq!(LF, x.partial(&vec![vec![F], vec![T], vec![F]]));
        assert_eq!(LF, x.partial(&vec![vec![F], vec![F], vec![T]]));
        assert_eq!(LF, x.partial(&vec![vec![F], vec![F], vec![F]]));
        assert_eq!(LU, x.partial(&vec![vec![T]]));
        assert_eq!(LF, x.partial(&vec![vec![F]]));
        assert_eq!(LU, x.partial(&vec![]));
    }

    #[test]
    fn modal_globally_next() {
        let x = LTLModal::Globally(Box::new(LTLModal::Next(Box::new(0))));
        assert_eq!(T, x.check(&vec![vec![T], vec![T], vec![T]]));
        assert_eq!(T, x.check(&vec![vec![F], vec![T], vec![T]]));
        assert_eq!(F, x.check(&vec![vec![T], vec![F], vec![F]]));
        assert_eq!(F, x.check(&vec![vec![T], vec![F], vec![T]]));
        assert_eq!(F, x.check(&vec![vec![F], vec![T], vec![F]]));
        assert_eq!(F, x.check(&vec![vec![F], vec![F], vec![T]]));
        assert_eq!(F, x.check(&vec![vec![F], vec![F], vec![F]]));
        assert_eq!(T, x.check(&vec![vec![T]]));
        assert_eq!(F, x.check(&vec![vec![F]]));
        assert_eq!(T, x.check(&vec![]));
    }

    #[test]
    fn modal_globally_next_partial() {
        let x = LTLModal::Globally(Box::new(LTLModal::Next(Box::new(0))));
        assert_eq!(LU, x.partial(&vec![vec![T], vec![T], vec![T]]));
        assert_eq!(LU, x.partial(&vec![vec![F], vec![T], vec![T]]));
        assert_eq!(LF, x.partial(&vec![vec![T], vec![F], vec![F]]));
        assert_eq!(LF, x.partial(&vec![vec![T], vec![F], vec![T]]));
        assert_eq!(LF, x.partial(&vec![vec![F], vec![T], vec![F]]));
        assert_eq!(LF, x.partial(&vec![vec![F], vec![F], vec![T]]));
        assert_eq!(LF, x.partial(&vec![vec![F], vec![F], vec![F]]));
        assert_eq!(LU, x.partial(&vec![vec![T]]));
        assert_eq!(LU, x.partial(&vec![vec![F]]));
        assert_eq!(LU, x.partial(&vec![]));
    }

    #[test]
    fn modal_until() {
        let x = LTLModal::Until(Box::new(0), Box::new(1));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![F, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, T], vec![F, F]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![F, T], vec![F, F]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![F, T], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![F, T], vec![F, F], vec![F, F]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![F, F], vec![F, T]]));
        assert_eq!(F, x.check(&vec![vec![F, F], vec![T, T], vec![T, T]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![T, F], vec![T, F]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![T, F], vec![F, F]]));
    }

    #[test]
    fn modal_until_partial() {
        let x = LTLModal::Until(Box::new(0), Box::new(1));
        assert_eq!(LT, x.partial(&vec![vec![T, F], vec![T, F], vec![F, T]]));
        assert_eq!(LT, x.partial(&vec![vec![T, F], vec![T, F], vec![T, T]]));
        assert_eq!(LT, x.partial(&vec![vec![T, F], vec![T, T], vec![F, F]]));
        assert_eq!(LT, x.partial(&vec![vec![T, F], vec![F, T], vec![F, F]]));
        assert_eq!(LT, x.partial(&vec![vec![T, F], vec![F, T], vec![T, T]]));
        assert_eq!(LT, x.partial(&vec![vec![F, T], vec![F, F], vec![F, F]]));
        assert_eq!(LF, x.partial(&vec![vec![T, F], vec![F, F], vec![F, T]]));
        assert_eq!(LF, x.partial(&vec![vec![F, F], vec![T, T], vec![T, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, F]]));
        assert_eq!(LF, x.partial(&vec![vec![T, F], vec![T, F], vec![F, F]]));
    }

    #[test]
    fn modal_release() {
        let x = LTLModal::Release(Box::new(0), Box::new(1));
        assert_eq!(T, x.check(&vec![vec![T, T], vec![F, F], vec![F, F]]));
        assert_eq!(T, x.check(&vec![vec![F, T], vec![T, T], vec![F, F]]));
        assert_eq!(T, x.check(&vec![vec![F, T], vec![T, T], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![F, T], vec![F, T], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![F, T], vec![F, T], vec![F, T]]));
        assert_eq!(F, x.check(&vec![vec![F, T], vec![F, F], vec![F, F]]));
        assert_eq!(F, x.check(&vec![vec![F, T], vec![T, F], vec![F, F]]));
        assert_eq!(F, x.check(&vec![vec![F, T], vec![T, F], vec![T, T]]));
        assert_eq!(F, x.check(&vec![vec![F, T], vec![F, T], vec![T, F]]));
        assert_eq!(F, x.check(&vec![vec![F, T], vec![F, T], vec![F, F]]));
    }

    #[test]
    fn modal_release_partial() {
        let x = LTLModal::Release(Box::new(0), Box::new(1));
        assert_eq!(LT, x.partial(&vec![vec![T, T], vec![F, F], vec![F, F]]));
        assert_eq!(LT, x.partial(&vec![vec![F, T], vec![T, T], vec![F, F]]));
        assert_eq!(LT, x.partial(&vec![vec![F, T], vec![T, T], vec![T, T]]));
        assert_eq!(LT, x.partial(&vec![vec![F, T], vec![F, T], vec![T, T]]));
        assert_eq!(LU, x.partial(&vec![vec![F, T], vec![F, T], vec![F, T]]));
        assert_eq!(LF, x.partial(&vec![vec![F, T], vec![F, F], vec![F, F]]));
        assert_eq!(LF, x.partial(&vec![vec![F, T], vec![T, F], vec![F, F]]));
        assert_eq!(LF, x.partial(&vec![vec![F, T], vec![T, F], vec![T, T]]));
        assert_eq!(LF, x.partial(&vec![vec![F, T], vec![F, T], vec![T, F]]));
        assert_eq!(LF, x.partial(&vec![vec![F, T], vec![F, T], vec![F, F]]));
    }

    #[test]
    fn modal_weak_until() {
        let x = LTLModal::WeakUntil(Box::new(0), Box::new(1));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![F, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, T], vec![F, F]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![F, T], vec![F, F]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![F, T], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![F, T], vec![F, F], vec![F, F]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![T, F]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![F, F], vec![F, T]]));
        assert_eq!(F, x.check(&vec![vec![F, F], vec![T, T], vec![T, T]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![T, F], vec![F, F]]));
    }

    #[test]
    fn modal_weak_until_partial() {
        let x = LTLModal::WeakUntil(Box::new(0), Box::new(1));
        assert_eq!(LT, x.partial(&vec![vec![T, F], vec![T, F], vec![F, T]]));
        assert_eq!(LT, x.partial(&vec![vec![T, F], vec![T, F], vec![T, T]]));
        assert_eq!(LT, x.partial(&vec![vec![T, F], vec![T, T], vec![F, F]]));
        assert_eq!(LT, x.partial(&vec![vec![T, F], vec![F, T], vec![F, F]]));
        assert_eq!(LT, x.partial(&vec![vec![T, F], vec![F, T], vec![T, T]]));
        assert_eq!(LT, x.partial(&vec![vec![F, T], vec![F, F], vec![F, F]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, F]]));
        assert_eq!(LF, x.partial(&vec![vec![T, F], vec![F, F], vec![F, T]]));
        assert_eq!(LF, x.partial(&vec![vec![F, F], vec![T, T], vec![T, T]]));
        assert_eq!(LF, x.partial(&vec![vec![T, F], vec![T, F], vec![F, F]]));
    }

    #[test]
    fn modal_strong_release() {
        let x = LTLModal::StrongRelease(Box::new(0), Box::new(1));
        assert_eq!(T, x.check(&vec![vec![T, T], vec![F, F], vec![F, F]]));
        assert_eq!(T, x.check(&vec![vec![F, T], vec![T, T], vec![F, F]]));
        assert_eq!(T, x.check(&vec![vec![F, T], vec![T, T], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![F, T], vec![F, T], vec![T, T]]));
        assert_eq!(F, x.check(&vec![vec![F, T], vec![F, T], vec![F, T]]));
        assert_eq!(F, x.check(&vec![vec![F, T], vec![F, F], vec![F, F]]));
        assert_eq!(F, x.check(&vec![vec![F, T], vec![T, F], vec![F, F]]));
        assert_eq!(F, x.check(&vec![vec![F, T], vec![T, F], vec![T, T]]));
        assert_eq!(F, x.check(&vec![vec![F, T], vec![F, T], vec![T, F]]));
        assert_eq!(F, x.check(&vec![vec![F, T], vec![F, T], vec![F, F]]));
    }

    #[test]
    fn modal_strong_release_partial() {
        let x = LTLModal::StrongRelease(Box::new(0), Box::new(1));
        assert_eq!(LT, x.partial(&vec![vec![T, T], vec![F, F], vec![F, F]]));
        assert_eq!(LT, x.partial(&vec![vec![F, T], vec![T, T], vec![F, F]]));
        assert_eq!(LT, x.partial(&vec![vec![F, T], vec![T, T], vec![T, T]]));
        assert_eq!(LT, x.partial(&vec![vec![F, T], vec![F, T], vec![T, T]]));
        assert_eq!(LU, x.partial(&vec![vec![F, T], vec![F, T], vec![F, T]]));
        assert_eq!(LF, x.partial(&vec![vec![F, T], vec![F, F], vec![F, F]]));
        assert_eq!(LF, x.partial(&vec![vec![F, T], vec![T, F], vec![F, F]]));
        assert_eq!(LF, x.partial(&vec![vec![F, T], vec![T, F], vec![T, T]]));
        assert_eq!(LF, x.partial(&vec![vec![F, T], vec![F, T], vec![T, F]]));
        assert_eq!(LF, x.partial(&vec![vec![F, T], vec![F, T], vec![F, F]]));
    }

    #[test]
    fn modal_always_and_new() {
        let x = LTLBoolean::And(vec![
            Box::new(LTLModal::Globally(Box::new(0))),
            Box::new(LTLModal::Finally(Box::new(LTLModal::Globally(Box::new(1))))),
        ]);

        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![T, T], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, T], vec![T, T], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![T, T], vec![T, T], vec![T, T], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![T, T], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, T], vec![T, T], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![T, T], vec![T, T], vec![T, T], vec![T, T]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![F, F], vec![T, F], vec![T, T]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![T, F], vec![T, F], vec![F, T]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![T, T], vec![T, F], vec![T, F]]));
        assert_eq!(F, x.check(&vec![vec![T, T], vec![T, F], vec![T, T], vec![T, F]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, F]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![T, F], vec![T, T], vec![T, F]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![T, F], vec![T, T], vec![F, T]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![T, T], vec![T, F], vec![T, F]]));
    }

    #[test]
    fn modal_always_and_new_partial() {
        let x = LTLBoolean::And(vec![
            Box::new(LTLModal::Globally(Box::new(0))),
            Box::new(LTLModal::Finally(Box::new(LTLModal::Globally(Box::new(1))))),
        ]);

        assert_eq!(LU, x.partial(&vec![vec![T, F]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, F]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, T], vec![T, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, T], vec![T, T], vec![T, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, T], vec![T, T], vec![T, T], vec![T, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, T], vec![T, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, T], vec![T, T], vec![T, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, T], vec![T, T], vec![T, T], vec![T, T]]));
        assert_eq!(LF, x.partial(&vec![vec![T, F], vec![F, F], vec![T, F], vec![T, T]]));
        assert_eq!(LF, x.partial(&vec![vec![T, F], vec![T, F], vec![T, F], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, T], vec![T, F], vec![T, F]]));
        assert_eq!(LU, x.partial(&vec![vec![T, T], vec![T, F], vec![T, T], vec![T, F]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, F]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, T], vec![T, F]]));
        assert_eq!(LF, x.partial(&vec![vec![T, F], vec![T, F], vec![T, T], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, T], vec![T, F], vec![T, F]]));
    }

    #[test]
    fn modal_until_globally() {
        let x = LTLModal::Until(Box::new(0), Box::new(LTLModal::Globally(Box::new(1))));

        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![T, F], vec![F, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![T, T], vec![F, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![F, T], vec![F, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, T], vec![F, T], vec![F, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![F, T], vec![T, T], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![F, T], vec![F, T], vec![F, T], vec![F, T]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, F]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![F, F], vec![T, T], vec![T, T]]));
        assert_eq!(F, x.check(&vec![vec![T, T], vec![F, T], vec![T, F], vec![T, T]]));
        assert_eq!(F, x.check(&vec![vec![F, T], vec![F, T], vec![F, T], vec![T, F]]));
    }

    #[test]
    fn modal_until_globally_partial() {
        let x = LTLModal::Until(Box::new(0), Box::new(LTLModal::Globally(Box::new(1))));

        assert_eq!(LU, x.partial(&vec![]));
        assert_eq!(LU, x.partial(&vec![vec![T, F]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F]]));
        assert_eq!(LF, x.partial(&vec![vec![T, F], vec![F, F]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![F, T], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, F], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, T], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![F, T], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, T], vec![F, T], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![F, T], vec![T, T], vec![T, T]]));
        assert_eq!(LU, x.partial(&vec![vec![F, T], vec![F, T], vec![F, T], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, F]]));
        assert_eq!(LF, x.partial(&vec![vec![T, F], vec![F, F], vec![T, T], vec![T, T]]));
        assert_eq!(LF, x.partial(&vec![vec![T, T], vec![F, T], vec![T, F], vec![T, T]]));
        assert_eq!(LF, x.partial(&vec![vec![F, T], vec![F, T], vec![F, T], vec![T, F]]));
    }

    #[test]
    fn modal_weak_until_globally() {
        let x = LTLModal::WeakUntil(Box::new(0), Box::new(LTLModal::Globally(Box::new(1))));

        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![T, F], vec![F, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![T, T], vec![F, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![F, T], vec![F, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, T], vec![F, T], vec![F, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![F, T], vec![T, T], vec![T, T]]));
        assert_eq!(T, x.check(&vec![vec![F, T], vec![F, T], vec![F, T], vec![F, T]]));
        assert_eq!(T, x.check(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, F]]));
        assert_eq!(F, x.check(&vec![vec![T, F], vec![F, F], vec![T, T], vec![T, T]]));
        assert_eq!(F, x.check(&vec![vec![T, T], vec![F, T], vec![T, F], vec![T, T]]));
        assert_eq!(F, x.check(&vec![vec![F, T], vec![F, T], vec![F, T], vec![T, F]]));
    }

    #[test]
    fn modal_weak_until_globally_partial() {
        let x = LTLModal::WeakUntil(Box::new(0), Box::new(LTLModal::Globally(Box::new(1))));

        assert_eq!(LU, x.partial(&vec![]));
        assert_eq!(LU, x.partial(&vec![vec![T, F]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F]]));
        assert_eq!(LF, x.partial(&vec![vec![T, F], vec![F, F]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![F, T], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, F], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, T], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![F, T], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, T], vec![F, T], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![F, T], vec![T, T], vec![T, T]]));
        assert_eq!(LU, x.partial(&vec![vec![F, T], vec![F, T], vec![F, T], vec![F, T]]));
        assert_eq!(LU, x.partial(&vec![vec![T, F], vec![T, F], vec![T, F], vec![T, F]]));
        assert_eq!(LF, x.partial(&vec![vec![T, F], vec![F, F], vec![T, T], vec![T, T]]));
        assert_eq!(LF, x.partial(&vec![vec![T, T], vec![F, T], vec![T, F], vec![T, T]]));
        assert_eq!(LF, x.partial(&vec![vec![F, T], vec![F, T], vec![F, T], vec![T, F]]));
    }

    #[test]
    fn boolean() {
        assert_eq!(T, LTLBoolean::Not(Box::new(F)).check(&vec![]));
        assert_eq!(F, LTLBoolean::Not(Box::new(T)).check(&vec![]));
        assert_eq!(F, LTLBoolean::Or(vec![Box::new(F), Box::new(F)]).check(&vec![]));
        assert_eq!(T, LTLBoolean::Or(vec![Box::new(T), Box::new(F)]).check(&vec![]));
        assert_eq!(T, LTLBoolean::Or(vec![Box::new(F), Box::new(T)]).check(&vec![]));
        assert_eq!(T, LTLBoolean::Or(vec![Box::new(T), Box::new(T)]).check(&vec![]));
        assert_eq!(F, LTLBoolean::And(vec![Box::new(F), Box::new(F)]).check(&vec![]));
        assert_eq!(F, LTLBoolean::And(vec![Box::new(T), Box::new(F)]).check(&vec![]));
        assert_eq!(F, LTLBoolean::And(vec![Box::new(F), Box::new(T)]).check(&vec![]));
        assert_eq!(T, LTLBoolean::And(vec![Box::new(T), Box::new(T)]).check(&vec![]));
        assert_eq!(F, LTLBoolean::Xor(Box::new(F), Box::new(F)).check(&vec![]));
        assert_eq!(T, LTLBoolean::Xor(Box::new(T), Box::new(F)).check(&vec![]));
        assert_eq!(T, LTLBoolean::Xor(Box::new(F), Box::new(T)).check(&vec![]));
        assert_eq!(F, LTLBoolean::Xor(Box::new(T), Box::new(T)).check(&vec![]));
        assert_eq!(T, LTLBoolean::Implies(Box::new(F), Box::new(F)).check(&vec![]));
        assert_eq!(F, LTLBoolean::Implies(Box::new(T), Box::new(F)).check(&vec![]));
        assert_eq!(T, LTLBoolean::Implies(Box::new(F), Box::new(T)).check(&vec![]));
        assert_eq!(T, LTLBoolean::Implies(Box::new(T), Box::new(T)).check(&vec![]));
        assert_eq!(T, LTLBoolean::Iff(Box::new(F), Box::new(F)).check(&vec![]));
        assert_eq!(F, LTLBoolean::Iff(Box::new(T), Box::new(F)).check(&vec![]));
        assert_eq!(F, LTLBoolean::Iff(Box::new(F), Box::new(T)).check(&vec![]));
        assert_eq!(T, LTLBoolean::Iff(Box::new(T), Box::new(T)).check(&vec![]));
    }

    #[test]
    fn boolean_partial() {
        fn u() -> LTLModal {
            LTLModal::Globally(Box::new(T))
        }
        assert_eq!(LU, LTLBoolean::Not(Box::new(u())).partial(&vec![]));
        assert_eq!(LU, LTLBoolean::Not(Box::new(u())).partial(&vec![]));
        assert_eq!(LU, LTLBoolean::Or(vec![Box::new(F), Box::new(u())]).partial(&vec![]));
        assert_eq!(LT, LTLBoolean::Or(vec![Box::new(T), Box::new(u())]).partial(&vec![]));
        assert_eq!(LF, LTLBoolean::And(vec![Box::new(F), Box::new(u())]).partial(&vec![]));
        assert_eq!(LU, LTLBoolean::And(vec![Box::new(T), Box::new(u())]).partial(&vec![]));
        assert_eq!(LU, LTLBoolean::Xor(Box::new(F), Box::new(u())).partial(&vec![]));
        assert_eq!(LU, LTLBoolean::Xor(Box::new(T), Box::new(u())).partial(&vec![]));
        assert_eq!(LT, LTLBoolean::Implies(Box::new(F), Box::new(u())).partial(&vec![]));
        assert_eq!(LU, LTLBoolean::Implies(Box::new(T), Box::new(u())).partial(&vec![]));
        assert_eq!(LU, LTLBoolean::Iff(Box::new(F), Box::new(u())).partial(&vec![]));
        assert_eq!(LU, LTLBoolean::Iff(Box::new(T), Box::new(u())).partial(&vec![]));
    }

    #[test]
    fn partial() {
        let x = LTLBoolean::Not(Box::new(LTLModal::Next(Box::new(0))));
        assert_eq!(LT, x.partial(&vec![vec![F], vec![F]]));
        assert_eq!(LT, x.partial(&vec![vec![T], vec![F]]));
        assert_eq!(LF, x.partial(&vec![vec![F], vec![T]]));
        assert_eq!(LF, x.partial(&vec![vec![T], vec![T]]));
        assert_eq!(LU, x.partial(&vec![vec![T]]));
        assert_eq!(LU, x.partial(&vec![vec![F]]));
    }

    fn test_watch(mut acq: Vec<usize>, mut exp: Vec<usize>) {
        acq.sort();
        acq.dedup();
        exp.sort();
        assert_eq!(acq, exp);
    }

    #[test]
    fn watch_boolean_not() {
        let x = ltl!(!0);
        test_watch(x.watch(&vec![vec![F]]), vec![0]);
        test_watch(x.watch(&vec![vec![T]]), vec![0]);
        test_watch(x.watch_partial(&vec![vec![F]]), vec![0]);
        test_watch(x.watch_partial(&vec![vec![T]]), vec![0]);
    }

    #[test]
    fn watch_boolean_or() {
        let x = ltl!(Or(0, 1, 2));
        test_watch(x.watch(&vec![vec![T, T, T]]), vec![0, 1, 2]);
        test_watch(x.watch(&vec![vec![T, T, F]]), vec![0, 1]);
        test_watch(x.watch(&vec![vec![T, F, F]]), vec![0]);
        test_watch(x.watch(&vec![vec![F, F, F]]), vec![0, 1, 2]);
        test_watch(x.watch_partial(&vec![vec![T, T, T]]), vec![0, 1, 2]);
        test_watch(x.watch_partial(&vec![vec![T, T, F]]), vec![0, 1]);
        test_watch(x.watch_partial(&vec![vec![T, F, F]]), vec![0]);
        test_watch(x.watch_partial(&vec![vec![F, F, F]]), vec![0, 1, 2]);

        let x = ltl!(0 | 1 | 2);
        test_watch(x.watch(&vec![vec![T, T, T]]), vec![0, 1, 2]);
        test_watch(x.watch(&vec![vec![T, T, F]]), vec![0, 1]);
        test_watch(x.watch(&vec![vec![T, F, F]]), vec![0]);
        test_watch(x.watch(&vec![vec![F, F, F]]), vec![0, 1, 2]);
        test_watch(x.watch_partial(&vec![vec![T, T, T]]), vec![0, 1, 2]);
        test_watch(x.watch_partial(&vec![vec![T, T, F]]), vec![0, 1]);
        test_watch(x.watch_partial(&vec![vec![T, F, F]]), vec![0]);
        test_watch(x.watch_partial(&vec![vec![F, F, F]]), vec![0, 1, 2]);
    }

    #[test]
    fn watch_boolean_and() {
        let x = ltl!(And(0, 1, 2));
        test_watch(x.watch(&vec![vec![T, T, T]]), vec![0, 1, 2]);
        test_watch(x.watch(&vec![vec![T, T, F]]), vec![2]);
        test_watch(x.watch(&vec![vec![T, F, F]]), vec![1, 2]);
        test_watch(x.watch(&vec![vec![F, F, F]]), vec![0, 1, 2]);
        test_watch(x.watch_partial(&vec![vec![T, T, T]]), vec![0, 1, 2]);
        test_watch(x.watch_partial(&vec![vec![T, T, F]]), vec![2]);
        test_watch(x.watch_partial(&vec![vec![T, F, F]]), vec![1, 2]);
        test_watch(x.watch_partial(&vec![vec![F, F, F]]), vec![0, 1, 2]);

        let x = ltl!(0 & 1 & 2);
        test_watch(x.watch(&vec![vec![T, T, T]]), vec![0, 1, 2]);
        test_watch(x.watch(&vec![vec![T, T, F]]), vec![2]);
        test_watch(x.watch(&vec![vec![T, F, F]]), vec![1, 2]);
        test_watch(x.watch(&vec![vec![F, F, F]]), vec![0, 1, 2]);
        test_watch(x.watch_partial(&vec![vec![T, T, T]]), vec![0, 1, 2]);
        test_watch(x.watch_partial(&vec![vec![T, T, F]]), vec![2]);
        test_watch(x.watch_partial(&vec![vec![T, F, F]]), vec![1, 2]);
        test_watch(x.watch_partial(&vec![vec![F, F, F]]), vec![0, 1, 2]);
    }

    #[test]
    fn watch_boolean_xor() {
        let x = ltl!(0 ^ 1);
        test_watch(x.watch(&vec![vec![T, T]]), vec![0, 1]);
        test_watch(x.watch(&vec![vec![F, T]]), vec![0, 1]);
        test_watch(x.watch(&vec![vec![T, F]]), vec![0, 1]);
        test_watch(x.watch(&vec![vec![F, F]]), vec![0, 1]);
        test_watch(x.watch_partial(&vec![vec![T, T]]), vec![0, 1]);
        test_watch(x.watch_partial(&vec![vec![F, T]]), vec![0, 1]);
        test_watch(x.watch_partial(&vec![vec![T, F]]), vec![0, 1]);
        test_watch(x.watch_partial(&vec![vec![F, F]]), vec![0, 1]);
    }

    #[test]
    fn watch_boolean_implies() {
        let x = ltl!(0 >> 1);
        test_watch(x.watch(&vec![vec![T, T]]), vec![1]);
        test_watch(x.watch(&vec![vec![F, T]]), vec![0, 1]);
        test_watch(x.watch(&vec![vec![T, F]]), vec![0, 1]);
        test_watch(x.watch(&vec![vec![F, F]]), vec![0]);
        test_watch(x.watch_partial(&vec![vec![T, T]]), vec![1]);
        test_watch(x.watch_partial(&vec![vec![F, T]]), vec![0, 1]);
        test_watch(x.watch_partial(&vec![vec![T, F]]), vec![0, 1]);
        test_watch(x.watch_partial(&vec![vec![F, F]]), vec![0]);
    }

    #[test]
    fn watch_modal_next() {
        let x = ltl!(Next(0));
        test_watch(x.watch(&vec![vec![F], vec![T]]), vec![0]);
        test_watch(x.watch(&vec![vec![T], vec![F]]), vec![0]);
        test_watch(x.watch(&vec![vec![F], vec![F]]), vec![0]);
        test_watch(x.watch(&vec![vec![F]]), vec![0]);
        test_watch(x.watch(&vec![vec![T]]), vec![0]);
        test_watch(x.watch_partial(&vec![vec![F], vec![T]]), vec![0]);
        test_watch(x.watch_partial(&vec![vec![T], vec![F]]), vec![0]);
        test_watch(x.watch_partial(&vec![vec![F], vec![F]]), vec![0]);
        test_watch(x.watch_partial(&vec![vec![F]]), vec![]);
        test_watch(x.watch_partial(&vec![vec![T]]), vec![]);
    }

    #[test]
    fn watch_modal_finally() {
        let x = ltl!(Finally(0 & 1));
        test_watch(x.watch(&vec![vec![F, F], vec![T, T]]), vec![0, 1]);
        test_watch(x.watch(&vec![vec![T, T], vec![T, T]]), vec![0, 1]);
        test_watch(x.watch(&vec![vec![T, F], vec![T, T]]), vec![0, 1]);
        test_watch(x.watch(&vec![vec![T, F], vec![T, F]]), vec![1]);
        test_watch(x.watch(&vec![vec![F, T], vec![F, T]]), vec![0]);
        test_watch(x.watch(&vec![vec![T, F], vec![F, T]]), vec![0, 1]);
        test_watch(x.watch_partial(&vec![vec![F, F], vec![T, T]]), vec![0, 1]);
        test_watch(x.watch_partial(&vec![vec![T, T], vec![T, T]]), vec![0, 1]);
        test_watch(x.watch_partial(&vec![vec![T, F], vec![T, T]]), vec![0, 1]);
        test_watch(x.watch_partial(&vec![vec![T, F], vec![T, F]]), vec![]);
        test_watch(x.watch_partial(&vec![vec![F, T], vec![F, T]]), vec![]);
        test_watch(x.watch_partial(&vec![vec![T, F], vec![F, T]]), vec![]);
    }

    #[test]
    fn watch_modal_globally() {
        let x = ltl!(Globally(0 & 1));
        test_watch(x.watch(&vec![vec![T, T], vec![T, T]]), vec![0, 1]);
        test_watch(x.watch(&vec![vec![T, F], vec![T, T]]), vec![1]);
        test_watch(x.watch(&vec![vec![T, T], vec![F, T]]), vec![0]);
        test_watch(x.watch(&vec![vec![F, T], vec![F, T]]), vec![0]);
        test_watch(x.watch(&vec![vec![F, F], vec![F, T]]), vec![0, 1]);
        test_watch(x.watch_partial(&vec![vec![T, T], vec![T, T]]), vec![]);
        test_watch(x.watch_partial(&vec![vec![T, F], vec![T, T]]), vec![1]);
        test_watch(x.watch_partial(&vec![vec![T, T], vec![F, T]]), vec![0]);
        test_watch(x.watch_partial(&vec![vec![F, T], vec![F, T]]), vec![0]);
        test_watch(x.watch_partial(&vec![vec![F, F], vec![F, T]]), vec![0, 1]);
    }

    #[test]
    fn watch_modal_until() {
        let x = ltl!(Until(0 & 1, 2 & 3));
        test_watch(x.watch(&vec![vec![T, T, T, F], vec![T, T, F, F], vec![T, F, T, T]]), vec![0, 1, 2, 3]);
        test_watch(x.watch(&vec![vec![T, T, T, F], vec![T, F, T, T], vec![T, F, F, T]]), vec![0, 1, 2, 3]);
        test_watch(x.watch(&vec![vec![T, F, T, T], vec![T, T, T, T], vec![T, T, T, T]]), vec![2, 3]);
        test_watch(x.watch(&vec![vec![T, T, T, F], vec![T, T, F, T], vec![T, T, F, T]]), vec![2, 3]);
        test_watch(x.watch(&vec![vec![T, T, T, F], vec![T, T, F, T], vec![T, F, F, T]]), vec![1, 2, 3]);
        test_watch(x.watch(&vec![vec![T, T, F, T], vec![T, T, F, T], vec![T, T, F, T]]), vec![2]);
        test_watch(x.watch(&vec![vec![F, T, F, T], vec![T, T, T, T], vec![T, T, T, T]]), vec![0, 2]);
        test_watch(x.watch(&vec![vec![F, F, T, T], vec![T, T, T, T], vec![T, T, T, T]]), vec![2, 3]);
        test_watch(x.watch(&vec![vec![F, F, T, T], vec![F, F, F, F], vec![T, T, T, T]]), vec![2, 3]);
        test_watch(x.watch(&vec![vec![F, F, T, T], vec![T, T, T, T], vec![F, F, F, F]]), vec![2, 3]);
        test_watch(x.watch(&vec![vec![F, T, F, T], vec![F, T, T, T], vec![T, T, T, T]]), vec![0, 2]);
        test_watch(x.watch(&vec![vec![T, T, F, T], vec![F, T, T, T], vec![T, T, T, T]]), vec![0, 1, 2, 3]);
        test_watch(x.watch(&vec![vec![T, F, T, F], vec![T, T, T, T], vec![T, T, T, T]]), vec![1, 3]);
        test_watch(x.watch(&vec![vec![T, F, T, F], vec![F, T, F, T], vec![F, F, F, F]]), vec![0, 1, 2, 3]);
        test_watch(x.watch(&vec![vec![T, T, T, F], vec![F, T, T, F], vec![F, T, T, F]]), vec![0, 3]);
        test_watch(x.watch(&vec![vec![T, T, T, F], vec![F, T, T, F], vec![F, T, F, T]]), vec![0, 2, 3]);

        test_watch(x.watch_partial(&vec![vec![T, T, T, F], vec![T, T, F, F], vec![T, F, T, T]]), vec![0, 1, 2, 3]);
        test_watch(x.watch_partial(&vec![vec![T, T, T, F], vec![T, F, T, T], vec![T, F, F, T]]), vec![0, 1, 2, 3]);
        test_watch(x.watch_partial(&vec![vec![T, F, T, T], vec![T, T, T, T], vec![T, T, T, T]]), vec![2, 3]);
        test_watch(x.watch_partial(&vec![vec![T, T, T, F], vec![T, T, F, T], vec![T, T, F, T]]), vec![]);
        test_watch(x.watch_partial(&vec![vec![T, T, T, F], vec![T, T, F, T], vec![T, F, F, T]]), vec![1, 2, 3]);
        test_watch(x.watch_partial(&vec![vec![T, T, F, T], vec![T, T, F, T], vec![T, T, F, T]]), vec![]);
        test_watch(x.watch_partial(&vec![vec![F, T, F, T], vec![T, T, T, T], vec![T, T, T, T]]), vec![0, 2]);
        test_watch(x.watch_partial(&vec![vec![F, F, T, T], vec![T, T, T, T], vec![T, T, T, T]]), vec![2, 3]);
        test_watch(x.watch_partial(&vec![vec![F, F, T, T], vec![F, F, F, F], vec![T, T, T, T]]), vec![2, 3]);
        test_watch(x.watch_partial(&vec![vec![F, F, T, T], vec![T, T, T, T], vec![F, F, F, F]]), vec![2, 3]);
        test_watch(x.watch_partial(&vec![vec![F, T, F, T], vec![F, T, T, T], vec![T, T, T, T]]), vec![0, 2]);
        test_watch(x.watch_partial(&vec![vec![T, T, F, T], vec![F, T, T, T], vec![T, T, T, T]]), vec![0, 1, 2, 3]);
        test_watch(x.watch_partial(&vec![vec![T, F, T, F], vec![T, T, T, T], vec![T, T, T, T]]), vec![1, 3]);
        test_watch(x.watch_partial(&vec![vec![T, F, T, F], vec![F, T, F, T], vec![F, F, F, F]]), vec![0, 1, 2, 3]);
        test_watch(x.watch_partial(&vec![vec![T, T, T, F], vec![F, T, T, F], vec![F, T, T, F]]), vec![0, 3]);
        test_watch(x.watch_partial(&vec![vec![T, T, T, F], vec![F, T, T, F], vec![F, T, F, T]]), vec![0, 2, 3]);

        let x = ltl!(Until(0 | 1, 2 | 3));
        test_watch(x.watch(&vec![vec![T, F, F, F], vec![T, F, F, F], vec![T, F, F, T]]), vec![0, 3]);
        test_watch(x.watch(&vec![vec![T, F, F, F], vec![F, T, F, F], vec![T, F, F, T]]), vec![0, 1, 3]);
        test_watch(x.watch(&vec![vec![T, F, F, F], vec![F, T, F, F], vec![T, F, T, T]]), vec![0, 1, 2, 3]);
        test_watch(x.watch_partial(&vec![vec![T, F, F, F], vec![T, F, F, F], vec![T, F, F, T]]), vec![0, 3],);
        test_watch(x.watch_partial(&vec![vec![T, F, F, F], vec![F, T, F, F], vec![T, F, F, T]]), vec![0, 1, 3],);
        test_watch(x.watch_partial(&vec![vec![T, F, F, F], vec![F, T, F, F], vec![T, F, T, T]]), vec![0, 1, 2, 3],);
    }

    #[test]
    fn watch_modal_release() {
        let x = ltl!(Release(2 & 3, 0 & 1));

        test_watch(x.watch(&vec![vec![T, T, F, F], vec![T, T, T, T], vec![F, F, F, F]]), vec![0, 1, 2, 3]);
        test_watch(x.watch(&vec![vec![T, T, F, F], vec![T, T, F, T], vec![T, T, T, T]]), vec![0, 1, 2, 3]);
        test_watch(x.watch(&vec![vec![T, T, F, F], vec![T, T, F, T], vec![T, T, T, T]]), vec![0, 1, 2, 3]);
        test_watch(x.watch(&vec![vec![F, F, F, F], vec![T, T, F, T], vec![T, T, T, T]]), vec![0, 1, 2, 3]);
        test_watch(x.watch(&vec![vec![T, F, F, T], vec![T, T, F, T], vec![T, T, T, T]]), vec![1, 2]);
        test_watch(x.watch(&vec![vec![T, F, F, T], vec![T, T, T, F], vec![T, T, T, T]]), vec![1, 2, 3]);
        test_watch(x.watch(&vec![vec![T, F, F, T], vec![T, T, T, F], vec![F, T, T, T]]), vec![0, 1, 2, 3]);

        let x = ltl!(Release(2 | 3, 0 | 1));

        test_watch(x.watch(&vec![vec![T, T, F, F], vec![T, T, T, T], vec![F, F, F, F]]), vec![0, 1, 2, 3]);
        test_watch(x.watch(&vec![vec![T, T, F, F], vec![T, T, F, T], vec![T, T, T, T]]), vec![0, 1, 2, 3]);
        test_watch(x.watch(&vec![vec![T, F, F, F], vec![T, F, T, T], vec![F, F, F, F]]), vec![0, 2, 3]);
        test_watch(x.watch(&vec![vec![T, T, F, F], vec![T, T, T, F], vec![F, F, F, F]]), vec![0, 1, 2]);
        test_watch(x.watch(&vec![vec![T, F, F, F], vec![T, F, T, F], vec![F, F, F, F]]), vec![0, 2]);
        test_watch(x.watch(&vec![vec![T, F, F, F], vec![F, T, T, F], vec![F, F, F, F]]), vec![0, 1, 2]);
    }
}
