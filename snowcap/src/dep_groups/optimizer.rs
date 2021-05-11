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

//! # DepGroupsOptimizer
//!
//! This module contains the implementation of the `DepGroupsOptimizer`.

use super::utils;
use crate::hard_policies::{HardPolicy, PolicyError};
use crate::modifier_ordering::SimpleOrdering;
use crate::netsim::config::ConfigModifier;
use crate::netsim::{Network, NetworkError};
use crate::optimizers::{Optimizer, TreeOptimizer};
use crate::permutators::{Permutator, PermutatorItem, RandomTreePermutator};
use crate::soft_policies::SoftPolicy;
use crate::strategies::{GroupStrategy, PushBackTreeStrategy, Strategy};
use crate::{Error, Stopper};

use log::*;
use rand::prelude::*;
use std::marker::PhantomData;
use std::time::{Duration, SystemTime};

const NUM_WORSE_SOLUTIONS_ALLOWED: usize = 9;

/// # DepGroupsOptimizer
///
/// This optimizer is similar to [`DepGroupsStrategy`](crate::strategies::DepGroupsStrategy), but
/// it also tries to minimize the soft-policies during synthesis. The optimizer starts exactly in
/// the same way as the strategy. It also uses a sub-strategy to solve individual groups. Once a
/// valid solution was found, then we solve each individual dependency group again, but this time
/// using an optimizer. But we also use the state of the network where the group is applied. This
/// way, we can get the best ordering for the sub groups for the valid solution. Once we have found
/// a valid solution, we reset the permutator and try again. During this, we always store the best
/// solution. If we have found 10 new solutions, where no one does improve the best score, we stop
/// the algorithm and return the best one found.
pub struct DepGroupsOptimizer<
    P,
    Perm = RandomTreePermutator<usize>,
    S = PushBackTreeStrategy<SimpleOrdering>,
    O = TreeOptimizer<P>,
> where
    P: SoftPolicy,
    O: Optimizer<P>,
    S: Strategy + GroupStrategy,
    Perm: Permutator<usize>,
    Perm::Item: PermutatorItem<usize>,
{
    net: Network,
    groups: Vec<Vec<ConfigModifier>>,
    hard_policy: HardPolicy,
    soft_policy: P,
    permutator: Perm,
    rng: ThreadRng,
    stop_time: Option<SystemTime>,
    max_group_solve_time: Option<Duration>,
    phantom: PhantomData<(O, S)>,
    #[cfg(feature = "count-states")]
    num_states: usize,
}

