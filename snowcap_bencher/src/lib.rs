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

//! # Net Update Bencher
//!
//! This library benchmarks the system based on a specific reconfiguration scenario.
#![deny(missing_docs)]

mod runner_optimizer;
mod runner_strategy;
mod utils;

use runner_optimizer::bench as optimizer_bench;
use runner_strategy::bench as strategy_bench;

use snowcap::{
    hard_policies::HardPolicy,
    netsim::{config::Config, Network},
};

use clap::Clap;
use serde::Serialize;
use std::error::Error;

/// Perform a benchmark of the specified configuration
pub fn bench(
    net: Network,
    final_config: Config,
    hard_policy: HardPolicy,
    scenario: String,
    args: BencherArguments,
) -> Result<BencherResult, Box<dyn Error>> {
    match args.bench_type {
        BencherType::Strategy => strategy_bench(net, final_config, hard_policy, scenario, args),
        BencherType::Optimizer => optimizer_bench(net, final_config, hard_policy, scenario, args),
    }
}

/// Arguments required for the bencher
#[derive(Clap, Debug, Clone)]
pub struct BencherArguments {
    /// Type of benchmark
    #[clap(arg_enum, default_value = "Optimizer")]
    pub bench_type: BencherType,
    /// Number of iterations to repeat
    #[clap(short = 'i', long, default_value = "10000")]
    pub iterations: usize,
    /// Maximum allowed time per run, in seconds. If the time exceeds the limit, then the cost will
    /// be set to nan.
    #[clap(short = 't', long, default_value = "300")]
    pub max_time: u64,
    /// If this flag is set, then nan values will be ignored, and not added to the resulting csv
    /// file
    #[clap(short = 'n', long)]
    pub ignore_nan: bool,
    /// Perform benching the random baseline
    #[clap(long)]
    pub random: bool,
    /// Perform benching the tree strategy
    #[clap(long)]
    pub tree: bool,
    /// Perform benching the main strategy
    #[clap(long)]
    pub main: bool,
    /// Perform benching the most-important-first baseline strategy
    #[clap(long)]
    pub mif: bool,
    /// Perform benching the most-important-first baseline strategy
    #[clap(long)]
    pub mil: bool,
    /// search for the global optimum
    #[clap(long = "optimum")]
    pub global_optimum: bool,
    /// Number of threads to use. Defaults to the number of threads available on the system.
    #[clap(short = 'p', long)]
    pub threads: Option<usize>,
    /// Output file to store the results. Two different files will be created: "NAME_strategy.csv",
    /// "NAME_tree.csv" and "NAME_random.csv"! Don't provide the file ending ".csv"!
    #[clap(long = "csv")]
    pub output_csv: Option<String>,
    /// Output file to store the results in json format. Give the entire path, including the json
    /// ending.
    #[clap(long = "json")]
    pub output_json: Option<String>,
}

/// Type of benchmark to perform
#[derive(Clap, Debug, Clone, PartialEq, Eq, Copy)]
pub enum BencherType {
    /// Benchmark the strategy
    #[clap(name = "strategy")]
    Strategy,
    /// Benchmark the optimizers
    #[clap(name = "optimizer")]
    Optimizer,
}

/// Result type that contains the entire output
#[derive(Debug, Clone, Serialize)]
pub struct BencherResult {
    /// String describing the scenario
    pub scenario: String,
    /// Ideal cost of the scenario
    pub ideal_cost: f64,
    /// Optimal cost of the scenario, if computed
    pub optimal_cost: Option<f64>,
    /// Time for computing the optimal cost (in seconds)
    pub optimal_cost_time: Option<f64>,
    /// Number of nodes in the network
    pub num_nodes: usize,
    /// Number of edges in the network
    pub num_edges: usize,
    /// Number of commands for the reconfiguration
    pub num_commands: usize,
    /// Result of the strategy
    pub strategy_result: Vec<Run>,
    /// Result of the tree strategy
    pub tree_result: Vec<Run>,
    /// Result of the random approach
    pub random_result: Vec<Run>,
    /// Result of the most-important-first baseline approach
    pub baseline_mif_result: Vec<Run>,
    /// Result of the most-important-last baseline approach
    pub baseline_mil_result: Vec<Run>,
}

/// Result of a single run
#[derive(Debug, Clone, Serialize)]
pub struct Run {
    /// Cost of the run
    cost: f64,
    /// Running time, measured in seconds
    time: f64,
    /// Number of states explored
    num_states: usize,
}
