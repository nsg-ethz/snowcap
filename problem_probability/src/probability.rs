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

//! Computes the probability

use rand::prelude::*;
use snowcap::netsim::{config::ConfigModifier, Network};
use snowcap::Stopper;
use std::error::Error;

use crate::utils::*;
use console::{style, Term};
use indicatif::ProgressBar;
use num_cpus;

use std::sync::mpsc::{channel, Sender};
use std::thread::{spawn, JoinHandle};

pub fn run(
    num_iter: usize,
    num_networks: usize,
    num_threads: Option<usize>,
    c: TopoConfig,
    only_statistics: bool,
    output_file: Option<String>,
) -> Result<(), Box<dyn Error>> {
    //pretty_env_logger::init();
    let mut results = if num_networks == 1 {
        vec![match single_run(num_iter, num_threads, &c, 0) {
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
        }]
    } else {
        multiple_runs(num_iter, num_networks, num_threads, c)?
    };

    if let Some(filename) = output_file {
        if only_statistics {
            for d in results.iter_mut() {
                d.random_permutations.total_severity.values = Vec::new();
                d.random_permutations.per_step_severity.values = Vec::new();
                d.random_router_order.total_severity.values = Vec::new();
                d.random_router_order.per_step_severity.values = Vec::new();
                d.insert_before_order.total_severity.values = Vec::new();
                d.insert_before_order.per_step_severity.values = Vec::new();
            }
        }
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
) -> Result<Vec<ProblemSeverityResult>, Box<dyn Error>> {
    let mut result = Vec::with_capacity(num_networks);
    let mut num_retry = 0;
    let mut i = 0;
    let term = Term::stdout();
    while i < num_networks {
        result.push(match single_run(num_iter, num_threads, &c, i) {
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
        });
        c.seed += 1;
        i += 1;
    }
    Ok(result)
}

fn single_run(
    num_iter: usize,
    num_threads: Option<usize>,
    c: &TopoConfig,
    run_id: usize,
) -> Result<ProblemSeverityResult, Box<dyn Error>> {
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

    // get the network
    let (net, config_b) = get_net_config(c)?;

    term.clear_last_lines(1)?;
    term.write_line(&format!(
        "{} Simulating random permutations..",
        style("[2/4]").bright().black().bold()
    ))?;

    let bar = ProgressBar::new(num_iter as u64);

    // prepare constraints and modifiers
    let modifiers = net.current_config().get_diff(&config_b).modifiers;

    // initialize counter
    let mut permut_num_failed: usize = 0;
    let mut permut_num_success: usize = 0;
    let mut permut_tot_magnitudes: Vec<f64> = Vec::with_capacity(num_iter);
    let mut permut_step_magnitudes: Vec<f64> = Vec::with_capacity(num_iter * modifiers.len());
    bar.tick();
    let (sender, receiver) = channel::<Option<(f64, Vec<f64>)>>();
    let abort = Stopper::new();
    let num_threads = num_threads.unwrap_or_else(|| num_cpus::get());

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
            Some((tot, step)) => {
                // got a new result
                bar.inc(1);
                permut_num_failed += 1;
                permut_tot_magnitudes.push(tot);
                permut_step_magnitudes.extend(step.iter());
            }
            None => {
                // failed try
                bar.inc(1);
                permut_num_success += 1;
            }
        }
        if permut_num_success + permut_num_failed == num_iter {
            abort.send_stop();
            break;
        }
    }

    bar.finish();
    term.clear_last_lines(2)?;
    term.write_line(&format!(
        "{} Simulating random router order..",
        style("[3/4]").bright().black().bold()
    ))?;

    let bar = ProgressBar::new(num_iter as u64);

    // initialize counter
    let mut router_num_failed: usize = 0;
    let mut router_num_success: usize = 0;
    let mut router_tot_magnitudes: Vec<f64> = Vec::with_capacity(num_iter);
    let mut router_step_magnitudes: Vec<f64> = Vec::with_capacity(num_iter * modifiers.len());
    let (sender, receiver) = channel::<Option<(f64, Vec<f64>)>>();
    let abort = Stopper::new();

    let _workers: Vec<JoinHandle<()>> = (0..num_threads)
        .map(|_| {
            let tx = sender.clone();
            let n = net.clone();
            let m = modifiers.clone();
            let kill = abort.clone();
            spawn(|| router_based_random_permutations_parallel(n, m, tx, kill))
        })
        .collect();
    loop {
        match receiver.recv().unwrap() {
            Some((tot, step)) => {
                // got a new result
                bar.inc(1);
                router_num_failed += 1;
                router_tot_magnitudes.push(tot);
                router_step_magnitudes.extend(step.iter());
            }
            None => {
                // failed try
                bar.inc(1);
                router_num_success += 1;
            }
        }
        if router_num_success + router_num_failed == num_iter {
            abort.send_stop();
            break;
        }
    }

    bar.finish();
    term.clear_last_lines(2)?;
    term.write_line(&format!(
        "{} Simulating insert-before-remove..",
        style("[4/4]").bright().black().bold()
    ))?;

    let bar = ProgressBar::new(num_iter as u64);

    // Insert Before Remove
    let mut ibr_num_failed: usize = 0;
    let mut ibr_num_success: usize = 0;
    let mut ibr_tot_magnitudes: Vec<f64> = Vec::with_capacity(num_iter);
    let mut ibr_step_magnitudes: Vec<f64> = Vec::with_capacity(num_iter * modifiers.len());
    let (sender, receiver) = channel::<Option<(f64, Vec<f64>)>>();
    let abort = Stopper::new();

    let _workers: Vec<JoinHandle<()>> = (0..num_threads)
        .map(|_| {
            let tx = sender.clone();
            let n = net.clone();
            let m = modifiers.clone();
            let kill = abort.clone();
            spawn(|| ibr_based_random_permutations_parallel(n, m, tx, kill))
        })
        .collect();
    loop {
        match receiver.recv().unwrap() {
            Some((tot, step)) => {
                // got a new result
                bar.inc(1);
                ibr_num_failed += 1;
                ibr_tot_magnitudes.push(tot);
                ibr_step_magnitudes.extend(step.iter());
            }
            None => {
                // failed try
                bar.inc(1);
                ibr_num_success += 1;
            }
        }
        if ibr_num_success + ibr_num_failed == num_iter {
            abort.send_stop();
            break;
        }
    }

    bar.finish();
    term.clear_last_lines(3)?;

    // build the result
    let result = ProblemSeverityResult {
        scenario: c.clone(),
        random_permutations: StrategySeverity {
            result: StrategyResult::new(permut_num_success, permut_num_failed),
            total_severity: StatisticsResult::new(permut_tot_magnitudes),
            per_step_severity: StatisticsResult::new(permut_step_magnitudes),
        },
        random_router_order: StrategySeverity {
            result: StrategyResult::new(router_num_success, router_num_failed),
            total_severity: StatisticsResult::new(router_tot_magnitudes),
            per_step_severity: StatisticsResult::new(router_step_magnitudes),
        },
        insert_before_order: StrategySeverity {
            result: StrategyResult::new(ibr_num_success, ibr_num_failed),
            total_severity: StatisticsResult::new(ibr_tot_magnitudes),
            per_step_severity: StatisticsResult::new(ibr_step_magnitudes),
        },
    };

    term.write_line(&format!(
        "{} {} ({}) {} {}",
        style("Topology").bold().blue(),
        c.file.split("/").last().unwrap_or_default(),
        run_id,
        style("Done").green().bold(),
        style(&format!(
            "[permutations: {:.1}%, router-order: {:.1}%, insert-before-remove: {:.1}%]",
            result.random_permutations.result.success_rate * 100.0,
            result.random_router_order.result.success_rate * 100.0,
            result.insert_before_order.result.success_rate * 100.0,
        ))
        .bright()
        .black()
        .bold()
    ))?;

    Ok(result)
}

