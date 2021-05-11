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

//! # One Optimizer To Rule Them All

use super::utils;
use crate::hard_policies::{HardPolicy, PolicyError};
use crate::modifier_ordering::RandomOrdering;
use crate::netsim::config::ConfigModifier;
use crate::netsim::Network;
use crate::optimizers::Optimizer;
use crate::soft_policies::SoftPolicy;
use crate::strategies::PushBackTreeStrategy;
use crate::{Error, Stopper};

use log::*;
use rand::prelude::*;
use std::time::{Duration, SystemTime};

/// # One Optimizer To Rule Them All
///
/// This is the one optimizer to rule them all, combining the best from the
/// [`TreeOptimizer`](crate::optimizers::TreeOptimizer) and the
/// [`DepGroupsOptimizer`](crate::optimizers::DepGroupsOptimizer) into one single optimizer.
///
/// ## Description
///
/// The optimizer works by exploring the search space of all possible orderings in a tree-like
/// manner. This means, that it proceeds by taking at each step the best possible modifier in terms
/// of minimizing the cost. All others are remembered to be explored later. Once there are no valid
/// modifiers left to try, we are stuck.
///
/// When we are stuck, we try to solve the current problem by finding a dependency group. This
/// procedure is explained in detail
/// [here](crate::strategies::StrategyTRTA#detailed-explenation-of-finding-dependencies). If
/// a dependency with a valid solution could be found, then we reset the exploration tree, but with
/// the group now treated as one single modifier. If however no dependency group could be learned,
/// then backtrack in the current exploration tree, until we have either explored everything, or
/// found a valid solution.
pub struct OptimizerTRTA<P>
where
    P: SoftPolicy + Clone,
{
    net: Network,
    groups: Vec<Vec<ConfigModifier>>,
    hard_policy: HardPolicy,
    soft_policy: P,
    rng: ThreadRng,
    stop_time: Option<SystemTime>,
    max_group_solve_time: Option<Duration>,
    #[cfg(feature = "count-states")]
    num_states: usize,
}

