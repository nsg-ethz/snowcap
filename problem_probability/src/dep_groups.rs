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

//! Computes the cost

use snowcap::hard_policies::HardPolicy;
use snowcap::modifier_ordering::*;
use snowcap::netsim::config::Config;
use snowcap::netsim::Network;
use snowcap::optimizers::*;
use snowcap::permutators::*;
use snowcap::soft_policies::*;
use snowcap::strategies::*;
use snowcap::Stopper;

use num_cpus;
use statistical as stats;
use std::error::Error;
use std::time::Duration;

use std::sync::mpsc::{channel, Sender};
use std::thread::{spawn, JoinHandle};

use crate::utils::*;
use indicatif::ProgressBar;

const N_BINS: usize = 10;

pub fn run(num_iter: usize, num_networks: usize, c: TopoConfig) -> Result<(), Box<dyn Error>> {
    if num_networks == 1 {
        single_run(num_iter, &c)
    } else {
        multiple_runs(num_iter, num_networks, c)
    }
}

fn multiple_runs(
    num_iter: usize,
    num_networks: usize,
    mut c: TopoConfig,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..num_networks {
        println!("\n");
        single_run(num_iter, &c)?;
        c.seed += 1;
    }
    Ok(())
}

fn single_run(num_iter: usize, c: &TopoConfig) -> Result<(), Box<dyn Error>> {
    let (net, config_b) = get_net_config(c)?;

    println!(
        "Number of modifiers: {}",
        net.current_config().get_diff(&config_b).modifiers.len()
    );

    println!("All checks complete!\n");

    println!("Compute theoretical ideal cost...");
    let mut soft_policy = MinimizeTrafficShift::new(&mut net.get_forwarding_state(), &net);
    let mut net_b = net.clone();
    net_b.set_config(&config_b)?;
    soft_policy.update(&mut net_b.get_forwarding_state(), &net_b);
    let ideal_cost = soft_policy.cost();

    // compute the DepGorups optimizer

    println!("Approximate distribution of results...");

    // initialize counter
    let mut num_failed: usize = 0;
    let mut num_success: usize = 0;
    let mut result: Vec<f64> = Vec::with_capacity(num_iter);

    // start the process
    let bar = ProgressBar::new(num_iter as u64);
    let (sender, receiver) = channel::<Option<f64>>();
    let abort = Stopper::new();
    let num_threads = num_cpus::get();

    // spawn all workers
    let _workers: Vec<JoinHandle<()>> = (0..num_threads)
        .map(|_| {
            let tx = sender.clone();
            let n = net.clone();
            let c = config_b.clone();
            let kill = abort.clone();
            spawn(|| compute_parallel(n, c, tx, kill))
        })
        .collect();

    // the remaining threads checks the progress and prints the result
    loop {
        match receiver.recv().unwrap() {
            Some(cost) => {
                bar.inc(1);
                num_success += 1;
                result.push(cost);
            }
            None => {
                bar.inc(1);
                num_failed += 1;
            }
        }
        if num_success + num_failed >= num_iter {
            break;
        }
    }

    // kill all workers
    abort.send_stop();
    bar.finish();

    result.sort_by(|a, b| a.partial_cmp(&b).unwrap());
    let min_cost = result.get(0).unwrap();
    let max_cost = result.last().unwrap();

    println!(
        "\nReport:\n{}/{} iterations failed ({}%)",
        num_failed,
        num_failed + num_success,
        (100.0 * (num_failed as f64) / ((num_failed + num_success) as f64))
    );

    println!("theoretical: {:.4}", ideal_cost);
    println!("best-case:   {:.4}", min_cost);
    println!("worst-case:  {:.4}", max_cost);
    println!("average:     {:.4}", stats::mean(&result));
    println!("median:      {:.4}", stats::median(&result));
    println!("variance:    {:.4}", stats::variance(&result, None).sqrt());

    print_bins(&result, *min_cost, *max_cost, N_BINS);

    Ok(())
}

fn compute_parallel(
    net: Network,
    config_b: Config,
    sender: Sender<Option<f64>>,
    mut kill: Stopper,
) {
    let soft_policy = MinimizeTrafficShift::new(&mut net.get_forwarding_state(), &net);
    let hard_policy =
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter());
    loop {
        if kill.try_is_stop().unwrap_or(false) {
            break;
        }

        match DepGroupsOptimizer::<
            _,
            RandomTreePermutator<usize>,
            PushBackTreeStrategy<RandomOrdering>,
            TreeOptimizer<_>,
        >::synthesize(
            net.clone(),
            config_b.clone(),
            hard_policy.clone(),
            soft_policy.clone(),
            Some(Duration::from_secs(300)),
            kill.clone(),
        ) {
            Ok((_, cost)) => match sender.send(Some(cost)) {
                Ok(_) => {}
                Err(_) => break,
            },
            Err(_) => match sender.send(None) {
                Ok(_) => {}
                Err(_) => break,
            },
        }
    }
}
