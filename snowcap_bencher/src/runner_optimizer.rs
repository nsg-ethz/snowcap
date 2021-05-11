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

//! Runner for the Benchmark, based on the provided configuration

use super::utils::*;
use super::{BencherArguments, BencherResult, Run};

use snowcap::{
    hard_policies::HardPolicy,
    netsim::{
        config::{Config, ConfigModifier},
        Network,
    },
    optimizers::*,
    soft_policies::{MinimizeTrafficShift, SoftPolicy},
    synthesize_parallel, Stopper,
};

use console::{style, Term};
use indicatif::ProgressBar;
use num_cpus;
use rand::prelude::*;

use std::error::Error;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{spawn, JoinHandle};
use std::time::{Duration, SystemTime};

/// Benches a scenario with the given configuration, producing a result, and generating the files
/// (if necessary). The Soft Policy is automatically chosen to be
/// [`MinimizeTrafficShift`](snowcap::soft_policies::SoftPolicy).
pub fn bench(
    net: Network,
    final_config: Config,
    hard_policy: HardPolicy,
    scenario: String,
    args: BencherArguments,
) -> Result<BencherResult, Box<dyn Error>> {
    // get the number of threads
    let num_threads = args.threads.unwrap_or_else(num_cpus::get);

    // generate a TERM for nicer outputs
    let term = Term::stdout();
    term.write_line(&format!(
        "{} {}...",
        style("Scenario:").bold().blue(),
        scenario,
    ))?;

    // generate the soft policy
    let mut fw_state = net.get_forwarding_state();
    let soft_policy = MinimizeTrafficShift::new(&mut fw_state, &net);

    term.write_line(&format!(
        "{} {}",
        style("[0/8]").bright().black(),
        "checking initial and final configuration..."
    ))?;
    // check the configuration
    let ideal_cost = match check_config::<MinimizeTrafficShift>(&net, &final_config, &hard_policy) {
        Some(c) => c,
        None => {
            term.clear_last_lines(2)?;
            term.write_line(&format!(
                "{} {}... {} {}",
                style("Scenario:").bold().blue(),
                scenario,
                style("Error").bold().red(),
                "Initial or final configuration is invalid!"
            ))?;
            return Ok(BencherResult {
                scenario,
                ideal_cost: f64::NAN,
                optimal_cost: None,
                optimal_cost_time: None,
                num_nodes: net.num_devices(),
                num_edges: net.links_symmetric().count(),
                num_commands: net.current_config().get_diff(&final_config).modifiers.len(),
                strategy_result: Vec::new(),
                tree_result: Vec::new(),
                random_result: Vec::new(),
                baseline_mil_result: Vec::new(),
                baseline_mif_result: Vec::new(),
            });
        }
    };

    // check that there exists a valid reconfiguration scenario
    term.clear_last_lines(1)?;
    term.write_line(&format!(
        "{} {}",
        style("[1/8]").bright().black(),
        "Checking if there exists a valid sequence"
    ))?;

    match synthesize_parallel(
        net.clone(),
        net.current_config().clone(),
        final_config.clone(),
        hard_policy.clone(),
        Duration::from_secs(args.max_time),
        None,
    ) {
        Ok(_) => {}
        Err(e) => {
            term.clear_last_lines(2)?;
            term.write_line(&format!(
                "{} {}... {} {}",
                style("Scenario:").bold().blue(),
                scenario,
                style("Error:").bold().red(),
                e
            ))?;
            return Ok(BencherResult {
                scenario,
                ideal_cost: f64::NAN,
                optimal_cost: None,
                optimal_cost_time: None,
                num_nodes: net.num_devices(),
                num_edges: net.links_symmetric().count(),
                num_commands: net.current_config().get_diff(&final_config).modifiers.len(),
                strategy_result: Vec::new(),
                tree_result: Vec::new(),
                random_result: Vec::new(),
                baseline_mil_result: Vec::new(),
                baseline_mif_result: Vec::new(),
            });
        }
    }

    // Performing the benchmark on our strategy
    term.clear_last_lines(1)?;
    term.write_line(&format!(
        "{} {}",
        style("[2/8]").bright().black(),
        "Benchmarking Optimizer..."
    ))?;

    let strategy_result = if args.main {
        worker_runner::<OptimizerTRTA<MinimizeTrafficShift>>(
            &net,
            &final_config,
            &hard_policy,
            &soft_policy,
            args.max_time,
            args.iterations,
            args.ignore_nan,
            num_threads,
        )
    } else {
        Vec::new()
    };

    // Performing the benchmark on our strategy
    term.clear_last_lines(1)?;
    term.write_line(&format!(
        "{} {}",
        style("[3/8]").bright().black(),
        "Benchmarking Tree Optimizer..."
    ))?;

    let tree_result = if args.tree {
        worker_runner::<TreeOptimizer<MinimizeTrafficShift>>(
            &net,
            &final_config,
            &hard_policy,
            &soft_policy,
            args.max_time,
            args.iterations,
            args.ignore_nan,
            num_threads,
        )
    } else {
        Vec::new()
    };

    // Performing the benchmark on the random baseline approach
    term.clear_last_lines(1)?;
    term.write_line(&format!(
        "{} {}",
        style("[4/8]").bright().black(),
        "Benchmarking random (baseline) approach..."
    ))?;

    let random_result = if args.random {
        worker_runner::<NaiveRandomOptimizer<MinimizeTrafficShift>>(
            &net,
            &final_config,
            &hard_policy,
            &soft_policy,
            args.max_time,
            args.iterations,
            args.ignore_nan,
            num_threads,
        )
    } else {
        Vec::new()
    };

    // Performing the benchmark on the random baseline approach
    term.clear_last_lines(1)?;
    term.write_line(&format!(
        "{} {}",
        style("[5/8]").bright().black(),
        "Benchmarking Most-Important-First (baseline) approach..."
    ))?;

    let baseline_mif_result = if args.mif {
        worker_runner::<NaiveMostImportantFirst<MinimizeTrafficShift>>(
            &net,
            &final_config,
            &hard_policy,
            &soft_policy,
            args.max_time,
            args.iterations,
            args.ignore_nan,
            num_threads,
        )
    } else {
        Vec::new()
    };

    // Performing the benchmark on the random baseline approach
    term.clear_last_lines(1)?;
    term.write_line(&format!(
        "{} {}",
        style("[6/8]").bright().black(),
        "Benchmarking Most-Important-Last (baseline) approach..."
    ))?;

    let baseline_mil_result = if args.mil {
        worker_runner::<NaiveMostImportantLast<MinimizeTrafficShift>>(
            &net,
            &final_config,
            &hard_policy,
            &soft_policy,
            args.max_time,
            args.iterations,
            args.ignore_nan,
            num_threads,
        )
    } else {
        Vec::new()
    };

    term.clear_last_lines(1)?;
    term.write_line(&format!(
        "{} {}",
        style("[7/8]").bright().black(),
        "Computing the global optimum... (this may take a while)"
    ))?;

    // compute the global optimum
    let (optimal_cost, optimal_cost_time) = if args.global_optimum {
        let start_time = SystemTime::now();
        match find_global_optimum(
            net.clone(),
            &final_config,
            hard_policy.clone(),
            soft_policy.clone(),
            Duration::from_secs(args.max_time),
        ) {
            Some(c) => (Some(c), Some(start_time.elapsed().unwrap().as_secs_f64())),
            None => {
                term.clear_last_lines(2)?;
                term.write_line(&format!(
                    "{} {}... {} Global optimum was not found with the given time budget!",
                    style("Scenario:").bold().blue(),
                    scenario,
                    style("Error:").bold().red(),
                ))?;
                return Ok(BencherResult {
                    scenario,
                    ideal_cost: f64::NAN,
                    optimal_cost: None,
                    optimal_cost_time: None,
                    num_nodes: net.num_devices(),
                    num_edges: net.links_symmetric().count(),
                    num_commands: net.current_config().get_diff(&final_config).modifiers.len(),
                    strategy_result: Vec::new(),
                    tree_result: Vec::new(),
                    random_result: Vec::new(),
                    baseline_mil_result: Vec::new(),
                    baseline_mif_result: Vec::new(),
                });
            }
        }
    } else {
        (None, None)
    };

    let result = BencherResult {
        scenario: scenario.clone(),
        ideal_cost,
        optimal_cost,
        optimal_cost_time,
        num_nodes: net.num_devices(),
        num_edges: net.links_symmetric().count(),
        num_commands: net.current_config().get_diff(&final_config).modifiers.len(),
        strategy_result,
        random_result,
        tree_result,
        baseline_mif_result,
        baseline_mil_result,
    };

    term.clear_last_lines(1)?;
    term.write_line(&format!(
        "{} {}",
        style("[8/8]").bright().black(),
        "Collecting results..."
    ))?;

    let summ = summary(&result, &args);

    export_result(&result, &args)?;

    term.clear_last_lines(2)?;
    term.write_line(&format!(
        "{}{}... {} {}",
        style("Scenario: ").bold().blue(),
        scenario,
        style("Done").bold().green(),
        style(&summ).bright().black(),
    ))?;

    Ok(result)
}

