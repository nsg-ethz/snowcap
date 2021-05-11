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

//! # Runtime System
//!
//! This system generates a virtual network inside GNS3, configures all nodes and performs the
//! migration scenario, while monitoring the forwarding state. For simplified usage, check the
//! function [`perform_migration`].

#![deny(missing_docs, missing_debug_implementations)]

pub mod checker;
pub mod config;
pub mod frr_conn;
pub mod pcap_reader;
pub mod physical_network;
pub mod python_conn;

use physical_network::PhysicalNetwork;
use snowcap::netsim::{config::ConfigModifier, printer, Network, Prefix, RouterId};

use log::*;
use serde::Serialize;
use std::collections::HashMap;
use std::error::Error;

/// # Perform the migraiton
///
/// Based on the network (in the initial state), the migration sequence and some invariants, perform
/// perform the migration and check that the invariants are satisfied, on a simulated network using
/// GNS3 and FRRouting.
///
/// This funciton does the following:
///
/// 1. Creating the GNS project, and setting up all devices and links
/// 2. Configure all devices, such that the same state as the network is achieved
/// 3. Wait until the network has converged, and compare the paths with the paths that are expected
///    based on the network
/// 4. For each migration step, perform the modification. Then, while waiting for the network to
///    converge, inject traffic into the network and capture their path. After the network has
///    converged, infer the path of each packet by analyzing the traces on the links. Then, check
///    the invariants, that every step is correct.
#[allow(clippy::type_complexity)]
pub fn perform_migration(
    net: &Network,
    migration_sequence: &[ConfigModifier],
    persistent_gns_project: bool,
    json_filename: Option<String>,
    reconfiguration_at_once: bool,
) -> Result<bool, Box<dyn Error>> {
    info!("Generating the network...");
    let mut phys_net = PhysicalNetwork::new(&net, "RuntimeNet", persistent_gns_project)?;

    info!("performing all traceroutes!");
    let all_paths = phys_net.get_all_paths()?;
    let mut fw_state = net.get_forwarding_state();
    for (router, paths) in all_paths {
        for (prefix, path) in paths {
            // check the expected path in the forwarding state
            let expected_path = fw_state.get_route(router, prefix).ok();
            let path_repr = path
                .map(|p| {
                    let len = p.len();
                    std::iter::once(&router)
                        .chain(p.iter())
                        .enumerate()
                        .filter(|(i, _)| *i < len)
                        .map(|(_, r)| phys_net.get_router_name(*r))
                        .collect::<Vec<_>>()
                        .join(" -> ")
                })
                .unwrap_or_else(|| "NONE".to_string());
            let expected_path_repr = expected_path
                .map(|p| {
                    p.iter().map(|r| phys_net.get_router_name(*r)).collect::<Vec<_>>().join(" -> ")
                })
                .unwrap_or_else(|| "NONE".to_string());

            info!("[{}] (correct: {})", path_repr, path_repr == expected_path_repr)
        }
    }

    info!("Starting the migration");

    let mut flows: HashMap<(RouterId, Prefix), Vec<HashMap<Option<Vec<RouterId>>, usize>>> =
        HashMap::new();

    if reconfiguration_at_once {
        info!("Applying all modifiers...");
        let new_flows =
            phys_net.apply_all_modifiers_wait_convergence_check_flows(&migration_sequence, 2)?;
        checker::print_paths(&new_flows, &phys_net);

        // append the new flows to the existing ones
        for (key, paths) in new_flows {
            let flow = flows.entry(key).or_default();
            flow.push(paths);
        }
    } else {
        for modifier in migration_sequence.iter() {
            info!("Applying the modifier {}", printer::config_modifier(&net, modifier)?);
            let new_flows = phys_net.apply_modifier_wait_convergence_check_flows(modifier)?;
            checker::print_paths(&new_flows, &phys_net);

            // append the new flows to the existing ones
            for (key, paths) in new_flows {
                let flow = flows.entry(key).or_default();
                flow.push(paths);
            }
        }
    }

    if let Some(json_filename) = json_filename {
        // transform the data into the storable format
        let data = flows
            .into_iter()
            .map(|((router, prefix), paths)| FlowInformation {
                router: phys_net.router_name(router).to_string(),
                prefix: prefix.0,
                paths: paths
                    .into_iter()
                    .map(|v| {
                        v.into_iter()
                            .map(|(path, count)| PathInformation {
                                count,
                                path: path
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(|r| phys_net.router_name(r).to_string())
                                    .collect(),
                            })
                            .collect()
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();

        let data_string = serde_json::to_string(&data)?;
        std::fs::write(json_filename, data_string)?;
    }

    Ok(true)
}

#[derive(Debug, Clone, Serialize)]
struct FlowInformation {
    router: String,
    prefix: u32,
    paths: Vec<Vec<PathInformation>>,
}

#[derive(Debug, Clone, Serialize)]
struct PathInformation {
    count: usize,
    path: Vec<String>,
}
