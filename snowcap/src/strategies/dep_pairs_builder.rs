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

//! This module contains the strategy, which builds dependency pairs while exploring the space, to
//! navigate through the space more effectively. This strategy is not exhaustive, and might not work
//! even though there is a solution!

use super::Strategy;
use crate::hard_policies::{ConstraintsChecker, PolicyError};
use crate::netsim::config::ConfigModifier;
use crate::netsim::{printer, Network, NetworkError};
use crate::{Error, Stopper};
use log::*;
use rand::prelude::*;
use std::time::{Duration, SystemTime};

const NOTHING_LEARNED_THRESHOLD: usize = 10;

/// # The Dependency Pair Builder Strategy
///
/// **Warning** This strategy does not work with most networks. It does not find the relevant
/// dependencies, becuase they are *state specific* and most dependnecy groups have more than two
/// modifiers.
///
/// This strategy builds a dependency tree while navigating the search space more or less randomly,
/// while building dependency pairs to navigate the search space more effectively. It assumes that
/// there does not exist one single possible solution, which would mean that all instructions are
/// dependent on eachother. It is a **non-exhaustive** strategy, meaning that it might not find a
/// solution even though one exists. It **doesn't work** on networks containing dependecies of type
/// 3, 4 and 5.
///
/// The `DepsBuilder` tries random permutations. If one iteration works, then a valid solution was
/// found and it is returned. Assume w.l.o.g, that the modifier ordering `[m1, m2, ..., mx]` does
/// not work, after applying `mx`. `DepsBuilder` will then go through all `mi` with `0 < i < x`, and
/// remove remove the modification and try again. We now have four different possibilities:
///
/// 1. The new ordering still doesn't work. Then, we know that `mi` and `mx` are *probably* not
///    dependent. In this case, we remove `mi` from the working modifier list, and continue the
///    procedure.
/// 2. The ordering suddenly works. Then, we know that `mi` and `mx` are dependent. In this case,
///    we have learned something, and we store this dependency in a set and continue by chooseing a
///    different random ordering, which does comply with everything we have learned so far.
/// 3. The new ordering still doesn't work, and it breaks down after `mj` is applied, where
///    `i < j < x`. In this case, we know that `mi` and `mj` are dependent of eachother, and that
///    `mi` should happen before `mj`. Store this dependency and continue by choosing a different
///    random ordering, which does comply with everything we have learned so far.
/// 4. The modifier in question results in an incorrect network even when applied on the initial
///    network. In this case, we try to iterate over all remaining modifiers (modifiers which were)
///    behind the problematic one in the chosen ordering), and check if the problem goes away when
///    the modifier is applied after the remaining one. If so, we have found a dependency. If not,
///    we have found nothing.
///
/// To choose a new random ordering, we basically prepare a set of possible next choices, and choose
/// a random entry from this list. The possible choices are built by checking the part that is
/// already created, and *unlocking* certain elements if their requirement is already added to the
/// list.
///
/// ## Main Problem of this Strategy
///
/// The main problem is that either, the strategy learns too many wrong dependencies, or doesn't
/// learn them at all. It depends wether we add the check before adding new dependencies, that the
/// ordering is possible in one way and impossible in the other way. This is due to some networks
/// having state-specific dependencies, and they cannot be modelled when the state is not reached.
/// This results in this strategy being equivalent to a random, non-exhaustive and non-unique
/// navigation of the search space without learning anything.
///
/// On the other hand, if we don't add the check, then it immediately finds dependencies, which
/// are actually state-specific, without learning what that state is. There seems to be no solution
/// for solving this problem.
pub struct DepPairsBuilder {
    net: Network,
    modifiers: Vec<ConfigModifier>,
    constraints: ConstraintsChecker,
    /// `dependencies[mx]` is an array of modifiers which need to be applied before mx.
    dependencies: Vec<Vec<bool>>,
    /// `num_deps[mx]` counts how many dependencies `mx` has
    num_deps: Vec<usize>,
    rng: ThreadRng,
    stop_time: Option<SystemTime>,
}

impl Strategy for DepPairsBuilder {
    fn new(
        net: Network,
        modifiers: Vec<ConfigModifier>,
        constraints: ConstraintsChecker,
        time_budget: Option<Duration>,
    ) -> Result<Box<Self>, Error> {
        let mut dependencies = Vec::with_capacity(modifiers.len());
        let mut num_deps = Vec::with_capacity(modifiers.len());
        for _ in 0..modifiers.len() {
            let mut v = Vec::with_capacity(modifiers.len());
            for _ in 0..modifiers.len() {
                v.push(false)
            }
            dependencies.push(v);
            num_deps.push(0);
        }
        constraints
            .check(&mut net.get_forwarding_state())
            .map_err(|e| Error::InvalidInitialState(e))?;
        let stop_time: Option<SystemTime> = time_budget.map(|dur| SystemTime::now() + dur);
        Ok(Box::new(Self {
            net,
            modifiers,
            constraints,
            dependencies,
            num_deps,
            rng: rand::thread_rng(),
            stop_time,
        }))
    }

