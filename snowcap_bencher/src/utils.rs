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

//! Utility Functions for the bencher

use super::{BencherArguments, BencherResult, Run};

use snowcap::{
    hard_policies::HardPolicy,
    netsim::{config::Config, Network, NetworkError},
    soft_policies::SoftPolicy,
};

use csv::Writer;
use serde_json;

use std::error::Error;

pub fn export_result(
    result: &BencherResult,
    args: &BencherArguments,
) -> Result<(), Box<dyn Error>> {
    if let Some(csv_base) = args.output_csv.as_ref() {
        if args.main {
            let strategy_file = format!("{}_strategy.csv", csv_base);
            let mut wtr = Writer::from_path(strategy_file)?;
            for run in result.strategy_result.iter() {
                wtr.serialize(run)?;
            }
            wtr.flush()?;
        }

        if args.tree {
            let tree_file = format!("{}_tree.csv", csv_base);
            let mut wtr = Writer::from_path(tree_file)?;
            for run in result.tree_result.iter() {
                wtr.serialize(run)?;
            }
            wtr.flush()?;
        }

        if args.random {
            let random_file = format!("{}_random.csv", csv_base);
            let mut wtr = Writer::from_path(random_file)?;
            for run in result.random_result.iter() {
                wtr.serialize(run)?;
            }
            wtr.flush()?;
        }

        if args.mil {
            let mil_file = format!("{}_baseline_mil.csv", csv_base);
            let mut wtr = Writer::from_path(mil_file)?;
            for run in result.baseline_mil_result.iter() {
                wtr.serialize(run)?;
            }
            wtr.flush()?;
        }

        if args.mif {
            let mif_file = format!("{}_baseline_mif.csv", csv_base);
            let mut wtr = Writer::from_path(mif_file)?;
            for run in result.baseline_mif_result.iter() {
                wtr.serialize(run)?;
            }
            wtr.flush()?;
        }
    }

    if let Some(json_file) = args.output_json.as_ref() {
        let result_str = serde_json::to_string_pretty(result)?;
        std::fs::write(json_file, result_str)?;
    }

    Ok(())
}

pub fn summary(result: &BencherResult, args: &BencherArguments) -> String {
    format!(
        "[info: c={:.3}, n={}, e={}{}, m={}]{}{}{}{}{}",
        result.ideal_cost,
        result.num_nodes,
        result.num_edges,
        if let Some(optimal) = result.optimal_cost {
            format!(
                ", optimal={:.5} ({:.1}s)",
                optimal,
                result.optimal_cost_time.unwrap_or(0.0)
            )
        } else {
            "".to_string()
        },
        result.num_commands,
        if args.main {
            summary_bench("optimizer", &result.strategy_result)
        } else {
            "".to_string()
        },
        if args.tree {
            summary_bench("tree", &result.tree_result)
        } else {
            "".to_string()
        },
        if args.random {
            summary_bench("random", &result.random_result)
        } else {
            "".to_string()
        },
        if args.mif {
            summary_bench("MIF", &result.baseline_mif_result)
        } else {
            "".to_string()
        },
        if args.mil {
            summary_bench("MIL", &result.baseline_mil_result)
        } else {
            "".to_string()
        },
    )
}

fn summary_bench(title: &str, bench: &[Run]) -> String {
    let len = bench.len() as f64;
    let len_cost = bench.iter().filter(|r| !r.cost.is_nan()).count() as f64;
    format!(
        " [{}: c={:.3}, t={:.3}s, i={:.1}]",
        title,
        bench
            .iter()
            .fold(0.0, |x, r| if r.cost.is_nan() { x } else { x + r.cost })
            / len_cost,
        bench.iter().fold(0.0, |x, r| x + r.time) / len,
        bench.iter().fold(0.0, |x, r| x + (r.num_states as f64)) / len,
    )
}

pub fn check_config<SP: SoftPolicy + Clone>(
    net: &Network,
    final_config: &Config,
    hard_policy: &HardPolicy,
) -> Option<f64> {
    let mut net = net.clone();
    let mut hard_policy = hard_policy.clone();
    let mut fw_state = net.get_forwarding_state();
    let mut soft_policy = SP::new(&mut fw_state, &net);

    let mut fw_state = net.get_forwarding_state();
    hard_policy.set_num_mods_if_none(2);
    hard_policy.step(&mut net, &mut fw_state).ok()?;
    if !hard_policy.check() {
        return None;
    }
    match net.set_config(final_config) {
        Ok(()) => {}
        Err(NetworkError::ConvergenceLoop(_, _)) => return Some(f64::NAN),
        Err(NetworkError::NoConvergence) => return Some(f64::NAN),
        Err(_) => return None,
    }
    let mut fw_state = net.get_forwarding_state();
    hard_policy.step(&mut net, &mut fw_state).ok()?;
    if !hard_policy.check() {
        return None;
    }
    soft_policy.update(&mut fw_state, &net);
    Some(soft_policy.cost())
}
