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

//! # One Strategy To Rule Them All

use super::utils;
use crate::hard_policies::{HardPolicy, PolicyError};
use crate::modifier_ordering::RandomOrdering;
use crate::netsim::config::ConfigModifier;
use crate::netsim::Network;
use crate::strategies::{PushBackTreeStrategy, Strategy};
use crate::{Error, Stopper};

use log::*;
use rand::prelude::*;
use std::time::{Duration, SystemTime};
use utils::fmt_err;

/// # One Strategy To Rule Them All
///
/// This is the one strategy to rule them all, combining the best from the
/// [`TreeStrategy`](crate::strategies::TreeStrategy) and the
/// [`DepGroupsStrategy`](crate::strategies::DepGroupsStrategy) into one single strategy.
///
/// ## Description
///
/// The stretegy works by exploring the search space of all possible orderings in a tree-like
/// manner. This means, that it proceeds by taking one valid modifier, while building a tree of
/// which leaves need to be explored later. Once there are no other valid modifiers to try, we are
/// stuck.
///
/// When we are stuck, we try to solve the current problem by finding a dependency group. This
/// procedure is explained in detail [here](#detailed-explenation-of-finding-dependencies). If a
/// dependency with a valid solution could be found, then we reset the exploration tree, but with
/// the group now treated as one single modifier. If however no dependency group could be learned,
/// then backtrack in the current exploration tree, until we have either explored everything, or
/// found a valid solution.
///
/// ### Detailed Explenation of finding dependencies
///
/// When we are stuck, we try to solve the current problem by finding a dependency group. This is
/// done in three distinct phases. The input to all phases is the current ordering of all groups,
/// including the point where applying the first group fails.
///
/// 1. **Reduction Phase**: In reduciton phase, we try to eliminate groups that seem to have no
///    impact on the dependency group. To do this, we iterate over all groups in the orering (except
///    the problematic group), remove it temporarily from the ordering (to obtain `probe_ordering`)
///    and simulate this new ordering. If this removal has absolutely no effect on the outcome, we
///    call this group to be independent of the problem.
///
///    Notice, that it might happen that the new ordering now fails at an earlier position. In this
///    case, we recursively call `reduce` again, but with the probed group removed. Then, we ad the
///    group back to the beginning of the resulting ordering, but only if the call resulted in no
///    additional recursion.
///
///    The following pseudocode illustrates the procedure:
///
///    ```rust,ignore
///    fn reduce(ordering: Vec<Group>, error: Error) -> Vec<Group> {
///
///        for i in 0..ordering.len() - 1 {
///            // generate the probe ordering by removing the group at position i.
///            let probe_group = ordering[i];
///            let probe_ordering = ordering.clone().remove(i);
///
///            // check the new group
///            match check(probe_ordering) {
///                Ok(()) => {
///                    // Current ordering is dependent on the probed group, because it solves the
///                    // problem.
///                }
///                Err(new_error) if new_error.position != error.position() => {
///                    // The probed ordering fails at a different position! Recursive make the
///                    // problem smaller
///                    let reduced_ordering = reduce(probe_ordering[..new_error.position + 1]);
///                    // insert the probe group back (but only if the level of recursion is
///                    // only 1, i.e., the problem was not made smaller by the call to `reduce`
///                    // above.)
///                    reduced_ordering.insert(0, probe_group);
///                    return reduced_ordering;
///                }
///                Err(new_error) if new_error != error => {
///                    // Current ordering is dependent on the probed group, because it changes the
///                    // error
///                }
///                Err(new_error) if new_error == error => {
///                    // Current ordering is independent on the probed group, because it has no
///                    // effect on the outcome of the network. Remove the group indefinately from
///                    // the ordering.
///                    ordering.remove(i)
///                }
///            }
///        }
///        return ordering
///
///    }
///    ```
///
/// 2. **Solving Phase**: In this phase, we try to find a solution to the reduced problem. This is
///    done using the already existing [`TreeStrategy`](crate::strategies::TreeStrategy). If we can
///    find a valid solution to this problem, then we have found a dependency, and we add it to the
///    list of dependency groups, in the ordering that we have determined. However, if we cannot
///    find any valid solution, we go to step 3 and try to expand the group.
///
/// 3. **Expansion Phase**: In this phase, we try to expand the problem in order to still be abe to
///    find a valid solution. To do this, we iterate over all not yet used groups (excluding those
///    who have already been removed in the reduction phase) and try to place this group at every
///    possible position in the ordering. If the error changes at any point, where the probed grou
///    is moved to, then we add this group to the reduced problem. Once we have found one group with
///    which to extend the problem, we exit and go back to step 2.
///
///    There might be the case, where the probed group changes the problematic group (i.e., the
///    group where the problem happens). In this case, we insert the probed group into this position
///    and go back to step 1, reducing the problem even further.
///
///    The following pseudocode illustrates the procedure:
///
///    ```rust,ignore
///    fn expand(ordering: Vec<Group>, unused: Vec<Group>, error: Error) -> Result<Vec<Group>> {
///
///        // iterate over all unused groups
///        for probe_group in unused {
///            // iterate over all positions where this gorup might be added
///            for i in 0..ordering.range() {
///
///                // generate the probe ordering by removing the group at position i.
///                let probe_ordering = ordering.clone().insert(i, probe_group);
///
///                // check the new group
///                match check(probe_ordering) {
///                    Ok(()) => {
///                        // The probed group seems to be dependent. Add it to the group. Since we
///                        // know, that this is already a solved gorup, we can skip the solving
///                        // phase, and directly call this a new group
///                        return Finish(probe_group)
///                    }
///                    Err(new_error) if new_error.position != error.position() => {
///                        // The probed ordering fails at a different position! Go back to the
///                        // reduction phase and make the problem smaller!
///                        let reduced_ordering = reduce(probe_ordering[..new_error.position + 1]);
///                        // Now, continue to the solving phase
///                        return Ok(reduced_ordering);
///                    }
///                    Err(new_error) if new_error != error => {
///                        // Current ordering is dependent on the probed group, because it changes
///                        // the error. Continue to the solving phase
///                        group.insert(i, probe_group);
///                        return Ok(probe_group)
///                    }
///                    Err(new_error) if new_error == error => {
///                        // The probed ordering has the exact same effect as the original one.
///                        // Continue moving the probe group to other positions in the ordering,
///                        // or continue by going to the next unused group.
///                    }
///                }
///            }
///        }
///        return Err
///    }
///    ```
pub struct StrategyTRTA {
    net: Network,
    groups: Vec<Vec<ConfigModifier>>,
    hard_policy: HardPolicy,
    rng: ThreadRng,
    stop_time: Option<SystemTime>,
    max_group_solve_time: Option<Duration>,
    #[cfg(feature = "count-states")]
    num_states: usize,
    #[cfg(feature = "count-states")]
    seen_difficult_dependency: bool,
}

