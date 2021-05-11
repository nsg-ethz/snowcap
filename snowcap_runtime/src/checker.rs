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

//! This module checks the paths if the conditions are ok

use snowcap::hard_policies::*;
use snowcap::netsim::{Prefix, RouterId};

use log::*;
use std::collections::HashMap;

use super::physical_network::{PhysicalNetwork, CLIENT_ID_BASE};

/// Checks if the conditions supplied are satisfied. This function excepts a vector of path
/// conditions. It is not yet generalized to also accept
/// [LTL formulas](snowcap::hard_policies).
pub fn check(
    paths: HashMap<(RouterId, Prefix), HashMap<Option<Vec<RouterId>>, usize>>,
    conds: &Vec<Condition>,
    phys_net: &PhysicalNetwork,
) -> bool {
    let mut conds_ok: bool = true;
    for cond in conds {
        match cond {
            Condition::Reachable(router, prefix, None) => {
                let client: RouterId = (router.index() as u32 + CLIENT_ID_BASE).into();
                let router_name = phys_net.router_name(*router);
                info!("Checking condition: {} can reach prefix {}", router_name, prefix.0);
                let p = match paths.get(&(client, *prefix)) {
                    Some(p) => p,
                    None => {
                        warn!(
                            "No packets from router {} to prefix {} were found!",
                            router_name, prefix.0
                        );
                        conds_ok = false;
                        continue;
                    }
                };
                for (path, count) in p.iter() {
                    match path {
                        None => {
                            warn!("    {} packets dropped!", count);
                            conds_ok = false;
                        }
                        Some(path) => info!(
                            "    {} packets took the path: [{}]",
                            count,
                            path_str(phys_net, path)
                        ),
                    }
                }
            }
            Condition::Reachable(router, prefix, Some(cond)) => {
                let client: RouterId = (router.index() as u32 + CLIENT_ID_BASE).into();
                let router_name = phys_net.router_name(*router);
                info!(
                    "Checking condition: {} can reach prefix {} with path condition: {}",
                    router_name, prefix.0, cond
                );
                let p = match paths.get(&(client, *prefix)) {
                    Some(p) => p,
                    None => {
                        warn!(
                            "No packets from router {} to prefix {} were found!",
                            router_name, prefix.0
                        );
                        conds_ok = false;
                        continue;
                    }
                };
                for (path, count) in p.iter() {
                    match path {
                        None => {
                            warn!("    {} packets dropped!", count);
                            conds_ok = false;
                        }
                        Some(path) => {
                            let path_len = path.len();
                            let path_repr = path_str(phys_net, path);
                            let short_path = (&path[1..path_len - 1]).to_vec();
                            if cond.check(&short_path, *prefix).is_ok() {
                                info!("    {} packets took path {}", count, path_repr);
                            } else {
                                conds_ok = false;
                                warn!("    {} packets took path {}", count, path_repr);
                            }
                        }
                    }
                }
            }
            Condition::NotReachable(router, prefix) => {
                let client: RouterId = (router.index() as u32 + CLIENT_ID_BASE).into();
                let router_name = phys_net.router_name(*router);
                info!("Checking condition: {} cannot reach prefix {}", router_name, prefix.0);
                let p = match paths.get(&(client, *prefix)) {
                    Some(p) => p,
                    None => {
                        warn!(
                            "No packets from router {} to prefix {} were found!",
                            router_name, prefix.0
                        );
                        conds_ok = false;
                        continue;
                    }
                };
                for (path, count) in p.iter() {
                    match path {
                        None => info!("    {} packets dropped!", count),
                        Some(path) => {
                            warn!(
                                "    {} packets took the path: [{}]",
                                count,
                                path_str(phys_net, path)
                            );
                            conds_ok = false;
                        }
                    }
                }
            }
            Condition::Reliable(_, _, _) => info!("Skipping reliability condition"),
            Condition::TransientPath(_, _, _) => info!("Skipping transient path condition"),
        }
    }

    conds_ok
}

/// Print all paths as info logs
pub fn print_paths(
    flows: &HashMap<(RouterId, Prefix), HashMap<Option<Vec<RouterId>>, usize>>,
    phys_net: &PhysicalNetwork,
) {
    for ((router, prefix), paths) in flows {
        info!("Paths from {} for prefix {}", phys_net.router_name(*router), prefix.0);
        for (path, num) in paths {
            if let Some(path) = path {
                info!("    {} packets: [{}]", num, path_str(phys_net, path));
            } else {
                info!("    {} packets dropped!", num);
            }
        }
    }
}

fn path_str(phys_net: &PhysicalNetwork, path: &Vec<RouterId>) -> String {
    path.iter().map(|r| phys_net.router_name(*r)).collect::<Vec<_>>().join(" -> ")
}