fn random_permutations_parallel(
    net: Network,
    mut modifiers: Vec<ConfigModifier>,
    sender: Sender<Option<(f64, Vec<f64>)>>,
    mut kill: Stopper,
) {
    loop {
        if kill.try_is_stop().unwrap_or(false) {
            break;
        }

        match do_random_reconfiguration_with_fail_magnitude(
            net.clone(),
            &mut modifiers,
            &mut thread_rng(),
            false,
        ) {
            Ok(_) => match sender.send(None) {
                Ok(_) => {}
                Err(_) => break,
            },
            Err((tot, step)) => match sender.send(Some((tot, step))) {
                Ok(_) => {}
                Err(_) => break,
            },
        }
    }
}

fn router_based_random_permutations_parallel(
    net: Network,
    mut modifiers: Vec<ConfigModifier>,
    sender: Sender<Option<(f64, Vec<f64>)>>,
    mut kill: Stopper,
) {
    loop {
        if kill.try_is_stop().unwrap_or(false) {
            break;
        }

        match do_router_based_random_reconfiguration_with_fail_magnitude(
            net.clone(),
            &mut modifiers,
            &mut thread_rng(),
        ) {
            Ok(_) => match sender.send(None) {
                Ok(_) => {}
                Err(_) => break,
            },
            Err((tot, step)) => match sender.send(Some((tot, step))) {
                Ok(_) => {}
                Err(_) => break,
            },
        }
    }
}

fn ibr_based_random_permutations_parallel(
    net: Network,
    mut modifiers: Vec<ConfigModifier>,
    sender: Sender<Option<(f64, Vec<f64>)>>,
    mut kill: Stopper,
) {
    loop {
        if kill.try_is_stop().unwrap_or(false) {
            break;
        }

        match do_random_reconfiguration_with_fail_magnitude(
            net.clone(),
            &mut modifiers,
            &mut thread_rng(),
            true,
        ) {
            Ok(_) => match sender.send(None) {
                Ok(_) => {}
                Err(_) => break,
            },
            Err((tot, step)) => match sender.send(Some((tot, step))) {
                Ok(_) => {}
                Err(_) => break,
            },
        }
    }
}
