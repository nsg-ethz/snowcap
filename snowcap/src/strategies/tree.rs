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

//! # The Tree Strategy

use super::{ExhaustiveStrategy, Strategy};
use crate::hard_policies::HardPolicy;
use crate::modifier_ordering::ModifierOrdering;
use crate::netsim::config::ConfigModifier;
use crate::netsim::{printer, Network};
use crate::{Error, Stopper};

use log::*;
use std::marker::PhantomData;
use std::time::{Duration, SystemTime};

/// # The Tree Strategy
///
/// The Tree strategy recursively builds a tree by choosing one of the remaining modifiers and
/// simulating the result. If all policies are satisfied, continue by choosing one of the remaining
/// modifiers. if nont of the remaining modifiers work, then fall back and declare this current
/// modifier as not-satisfying the policies.
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
pub struct TreeStrategy<O>
where
    O: ModifierOrdering<ConfigModifier>,
{
    net: Network,
    modifiers: Vec<ConfigModifier>,
    hard_policy: HardPolicy,
    stop_time: Option<SystemTime>,
    phantom: PhantomData<O>,
    #[cfg(feature = "count-states")]
    num_states: usize,
}

impl<O> Strategy for TreeStrategy<O>
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

        let mut fw_state = net.get_forwarding_state();
        hard_policy.set_num_mods_if_none(modifiers.len());
        hard_policy.step(&mut net, &mut fw_state)?;
        if !hard_policy.check() {
            error!(
                "{:#?}",
                hard_policy
                    .last_errors()
                    .iter()
                    .map(|e| e.repr_with_name(&net))
                    .collect::<Vec<_>>()
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
        // initialize the stack
        let mut stack: Vec<Stack> = vec![Stack { rem_mod: self.modifiers.clone(), cur_idx: 0 }];
        let mut mod_sequence: Vec<ConfigModifier> = Vec::new();

        let mut net = self.net.clone();
        let mut hard_policy = self.hard_policy.clone();

        loop {
            let mut pop_stack: bool = false;
            let mut push_stack: Option<Stack> = None;
            if let Some(s) = stack.last_mut() {
                // we are done if s.rem_mod is empty
                if s.rem_mod.is_empty() {
                    break Ok(mod_sequence);
                }
                if s.cur_idx >= s.rem_mod.len() {
                    // the current modifier is equal to the length of s.rem_mod! the current
                    // modifier does not work, pop the stack!
                    pop_stack = true;
                } else {
                    // try the current modifier
                    let cur_idx = s.cur_idx;
                    // move cur_idx to the next position for the next iteration
                    s.cur_idx += 1;
                    // get the current modifier and clone the current network
                    let current_mod: &ConfigModifier = &s.rem_mod[cur_idx];

                    // print the current sequence
                    if STATIC_MAX_LEVEL >= LevelFilter::Debug {
                        let mut print_vec: Vec<usize> = Vec::new();
                        for m in mod_sequence.iter() {
                            print_vec.push(self.modifiers.iter().position(|x| x == m).unwrap());
                        }
                        print_vec
                            .push(self.modifiers.iter().position(|x| x == current_mod).unwrap());
                        debug!("{:?}", print_vec);
                    }

                    // apply the modifier
                    #[cfg(feature = "count-states")]
                    {
                        self.num_states += 1;
                    }

                    let (mod_ok, undo_policy) = if net.apply_modifier(current_mod).is_ok() {
                        let mut fw_state = net.get_forwarding_state();
                        hard_policy.step(&mut net, &mut fw_state)?;
                        if hard_policy.check() {
                            (true, false)
                        } else {
                            (false, true)
                        }
                    } else {
                        (false, false)
                    };

                    if mod_ok {
                        // this single modification works! continue with it
                        let mut new_mod = s.rem_mod.clone();
                        new_mod.remove(cur_idx);
                        push_stack = Some(Stack { rem_mod: new_mod, cur_idx: 0 });
                        mod_sequence.push(current_mod.clone());
                    } else {
                        net.undo_action()?;
                        if undo_policy {
                            hard_policy.undo();
                        }
                    }
                }
            } else {
                // the stack is empty! We found nothing!
                break Err(Error::NoSafeOrdering);
            }

            if pop_stack {
                // undo the network
                net.undo_action()?;
                hard_policy.undo();
                // pop the stack
                stack.pop();
                mod_sequence.pop();
                debug!("Backtrack from tree, current levels: {}", stack.len());

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
            }

            if let Some(new_stack_element) = push_stack.take() {
                stack.push(new_stack_element);
            }
        }
    }

    #[cfg(feature = "count-states")]
    fn num_states(&self) -> usize {
        self.num_states
    }
}

impl<O> ExhaustiveStrategy for TreeStrategy<O> where O: ModifierOrdering<ConfigModifier> {}

struct Stack {
    pub rem_mod: Vec<ConfigModifier>,
    pub cur_idx: usize,
}
