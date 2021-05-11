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

//! # DepGroupsUtils
//!
//! Utility functions used by both the `DepGroupsStrategy` and the `DepGroupsOptimizer`. These
//! functions are necessary for reducing the dependency group, and expanding it. However, it is
//! agnostic to wether we try to optimize for soft-policies, or only consider hard-policy.

use crate::hard_policies::{HardPolicy, PolicyError, WatchErrors};
use crate::netsim::config::ConfigModifier;
use crate::netsim::{printer, Network, NetworkError};
use crate::strategies::{GroupStrategy, Strategy};
use crate::{Error, Stopper};

use log::*;
use std::time::{Duration, SystemTime};

/// # Finding Dependencies
///
/// This function tries to find a dependency based on the current position. The arguments
/// are as follows:
///
/// - `net`: Network at the initial state
/// - `groups`: Vector containing all groups
/// - `hard_policy`: Hard Policies at the initial state
/// - `ordering`: Slice to the sequence, where the last element is the one which causes a problem.
///
/// If a dependency was found successfully, then this function will return the new dependency
/// (first argument), along with the set of groups that are part of this new dependency (second
/// argument). If no dependency group could be found, then `None` is returned.
///
/// ## Description of the algorithm
///
/// The algorithm is split into three distinct steps:
///
/// 1. The algorithm reduces the problem by removing modifier groups that seem to be independent of
///    the current problem.
/// 2. Try to solve the minimal problem. If there exists a valid solution, return it. If not, to to
///    step 3.
/// 3. Try to extend the problem by adding yet unexplored groups to the problem. If the problem
///    changes, then go back to step 2 to solve the problem. If the problem group cannot be
///    expanded, finding dependencies failed, and return `None`.
#[allow(clippy::too_many_arguments)]
pub(super) fn find_dependency<S>(
    net: &Network,
    groups: &[Vec<ConfigModifier>],
    hard_policy: &HardPolicy,
    ordering: &[usize],
    errors: WatchErrors,
    stop_time: Option<SystemTime>,
    max_group_solve_time: Option<Duration>,
    abort: Stopper,
    #[cfg(feature = "count-states")] num_states: &mut usize,
) -> Option<(Vec<ConfigModifier>, Vec<usize>)>
where
    S: Strategy + GroupStrategy,
{
    // compute the remaining groups
    let mut remaining_groups =
        (0..groups.len()).filter(|x| !ordering.contains(x)).collect::<Vec<usize>>();

    // .--------.
    // | Step 1 | Reduction Phase
    // '--------'
    let (mut reduced_ordering, mut errors) = reduce_to_minimal_problem(
        net,
        groups,
        hard_policy,
        ordering,
        errors,
        #[cfg(feature = "count-states")]
        num_states,
    );

    loop {
        // .--------.
        // | Step 2 | Solving Phase
        // '--------'
        info!("Brute-Forcing a problem of length: {}", reduced_ordering.len());

        // get the time budget for the subsolver. It is the time left in the time budget,
        // but at most as much as a third of the total time budget.
        let solver_time_budget = stop_time.as_ref().map(|time| {
            let time_remaining =
                time.duration_since(SystemTime::now()).unwrap_or_else(|_| Duration::new(0, 0));
            if &time_remaining > max_group_solve_time.as_ref().unwrap() {
                *max_group_solve_time.as_ref().unwrap()
            } else {
                time_remaining
            }
        });

        match check_minimal_problem::<S>(
            net,
            groups,
            hard_policy,
            &reduced_ordering,
            solver_time_budget,
            abort.clone(),
            #[cfg(feature = "count-states")]
            num_states,
        ) {
            Ok(new_group) => {
                // Found a group which works!
                return Some((new_group, reduced_ordering));
            }
            Err(Error::Timeout) | Err(Error::Abort) => {
                return None;
            }
            Err(_) => {
                if !super::DO_EXPANSION {
                    return None;
                }
            }
        }

        // .--------.
        // | Step 3 | Expansion phase
        // '--------'
        match extend_minimal_problem(
            net,
            groups,
            hard_policy,
            &reduced_ordering,
            &mut remaining_groups,
            &errors,
            #[cfg(feature = "count-states")]
            num_states,
        ) {
            Ok((new_reduced_ordering, None)) => {
                // We could expand the minimal problem, and the `new_reduced_ordering` is
                // already a working solution!
                return Some((
                    finalize_ordering(groups, &new_reduced_ordering),
                    new_reduced_ordering,
                ));
            }
            Ok((new_reduced_ordering, Some(new_errors))) => {
                // We could expand the minimal problem. update the running net, the reduced
                // ordering and the remaining groups.
                reduced_ordering = new_reduced_ordering;
                errors = new_errors;
            }
            Err(_) => {
                // Unable to extend the running group!
                return None;
            }
        }
    }
}

