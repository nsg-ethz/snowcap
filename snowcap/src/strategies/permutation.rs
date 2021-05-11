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

//! # The Permutation Strategy

use super::{ExhaustiveStrategy, Strategy};
use crate::hard_policies::HardPolicy;
use crate::netsim::config::ConfigModifier;
use crate::netsim::{printer, Network, NetworkError};
use crate::permutators::{Permutator, PermutatorItem};
use crate::{Error, Stopper};

use log::*;
use std::marker::PhantomData;
use std::time::{Duration, SystemTime};

/// # The Permutation Strategy
///
/// The permutation strategy simply tries all possible sequences of the ConfigModifier, and stops
/// as soon as the first valid sequence was found. You must provide one specific `Permutator`,
/// which is used to navigate through the search space.
///
/// ## Properties
///
/// This strategy does not benefit from dependencies with an *immediate effect*. It performs very
/// bad with all dependencies with a *sparse solution*. However, it is very fast if the chosen
/// ordering is correct, since the network is only cloned for each new permutation.
///
/// ## Type Arguments:
/// - `P` is the chosen [`Permutator`](crate::permutators::Permutator), with an ordering of your
///   choice.
pub struct PermutationStrategy<P> {
    net: Network,
    modifiers: Vec<ConfigModifier>,
    hard_policy: HardPolicy,
    stop_time: Option<SystemTime>,
    phantom: PhantomData<P>,
    #[cfg(feature = "count-states")]
    num_states: usize,
}

impl<P> Strategy for PermutationStrategy<P>
where
    P: Permutator<ConfigModifier> + Iterator,
    P::Item: PermutatorItem<ConfigModifier>,
{
    fn new(
        mut net: Network,
        modifiers: Vec<ConfigModifier>,
        mut hard_policy: HardPolicy,
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

        let mut fw_state = net.get_forwarding_state();
        hard_policy.set_num_mods_if_none(modifiers.len());
        hard_policy.step(&mut net, &mut fw_state)?;
        if !hard_policy.check() {
            error!(
                "Initial state errors: \n    {}",
                hard_policy
                    .last_errors()
                    .into_iter()
                    .map(|e| e.repr_with_name(&net))
                    .collect::<Vec<_>>()
                    .join("\n    "),
            );
            return Err(Error::InvalidInitialState);
        }
        let stop_time: Option<SystemTime> = time_budget.map(|dur| SystemTime::now() + dur);
        Ok(Box::new(Self {
            net,
            modifiers,
            hard_policy,
            stop_time,
            phantom: PhantomData,
            #[cfg(feature = "count-states")]
            num_states: 0,
        }))
    }

    fn work(&mut self, mut abort: Stopper) -> Result<Vec<ConfigModifier>, Error> {
        // check all permutations
        let mut permutator = P::new(self.modifiers.clone());
        while let Some(possible_try) = permutator.next() {
            // check for time budget
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

            let possible_try = possible_try.as_patches();
            debug!(
                "{:?}",
                possible_try
                    .iter()
                    .map(|m| self.modifiers.iter().position(|x| x == m).unwrap())
                    .collect::<Vec<usize>>()
            );
            match self.check_sequence(&possible_try) {
                Ok(()) => return Ok(possible_try),
                Err(index) => {
                    // tell the permutator that we failed
                    permutator.fail_pos(index);
                }
            }
        }

        Err(Error::NoSafeOrdering)
    }

    #[cfg(feature = "count-states")]
    fn num_states(&self) -> usize {
        self.num_states
    }
}

impl<P> ExhaustiveStrategy for PermutationStrategy<P>
where
    P: Permutator<ConfigModifier> + Iterator,
    P::Item: PermutatorItem<ConfigModifier>,
{
}

impl<P> PermutationStrategy<P>
where
    P: Permutator<ConfigModifier> + Iterator,
    P::Item: PermutatorItem<ConfigModifier>,
{
    fn check_sequence(&mut self, patch_seq: &[ConfigModifier]) -> Result<(), usize> {
        let mut net = self.net.clone();

        let mut hard_policy = self.hard_policy.clone();

        // apply every step in sequence
        for (i, modifier) in patch_seq.iter().enumerate() {
            #[cfg(features = "count-states")]
            {
                self.num_states += 1;
            }

            match net.apply_modifier(modifier) {
                Ok(()) => {} // nothing to do
                Err(NetworkError::NoConvergence) => return Err(i),
                Err(NetworkError::ConvergenceLoop(_, _)) => return Err(i),
                Err(e) => panic!("Unrecoverable network error: {}", e),
            }
            let mut fw_state = net.get_forwarding_state();
            if let Err(e) = hard_policy.step(&mut net, &mut fw_state) {
                warn!("Error while checking hard policies: {}", e);
                return Err(i);
            };
            if !hard_policy.check() {
                return Err(i);
            }
        }

        Ok(())
    }
}