fn worker_runner<O: Optimizer<MinimizeTrafficShift>>(
    net: &Network,
    final_config: &Config,
    hard_policy: &HardPolicy,
    soft_policy: &MinimizeTrafficShift,
    max_time: u64,
    iterations: usize,
    ignore_nan: bool,
    num_threads: usize,
) -> Vec<Run> {
    let mut result = Vec::new();

    let (sender, receiver) = channel::<Run>();
    let abort = Stopper::new();
    let jobs_todo = Arc::new(Mutex::new(iterations));
    let time_budget = Some(Duration::from_secs(max_time));

    let bar = ProgressBar::new(iterations as u64);
    bar.tick();

    // spawn all workers
    let _workers: Vec<JoinHandle<()>> = (0..num_threads)
        .map(|_| {
            let n = net.clone();
            let m = net.current_config().get_diff(final_config).modifiers;
            let hp = hard_policy.clone();
            let sp = soft_policy.clone();
            let tx = sender.clone();
            let kill = abort.clone();
            let todo = jobs_todo.clone();
            let time = time_budget.clone();
            spawn(move || worker::<O>(n, m, hp, sp, time, tx, kill, todo))
        })
        .collect();

    for _ in 0..iterations {
        let run = receiver.recv().unwrap();
        bar.inc(1);
        if !(run.cost.is_nan() && ignore_nan) {
            result.push(run);
        }
    }

    abort.send_stop();
    bar.finish_and_clear();

    result
}