/// This function checks the group ordering. If the ordering is correct, `Ok(())` is returned. If
/// the ordering is not correct, and it failed, it will return the final hard_policy in order to
/// be able to compute the watch errors later. If the hard policies is None, then there was a
/// convergence error.
pub(super) fn check_group_ordering(
    mut net: Network,
    groups: &[Vec<ConfigModifier>],
    hard_policy: &HardPolicy,
    ordering: &[usize],
    #[cfg(feature = "count-states")] num_states: &mut usize,
) -> Result<Network, (Network, usize, Option<HardPolicy>)> {
    // apply every step in sequence
    let mut hard_policy = hard_policy.clone();
    for (g_idx, group_idx) in ordering.iter().enumerate() {
        for modifier in groups[*group_idx].iter() {
            #[cfg(feature = "count-states")]
            {
                *num_states += 1;
            }
            // apply
            match net.apply_modifier(&modifier) {
                Ok(()) => {} // nothing to do
                Err(NetworkError::NoConvergence) | Err(NetworkError::ConvergenceLoop(_, _)) => {
                    return Err((net, g_idx, None));
                }
                Err(e) => panic!("Unrecoverable network error: {}", e),
            }
            // check
            let mut fw_state = net.get_forwarding_state();
            if let Err(e) = hard_policy.step(&mut net, &mut fw_state) {
                warn!("Error while checking hard policies: {}", e);
                panic!("Error while checking hard policies: {}", e);
            }
            if !hard_policy.check() {
                // policy failed!
                return Err((net, g_idx, Some(hard_policy)));
            }
        }
    }
    Ok(net)
}