impl Strategy for StrategyTRTA {
    fn new(
        mut net: Network,
        modifiers: Vec<ConfigModifier>,
        mut hard_policy: HardPolicy,
        time_budget: Option<Duration>,
    ) -> Result<Box<Self>, Error> {
        // clear the undo stack
        net.clear_undo_stack();

        // check the state
        hard_policy.set_num_mods_if_none(modifiers.len());
        let mut fw_state = net.get_forwarding_state();
        hard_policy.step(&mut net, &mut fw_state)?;
        if !hard_policy.check() {
            error!("Initial state errors::\n{}", fmt_err(&hard_policy.get_watch_errors(), &net));
            return Err(Error::InvalidInitialState);
        }

        // prepare the groups
        let mut groups: Vec<Vec<ConfigModifier>> = Vec::with_capacity(modifiers.len());
        for modifier in modifiers {
            groups.push(vec![modifier]);
        }

        // prepare the timings
        let max_group_solve_time: Option<Duration> =
            time_budget.as_ref().map(|dur| *dur / super::TIME_FRACTION);
        let stop_time: Option<SystemTime> = time_budget.map(|dur| SystemTime::now() + dur);
        Ok(Box::new(Self {
            net,
            groups,
            hard_policy,
            rng: rand::thread_rng(),
            stop_time,
            max_group_solve_time,
            #[cfg(feature = "count-states")]
            num_states: 0,
            #[cfg(feature = "count-states")]
            seen_difficult_dependency: false,
        }))
    }

