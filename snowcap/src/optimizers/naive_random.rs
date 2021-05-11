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

use super::Optimizer;
use crate::hard_policies::HardPolicy;
use crate::netsim::{config::ConfigModifier, Network, NetworkError};
use crate::soft_policies::SoftPolicy;
use crate::strategies::{NaiveRandomStrategy, Strategy};
use crate::{Error, Stopper};

use log::*;
use std::time::Duration;

/// # The Naive Random Optimizer
///
/// This strategy exists only for evaluation purpose. The idea is, that it tries completely random
/// orderings, until it succeeds.
pub struct NaiveRandomOptimizer<P> {
    net: Network,
    modifiers: Vec<ConfigModifier>,
    hard_policy: HardPolicy,
    soft_policy: P,
    time_budget: Option<Duration>,
    #[cfg(feature = "count-states")]
    num_states: usize,
}

impl<P> Optimizer<P> for NaiveRandomOptimizer<P>
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
        Ok(Box::new(Self {
            net,
            modifiers,
            hard_policy,
            soft_policy,
            time_budget,
            #[cfg(feature = "count-states")]
            num_states: 0,
        }))
    }

    fn work(&mut self, abort: Stopper) -> Result<(Vec<ConfigModifier>, f64), Error> {
        let mut child = NaiveRandomStrategy::new(
            self.net.clone(),
            self.modifiers.clone(),
            self.hard_policy.clone(),
            self.time_budget,
        )?;
        let child_result = child.work(abort);
        #[cfg(feature = "count-states")]
        {
            self.num_states += child.num_states();
        }
        let sequence = child_result?;
        // compute the cost of this sequence
        let cost = self.get_cost_of_ordering(&sequence);
        Ok((sequence, cost))
    }

    #[cfg(feature = "count-states")]
    fn num_states(&self) -> usize {
        self.num_states
    }
}

impl<P: SoftPolicy + Clone> NaiveRandomOptimizer<P> {
    /// Returns the cost of the ordering, without checking its validity!
    fn get_cost_of_ordering(&self, sequence: &[ConfigModifier]) -> f64 {
        let mut soft_policy = self.soft_policy.clone();
        let mut net = self.net.clone();
        let mut cost: f64 = 0.0;
        for m in sequence {
            match net.apply_modifier(m) {
                Ok(_) => {}
                Err(NetworkError::NoConvergence) => {}
                Err(e) => panic!("Unrecoverable network error: {}", e),
            }
            let mut fw_state = net.get_forwarding_state();
            soft_policy.update(&mut fw_state, &net);
            cost += soft_policy.cost();
        }
        cost
    }
}
