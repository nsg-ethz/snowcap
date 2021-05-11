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

//! # Tree Optimizer

use super::Optimizer;
use crate::hard_policies::HardPolicy;
use crate::netsim::config::ConfigModifier;
use crate::netsim::{printer, Network, NetworkError};
use crate::soft_policies::SoftPolicy;
use crate::{Error, Stopper};

use std::time::{Duration, SystemTime};

use log::*;

/// # Tree Optimizer
///
/// Optimizer that builds a tree of possible modifiers (based on the hard hard_policy), and
/// traverses it by minimizing the soft policies in each step. To achieve this, this optimizer will,
/// at each step of the tree traversal, compute the cost of all possible next choices (modifiers),
/// and take the one with the lowest cost associated with it. During this, we immediately discard
/// modifiers which would lead to an invalid state, with respect to the given hard hard_policy.
///
/// This optimizer is very similar to the [`TreeStrategy`](crate::strategies::TreeStrategy).
/// Hence, this algorithm also works very well with problems consisting only of dependency groups
/// with an immediate effect. In this case, this algorithm is extremely fast. However, when the
/// problems have no immedaite effect, this algorithm will have problems if the number of modidfiers
/// is too large.
#[derive(Debug)]
pub struct TreeOptimizer<P>
where
    P: SoftPolicy + Clone,
{
    net: Network,
    modifiers: Vec<ConfigModifier>,
    hard_policy: HardPolicy,
    soft_policy: P,
    stop_time: Option<SystemTime>,
    max_backtrack_level: usize,
    #[cfg(feature = "count-states")]
    num_states: usize,
}

impl<P> Optimizer<P> for TreeOptimizer<P>
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
        trace!(
            "Modifiers:\n{}",
            modifiers
                .iter()
                .enumerate()
                .map(|(i, m)| format!("M{:02} {}", i, printer::config_modifier(&net, m).unwrap()))
                .collect::<Vec<String>>()
                .join("\n")
        );

        hard_policy.set_num_mods_if_none(modifiers.len());
        let mut fw_state = net.get_forwarding_state();
        hard_policy.step(&mut net, &mut fw_state)?;
        if !hard_policy.check() {
            return Err(Error::InvalidInitialState);
        }
        let stop_time: Option<SystemTime> = time_budget.map(|dur| SystemTime::now() + dur);
        Ok(Box::new(Self {
            net,
            modifiers,
            hard_policy,
            soft_policy,
            stop_time,
            max_backtrack_level: usize::MAX,
            #[cfg(feature = "count-states")]
            num_states: 0,
        }))
    }

    fn work(&mut self, mut abort: Stopper) -> Result<(Vec<ConfigModifier>, f64), Error> {
        let num_mod = self.modifiers.len();

        let mut net = self.net.clone();
        let mut hard_policy = self.hard_policy.clone();

        // backtrack counter
        let mut num_backtrack: usize = 0;

        // setup the stack
        let mut stack: Vec<Vec<StepOption<P>>> = Vec::with_capacity(num_mod);
        stack.push(self.compute_next_options(
            &StepOption {
                mod_idx: 0,
                cost: 0.0,
                choices: (0..num_mod).collect(),
                soft_policy: self.soft_policy.clone(),
            },
            &mut net,
            &mut hard_policy,
        ));

        // generate a vector that stores the current ordering
        let mut current_ord: Vec<StepOption<P>> = Vec::with_capacity(num_mod);

        // start the procedure
        loop {
            // check for max iterations
            if self.stop_time.as_ref().map(|time| time.elapsed().is_ok()).unwrap_or(false) {
                // time budget is used up!
                error!("Time budget is used up! No solution was found yet!");
                break Err(Error::Timeout);
            }

            //eprintln!("stack: {}", stack.len());

            // check for abort criteria
            if abort.try_is_stop().unwrap_or(false) {
                info!("Operation was aborted!");
                break Err(Error::Abort);
            }

            // check if we have already a complete ordering
            if current_ord.len() == num_mod {
                let cost = current_ord.iter().fold(0.0, |acc, x| acc + x.cost);
                let ordering =
                    current_ord.iter().map(|x| self.modifiers[x.mod_idx].clone()).collect();
                break Ok((ordering, cost));
            }

            // check if the stack is empty. If it is, then there exists no valid solution
            if stack.is_empty() {
                error!("No valid solution found!");
                break Err(Error::NoSafeOrdering);
            }

            if let Some(next_best_option) = stack.last_mut().unwrap().pop() {
                // try the current option that is returned
                net.apply_modifier(&self.modifiers[next_best_option.mod_idx]).unwrap();
                let mut fw_state = net.get_forwarding_state();
                hard_policy.step(&mut net, &mut fw_state)?;
                stack.push(self.compute_next_options(
                    &next_best_option,
                    &mut net,
                    &mut hard_policy,
                ));
                current_ord.push(next_best_option);
            } else {
                // pop the stack, we need to go back because the top stack frame has no options left
                stack.pop();
                current_ord.pop();
                net.undo_action()?;
                hard_policy.undo();

                // check backtack counter
                num_backtrack += 1;
                if num_backtrack > self.max_backtrack_level {
                    info!("Maximum allowed backtrack level is reached! Exit early");
                    break Err(Error::ReachedMaxBacktrack);
                }
            }
        }
    }

    #[cfg(feature = "count-states")]
    fn num_states(&self) -> usize {
        self.num_states
    }
}

