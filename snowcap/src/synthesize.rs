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

//! # Snowcap
//! Wrapper function to synthesize configuration updates

use crate::hard_policies::HardPolicy;
use crate::netsim::config::{Config, ConfigModifier};
use crate::netsim::Network;
use crate::optimizers::{Optimizer, OptimizerTRTA};
use crate::soft_policies::SoftPolicy;
use crate::strategies::{Strategy, StrategyTRTA};
use crate::{Error, Stopper};

use log::*;
use std::thread;
use std::time::Duration;

/// # Synthesize Configuration Updates
///
/// This is the main function to interact with the system. It uses the
/// [`StrategyTRTA`](crate::strategies::StrategyTRTA).
///
/// ## Usage
///
/// ```
/// use snowcap::hard_policies::*;
/// use snowcap::synthesize;
/// use snowcap::Error;
/// use snowcap::netsim::Network;
/// use snowcap::netsim::config::Config;
/// # use snowcap::example_networks::*;
///
/// fn main() -> Result<(), Error> {
///     // prepare the network
///     // let net = ...
///     // let initial_config = ...
///     // let final_config = ...
/// # let net = SimpleNet::net(0);
/// # let initial_config = net.current_config().clone();
/// # let final_config = SimpleNet::final_config(&net, 0);
///
///     // prepare the policies
///     // let hard_policy = ...
/// # let hard_policy = HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter());
///
///     // synthesize the reconfiguration
///     let sequence = synthesize(net, initial_config, final_config, hard_policy, None)?;
///
///     Ok(())
/// }
/// ```
///
pub fn synthesize(
    mut net: Network,
    config_a: Config,
    config_b: Config,
    hard_policy: HardPolicy,
    time_limit: Option<Duration>,
) -> Result<Vec<ConfigModifier>, Error> {
    // setup the network and reset the undo tracker
    net.set_config(&config_a)?;
    net.clear_undo_stack();

    // compute the set of modifiers
    let patch = config_a.get_diff(&config_b);
    let modifiers: Vec<ConfigModifier> = patch.modifiers;

    info!("Solving the problem...");

    // generate PushBackTreeStrategy
    let mut strategy = StrategyTRTA::new(net, modifiers, hard_policy, time_limit)?;

    // try to solve the problem
    match strategy.work(Stopper::new()) {
        Ok(sequence) => {
            info!("Found a valid solution!");
            Ok(sequence)
        }
        Err(e) => {
            error!("Could not solve the problem: {}", e);
            Err(e)
        }
    }
}