    fn work(&mut self, mut abort: Stopper) -> Result<Vec<ConfigModifier>, Error> {
        // setup the stack with a randomized frame
        let mut stack = vec![StackFrame::new(0..self.groups.len(), 0, &mut self.rng)];
        let mut current_sequence: Vec<usize> = vec![];

        // clone the network and the hard policies to work with them for the tree exploration
        let mut net = self.net.clone();
        let mut hard_policy = self.hard_policy.clone();

        loop {
            // check for iter overflow
            if self.stop_time.as_ref().map(|time| time.elapsed().is_ok()).unwrap_or(false) {
                // time budget is used up!
                error!("Time budget is used up! No solution was found yet!");
                return Err(Error::Timeout);
            }

            // check for abort criteria
            if abort.try_is_stop().unwrap_or(false) {
                info!("Operation was aborted!");
                return Err(Error::Abort);
            }

            // get the latest stack frame
            let frame = match stack.last_mut() {
                Some(frame) => frame,
                None => {
                    error!("Could not find any valid ordering!");
                    return Err(Error::ProbablyNoSafeOrdering);
                }
            };

            // search the current stack frame for the next
            let action: StackAction = match self.get_next_option(&mut net, &mut hard_policy, frame)
            {
                Ok(next_idx) => {
                    // update the current stack frame and prepare the next one
                    frame.idx = next_idx + 1;
                    // There exists a valid next step! Update the current sequence and the stack
                    let next_group_idx = frame.rem_groups[next_idx];
                    current_sequence.push(next_group_idx);

                    // check if all groups have been added to the sequence
                    if current_sequence.len() == self.groups.len() {
                        // We are done! found a valid solution!
                        info!(
                            "Valid solution was found! Learned {} groups",
                            self.groups.iter().filter(|g| g.len() > 1).count()
                        );
                        return Ok(utils::finalize_ordering(&self.groups, &current_sequence));
                    }

                    // Prepare the stack action with the new stack frame
                    StackAction::Push(StackFrame::new(
                        frame.rem_groups.iter().cloned().filter(|x| *x != next_group_idx),
                        self.groups[next_group_idx].len(),
                        &mut self.rng,
                    ))
                }
                Err(check_idx) => {
                    #[cfg(feature = "count-states")]
                    {
                        self.seen_difficult_dependency = true;
                    }
                    // There exists no option, that we can take, which would lead to a good result!
                    // First, we set the next index to the length of the options, in order to
                    // remember that we have checked everything
                    frame.idx = frame.rem_groups.len();
                    // What we do here is try to find a dependency!
                    match self.find_dependency(
                        &mut net,
                        &mut hard_policy,
                        &current_sequence,
                        frame.rem_groups[check_idx],
                        abort.clone(),
                    ) {
                        Some((new_group, old_groups)) => {
                            info!("Found a new dependency group!");
                            // add the new ordering to the known groups
                            utils::add_minimal_ordering_as_new_gorup(
                                &mut self.groups,
                                old_groups,
                                Some(new_group),
                            );
                            // reset the stack frame
                            StackAction::Reset
                        }
                        None => {
                            // No dependency group could be found! Continue exploring the search
                            // space
                            info!("Could not find a new dependency group!");
                            StackAction::Pop
                        }
                    }
                }
            };

            // at this point, the mutable reference to `stack` (i.e., `frame`) is dropped, which
            // means that `stack` is no longer borrowed exclusively.

            match action {
                StackAction::Pop => {
                    // pop the stack, as long as the top frame has no options left
                    'backtrace: while let Some(frame) = stack.last() {
                        if frame.idx < frame.rem_groups.len() {
                            break 'backtrace;
                        } else {
                            // undo the net, the hard policy and pop the current sequence
                            current_sequence.pop();
                            (0..frame.num_undo).for_each(|_| {
                                net.undo_action().expect("Cannot undo the action on the network");
                                hard_policy.undo();
                            });
                            // pop the stack
                            stack.pop();
                        }
                    }
                }
                StackAction::Push(new_frame) => stack.push(new_frame),
                StackAction::Reset => {
                    // reset the stack for the new groups, as well as the sequence, the network and
                    // the hard policies
                    stack = vec![StackFrame::new(0..self.groups.len(), 0, &mut self.rng)];
                    current_sequence.clear();
                    net = self.net.clone();
                    hard_policy = self.hard_policy.clone();
                }
            }
        }
    }

    #[cfg(feature = "count-states")]
    fn num_states(&self) -> usize {
        self.num_states
    }
}

impl StrategyTRTA {
    /// Check all remaining possible choices at the current position in the stack. The first option,
    /// that works is returned (with `Ok(idx)`). However, if none of them seem to work, then one of
    /// the checked and failed groups is returned at random, which should be used to find a
    /// dependency group. The returned index corresponds to the position in `frame.rem_groups`!
    ///
    /// In the OK case, the network and the hard policy will remain in the state of the modification
    /// of which the index is returned
    fn get_next_option(
        &mut self,
        net: &mut Network,
        hard_policy: &mut HardPolicy,
        frame: &StackFrame,
    ) -> Result<usize, usize> {
        assert!(frame.idx < frame.rem_groups.len());
        for group_pos in frame.idx..frame.rem_groups.len() {
            let group_idx = *frame.rem_groups.get(group_pos).unwrap();
            // perform the modification group
            let mut mod_ok: bool = true;
            let mut num_undo: usize = 0;
            let mut num_undo_policy: usize = 0;
            'apply_group: for modifier in self.groups[group_idx].iter() {
                #[cfg(feature = "count-states")]
                {
                    self.num_states += 1;
                }
                num_undo += 1;
                if net.apply_modifier(modifier).is_ok() {
                    num_undo_policy += 1;
                    let mut fw_state = net.get_forwarding_state();
                    hard_policy.step(net, &mut fw_state).expect("cannot check policies!");
                    if !hard_policy.check() {
                        mod_ok = false;
                        break 'apply_group;
                    }
                } else {
                    mod_ok = false;
                    break 'apply_group;
                }
            }

            // check if the modifier is ok
            if mod_ok {
                // everything fine, return the index
                return Ok(group_pos);
            } else {
                // undo the hard policy and the network
                (0..num_undo_policy).for_each(|_| hard_policy.undo());
                (0..num_undo).for_each(|_| {
                    net.undo_action().expect("Cannot perform undo!");
                });
            }
        }