/// This funciton executed the phase 3.1: reduction phase. It goes through all groups until the
/// problematic group, and checks if removing them changes anything of the final result. If nothing
/// has changed, we assume that this group is independent of the current problem, and we remove it
/// from the ordering. If not, the group is kept in the problem. This funciton returns the potential
/// minimal problem. If we notice that the `problem_group_idx` is no longer the same, we know that
/// there is a different, even smaller problem, and we remove the groups after the new problematic
/// group up to the previous `problem_group_idx` from the ordering.
///
/// # Arguments
/// - `net`: Reference to the `Network` in the initial state.
/// - `groups`: Reference to the already learned dependency groups
/// - `hard_policy`: Reference to the hard_policy
/// - `ordering`: Current ordering of group indices to reduce, up to and including the problem.
/// - `errors`: Set of the errors that were caused by applying the `ordering` on the `net`.
pub(super) fn reduce_to_minimal_problem(
    net: &Network,
    groups: &[Vec<ConfigModifier>],
    hard_policy: &HardPolicy,
    ordering: &[usize],
    errors: WatchErrors,
    #[cfg(feature = "count-states")] num_states: &mut usize,
) -> (Vec<usize>, WatchErrors) {
    let mut current_pos: usize = 0;
    let mut ordering = ordering.to_vec();

    debug!(
        "Problematic sequence:\n{}\nIssues with the found problem:\n    {}",
        fmt_group_ord(groups, &ordering, net),
        fmt_err(&errors, net),
    );

    // go through the entire ordering
    while current_pos + 1 < ordering.len() {
        let mut tmp_ordering = ordering.clone();
        let current_group = tmp_ordering.remove(current_pos);
        // check if the errors have changed
        match check_group_ordering(
            net.clone(),
            groups,
            hard_policy,
            &tmp_ordering,
            #[cfg(feature = "count-states")]
            num_states,
        ) {
            Ok(_) => {
                // the current group seems to solve the problem! It is definately part of the
                // current problem!
                debug!(
                    "Removing G{:02} seems to solve the problem, and thus, is part of it!",
                    current_group
                );
                current_pos += 1;
            }
            Err((_, new_idx, new_hard_policy)) if new_idx + 1 != tmp_ordering.len() => {
                // the ordering fails at a different group! Thus, Remove all groups after the
                // `problem_group_idx` from the ordering, and rerun this algorithm, in order to
                // search from the beginning for new errors.
                let new_err = generate_watch_errors(&new_hard_policy);
                debug!(
                    "Removing G{:02} seems to change the errors to a different, smaller problem:\n{}",
                    current_group,
                    fmt_err(&new_err, net),
                    );
                let (mut o, e) = reduce_to_minimal_problem(
                    net,
                    groups,
                    hard_policy,
                    &tmp_ordering[..new_idx + 1],
                    new_err,
                    #[cfg(feature = "count-states")]
                    num_states,
                );

                // if the last element of the resulting ordering is still the same, we need to
                // add back the current group to the ordering, which was removed for the
                // recurrsion. This is not necessary when the recurrsion again called itself,
                // reducing the problem even further. In this case, the last element is no
                // longer the same, and we don't need to add it back.
                if o.last() == Some(&tmp_ordering[new_idx]) {
                    debug!(
                        "Insert G{:02} in the beginning to complete the group (due to recurrsion).",
                        current_group
                    );
                    o.insert(0, current_group);
                }
                return (o, e);
            }
            Err((_, _, Some(hp)))
                if super::REDUCTION_CHECK_ERRORS && !hp.compare_watch_errors(&errors) =>
            {
                // the current group seems to change the problem! It is definately part of the
                // current problem!
                debug!("Removing G{:02} seems to change the errors!", current_group,);
                current_pos += 1;
            }
            Err((_, _, None))
                if super::REDUCTION_CHECK_ERRORS
                    && errors.1 != vec![Some(PolicyError::NoConvergence)] =>
            {
                // the current group seems to change the problem! It is definately part of the
                // current problem!
                debug!("Removing G{:02} seems to change the errors!", current_group,);
                current_pos += 1;
            }
            Err(_) if current_pos == 0 => {
                // the current group does not chnage the problem. Also, the current group is at
                // the first position in the ordering. Thus, we don't need to check if inserting
                // the group at the beginning changes anything. This modifier is not part of the
                // problem (probably)
                // TODO proof this!
                debug!("G{:02} seems to be idependent", current_group);
                // remember that `tmp_ordering` is the ordering where the current group is
                // already removed.
                ordering = tmp_ordering;
            }
            Err(_) => {
                // The current group does not change if it is removed, but we don't yet know if
                // it would change the problem if we insert it at the beginning. Do this now:
                tmp_ordering.insert(0, current_group);
                match check_group_ordering(
                    net.clone(),
                    groups,
                    hard_policy,
                    &tmp_ordering,
                    #[cfg(feature = "count-states")]
                    num_states,
                ) {
                    Ok(_) => {
                        // the group solves the problem when moved to the beginning!
                        debug!(
                            "G{:02} seems to solve the problem when moved to the beginning, and thus, is part of the problem!",
                            current_group
                            );
                        current_pos += 1;
                    }
                    Err((_, new_idx, new_hard_policy)) if new_idx + 1 != tmp_ordering.len() => {
                        // the ordering fails at a different group! Thus, Remove all groups
                        // after the `problem_group_idx` from the ordering, and rerun this
                        // algorithm, in order to search from the beginning for new errors.
                        let new_err = generate_watch_errors(&new_hard_policy);
                        debug!(
                            "G{:02} seems to change the errors to a different, smaller problem, when moved to the beginning:\n{}",
                            current_group,
                            fmt_err(&new_err, net),
                        );
                        return reduce_to_minimal_problem(
                            net,
                            groups,
                            hard_policy,
                            &tmp_ordering[..new_idx + 2],
                            new_err,
                            #[cfg(feature = "count-states")]
                            num_states,
                        );
                    }
                    Err((_, _, Some(hp)))
                        if super::REDUCTION_CHECK_ERRORS && !hp.compare_watch_errors(&errors) =>
                    {
                        // the current group seems to change the problem! It is definately part
                        // of the current problem!
                        debug!(
                            "G{:02} seems to change the errors if moved to the beginning!",
                            current_group,
                        );
                        current_pos += 1;
                    }
                    Err((_, _, None))
                        if super::REDUCTION_CHECK_ERRORS
                            && errors.1 != vec![Some(PolicyError::NoConvergence)] =>
                    {
                        // the current group seems to change the problem! It is definately part
                        // of the current problem!
                        debug!(
                            "G{:02} seems to change the errors if moved to the beginning!",
                            current_group,
                        );
                        current_pos += 1;
                    }
                    Err(_) => {
                        // the current group probably does not change the problem, we have tried
                        // it without the current group, and with the current group at the
                        // beginning.
                        // TODO proof this
                        debug!("G{:02} seems to be idependent", current_group);
                        ordering.remove(current_pos);
                    }
                }
            }
        }
    }

    debug!("Reduced the problem to:\n{}", fmt_group_ord(groups, &ordering, net),);

    (ordering, errors)
}

