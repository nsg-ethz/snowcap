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
use snowcap::netsim::config::{Config, ConfigModifier};
use snowcap::netsim::Network;
use snowcap::optimizers::*;
use snowcap::soft_policies::*;
use snowcap::Stopper;

use num_cpus;
use rand::prelude::*;
use std::error::Error;
use std::time::Duration;

use std::sync::mpsc::{channel, Sender};
use std::thread::{spawn, JoinHandle};

use crate::utils::*;
use console::{style, Term};
use indicatif::ProgressBar;

pub fn run(
    num_iter: usize,
    num_networks: usize,
    num_threads: Option<usize>,
    c: TopoConfig,
    do_all: bool,
    only_statistics: bool,
    optimizer_fraction: usize,
    output_file: Option<String>,
) -> Result<(), Box<dyn Error>> {
    //pretty_env_logger::init();
    let mut results = if num_networks == 1 {
        vec![
            match single_run(num_iter, num_threads, &c, do_all, optimizer_fraction, 0) {
                Ok(r) => r,
                Err(_) => {
                    let term = Term::stdout();
                    term.clear_last_lines(2)?;
                    term.write_line(&format!(
                        "{} {} (0): {}",
                        style("Topology").bold().blue(),
                        c.file.split("/").last().unwrap_or_default(),
                        style("Checks failed!").red().bright()
                    ))?;
                    return Ok(());
                }
            },
        ]
    } else {
        multiple_runs(
            num_iter,
            num_networks,
            num_threads,
            c,
            do_all,
            optimizer_fraction,
        )?
    };

    if only_statistics {
        results.iter_mut().for_each(|r| {
            r.random_permutations.cost.values.clear();
            r.optimizer.cost.values.clear();
        });
    }

    if let Some(filename) = output_file {
        let result_str = serde_json::to_string_pretty(&results)?;
        std::fs::write(filename, result_str)?;
    }

    Ok(())
}

fn multiple_runs(
    num_iter: usize,
    num_networks: usize,
    num_threads: Option<usize>,
    mut c: TopoConfig,
    do_all: bool,
    optimizer_fraction: usize,
) -> Result<Vec<CostResult>, Box<dyn Error>> {
    let mut result = Vec::with_capacity(num_networks);
    let mut num_retry = 0;
    let mut i = 0;
    let term = Term::stdout();
    while i < num_networks {
        result.push(
            match single_run(num_iter, num_threads, &c, do_all, optimizer_fraction, i) {
                Ok(r) => r,
                Err(e) => {
                    num_retry += 1;
                    c.seed += 1;
                    if num_retry > 20 {
                        term.clear_last_lines(1)?;
                        term.write_line(&format!(
                            "{} {} ({}): {}",
                            style("Topology").bold().blue(),
                            c.file.split("/").last().unwrap_or_default(),
                            i,
                            style("Checks failed!").red().bright()
                        ))?;
                        term.write_line(&format!(
                            "{} procedure failed more than 20 times!",
                            style("ERROR").bold().bright()
                        ))?;
                        return Err(e);
                    } else {
                        term.clear_last_lines(2)?;
                        term.write_line(&format!(
                            "{} {} ({}): {} Trying again with a different seed!",
                            style("Topology").bold().blue(),
                            c.file.split("/").last().unwrap_or_default(),
                            i,
                            style("Checks failed!").red().bright()
                        ))?;
                    }
                    continue;
                }
            },
        );
        c.seed += 1;
        i += 1;
    }
    Ok(result)
}