        // if we reach this position, we know that every possible option is bad!
        Err(self.rng.gen_range(frame.idx, frame.rem_groups.len()))
    }

    /// This function tries to find a dependency based on the current position. The arguments
    /// are as follows:
    ///
    /// - `net`: Network at state of the good ordering. After returning, the net will have the exact
    ///   same state as before.
    /// - `hard_policy`: Hard Policy at state of the good ordering. After returning, the hard policy
    ///   will have the exact same state as before.
    /// - `good_ordering`: Ordering of groups, which work up to the point of the bad group
    /// - `bad_group`: Index of the bad group which causes the problme. This function will search a
    ///    dependency to solve this bad group.
    ///
    /// If a dependency was found successfully, then this function will return the new dependency
    /// (first argument), along with the set of groups that are part of this new dependency (second
    /// argument). If no dependency group could be found, then `None` is returned.
    fn find_dependency(
        &mut self,
        net: &mut Network,
        hard_policy: &mut HardPolicy,
        good_ordering: &[usize],
        bad_group: usize,
        abort: Stopper,
    ) -> Option<(Vec<ConfigModifier>, Vec<usize>)> {
        // apply the modifier to the network to get the errors
        let mut num_undo = 0;
        let mut num_undo_policy = 0;
        let mut errors = None;
        'apply_group: for modifier in self.groups[bad_group].iter() {
            num_undo += 1;
            if net.apply_modifier(modifier).is_ok() {
                num_undo_policy += 1;
                let mut fw_state = net.get_forwarding_state();
                hard_policy.step(net, &mut fw_state).expect("cannot check policies!");
                if !hard_policy.check() {
                    errors = Some(hard_policy.get_watch_errors());
                    break 'apply_group;
                }
            } else {
                errors = Some((Vec::new(), vec![Some(PolicyError::NoConvergence)]));
                break 'apply_group;
            }
        }

        // undo the hard policy and the network
        (0..num_undo_policy).for_each(|_| hard_policy.undo());
        (0..num_undo).for_each(|_| {
            net.undo_action().expect("Cannot perform undo!");
        });

        match errors {
            Some(errors) => {
                let ordering = good_ordering
                    .iter()
                    .cloned()
                    .chain(std::iter::once(bad_group))
                    .collect::<Vec<usize>>();
                utils::find_dependency::<PushBackTreeStrategy<RandomOrdering>>(
                    &self.net,
                    &self.groups,
                    &self.hard_policy,
                    &ordering,
                    errors,
                    self.stop_time,
                    self.max_group_solve_time,
                    abort,
                    #[cfg(feature = "count-states")]
                    &mut self.num_states,
                )
            }
            None => panic!("The bad group, passed into this function seems to be fine!"),
        }
    }

    /// Returns true if, during exploration, we encountered a dependency without immediate effect.
    ///
    /// *This method is only available if the `"count-states"` feature is enabled!*
    #[cfg(feature = "count-states")]
    pub fn seen_dependency_without_immediage_effect(&self) -> bool {
        self.seen_difficult_dependency
    }
}

#[derive(Debug, Clone)]
enum StackAction {
    Pop,
    Push(StackFrame),
    Reset,
}

/// Single stack frame for the iteration
#[derive(Debug, Clone)]
struct StackFrame {
    /// Number of calls to undo, in order to undo this step
    num_undo: usize,
    /// Remaining groups to try at this position
    rem_groups: Vec<usize>,
    /// index into rem_groups to check next, after all previous branches have been explroed.
    idx: usize,
}

impl StackFrame {
    fn new(options: impl Iterator<Item = usize>, num_undo: usize, rng: &mut ThreadRng) -> Self {
        let mut rem_groups: Vec<usize> = options.collect();
        rem_groups.shuffle(rng);
        Self { num_undo, rem_groups, idx: 0 }
    }
}