/// # Synthesize Configuration Updates using multiple parallel threads
///
/// This funciton spawns `N` [`StrategyTRTA`](crate::strategies::StrategyTRTA) threads, that search
/// for a solution in parallel, using different random seeds.. The first solution found will be
/// used, and all other threads will be killed.
///
/// ## Usage
///
/// ```
/// use snowcap::hard_policies::*;
/// use snowcap::synthesize_parallel;
/// use snowcap::Error;
/// use snowcap::netsim::Network;
/// use snowcap::netsim::config::Config;
/// use std::time::Duration;
/// # use snowcap::example_networks::*;
///
/// fn main() -> Result<(), Error> {
///     // prepare the network
///     // let net = ...
///     // let initial_config = ...
///     // let final_config = ...
/// # let net = SimpleNet::net(0);
/// # let initial_config = net.current_config().clone();
/// # let final_config = SimpleNet::final_config(&net, 0);
///
///     // prepare the policies
///     // let hard_policy = ...
/// # let hard_policy = HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter());
///
///     // synthesize the reconfiguration
///     let sequence = synthesize_parallel(
///         net,
///         initial_config,
///         final_config,
///         hard_policy,
///         Duration::from_secs(60),
///         None
///     )?;
///
///     Ok(())
/// }
/// ```
pub fn synthesize_parallel(
    mut net: Network,
    config_a: Config,
    config_b: Config,
    hard_policy: HardPolicy,
    time_limit: Duration,
    n_threads: Option<usize>,
) -> Result<Vec<ConfigModifier>, Error> {
    // setup the network and reset the undo tracker
    net.set_config(&config_a)?;
    net.clear_undo_stack();

    // compute the set of modifiers
    let patch = config_a.get_diff(&config_b);
    let modifiers: Vec<ConfigModifier> = patch.modifiers;

    // create the atomic bool to communicate when a solution was found
    let abort = Stopper::new();

    let n_threads = n_threads.unwrap_or_else(num_cpus::get);
    info!("Spawning {} threads", n_threads);

    let handles = (0..n_threads)
        .map(|_| {
            let n = net.clone();
            let m = modifiers.clone();
            let p = hard_policy.clone();
            let a = abort.clone();
            thread::spawn(move || {
                let mut strategy = StrategyTRTA::new(n, m, p, Some(time_limit))?;
                let result = strategy.work(a.clone());
                if result.is_ok() {
                    a.send_stop();
                }
                result
            })
        })
        .collect::<Vec<_>>();

    let mut correct_result = None;
    let mut some_error = None;

    // wait until all threads are done
    for handle in handles {
        match handle.join().unwrap() {
            Ok(valid) => correct_result = Some(valid),
            Err(e) => {
                warn!("Thread had a problem solving the problem: {}", e);
                some_error = Some(e)
            }
        }
    }

    // try to solve the problem
    match (correct_result, some_error) {
        (Some(sequence), _) => {
            info!("Found a valid solution!");
            Ok(sequence)
        }
        (None, Some(e)) => {
            error!("Could not find any result: {}", e);
            Err(e)
        }
        _ => unreachable!(),
    }
}

/// # Synthesize Configuration Updates while optimizing soft policies
///
/// This is the main function to interact with the system. It uses the
/// [`OptimizerTRTA`](crate::optimizers::Optimizer).
///
/// ## Usage
///
/// ```
/// use snowcap::hard_policies::*;
/// use snowcap::soft_policies::*;
/// use snowcap::optimize;
/// use snowcap::Error;
/// use snowcap::netsim::Network;
/// use snowcap::netsim::config::Config;
/// # use snowcap::example_networks::*;
///
/// fn main() -> Result<(), Error> {
///     // prepare the network
///     // let net = ...
///     // let initial_config = ...
///     // let final_config = ...
/// # let net = SimpleNet::net(0);
/// # let initial_config = net.current_config().clone();
/// # let final_config = SimpleNet::final_config(&net, 0);
///
///     // prepare the policies
///     // let hard_policy = ... (e.g., HardPolicy::reachability(...))
///     // type SP = ... (e.g., MinimizeTrafficShift)
/// # let hard_policy = HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter());
/// # type SP = MinimizeTrafficShift;
///
///     // synthesize the reconfiguration
///     let (sequence, _) = optimize::<SP>(net, initial_config, final_config, hard_policy, None)?;
///
///     Ok(())
/// }
/// ```
///
pub fn optimize<SP: SoftPolicy + Clone>(
    mut net: Network,
    config_a: Config,
    config_b: Config,
    hard_policy: HardPolicy,
    time_limit: Option<Duration>,
) -> Result<(Vec<ConfigModifier>, f64), Error> {
    // setup the network and reset the undo tracker
    net.set_config(&config_a)?;
    net.clear_undo_stack();

    // setup soft policy
    let mut fw_state = net.get_forwarding_state();
    let soft_policy = SP::new(&mut fw_state, &net);

    // compute the set of modifiers
    let patch = config_a.get_diff(&config_b);
    let modifiers: Vec<ConfigModifier> = patch.modifiers;

    let mut optimizer =
        OptimizerTRTA::<SP>::new(net, modifiers, hard_policy, soft_policy, time_limit)?;

    info!("Solving the problem...");

    // try to solve the problem
    match optimizer.work(Stopper::new()) {
        Ok((sequence, cost)) => {
            info!("Found a valid solution!");
            Ok((sequence, cost))
        }
        Err(e) => {
            error!("Could not solve the problem: {}", e);
            Err(e)
        }
    }
}