fn worker<O: Optimizer<MinimizeTrafficShift>>(
    net: Network,
    mut modifiers: Vec<ConfigModifier>,
    hard_policy: HardPolicy,
    soft_policy: MinimizeTrafficShift,
    time_budget: Option<Duration>,
    sender: Sender<Run>,
    mut kill: Stopper,
    jobs_todo: Arc<Mutex<usize>>,
) {
    let mut rng = thread_rng();
    loop {
        // check if kill switch was toggled
        if kill.try_is_stop().unwrap_or(false) {
            break;
        }

        // check if there are jobs todo
        {
            let mut jobs_todo_lock = jobs_todo.lock().unwrap();
            if *jobs_todo_lock > 0 {
                *jobs_todo_lock -= 1;
            } else {
                break;
            }
        }

        modifiers.shuffle(&mut rng);
        let mut optim = O::new(
            net.clone(),
            modifiers.clone(),
            hard_policy.clone(),
            soft_policy.clone(),
            time_budget,
        )
        .unwrap();
        let start_time = SystemTime::now();
        // synthesize the solution
        let cost = optim
            .work(kill.clone())
            .map(|(_, cost)| cost)
            .unwrap_or(f64::NAN);
        let time = start_time.elapsed().unwrap().as_secs_f64();
        let num_states = optim.num_states();
        if sender
            .send(Run {
                cost,
                time,
                num_states,
            })
            .is_err()
        {
            break;
        }
    }
}

fn find_global_optimum(
    net: Network,
    final_config: &Config,
    hard_policy: HardPolicy,
    soft_policy: MinimizeTrafficShift,
    max_time: Duration,
) -> Option<f64> {
    let modifiers = net.current_config().get_diff(final_config).modifiers;
    let mut optim =
        GlobalOptimizer::new(net, modifiers, hard_policy, soft_policy, Some(max_time)).unwrap();
    match optim.work(Stopper::new()) {
        Ok((_, cost)) => Some(cost),
        Err(_) => None,
    }
}