impl<P> Optimizer<P> for OptimizerTRTA<P>
where
    P: SoftPolicy + Clone,
{
    fn new(
        mut net: Network,
        modifiers: Vec<ConfigModifier>,
        mut hard_policy: HardPolicy,
        soft_policy: P,
        time_budget: Option<Duration>,
    ) -> Result<Box<Self>, Error> {
        // clear the undo stack
        net.clear_undo_stack();

        let num_modifiers = modifiers.len();
        let mut groups: Vec<Vec<ConfigModifier>> = Vec::with_capacity(modifiers.len());
        for modifier in modifiers {
            groups.push(vec![modifier]);
        }
        let mut fw_state = net.get_forwarding_state();
        hard_policy.set_num_mods_if_none(num_modifiers);
        hard_policy.step(&mut net, &mut fw_state)?;
        if !hard_policy.check() {
            error!(
                "Initial state errors::\n{}",
                utils::fmt_err(&hard_policy.get_watch_errors(), &net)
            );
            return Err(Error::InvalidInitialState);
        }
        let max_group_solve_time: Option<Duration> =
            time_budget.as_ref().map(|dur| *dur / super::TIME_FRACTION);
        let stop_time: Option<SystemTime> = time_budget.map(|dur| SystemTime::now() + dur);
        Ok(Box::new(Self {
            net,
            groups,
            hard_policy,
            soft_policy,
            rng: rand::thread_rng(),
            stop_time,
            max_group_solve_time,
            #[cfg(feature = "count-states")]
            num_states: 0,
        }))
    }

    fn work(&mut self, mut abort: Stopper) -> Result<(Vec<ConfigModifier>, f64), Error> {
        // clone the network and the hard policies to work with them for the tree exploration
        let mut net = self.net.clone();
        let mut hard_policy = self.hard_policy.clone();
        let mut soft_policy = self.soft_policy.clone();

        // compute the initial options
        let (valid_groups, invalid_groups) = self.prepare_next_option(
            &mut net,
            &mut hard_policy,
            &soft_policy,
            0.0,
            0..self.groups.len(),
        );

        // setup the stack with a randomized frame
        let mut stack =
            vec![StackFrame { num_undo: 0, valid_groups, invalid_groups, idx: 0, soft_policy }];
        let mut current_sequence: Vec<usize> = vec![];

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

            // Check if there are valid options to try
            let action: StackAction<P> = if frame.idx < frame.valid_groups.len() {
                // There exists a valid next step! Get the next option and move the pointer along
                let (next_group_idx, current_cost) = frame.valid_groups[frame.idx];
                frame.idx += 1;

                // push the next step to the sequence
                current_sequence.push(next_group_idx);

                // check if all groups have been added to the sequence
                if current_sequence.len() == self.groups.len() {
                    // We are done! found a valid solution!
                    info!(
                        "Valid solution was found! Learned {} groups",
                        self.groups.iter().filter(|g| g.len() > 1).count()
                    );
                    return Ok((
                        utils::finalize_ordering(&self.groups, &current_sequence),
                        current_cost,
                    ));
                }

                let mut soft_policy = frame.soft_policy.clone();

                // perform the step
                for modifier in self.groups[next_group_idx].iter() {
                    net.apply_modifier(modifier).expect("Modifier should be ok!");
                    let mut fw_state = net.get_forwarding_state();
                    hard_policy.step(&mut net, &mut fw_state).expect("Modifier should be ok!");
                    soft_policy.update(&mut fw_state, &net);
                }

                // compute the next options
                let (valid_groups, invalid_groups) = self.prepare_next_option(
                    &mut net,
                    &mut hard_policy,
                    &soft_policy,
                    current_cost,
                    frame
                        .valid_groups
                        .iter()
                        .map(|(g, _)| g)
                        .filter(|g| **g != next_group_idx)
                        .chain(frame.invalid_groups.iter())
                        .cloned(),
                );

                // Prepare the stack action with the new stack frame
                StackAction::Push(StackFrame {
                    num_undo: self.groups[next_group_idx].len(),
                    valid_groups,
                    invalid_groups,
                    idx: 0,
                    soft_policy,
                })
            } else {
                // There exists no option, that we can take, which would lead to a good result!
                // What we do here is try to find a dependency!
                let random_group_pos = self.rng.gen_range(0, frame.invalid_groups.len());
                match self.find_dependency(
                    &mut net,
                    &mut hard_policy,
                    &current_sequence,
                    frame.invalid_groups[random_group_pos],
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
                        // check the length of the probed sequence. If it is the same length as
                        // the groups, it means that we have already exhaustively checked every
                        // possible permutation, and we can exit here!
                        if current_sequence.len() + 1 == self.groups.len() {
                            return Err(Error::NoSafeOrdering);
                        }
                        StackAction::Pop
                    }
                }
            };

            // at this point, the mutable reference to `stack` (i.e., `frame`) is dropped, which
            // means that `stack` is no longer borrowed exclusively.

            match action {
                StackAction::Pop => {
                    // pop the stack, as long as the top frame has no options left
                    'backtrace: while let Some(frame) = stack.last() {
                        if frame.idx < frame.valid_groups.len() {
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

                    // clone the network and the hard policies to work with them for the tree
                    // exploration
                    net = self.net.clone();
                    hard_policy = self.hard_policy.clone();
                    soft_policy = self.soft_policy.clone();

                    // compute the initial options
                    let (valid_groups, invalid_groups) = self.prepare_next_option(
                        &mut net,
                        &mut hard_policy,
                        &soft_policy,
                        0.0,
                        0..self.groups.len(),
                    );

                    // setup the stack with a randomized frame
                    stack = vec![StackFrame {
                        num_undo: 0,
                        valid_groups,
                        invalid_groups,
                        idx: 0,
                        soft_policy,
                    }];
                    current_sequence.clear();
                }
            }
        }
    }

    #[cfg(feature = "count-states")]
    fn num_states(&self) -> usize {
        self.num_states
    }
}