/// This function executes the phase 3.3: expansion phase. It tries to apply any single of the
/// remaining groups to the network, and checks if any of the errors change. If they have
/// changed, return the new ordering of the problem which changed the errors, and the resulting
/// errors. The errors is returned in an option. If this option is None, then no errors were
/// found, and the resulting ordering of the group already works!
///
/// This step may also make the problem smaller, if the error is now found at a different
/// position. But it will never split an already generated group in two.
///
/// In the new behavior, we move every remaining group to every possible position. If it changes the
/// outcome at any of the given positions, we know that it is dependent. If the problematic modifier
/// changes, then we reduce the problem by calling `reduce_minimal_problem` again.
///
/// The behavior of this function is different depending on `super::EXPANSION_CHECK_ERROR`. If it is
/// set to `true`, then the problem is allowed to be expanded if the problem statement changed. If
/// it is set to `false`, then the problem can only be expanded if it solves the problem.
///
/// # Arguments
/// - `net`: Reference to the `Network` in the initial state.
/// - `groups`: Reference to the already learned dependency groups
/// - `hard_policy`: Reference to the hard_policy
/// - `ordering`: Current ordering, which is not yet solvable. This vector does only contain the
///   dependency groups necessary, and not the ones already removed by the reduction phase, or those
///   not yet added by the expansion phase.
/// - `remaining_groups`: Mutable reference to the remaining groups. This vector will be changed by
///   this function, by removing the groups, which this funciton tries to add.
/// - `errors`: Set of the errors that were caused by applying the `ordering` on the `net`.
pub(super) fn extend_minimal_problem(
    net: &Network,
    groups: &[Vec<ConfigModifier>],
    hard_policy: &HardPolicy,
    ordering: &[usize],
    remaining_groups: &mut Vec<usize>,
    errors: &WatchErrors,
    #[cfg(feature = "count-states")] num_states: &mut usize,
) -> Result<(Vec<usize>, Option<WatchErrors>), ()> {
    // try all groups in remaining_groups
    let mut current_ordering = ordering.to_owned();
    for (i, probe_group) in remaining_groups.clone().into_iter().enumerate() {
        // try every possible position
        for probe_pos in 0..ordering.len() {
            // insert the group into the position
            current_ordering.insert(probe_pos, probe_group);
            // check if it works now
            match check_group_ordering(
                net.clone(),
                groups,
                hard_policy,
                &current_ordering,
                #[cfg(feature = "count-states")]
                num_states,
            ) {
                Ok(_) => {
                    // since this combination works, we can say that something has chaged, and
                    // return an OK.
                    debug!("Problem is solvable! Extending the problem with G{:02}.", probe_group);
                    remaining_groups.remove(i);
                    return Ok((current_ordering, None));
                }
                Err((_, new_pos, new_hard_policy)) if new_pos != current_ordering.len() - 1 => {
                    // new modifier changes the position! call reduce_problem_ordering!
                    let new_errors = generate_watch_errors(&new_hard_policy);
                    debug!(
                        "Problem is different, and smaller! New problem is:\n{}",
                        fmt_group_ord(groups, &current_ordering[..new_pos + 1], net),
                    );
                    // remove from remaining groups
                    remaining_groups.remove(i);
                    // call the reduction process
                    let (final_order, new_errors) = reduce_to_minimal_problem(
                        net,
                        groups,
                        hard_policy,
                        &current_ordering[..new_pos + 1],
                        new_errors,
                        #[cfg(feature = "count-states")]
                        num_states,
                    );
                    // we don't allow the extension to result in a single group (i.e., modifier,
                    // since every group is solvable by its own)
                    if final_order.len() == 1 {
                        info!("Cancel extend phase, since the extended problem has only one modifier.");
                        return Err(());
                    }
                    return Ok((final_order, Some(new_errors)));
                }
                Err((_, _, Some(hp)))
                    if super::EXPANSION_CHECK_ERRORS && !hp.compare_watch_errors(errors) =>
                {
                    // seems like the errors are different! we found some group that will
                    // further influence the current problem!
                    let new_errors = hp.get_watch_errors();
                    debug!(
                        "Problem is different! Extending the problem with G{:02}. New errors:\n    {}",
                        probe_group,
                        fmt_err(&new_errors, net),
                    );
                    // remove from remaining groups
                    remaining_groups.remove(i);
                    return Ok((current_ordering, Some(new_errors)));
                }
                Err((_, _, None))
                    if super::EXPANSION_CHECK_ERRORS
                        && errors.1 == vec![Some(PolicyError::NoConvergence)] =>
                {
                    // seems like the errors are different! we found some group that will
                    // further influence the current problem!
                    let new_errors = generate_watch_errors(&None);
                    debug!(
                        "Problem is different! Extending the problem with G{:02}. New errors:\n    {}",
                        probe_group,
                        fmt_err(&new_errors, net),
                    );
                    // remove from remaining groups
                    remaining_groups.remove(i);
                    return Ok((current_ordering, Some(new_errors)));
                }
                Err((_, _, _)) => {} // do nothing if the errors are actually equal.
            }

            // remove the item from the group, since it does not change the problem
            current_ordering.remove(probe_pos);
        }
    }

    info!("Could not extend the current problem!");

    Err(())
}

