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

// Experimental Results (50 networks, 5000 iterations, non-random root node, seed 42):
// Mean:   83.4%
// Median: 87.2%
// StdDev: 12.2%

use clap::Clap;
use pretty_env_logger;
use std::error::Error;

mod cost;
mod dep_groups;
mod plot;
mod probability;
mod tree;
pub mod utils;

fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let args = CommandLineArguments::parse();

    let topo_config = utils::TopoConfig {
        file: args.file,
        seed: args.seed,
        random_root: args.random_root,
        many_prefixes: args.many_prefixes,
        scenario: args.scenario,
    };

    match args.mode {
        CommandLineMode::Plot { bins, output } => {
            plot::show(topo_config.file, bins, output)?;
            Ok(())
        }
        CommandLineMode::Probability {
            only_statistics,
            output,
        } => probability::run(
            args.iterations,
            args.num_networks,
            args.num_threads,
            topo_config,
            only_statistics,
            output,
        ),
        CommandLineMode::Cost {
            all_strategies,
            only_statistics,
            optimizer_fraction,
            output,
        } => cost::run(
            args.iterations,
            args.num_networks,
            args.num_threads,
            topo_config,
            all_strategies,
            only_statistics,
            optimizer_fraction,
            output,
        ),
        CommandLineMode::DepGroups => {
            dep_groups::run(args.iterations, args.num_networks, topo_config)
        }
        CommandLineMode::Tree => tree::run(args.iterations, args.num_networks, topo_config),
    }
}

/// This is a tool for performing measurements on reconfiguration scenarios.
#[derive(Clap, Debug)]
#[clap(name = "ProblemProbability", author = "Tibor Schneider")]
struct CommandLineArguments {
    /// Number of iterations to perform per simulated network. This is different for different
    /// modes.
    #[clap(short, long, default_value = "1000")]
    iterations: usize,
    /// Number of networks to simulate
    #[clap(short, long, default_value = "1")]
    num_networks: usize,
    /// Seed for the random number generator
    #[clap(long, default_value = "42")]
    seed: u64,
    /// use many prefixes (i.e., 5 prefixes, distributed with a probability of 0.5)
    #[clap(long)]
    many_prefixes: bool,
    /// Use a random roots when generating configuration
    #[clap(short, long)]
    random_root: bool,
    /// Limit the number of threads to use for computation
    #[clap(long)]
    num_threads: Option<usize>,
    /// Select the reconfiguration scenario
    #[clap(arg_enum, short, long, default_value = "FM2RR")]
    scenario: utils::Scenario,
    /// GNS file (or result file) to read
    file: String,
    /// Type of measurement to perform
    #[clap(subcommand)]
    mode: CommandLineMode,
}

#[derive(Clap, Debug)]
enum CommandLineMode {
    /// Plot a previous measurement
    #[clap(name = "plot")]
    Plot {
        /// Number of bins for displaying the histograms
        #[clap(short, long, default_value = "20")]
        bins: usize,
        /// Output where to place the resulting HTML file
        #[clap(short, long)]
        output: Option<String>,
    },
    /// Approximate the probability of failing the migration scenario
    #[clap(name = "probability")]
    Probability {
        /// Store only statistics in the output file.
        #[clap(short = 's', long)]
        only_statistics: bool,
        /// Output where to place the measurement results.
        #[clap(short, long)]
        output: Option<String>,
    },
    /// Compare the cost (in terms of traffic shifts) for the migration scenario.
    #[clap(name = "cost")]
    Cost {
        /// Compare all strategies. If this is set to false, we only use the tree optimizer
        #[clap(short, long)]
        all_strategies: bool,
        /// Store only statistics in the output file.
        #[clap(short = 's', long)]
        only_statistics: bool,
        /// Factor to divide the number of iterations by, when computing the cost for the
        /// optimizers.
        #[clap(short = 'f', long, default_value = "100")]
        optimizer_fraction: usize,
        /// Output where to place the measurement results.
        #[clap(short, long)]
        output: Option<String>,
    },
    /// Check how the dependnecy gorups strategy performs
    #[clap(name = "dep-groups")]
    DepGroups,
    /// Check how the Tree strategy performs
    #[clap(name = "tree")]
    Tree,
}
