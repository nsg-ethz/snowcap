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

use super::Strategy;
use crate::hard_policies::HardPolicy;
use crate::netsim::{config::ConfigModifier, Network, NetworkError};
use crate::{Error, Stopper};

use log::*;
use rand::prelude::*;
use std::time::{Duration, SystemTime};

/// # The Random Strategy with Insert before Remove
///
/// This strategy exists only for evaluation purpose. The idea is, that it tries completely random
/// orderings, until it succeeds. However, it always shuffles the commands, such that insert will
/// be scheduled before modify, before remove.
pub struct NaiveRandomIBRStrategy {
    net: Network,
    modifiers: Vec<ConfigModifier>,
    hard_policy: HardPolicy,
    stop_time: Option<SystemTime>,
    #[cfg(feature = "count-states")]
    num_states: usize,
}

impl Strategy for NaiveRandomIBRStrategy {
    fn new(
        mut net: Network,
        modifiers: Vec<ConfigModifier>,
        mut hard_policy: HardPolicy,
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
        let stop_time: Option<SystemTime> = time_budget.map(|dur| SystemTime::now() + dur);
        Ok(Box::new(Self {
            net,
            modifiers,
            hard_policy,
            stop_time,
            #[cfg(feature = "count-states")]
            num_states: 0,
        }))
    }

    fn work(&mut self, mut abort: Stopper) -> Result<Vec<ConfigModifier>, Error> {
        let mut sequence_insert = self
            .modifiers
            .iter()
            .filter(|m| matches!(m, ConfigModifier::Insert(_)))
            .cloned()
            .collect::<Vec<_>>();
        let mut sequence_update = self
            .modifiers
            .iter()
            .filter(|m| matches!(m, ConfigModifier::Update{..}))
            .cloned()
            .collect::<Vec<_>>();
        let mut sequence_remove = self
            .modifiers
            .iter()
            .filter(|m| matches!(m, ConfigModifier::Remove(_)))
            .cloned()
            .collect::<Vec<_>>();
        let mut rng = thread_rng();
        loop {
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

            #[cfg(feature = "count-states")]
            {
                self.num_states += self.modifiers.len();
            }

            sequence_insert.shuffle(&mut rng);
            sequence_update.shuffle(&mut rng);
            sequence_remove.shuffle(&mut rng);
            if self.check_sequence(&sequence_insert, &sequence_update, &sequence_remove) {
                let sequence = [sequence_insert, sequence_update, sequence_remove].concat();
                return Ok(sequence);
            }
        }
    }

    #[cfg(feature = "count-states")]
    fn num_states(&self) -> usize {
        self.num_states
    }
}

impl NaiveRandomIBRStrategy {
    fn check_sequence(
        &self,
        seq_i: &[ConfigModifier],
        seq_u: &[ConfigModifier],
        seq_r: &[ConfigModifier],
    ) -> bool {
        let mut net = self.net.clone();
        let mut hard_policy = self.hard_policy.clone();

        // apply every step in sequence
        for modifier in seq_i.iter().chain(seq_u.iter()).chain(seq_r.iter()) {
            match net.apply_modifier(modifier) {
                Ok(()) => {} // nothing to do
                Err(NetworkError::NoConvergence) => return false,
                Err(NetworkError::ConvergenceLoop(_, _)) => return false,
                Err(e) => panic!("Unrecoverable network error: {}", e),
            }
            let mut fw_state = net.get_forwarding_state();
            if let Err(e) = hard_policy.step(&mut net, &mut fw_state) {
                warn!("Error while checking hard policies: {}", e);
                return false;
            };
            if !hard_policy.check() {
                return false;
            }
        }

        true
    }
}