impl<P, Perm, S, O> Optimizer<P> for DepGroupsOptimizer<P, Perm, S, O>
where
    P: SoftPolicy + Clone,
    O: Optimizer<P>,
    S: Strategy + GroupStrategy,
    Perm: Permutator<usize>,
    Perm::Item: PermutatorItem<usize>,
{
    fn new(
        mut net: Network,
        modifiers: Vec<ConfigModifier>,
        mut hard_policy: HardPolicy,
        soft_policy: P,
        time_budget: Option<Duration>,
    ) -> Result<Box<Self>, Error> {
        let num_modifiers = modifiers.len();
        let mut groups: Vec<Vec<ConfigModifier>> = Vec::with_capacity(num_modifiers);
        for modifier in modifiers {
            groups.push(vec![modifier]);
        }
        let permutator = Perm::new((0..groups.len()).collect());
        hard_policy.set_num_mods_if_none(num_modifiers);
        let mut fw_state = net.get_forwarding_state();
        hard_policy.step(&mut net, &mut fw_state)?;
        if !hard_policy.check() {
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
            permutator,
            rng: rand::thread_rng(),
            stop_time,
            max_group_solve_time,
            phantom: PhantomData,
            #[cfg(feature = "count-states")]
            num_states: 0,
        }))
    }

    fn work(&mut self, mut abort: Stopper) -> Result<(Vec<ConfigModifier>, f64), Error> {
        let mut best_solution: Option<(Vec<ConfigModifier>, f64)> = None;
        let mut num_no_best_found: usize = 0;

        'main_loop: loop {
            // check for time budget
            if self.stop_time.as_ref().map(|time| time.elapsed().is_ok()).unwrap_or(false) {
                // time budget is used up!
                if let Some(solution) = best_solution {
                    info!("Time budget is used up! Returning the best solution yet!");
                    return Ok(solution);
                }
                error!("Time budget is used up! No solution was found yet!");
                break Err(Error::Timeout);
            }

            // check for abort criteria
            if abort.try_is_stop().unwrap_or(false) {
                if let Some(solution) = best_solution {
                    info!("Operation was aborted! Returning the best solution yet!");
                    return Ok(solution);
                }
                info!("Operation was aborted before we found a solution!");
                break Err(Error::Abort);
            }

            // .--------.
            // | Step 1 | Choose random ordering
            // '--------'
            let ordering = match self.permutator.next() {
                Some(o) => o,
                None => {
                    if let Some(solution) = best_solution {
                        info!("Tried all permutations of the learned groups. Returning the best!");
                        return Ok(solution);
                    }
                    error!("Strategy was not able to solve the problem!");
                    return Err(Error::NoSafeOrdering);
                }
            }
            .as_patches();

            debug!(
                "ordering of groups:\n{}",
                utils::fmt_group_ord(&self.groups, &ordering, &self.net),
            );

            // .--------.
            // | Step 2 | Check Ordering
            // '--------'
            debug!("Step 2");
            // we already have the information if the group fails. Thus, we don't need to do anything here.
            let (problem_group_pos, errors) = match utils::check_group_ordering(
                self.net.clone(),
                &self.groups,
                &self.hard_policy,
                &ordering,
                #[cfg(feature = "count-states")]
                &mut self.num_states,
            ) {
                Ok(_) => {
                    // now, get the best possible result by optimizing every group by itself.
                    info!("Ordering works! Finding the best ordering of the groups in the ordering...");
                    let (finalized_ordering, cost) = match self
                        .optimize_groups_in_ordering(&ordering, abort.clone())
                    {
                        Ok(r) => r,
                        Err(_) => {
                            warn!("Reordering some groups seems to destroy the group ordering! Using the ordering which we know works...");
                            // compute the cost of the default ordering
                            let cost = self.get_cost_of_ordering(&ordering);
                            let finalized_ordering =
                                utils::finalize_ordering(&self.groups, &ordering);
                            (finalized_ordering, cost)
                        }
                    };
                    // print the resulting groups
                    info!("Found a valid ordering with cost {}", cost);
                    if cost < best_solution.as_ref().map(|s| s.1).unwrap_or(f64::INFINITY) {
                        info!("NEW BEST SOLUTION");
                        num_no_best_found = 0;
                        best_solution = Some((finalized_ordering, cost));
                    } else {
                        info!("Solution is not the best yet!");
                        num_no_best_found += 1;
                        if num_no_best_found > NUM_WORSE_SOLUTIONS_ALLOWED {
                            info!(
                                "The last {} valid solutions were no improvement! Abort",
                                num_no_best_found
                            );
                            return Ok(best_solution.unwrap());
                        }
                    }
                    // We want to take a different very much different permutation than before.
                    // Hence, we just restart the permutator. Since we use the random permutation,
                    // this will most likely result in a better solution. If not, then wi will
                    // immediately find the same solution and try again. This is infact the desired
                    // behavior.
                    let mut group_idx: Vec<usize> = (0..self.groups.len()).collect();
                    group_idx.shuffle(&mut self.rng);
                    self.permutator = Perm::new(group_idx);
                    continue 'main_loop;
                }
                Err((_, i, Some(hard_policy))) => (i, hard_policy.get_watch_errors()),
                Err((_, i, None)) => (i, (Vec::new(), vec![Some(PolicyError::NoConvergence)])),
            };

            // .--------.
            // | Step 3 | Find dependencies
            // '--------'
            match utils::find_dependency::<S>(
                &self.net,
                &self.groups,
                &self.hard_policy,
                &ordering,
                errors,
                self.stop_time,
                self.max_group_solve_time,
                abort.clone(),
                #[cfg(feature = "count-states")]
                &mut self.num_states,
            ) {
                Some((new_group, old_groups)) => {
                    info!("Found a new dependency group!");
                    // add the new ordering to the known groups
                    utils::add_minimal_ordering_as_new_gorup(
                        &mut self.groups,
                        old_groups,
                        Some(new_group),
                    );

                    // prepare a new permutator for the next iteration
                    let mut group_idx: Vec<usize> = (0..self.groups.len()).collect();
                    group_idx.shuffle(&mut self.rng);
                    self.permutator = Perm::new(group_idx);

                    continue 'main_loop;
                }
                None => {
                    // Unable to extend the running group! Declare this try as failed and try
                    // again. tell the permutator that we have failed at the position
                    info!("Could not find a new dependency group!");
                    self.permutator.fail_pos(problem_group_pos);
                    // continue with the permutation
                    continue 'main_loop;
                }
            }
        }
    }

    #[cfg(feature = "count-states")]
    fn num_states(&self) -> usize {
        self.num_states
    }
}

