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

//! # The Push-Back Tree Strategy

use super::{ExhaustiveStrategy, GroupStrategy, Strategy};
use crate::hard_policies::HardPolicy;
use crate::modifier_ordering::ModifierOrdering;
use crate::netsim::config::ConfigModifier;
use crate::netsim::{printer, Network};
use crate::{Error, Stopper};

use log::*;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::time::{Duration, SystemTime};

/// # The Push-Back Tree Strategy
///
/// The Push-Back Tree strategy is based on the `TreeStrategy`. The difference is that once it
/// encounteres a modifier it cannot apply, it will push the modifier to the tail of the list. Then,
/// when finding another modifier that works, the strategy still uses the sequence of remaining
/// modifiers, where the not-working modifier is at the end. Instead of using a list, we obviously use
/// a ring buffer, where moving an element from the front to the back is very cheap.
///
/// This strategy implements the [`GroupStrategy`](super::GroupStrategy). Here, groups are
/// considered single modifiers, and remain together until we have found a solution. If there exists
/// no solution with the provided groups, all groups are broken up into the individual modifiers,
/// and the algorithm starts from the beginning.
///
/// ## Properties
///
/// This strategy benefits from problems with an *immediate effect*, since it can massively reduce
/// the search space if a problem is detected in an early stage of the tree. Thus, it is able to
/// find a solution of a `sparse problem` with *immediate effect* very quickly (`O(n^3)`). However,
/// it has problems when dependencies have *no immediate effect*.
///
/// ## Type Arguments
/// - `O` represents the chosen [`ModifierOrdering`](crate::modifier_ordering::ModifierOrdering),
///   which is used to order the modifiers before the tree algorithm starts.
pub struct PushBackTreeStrategy<O> {
    net: Network,
    modifiers: Vec<ConfigModifier>,
    groups: Vec<Vec<usize>>,
    hard_policy: HardPolicy,
    stop_time: Option<SystemTime>,
    max_backtrack_level: usize,
    phantom: PhantomData<O>,
    #[cfg(feature = "count-states")]
    num_states: usize,
}