impl<P> OptimizerTRTA<P>
where
    P: SoftPolicy + Clone,
{
    /// Check all remaining possible choices at the current position in the stack. For all options,
    /// we check if it is possible and what the cost is. Once finished, this function will return a
    /// tuple, where the first vector contains all the valid options, including the cost, already
    /// sorted such that the cheapest is the first, and the second vector contains all invalid
    /// options. Every option is an index into `self.groups`.
    ///
    /// The network and the hard policy will remain in the same state as before!
    fn prepare_next_option(
        &mut self,
        net: &mut Network,
        hard_policy: &mut HardPolicy,
        soft_policy: &P,
        current_cost: f64,
        options: impl Iterator<Item = usize>,
    ) -> (Vec<(usize, f64)>, Vec<usize>) {
        let mut valid_options: Vec<(usize, f64)> = Vec::new();
        let mut invalid_options: Vec<usize> = Vec::new();

        for group_idx in options {
            // perform the modification group
            let mut mod_ok: bool = true;
            let mut num_undo: usize = 0;
            let mut num_undo_policy: usize = 0;
            let mut cost: f64 = current_cost;
            let mut sp = soft_policy.clone();
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
                    sp.update(&mut fw_state, net);
                    cost += sp.cost();
                } else {
                    mod_ok = false;
                    break 'apply_group;
                }
            }

            // undo the hard policy and the network
            (0..num_undo_policy).for_each(|_| hard_policy.undo());
            (0..num_undo).for_each(|_| {
                net.undo_action().expect("Cannot perform undo!");
            });

            // check if the modifier is ok
            if mod_ok {
                valid_options.push((group_idx, cost));
            } else {
                invalid_options.push(group_idx);
            }
        }

        // shuffle the options before sorting them. This gives better randomness during exploration!
        valid_options.shuffle(&mut self.rng);
        // sort the valid options by their cost
        valid_options.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        // return the options
        (valid_options, invalid_options)
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
}

#[derive(Debug, Clone)]
enum StackAction<P>
where
    P: SoftPolicy,
{
    Pop,
    Push(StackFrame<P>),
    Reset,
}

/// Single stack frame for the iteration
#[derive(Clone)]
struct StackFrame<P>
where
    P: SoftPolicy,
{
    /// Number of calls to undo, in order to undo this step
    num_undo: usize,
    /// valid groups
    valid_groups: Vec<(usize, f64)>,
    /// invalid groups
    invalid_groups: Vec<usize>,
    /// index into rem_groups to check next, after all previous branches have been explroed.
    idx: usize,
    /// soft policy at the current step
    soft_policy: P,
}

impl<P> std::fmt::Debug for StackFrame<P>
where
    P: SoftPolicy,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StackFrame")
            .field("num_undo", &self.num_undo)
            .field("valid_groups", &self.valid_groups)
            .field("invalid_groups", &self.invalid_groups)
            .field("idx", &self.idx)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::example_networks::repetitions::*;
    use crate::example_networks::*;
    use crate::hard_policies::*;
    use crate::soft_policies::*;
    use assert_approx_eq::assert_approx_eq;
    use std::time::Duration;

    #[test]
    fn chain_gadget() {
        type R = Repetition5;
        type T = ChainGadget<R>;
        let net = T::net(0);
        let cf = T::final_config(&net, 0);
        let patch = net.current_config().get_diff(&cf);
        let hard_policy =
            HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter());
        let soft_policy = MinimizeTrafficShift::new(&mut net.get_forwarding_state(), &net);

        let mut o = OptimizerTRTA::new(
            net,
            patch.modifiers,
            hard_policy,
            soft_policy,
            Some(Duration::from_secs(1000)),
        )
        .unwrap();

        let expected_cost = (R::get_count() as f64) / ((R::get_count() + 2) as f64);

        let (_, cost) = o.work(Stopper::new()).unwrap();
        assert_approx_eq!(expected_cost, cost);
    }

    #[test]
    fn state_specific_chain_gadget() {
        type R = Repetition5;
        type T = StateSpecificChainGadget<R>;
        let net = T::net(0);
        let cf = T::final_config(&net, 0);
        let patch = net.current_config().get_diff(&cf);
        let hard_policy =
            HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter());
        let soft_policy = MinimizeTrafficShift::new(&mut net.get_forwarding_state(), &net);

        let mut o = OptimizerTRTA::new(
            net,
            patch.modifiers,
            hard_policy,
            soft_policy,
            Some(Duration::from_secs(1000)),
        )
        .unwrap();

        let expected_cost = 0.0;

        let (_, cost) = o.work(Stopper::new()).unwrap();
        assert_approx_eq!(expected_cost, cost);
    }
}