impl<P, Perm, S, O> DepGroupsOptimizer<P, Perm, S, O>
where
    P: SoftPolicy + Clone,
    O: Optimizer<P>,
    S: Strategy + GroupStrategy,
    Perm: Permutator<usize>,
    Perm::Item: PermutatorItem<usize>,
{
    /// Returns the cost of the ordering, without checking its validity!
    fn get_cost_of_ordering(&self, sequence: &[usize]) -> f64 {
        let mut soft_policy = self.soft_policy.clone();
        let mut net = self.net.clone();
        let mut cost: f64 = 0.0;
        for gid in sequence.iter() {
            for m in self.groups[*gid].iter() {
                match net.apply_modifier(m) {
                    Ok(_) => {}
                    Err(NetworkError::NoConvergence) | Err(NetworkError::ConvergenceLoop(_, _)) => {
                    }
                    Err(e) => panic!("Unrecoverable network error: {}", e),
                }
                let mut fw_state = net.get_forwarding_state();
                soft_policy.update(&mut fw_state, &net);
                cost += soft_policy.cost();
            }
        }
        cost
    }

    /// This function optimizes each group separately in the sequence. The optimization is done
    /// based on the current state in which the network is in when the group is applied.
    fn optimize_groups_in_ordering(
        &mut self,
        sequence: &[usize],
        abort: Stopper,
    ) -> Result<(Vec<ConfigModifier>, f64), Error> {
        let mut net = self.net.clone();
        let mut hard_policy = self.hard_policy.clone();
        let mut soft_policy = self.soft_policy.clone();
        let mut cost: f64 = 0.0;
        let mut good_ordering = Vec::new();

        for gid in sequence {
            let current_group: &Vec<ConfigModifier> = self.groups.get(*gid).unwrap();
            let group_size = current_group.len();
            if group_size == 1 {
                // group is a single modifier, nothing to optimize here. Just apply and compute the
                // cost. We can already assume that the ordering works. if not, then we have done a
                // mistake, and not learned the dependencies correctly.
                net.apply_modifier(current_group.get(0).unwrap())?;
                let mut fw_state = net.get_forwarding_state();
                hard_policy.step(&mut net, &mut fw_state)?;
                if hard_policy.check() {
                    return Err(Error::ProbablyNoSafeOrdering);
                }
                soft_policy.update(&mut fw_state, &net);
                cost += soft_policy.cost();
                // insert the modifier into the good ordering
                good_ordering.push(current_group.get(0).unwrap().clone());
            } else {
                // group is not a single modifier! Use the optimizer to get the best result
                let time_budget = self.stop_time.as_ref().map(|time| {
                    time.duration_since(SystemTime::now()).unwrap_or_else(|_| Duration::new(0, 0))
                });
                let mut child = O::new(
                    net.clone(),
                    current_group.clone(),
                    hard_policy.clone(),
                    soft_policy.clone(),
                    time_budget,
                )?;
                let child_result = child.work(abort.clone());
                #[cfg(feature = "count-states")]
                {
                    self.num_states += 1;
                }
                let (group_sequence, group_cost) = child_result?;
                // apply this best result to the net
                for m in group_sequence.into_iter() {
                    net.apply_modifier(&m)?;
                    let mut fw_state = net.get_forwarding_state();
                    hard_policy.step(&mut net, &mut fw_state)?;
                    assert!(hard_policy.check());
                    good_ordering.push(m);
                }
                // update the cost and the soft policy
                cost += group_cost;
                soft_policy = P::new(&mut net.get_forwarding_state(), &net);
            }
        }
        Ok((good_ordering, cost))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::example_networks::repetitions::*;
    use crate::example_networks::*;
    use crate::hard_policies::*;
    use crate::modifier_ordering::*;
    use crate::permutators::*;
    use crate::soft_policies::*;
    use crate::strategies::*;
    use assert_approx_eq::assert_approx_eq;

    #[test]
    fn test_chain_gadget() {
        type R = Repetition5;
        type T = ChainGadget<R>;
        let net = T::net(0);
        let cf = T::final_config(&net, 0);
        let patch = net.current_config().get_diff(&cf);
        let hard_policy =
            HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter());
        let soft_policy = MinimizeTrafficShift::new(&mut net.get_forwarding_state(), &net);

        let mut o = DepGroupsOptimizer::<
            _,
            RandomTreePermutator<usize>,
            PushBackTreeStrategy<SimpleOrdering>,
            TreeOptimizer<_>,
        >::new(net, patch.modifiers, hard_policy, soft_policy, None)
        .unwrap();

        let expected_cost = (R::get_count() as f64) / ((R::get_count() + 2) as f64);

        let (_, cost) = o.work(Stopper::new()).unwrap();
        assert_approx_eq!(expected_cost, cost);
    }

    #[test]
    fn test_state_specific_chain_gadget() {
        type R = Repetition5;
        type T = StateSpecificChainGadget<R>;
        let net = T::net(0);
        let cf = T::final_config(&net, 0);
        let patch = net.current_config().get_diff(&cf);
        let hard_policy =
            HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter());
        let soft_policy = MinimizeTrafficShift::new(&mut net.get_forwarding_state(), &net);

        let mut o = DepGroupsOptimizer::<
            _,
            RandomTreePermutator<usize>,
            PushBackTreeStrategy<SimpleOrdering>,
            TreeOptimizer<_>,
        >::new(net, patch.modifiers, hard_policy, soft_policy, None)
        .unwrap();

        let expected_cost = 0.0;

        let (_, cost) = o.work(Stopper::new()).unwrap();
        assert_approx_eq!(expected_cost, cost);
    }
}