    fn net(&self) -> &Network {
        &self.net
    }

    fn work(&mut self) -> Result<Vec<ConfigModifier>, Error> {
        // start the endless loop
        info!("Start the DepPairsBuilder procedure");

        let mut nothing_learned_counter = 0;
        loop {
            // check for time budget
            if self.stop_time.as_ref().map(|time| time.elapsed().is_ok()).unwrap_or(false) {
                // time budget is used up!
                error!("Time budget is used up! No solution was found yet!");
                return Err(Error::Timeout);
            }
            // generate a possible ordering
            let ordering = match self.generate_possible_ordering() {
                Some(o) => o,
                None => {
                    error!("Unable to find a solution, encountered a dependency loop!");
                    warn!("Notice, that this is not an exhaustive strategy, so it there might still exist a solution!");
                    return Err(Error::ProbablyNoSafeOrdering);
                }
            };
            // check if everything works
            match self.check_sequence(&ordering) {
                Ok(()) => {
                    // build the sequence and return it, we found a valid sequence
                    let mut result = Vec::new();
                    for modifier_idx in ordering.iter() {
                        result.push(self.modifiers[*modifier_idx].clone());
                    }
                    return Ok(result);
                }
                Err((problematic_idx, _)) => {
                    info!("Trying to reduce the problem to a pair of dependent modifiers");
                    // reduce the problem to two modifications that had problems
                    match self.reduce_problematic_sequence(&ordering, problematic_idx) {
                        Ok(()) => {
                            // We were able to reduce the problematic sequence to a pair of
                            // modifiers. Thus, there is nothing left to do and we can continue with
                            // the next iteration.
                            nothing_learned_counter = 0;
                        }
                        Err(()) => {
                            // In this case, we were not able to solve the problematic modifier. The
                            // problem is that the problematic sequence cannot even be applied on
                            // the initial state of the network. Thus, we need to figure out wich
                            // modifier we can put in front. Since this is the strategy which only
                            // knows pairs of dependencies, we put other modifiers in front (which
                            // are positioned after the problematic modifier in the chosen ordering)
                            // and hope that it works. If not, we return an error, telling that this
                            // problem cannot be solved using the current strategy.
                            info!(
                                "Reduction was not possible! try to solve the problematic modifier!"
                            );
                            let future_ordering: Vec<usize> =
                                ordering.iter().skip(problematic_idx + 1).cloned().collect();
                            match self.find_modifier_before_problematic(
                                ordering[problematic_idx],
                                &future_ordering,
                            ) {
                                Ok(()) => {
                                    // Found a solution, and the dependencies are updated! continue
                                    // with the iteration
                                    nothing_learned_counter = 0;
                                }
                                Err(()) => {
                                    // found no solution. It seems like the strategy is too limited
                                    // for this applied problem. The strategy assumes that a
                                    // modifier is dependent on at most one other modifier. Retry
                                    // with another random sequence.
                                    warn!(
                                        "Could not find any pair of dependency for modifier:\n{}!",
                                        printer::config_modifier(
                                            &self.net,
                                            &self.modifiers[ordering[problematic_idx]]
                                        )
                                        .unwrap()
                                    );
                                    nothing_learned_counter += 1;
                                    if nothing_learned_counter >= NOTHING_LEARNED_THRESHOLD {
                                        error!("Max iterations reached while learning no new dependency!");
                                        warn!("Notice, that this is not an exhaustive strategy, so it there might still exist a solution!");
                                        return Err(Error::ProbablyNoSafeOrdering);
                                    }
                                }
                            };
                        }
                    }
                }
            }
        }
    }
}