fn single_run(
    num_iter: usize,
    num_threads: Option<usize>,
    c: &TopoConfig,
    do_all: bool,
    optimizer_fraction: usize,
    run_id: usize,
) -> Result<CostResult, Box<dyn Error>> {
    let num_threads = num_threads.unwrap_or_else(|| num_cpus::get());

    let term = Term::stdout();
    term.write_line(&format!(
        "{} {} ({})...",
        style("Topology").bold().blue(),
        c.file.split("/").last().unwrap_or_default(),
        run_id
    ))?;

    term.write_line(&format!(
        "{} Performing checks...",
        style("[1/4]").bright().black().bold()
    ))?;

    let (net, config_b) = get_net_config(c)?;

    // check if the number of routers and the number of prefixes is not 0.
    if net.get_routers().len() == 0 || net.get_known_prefixes().len() == 0 {
        println!("Number of routers or number of prefixes is 0! Abort!");
        return Err("Number of routers or number of prefixes is 0!".into());
    }

    let num_commands = net.current_config().get_diff(&config_b).modifiers.len();
    term.clear_last_lines(2)?;
    term.write_line(&format!(
        "{} {} ({}) {}...",
        style("Topology").bold().blue(),
        c.file.split("/").last().unwrap_or_default(),
        run_id,
        style(format!("[#c: {}]", num_commands))
            .bright()
            .black()
            .bold()
    ))?;

    term.write_line(&format!(
        "{} Compute the theoretical ideal cost...",
        style("[2/4]").bright().black().bold()
    ))?;

    let mut soft_policy = MinimizeTrafficShift::new(&mut net.get_forwarding_state(), &net);
    let mut net_b = net.clone();
    net_b.set_config(&config_b)?;
    soft_policy.update(&mut net_b.get_forwarding_state(), &net_b);
    let ideal_cost = soft_policy.cost();

    // #########################
    // # compute the Optimizer #
    // #########################

    term.clear_last_lines(1)?;
    term.write_line(&format!(
        "{} Compute the cost of the optimizer...",
        style("[3/4]").bright().black().bold()
    ))?;

    let n_optim = if do_all {
        num_iter / optimizer_fraction
    } else {
        1
    };
    let mut optim_values: Vec<f64> = Vec::with_capacity(n_optim);
    let mut optim_failed: usize = 0;
    let mut optim_success: usize = 0;
    let bar = ProgressBar::new(n_optim as u64);
    bar.tick();
    let (sender, receiver) = channel::<Option<f64>>();
    let abort = Stopper::new();
    let _workers: Vec<JoinHandle<()>> = (0..num_threads)
        .map(|_| {
            let n = net.clone();
            let c = config_b.clone();
            let tx = sender.clone();
            let kill = abort.clone();
            spawn(|| optimizer_parallel(n, c, tx, kill))
        })
        .collect();
    // the remaining threads checks the progress and prints the result
    loop {
        match receiver.recv().unwrap() {
            Some(cost) => {
                bar.inc(1);
                optim_success += 1;
                optim_values.push(cost);
            }
            None => optim_failed += 1,
        }
        if optim_success == n_optim {
            abort.send_stop();
            bar.finish();
            break;
        }
    }
    let optim_result = OptimizerResult::new(optim_success, optim_failed, optim_values);

    // ###################################
    // # compute the random permutations #
    // ###################################

    term.clear_last_lines(2)?;
    term.write_line(&format!(
        "{} Compute the cost of random permutations...",
        style("[4/4]").bright().black().bold()
    ))?;
    // prepare constraints and modifiers
    let modifiers = net.current_config().get_diff(&config_b).modifiers;

    // initialize counter
    let mut random_failed: usize = 0;
    let mut random_success: usize = 0;
    let mut random_values: Vec<f64> = Vec::with_capacity(num_iter);
    let bar = ProgressBar::new(num_iter as u64);
    bar.tick();
    let (sender, receiver) = channel::<Option<f64>>();
    let abort = Stopper::new();

    // spawn all workers
    let _workers: Vec<JoinHandle<()>> = (0..num_threads)
        .map(|_| {
            let tx = sender.clone();
            let n = net.clone();
            let m = modifiers.clone();
            let kill = abort.clone();
            spawn(|| random_permutations_parallel(n, m, tx, kill))
        })
        .collect();
    loop {
        match receiver.recv().unwrap() {
            Some(cost) => {
                // got a new result
                bar.inc(1);
                random_success += 1;
                random_values.push(cost);
            }
            None => {
                // failed try
                random_failed += 1;
            }
        }
        if random_success == num_iter {
            abort.send_stop();
            bar.finish();
            break;
        }
    }
    let random_result = OptimizerResult::new(random_success, random_failed, random_values);

    term.clear_last_lines(3)?;
    term.write_line(&format!(
        "{} {} ({}) {} {}",
        style("Topology").bold().blue(),
        c.file.split("/").last().unwrap_or_default(),
        run_id,
        style("Done").green().bold(),
        style(format!(
            "[#c: {}, ideal cost: {:.4}, snowcap: {:.4}, random permutations: {:.4}]",
            num_commands, ideal_cost, optim_result.cost.mean, random_result.cost.mean,
        ))
        .bright()
        .black()
        .bold()
    ))?;

    Ok(CostResult {
        scenario: c.clone(),
        ideal_cost,
        random_permutations: random_result,
        optimizer: optim_result,
    })
}

fn random_permutations_parallel(
    net: Network,
    mut modifiers: Vec<ConfigModifier>,
    sender: Sender<Option<f64>>,
    mut kill: Stopper,
) {
    loop {
        if kill.try_is_stop().unwrap_or(false) {
            break;
        }

        match do_random_reconfiguration(net.clone(), &mut modifiers, &mut thread_rng()) {
            Ok(_) => {
                let cost = compute_cost::<MinimizeTrafficShift>(&net, &modifiers).unwrap();
                match sender.send(Some(cost)) {
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            Err(_) => match sender.send(None) {
                Ok(_) => {}
                Err(_) => break,
            },
        }
    }
}

fn optimizer_parallel(
    net: Network,
    config: Config,
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

        match match OptimizerTRTA::synthesize(
            net.clone(),
            config.clone(),
            hard_policy.clone(),
            soft_policy.clone(),
            Some(Duration::from_secs(300)),
            kill.clone(),
        ) {
            Ok((_, cost)) => sender.send(Some(cost)),
            Err(_) => sender.send(None),
        } {
            Ok(_) => {}
            Err(_) => break,
        }
    }
}