impl<P> TreeOptimizer<P>
where
    P: SoftPolicy + Clone,
{
    /// Set the maximum backtrack level. If this level is reached, then the strategy will return
    /// `Err(ReachedMaxBacktrack)`. The backtrack level will always be reset if we enter a new
    /// leaf. As an example, if we set the backtrack level to 3, and we need to backtrack twice,
    /// but find another branch to enter, then it will be reset back to 0.
    pub fn set_max_backtrack(&mut self, max_backtrack: usize) {
        self.max_backtrack_level = max_backtrack;
    }

    /// Takes in a vector over the options to pick (as index of the modifiers), and returns a single
    /// stack frame with all the possible options to take. The network will be modified, but the
    /// returned state will remain exactly the same!
    fn compute_next_options(
        &mut self,
        state: &StepOption<P>,
        net: &mut Network,
        hard_policy: &mut HardPolicy,
    ) -> Vec<StepOption<P>> {
        let mut result = Vec::new();
        for (i, opt) in state.choices.clone().into_iter().enumerate() {
            #[cfg(feature = "count-states")]
            {
                self.num_states += 1;
            }
            // first, apply the modifier and get the new network
            let modifier = self.modifiers.get(opt).unwrap();
            match net.apply_modifier(modifier) {
                Ok(_) => {
                    // Network did converge! get the network state
                    let mut fw_state = net.get_forwarding_state();
                    // first, check the hard policies
                    if let Err(e) = hard_policy.step(net, &mut fw_state) {
                        error!("Error while checking the hard policy: {}", e);
                        panic!("Error while checking the hard policy: {}", e);
                    }
                    if hard_policy.check() {
                        // Hard hard_policy are met! Compute the cost and add to the stack frame
                        let mut soft_policy = state.soft_policy.clone();
                        soft_policy.update(&mut fw_state, &net);
                        let cost = soft_policy.cost();
                        let mut choices_left = state.choices.clone();
                        choices_left.remove(i);
                        result.push(StepOption {
                            mod_idx: opt,
                            cost,
                            choices: choices_left,
                            soft_policy,
                        });
                    }
                    hard_policy.undo();
                }
                Err(NetworkError::NoConvergence) | Err(NetworkError::ConvergenceLoop(_, _)) => {
                    // Network did not converge! Option is not possible. Nothing to do here!
                }
                Err(e) => panic! {"Unrecoverable network error: {}", e},
            }
            // undo the last action
            net.undo_action().unwrap();
        }
        // sort the frame sucht that lowest cost element is last (to be popped first)!
        result.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap());
        result
    }
}

#[derive(Debug)]
struct StepOption<P: SoftPolicy + Clone> {
    mod_idx: usize,
    cost: f64,
    choices: Vec<usize>,
    soft_policy: P,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::example_networks::repetitions::*;
    use crate::example_networks::*;
    use crate::hard_policies::*;
    use crate::soft_policies::*;
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

        let mut o =
            TreeOptimizer::new(net, patch.modifiers, hard_policy, soft_policy, None).unwrap();

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

        let mut o =
            TreeOptimizer::new(net, patch.modifiers, hard_policy, soft_policy, None).unwrap();

        let expected_cost = 0.0;

        let (_, cost) = o.work(Stopper::new()).unwrap();
        assert_approx_eq!(expected_cost, cost);
    }
}