impl DepPairsBuilder {
    /// Returns a valid random ordering based on the dependencies already generated. If there is no
    /// possible ordering, which can be prepared, then None is returned.
    ///
    /// # Proof why this algorithm works
    /// Dependencies are very simple in this strategy. New choices are unlocked by adding currently
    /// available choices to the sequence. Assume we are at step `i`, and the current sequence
    /// contains `i` elements. No matter the order of the current sequence, the same choices will be
    /// available. Thus, this algorithm is correct.
    fn generate_possible_ordering(&mut self) -> Option<Vec<usize>> {
        let num_mods = self.modifiers.len();
        // remembers the current order we have chosen.
        let mut order: Vec<usize> = Vec::with_capacity(num_mods);
        // remembers which modifiers can be chosen next.
        let mut choices: Vec<usize> = Vec::with_capacity(num_mods);
        // remembers which dependencies are yet to be fulfilled.
        let mut missing_choices: Vec<Vec<bool>> = self.dependencies.clone();
        // remembers how many dependencies any modifier still has.
        let mut num_deps: Vec<usize> = self.num_deps.clone();

        // initially build the choices vector from all choices which don't have any dependencies
        for mod_idx in 0..num_mods {
            if num_deps[mod_idx] == 0 {
                choices.push(mod_idx);
            }
        }

        // loop until all modifiers were added
        while order.len() < num_mods {
            let num_choices = choices.len();
            // check if there are choices left:
            if num_choices == 0 {
                return None;
            }
            // choose one random entry from choices
            let next_choice_idx: usize = self.rng.gen_range(0, num_choices);
            // take the choice away from the vector and continue
            let next_choice = choices.remove(next_choice_idx);
            // add this choice to the order
            order.push(next_choice);
            // add all options to the choices vector, which are now possible. For this, we need to
            // go through all elements in the dependencies and check if they are now all met.
            for missing_choice in 0..num_mods {
                if missing_choices[missing_choice][next_choice] {
                    missing_choices[missing_choice][next_choice] = false;
                    num_deps[missing_choice] -= 1;
                    if num_deps[missing_choice] == 0 {
                        choices.push(missing_choice);
                    }
                }
            }
        }

        assert_eq!(choices.len(), 0);

        Some(order)
    }

    /// This function checks if the sequence works or not. If the sequence does not work, we return
    /// how many modifiers could be applied successfully.
    fn check_sequence(&mut self, ordering: &Vec<usize>) -> Result<(), (usize, PolicyError)> {
        let mut net = self.net.clone();

        // apply every step in sequence
        for (i, modifier_idx) in ordering.iter().enumerate() {
            match net.apply_modifier(&self.modifiers[*modifier_idx]) {
                Ok(()) => {} // nothing to do
                Err(NetworkError::NoConvergence) => return Err((i, PolicyError::NoConvergence)),
                Err(e) => panic!("Unrecoverable network error: {}", e),
            }
            self.constraints.check(&mut net.get_forwarding_state()).map_err(|e| (i, e))?;
        }

        Ok(())
    }

    /// This function reduces the problematic sequence. The ordering must contain all modifier
    /// indices, and the problematic_idx must point at a specific position in the ordering array,
    /// which tells that this ordering index was the problem.
    ///
    /// We try to remove modifiers from the beginning. In every iteration, three things can happen:
    ///
    /// 1. The sequence still applies until the last one. In this case, we assume that the removed
    ///    modifier has nothing to do with the problematic modifier. Continue with the algorithm
    /// 2. If all modifiers can now be applied after removing `mx`, we declare `mx` to be dependent
    ///    on the problematic modifier, and we return from this funciton
    /// 3. After removing modifier `mx`, the seuqence already failes with modifier `my`. In this
    ///    case, we declare `my` to be dependent on `mx`.
    ///
    /// If, after all those tries, the problematic modifier is left and can still not be applied,
    /// we have figured out that this modifier must be preceeded by a different modifier, which lies
    /// after the problematic modifier in the current ordering. Solving this poblem however is not
    /// solved in this funciton.
    fn reduce_problematic_sequence(
        &mut self,
        ordering: &Vec<usize>,
        problematic_idx: usize,
    ) -> Result<(), ()> {
        let problem_mod_idx: usize = ordering[problematic_idx];

        // create a working vector where we remove elements from the beginning. The working orering
        // is a vector containing only the first elements up to the problematic idx.
        let mut working_ordering: Vec<usize> =
            ordering.iter().take(problematic_idx + 1).cloned().collect();
        // loop until the working ordering contains only one single element
        while working_ordering.len() > 1 {
            // remove the leading element
            let current_mod_idx = working_ordering.remove(0);
            // check if it still works
            match self.check_sequence(&working_ordering) {
                Ok(()) => {
                    // suddenly, the sequence works. We are in case 2, when current_mod_idx depends
                    // on the problematic_idx
                    match self.add_potential_dependency(current_mod_idx, problem_mod_idx) {
                        _ => return Ok(()),
                    }
                }
                Err((i, _)) if i == working_ordering.len() - 1 => {
                    // It does still not work, and it fails again at the same place => Case 1, we
                    // have nothing to do here!
                }
                Err((new_problematic_idx, _)) => {
                    // It appears that we have now a different problem. But we already know the
                    // dependency, and therefore we can add it and return
                    debug!("Found a different potential dependency while reducing the problem.");
                    let new_problem_mod_idx = ordering[new_problematic_idx];
                    match self.add_potential_dependency(new_problem_mod_idx, current_mod_idx) {
                        _ => return Ok(()),
                    }
                }
            }
        }

        // if we are still here, we have not found anything that can solve the problem. It seems
        // we need to apply some different modifier. However, this is not the job of this function.
        // just return an error and let another function do the remaining work.
        Err(())
    }