/// This function executes phase 3.2: Verification phase. It checks if the minimal problem is
/// actually solvable. For this, we use the provided strategy (which must be exhaustive, check
/// the type bounds). If it works, the function returns the working modifier ordering.
pub(super) fn check_minimal_problem<S: Strategy + GroupStrategy>(
    net: &Network,
    groups: &[Vec<ConfigModifier>],
    hard_policy: &HardPolicy,
    minimal_problem_ordering: &[usize],
    time_budget: Option<std::time::Duration>,
    abort: Stopper,
    #[cfg(feature = "count-states")] num_states: &mut usize,
) -> Result<Vec<ConfigModifier>, Error> {
    let mut hard_policy = hard_policy.clone();
    hard_policy.reset();
    let prep_groups: Vec<Vec<ConfigModifier>> =
        minimal_problem_ordering.iter().map(|i| groups.get(*i).unwrap().clone()).collect();
    let mut child = S::from_groups(net.clone(), prep_groups, hard_policy, time_budget)?;
    let child_result = child.work(abort);
    #[cfg(feature = "count-states")]
    {
        *num_states += child.num_states();
    }
    match child_result {
        Ok(group_ordering) => {
            // this group seems to work fine. Remove the existing groups from the
            // groups list, and add this new one at the end
            info!(
                "Problem was reduced to a minimal form, which is solvable!\n{}",
                group_ordering
                    .iter()
                    .map(|m| printer::config_modifier(&net, m).unwrap())
                    .collect::<Vec<String>>()
                    .join("\n")
            );
            Ok(group_ordering)
        }
        Err(Error::NoSafeOrdering) => {
            // Seems like this is not a minimal problem, because there exists no solution!
            debug!(
                "Current minimal problem is not solvable!\n{}",
                fmt_group_ord(groups, minimal_problem_ordering, net),
            );
            Err(Error::NoSafeOrdering)
        }
        Err(Error::Timeout) => {
            debug!(
                "Time Budget did not suffice to solve the problem!\n{}",
                fmt_group_ord(groups, minimal_problem_ordering, net),
            );
            Err(Error::Timeout)
        }
        Err(Error::Abort) => {
            info!("Operation was aborted!");
            Err(Error::Abort)
        }
        Err(Error::NetworkError(NetworkError::NoConvergence))
        | Err(Error::NetworkError(NetworkError::ConvergenceLoop(_, _))) => {
            error!("The GroupStrategy returned with a convergence error!");
            Err(Error::NoSafeOrdering)
        }
        Err(e) => panic!("Unexpected error returned: {}", e),
    }
}