impl<O> Strategy for PushBackTreeStrategy<O>
where
    O: ModifierOrdering<ConfigModifier>,
{
    fn new(
        mut net: Network,
        mut modifiers: Vec<ConfigModifier>,
        mut hard_policy: HardPolicy,
        time_budget: Option<Duration>,
    ) -> Result<Box<Self>, Error> {
        // clear the undo stack
        net.clear_undo_stack();

        // sort the modifiers
        O::sort(&mut modifiers);

        trace!(
            "Modifiers:\n{}",
            modifiers
                .iter()
                .enumerate()
                .map(|(i, m)| format!("M{:02} {}", i, printer::config_modifier(&net, m).unwrap()))
                .collect::<Vec<String>>()
                .join("\n")
        );

        let n_modifiers = modifiers.len();
        hard_policy.set_num_mods_if_none(n_modifiers);
        let mut fw_state = net.get_forwarding_state();
        hard_policy.step(&mut net, &mut fw_state)?;
        if !hard_policy.check() {
            error!(
                "Initial state errors: \n    {}",
                hard_policy
                    .get_watch_errors()
                    .1
                    .into_iter()
                    .filter_map(|e| e.map(|e| e.repr_with_name(&net)))
                    .collect::<Vec<_>>()
                    .join("\n    "),
            );
            return Err(Error::InvalidInitialState);
        }
        let stop_time: Option<SystemTime> = time_budget.map(|dur| SystemTime::now() + dur);

        Ok(Box::new(Self {
            net,
            modifiers,
            groups: (0..n_modifiers).map(|i| vec![i]).collect(),
            hard_policy,
            stop_time,
            max_backtrack_level: usize::MAX,
            phantom: PhantomData,
            #[cfg(feature = "count-states")]
            num_states: 0,
        }))
    }

    fn work(&mut self, mut abort: Stopper) -> Result<Vec<ConfigModifier>, Error> {
        // initialize the stack
        let mut stack: Vec<Stack> = vec![Stack::from_vec((0..self.groups.len()).collect(), 0)];
        // points into the groups vector
        let mut group_sequence: Vec<usize> = Vec::new();
        // number of modifiers applied on the network

        let mut net = self.net.clone();
        let mut hard_policy = self.hard_policy.clone();

        // backtrack level checker
        let mut num_backtrack: usize = 0;

        'main_loop: loop {
            let mut pop_stack: bool = false;
            let mut push_stack: Option<Stack> = None;
            if let Some(s) = stack.last_mut() {
                // we are done if s.rem_mod is empty
                if s.rem_group.is_empty() {
                    break Ok(self.finalize_ordering(group_sequence));
                }
                if s.cur_idx >= s.rem_group.len() {
                    // the current modifier is equal to the length of s.rem_mod! the current
                    // modifier does not work, pop the stack!
                    pop_stack = true;
                } else {
                    // get the current modifier and clone the current network
                    let current_group: usize = s.rem_group.pop_front().unwrap();

                    // print trace
                    debug!("Trying the sequence {:?}", group_sequence);

                    // perform the modification group
                    let mut mod_ok: bool = true;
                    let mut num_undo: usize = 0;
                    let mut num_undo_policy: usize = 0;
                    'apply_group: for m_idx in self.groups[current_group].iter() {
                        #[cfg(feature = "count-states")]
                        {
                            self.num_states += 1;
                        }

                        num_undo += 1;
                        if net.apply_modifier(self.modifiers.get(*m_idx).unwrap()).is_ok() {
                            num_undo_policy += 1;
                            let mut fw_state = net.get_forwarding_state();
                            hard_policy.step(&mut net, &mut fw_state)?;
                            if !hard_policy.check() {
                                mod_ok = false;
                                break 'apply_group;
                            }
                        } else {
                            mod_ok = false;
                            break 'apply_group;
                        }
                    }

                    if mod_ok {
                        // this single modification works! continue with it
                        push_stack =
                            Some(Stack { num_undo, rem_group: s.rem_group.clone(), cur_idx: 0 });
                        group_sequence.push(current_group);
                    } else {
                        // undo the changes
                        for _ in 0..num_undo {
                            net.undo_action()?;
                        }
                        for _ in 0..num_undo_policy {
                            hard_policy.undo();
                        }
                    }
                    // push the current modifier back into the ring buffer, at the last position.
                    s.rem_group.push_back(current_group);
                    // move cur_idx to the next position for the next iteration
                    s.cur_idx += 1;
                }
            } else {
                // the stack is empty! We found nothing!
                // check if no groups were submitted. If they were, we remove all the groups and
                // try again.
                if self.groups.len() != self.modifiers.len() {
                    warn!("Could not solve the problem using the given groups. Retrying without groups ({} modifiers)...", self.modifiers.len());
                    // the net must not have any history left
                    assert_eq!(net.undo_action()?, false);
                    // sort the modifiers
                    O::sort(&mut self.modifiers);
                    // print the order of the sorted modifiers
                    debug!(
                        "Sorted modifiers:\n{}",
                        self.modifiers
                            .iter()
                            .enumerate()
                            .map(|(i, m)| format!(
                                "{:02} {}",
                                i,
                                printer::config_modifier(&self.net, m).unwrap()
                            ))
                            .collect::<Vec<String>>()
                            .join("\n")
                    );
                    // set the group to the sorted modifiers.
                    self.groups = (0..self.modifiers.len()).map(|i| vec![i]).collect();
                    // re-initialize the stack
                    stack = vec![Stack::from_vec((0..self.groups.len()).collect(), 0)];
                    // clear the current sequence
                    group_sequence = Vec::new();
                    // continue with the loop
                    continue 'main_loop;
                } else {
                    // else, we cannot find anything! break out of the main loop
                    break 'main_loop Err(Error::NoSafeOrdering);
                }
            }

            if pop_stack {
                let stack_frame = stack.pop();
                // undo the network
                let num_undo = stack_frame.map(|s| s.num_undo).unwrap_or(0);
                for _ in 0..num_undo {
                    net.undo_action()?;
                    hard_policy.undo();
                }

                group_sequence.pop();
                trace!("Backtrack from tree, current levels: {}", stack.len());

                // check for time budget
                if self.stop_time.as_ref().map(|time| time.elapsed().is_ok()).unwrap_or(false) {
                    // time budget is used up!
                    error!("Time budget is used up! No solution was found yet!");
                    break Err(Error::Timeout);
                }

                // check for abort criteria
                if abort.try_is_stop().unwrap_or(false) {
                    info!("Operation was aborted!");
                    break Err(Error::Abort);
                }

                // check the backtrack counter
                num_backtrack += 1;
                if num_backtrack > self.max_backtrack_level {
                    info!("Maximum allowed backtrack level is reached! Exit early");
                    break Err(Error::ReachedMaxBacktrack);
                }
            }

            if let Some(new_stack_element) = push_stack.take() {
                stack.push(new_stack_element);
                num_backtrack = 0;
            }
        }
    }

    #[cfg(feature = "count-states")]
    fn num_states(&self) -> usize {
        self.num_states
    }
}