    /// This function must be called when the function `reduce_problematic_sequence` failed. In this
    /// case, we know that the problematic modifier cannot be applied at the beginning of the
    /// sequence.
    ///
    /// In this funciton, we try to put any of the remaining modifiers in front of the problematic
    /// one. If it works at some point, we know the dependency. If not, we have failed, and this
    /// strategy is unable to find a solution.
    fn find_modifier_before_problematic(
        &mut self,
        problem_mod_idx: usize,
        remaining_modifiers: &Vec<usize>,
    ) -> Result<(), ()> {
        // prepare the ordering vector, which will hold just two elements, and the second will be
        // the problematic modifier.
        let mut ordering: Vec<usize> = Vec::with_capacity(2);
        ordering.push(0); // placeholder
        ordering.push(problem_mod_idx);
        for remaining_mod in remaining_modifiers.iter().cloned() {
            ordering[0] = remaining_mod;
            match self.check_sequence(&ordering) {
                Ok(()) => {
                    // It finally works! We now know that the problematic modifier depends on the
                    // remaining_mod.
                    return self.add_potential_dependency(problem_mod_idx, remaining_mod);
                }
                Err(_) => {
                    // it does not work with the currently chosen remaining_mod. Try again with the
                    // next iteration and hope for the best!
                }
            }
        }

        // in this case, we have not found anything. We need to abort, the strategy is not able to
        // solve this hard problem :(
        Err(())
    }

    /// This function checks if there really exists a dependency between the two modifiers. it
    /// checks this by trying both orderings on the initial network. If only the one works where
    /// the `depends_on` is applied before the `modifier`, then the dependency is added. Else, we
    /// return `Err(())`.
    ///
    /// # TODO
    /// Maybe we need to add other modifiers, that we know need to come after both the modifier and
    /// the depends_on. In this case, we can also deal with more complex dependencies, i.e., type 3.
    /// Two things need to happen:
    /// - We need to add all dependencies (recursively) of both passed modifiers, before both
    ///   modifiers.
    /// - We need to add all modifiers, which are dependent on the two we already have, and all of
    ///   their dependencies (see step before). This way, we check it with the minimal setup.
    ///
    /// In addition to this change, we are able to check much more things. We can check when it
    /// fails, and consequently, why it fails.
    fn add_potential_dependency(&mut self, modifier: usize, depends_on: usize) -> Result<(), ()> {
        debug!(
            "Checking if the potential dependency exists:\napply  {}\nbefore {}",
            printer::config_modifier(&self.net, &self.modifiers[depends_on]).unwrap(),
            printer::config_modifier(&self.net, &self.modifiers[modifier]).unwrap(),
        );
        if modifier == depends_on {
            // the two modifiers must be different
            Err(())
        } else {
            // if the dependency exists, then this orering fails.
            if self.check_sequence(&vec![depends_on, modifier]).is_ok() {
                // also, if there exists a dependency, then the other way around doesn't work
                if self.check_sequence(&vec![modifier, depends_on]).is_err() {
                    // seems like everything is ok, and this is a valid dependency.
                    info!(
                        "Found a dependency:\napply  {}\nbefore {}",
                        printer::config_modifier(&self.net, &self.modifiers[depends_on]).unwrap(),
                        printer::config_modifier(&self.net, &self.modifiers[modifier]).unwrap(),
                    );
                    if !self.dependencies[modifier][depends_on] {
                        self.dependencies[modifier][depends_on] = true;
                        self.num_deps[modifier] += 1;
                    }
                    Ok(())
                } else {
                    // Seems like the two modifiers are in fact not dependent, and both orders work.
                    // This might be the case if the dependency includes other modifiers in between.
                    info!(
                        "Potential dependency turned out to be not a dependency, both cases work!\napply  {}\nbefore {}",
                        printer::config_modifier(&self.net, &self.modifiers[depends_on]).unwrap(),
                        printer::config_modifier(&self.net, &self.modifiers[modifier]).unwrap(),
                    );
                    Err(())
                }
            } else {
                // It seems like the two modifiers are not dependent, because the ordering already
                // fails in the order that we have expected. It might be the case that we have
                // different dependencies before.
                info!(
                        "Potential dependency turned out to be not a dependency, the correct case doesn't work!\napply  {}\nbefore {}",
                        printer::config_modifier(&self.net, &self.modifiers[depends_on]).unwrap(),
                        printer::config_modifier(&self.net, &self.modifiers[modifier]).unwrap(),
                    );
                Err(())
            }
        }
    }
}