/// This function adds the found ordering to the groups. The `groups` parameter is the vector
/// containing all previously found dependency groups. The `sub_groups` parameter is a list of group
/// indices, which will be removed from `groups`. The `ordering` is the ordering of modifiers in the
/// new group, which will be added as a new group. If `ordering` is None, then we build the ordering
/// from the groups, by just flattening the groups to a single vector.
pub(super) fn add_minimal_ordering_as_new_gorup(
    groups: &mut Vec<Vec<ConfigModifier>>,
    mut sub_groups: Vec<usize>,
    ordering: Option<Vec<ConfigModifier>>,
) {
    // prepare the ordering of the modifiers of the new group
    let ordering = ordering.unwrap_or_else(|| finalize_ordering(&groups, &sub_groups));

    sub_groups.sort_unstable();
    for group_to_remove in sub_groups.iter().rev() {
        groups.remove(*group_to_remove);
    }
    // now, insert the new group with the working ordering
    groups.push(ordering);
}

/// This function finalizes an ordering of a group, which passed the check, and returns a list of
/// all modifiers as their own, without the group information
pub(super) fn finalize_ordering(
    groups: &[Vec<ConfigModifier>],
    ordering: &[usize],
) -> Vec<ConfigModifier> {
    ordering.iter().map(|g| groups[*g].iter()).flatten().cloned().collect()
}

/// Format the group ordering into a nice multiline string
pub(super) fn fmt_group_ord(
    groups: &[Vec<ConfigModifier>],
    ordering: &[usize],
    net: &Network,
) -> String {
    ordering
        .iter()
        .map(|g| groups[*g].iter())
        .enumerate()
        .map(|(i, g)| {
            format!(
                "G{:03} {}",
                i,
                g.map(|m| printer::config_modifier(net, m).unwrap())
                    .collect::<Vec<String>>()
                    .join("\n     "),
            )
        })
        .collect::<Vec<String>>()
        .join("\n")
}

pub(super) fn fmt_err(errors: &WatchErrors, net: &Network) -> String {
    errors
        .1
        .iter()
        .filter_map(|e| e.as_ref().map(|e| e.repr_with_name(net)))
        .collect::<Vec<String>>()
        .join("\n")
}

fn generate_watch_errors(hard_policy: &Option<HardPolicy>) -> WatchErrors {
    match hard_policy {
        Some(hard_policy) => hard_policy.get_watch_errors(),
        None => (Vec::new(), vec![Some(PolicyError::NoConvergence)]),
    }
}