impl<O> GroupStrategy for PushBackTreeStrategy<O>
where
    O: ModifierOrdering<ConfigModifier>,
{
    fn from_groups(
        mut net: Network,
        groups: Vec<Vec<ConfigModifier>>,
        mut hard_policy: HardPolicy,
        time_budget: Option<Duration>,
    ) -> Result<Box<Self>, Error> {
        debug!(
            "Starting Push-Back Tree with groups:\n{}",
            groups
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    format!(
                        "G{:02} {}",
                        i,
                        v.iter()
                            .map(|m| printer::config_modifier(&net, m).unwrap())
                            .collect::<Vec<String>>()
                            .join("\n    ")
                    )
                })
                .collect::<Vec<String>>()
                .join("\n")
        );

        let mut groups_idx: Vec<Vec<usize>> = Vec::with_capacity(groups.len());
        let mut modifiers: Vec<ConfigModifier> = Vec::new();
        let mut idx: usize = 0;
        for group in groups {
            let mut group_idx: Vec<usize> = Vec::with_capacity(group.len());
            for m in group {
                group_idx.push(idx);
                modifiers.push(m);
                idx += 1;
            }
            groups_idx.push(group_idx);
        }

        let mut fw_state = net.get_forwarding_state();
        hard_policy.step(&mut net, &mut fw_state)?;
        if !hard_policy.check() {
            return Err(Error::InvalidInitialState);
        }
        let stop_time: Option<SystemTime> = time_budget.map(|dur| SystemTime::now() + dur);
        Ok(Box::new(Self {
            net,
            modifiers,
            groups: groups_idx,
            hard_policy,
            stop_time,
            max_backtrack_level: usize::MAX,
            phantom: PhantomData,
            #[cfg(feature = "count-states")]
            num_states: 0,
        }))
    }
}

impl<O> PushBackTreeStrategy<O>
where
    O: ModifierOrdering<ConfigModifier>,
{
    /// Set the maximum backtrack level. If this level is reached, then the strategy will return
    /// `Err(ReachedMaxBacktrack)`. The backtrack level will always be reset if we enter a new
    /// leaf. As an example, if we set the backtrack level to 3, and we need to backtrack twice,
    /// but find another branch to enter, then it will be reset back to 0.
    pub fn set_max_backtrack(&mut self, max_backtrack: usize) {
        self.max_backtrack_level = max_backtrack;
    }

    fn finalize_ordering(&self, group_ordering: Vec<usize>) -> Vec<ConfigModifier> {
        group_ordering
            .iter()
            .map(|i| self.groups[*i].iter())
            .flatten()
            .map(|i| self.modifiers[*i].clone())
            .collect()
    }
}

impl<O> ExhaustiveStrategy for PushBackTreeStrategy<O> where O: ModifierOrdering<ConfigModifier> {}

struct Stack {
    pub num_undo: usize,
    pub rem_group: VecDeque<usize>,
    pub cur_idx: usize,
}

impl Stack {
    pub fn from_vec(rem_mod: Vec<usize>, cur_idx: usize) -> Self {
        let mut rb: VecDeque<usize> = VecDeque::with_capacity(rem_mod.len());
        for m in rem_mod {
            rb.push_back(m);
        }
        Self { num_undo: 0, rem_group: rb, cur_idx }
    }
}
