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

//! Utility functions

use snowcap::hard_policies::*;
use snowcap::netsim::{
    config::{Config, ConfigExpr, ConfigModifier, ConfigPatch},
    BgpSessionType, Network, NetworkError, RouterId,
};
use snowcap::topology_zoo::{self, ZooTopology};

use clap::Clap;
use log::*;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::error::Error;

pub fn check_config(
    mut net: Network,
    final_config: &Config,
    mut policy: HardPolicy,
) -> Result<(), Box<dyn Error>> {
    let mut fw_state = net.get_forwarding_state();
    policy.set_num_mods_if_none(2);

    policy
        .step(&mut net, &mut fw_state)
        .expect("policy step failed");
    if !policy.check() {
        error!(
            "Policy error on initial config! Errors: \n    {}",
            policy
                .last_errors()
                .iter()
                .map(|e| e.repr_with_name(&net))
                .collect::<Vec<_>>()
                .join("\n    "),
        );
        return Err("Policy error on initial config!".into());
    }

    net.set_config(final_config)?;
    policy
        .step(&mut net, &mut fw_state)
        .expect("policy step failed");
    if !policy.check() {
        error!(
            "Policy error on final config! Errors: \n    {}",
            policy
                .last_errors()
                .iter()
                .map(|e| e.repr_with_name(&net))
                .collect::<Vec<_>>()
                .join("\n    "),
        );
        return Err("Policy error on final config!".into());
    }

    Ok(())
}

pub fn do_random_reconfiguration(
    mut net: Network,
    modifiers: &mut Vec<ConfigModifier>,
    rng: &mut ThreadRng,
) -> Result<(), (HashSet<PolicyError>, ConfigModifier)> {
    modifiers.shuffle(rng);

    let mut policy =
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter());
    policy.set_num_mods_if_none(modifiers.len() + 1);
    let mut fw_state = net.get_forwarding_state();
    policy
        .step(&mut net, &mut fw_state)
        .expect("policy step failed");

    for m in modifiers.into_iter() {
        match net.apply_modifier(&m) {
            Ok(_) => {}
            Err(NetworkError::NoConvergence) | Err(NetworkError::ConvergenceLoop(_, _)) => {
                let mut set = HashSet::with_capacity(1);
                set.insert(PolicyError::NoConvergence);
                return Err((set, m.clone()));
            }
            Err(e) => panic!("Unrecoverable network error: {}", e),
        }
        let mut fw_state = net.get_forwarding_state();
        policy
            .step(&mut net, &mut fw_state)
            .expect("policy step failed");
        if !policy.check() {
            return Err((policy.last_errors(), m.clone()));
        }
    }

    Ok(())
}

pub fn do_random_reconfiguration_with_fail_magnitude(
    mut net: Network,
    modifiers: &mut Vec<ConfigModifier>,
    rng: &mut ThreadRng,
    insert_before_remove: bool,
) -> Result<(), (f64, Vec<f64>)> {
    modifiers.shuffle(rng);
    if insert_before_remove {
        modifiers.sort_unstable_by(|a, b| match (a, b) {
            (ConfigModifier::Insert(_), ConfigModifier::Insert(_))
            | (ConfigModifier::Update { .. }, ConfigModifier::Update { .. })
            | (ConfigModifier::Remove(_), ConfigModifier::Remove(_)) => Ordering::Equal,
            (ConfigModifier::Insert(_), _) => Ordering::Less,
            (_, ConfigModifier::Insert(_)) => Ordering::Greater,
            (ConfigModifier::Update { .. }, _) => Ordering::Less,
            (_, ConfigModifier::Update { .. }) => Ordering::Greater,
        })
    }

    let mut num_fail: u64 = 0;
    let mut num_fail_per_step: u64;

    let mut magnitude_per_step: Vec<f64> = Vec::with_capacity(modifiers.len());

    let routers: Vec<_> = net.get_routers();
    let prefixes: Vec<_> = net.get_known_prefixes().iter().cloned().collect();

    let step_norm_factor: f64 = (routers.len() * prefixes.len()) as f64;

    let mut is_valid: bool = true;

    for m in modifiers.into_iter() {
        match net.apply_modifier(&m) {
            Ok(_) => {}
            Err(NetworkError::NoConvergence) => is_valid = false,
            Err(e) => panic!("Unrecoverable network error: {}", e),
        }
        num_fail_per_step = 0;
        let mut fw_state = net.get_forwarding_state();
        for r in routers.iter() {
            for p in prefixes.iter() {
                match fw_state.get_route(*r, *p) {
                    Ok(_) => {}
                    Err(_) => {
                        num_fail += 1;
                        num_fail_per_step += 1;
                        is_valid = false;
                    }
                }
            }
        }
        if num_fail_per_step > 0 {
            magnitude_per_step.push((num_fail_per_step as f64) / step_norm_factor);
        }
    }

    if is_valid {
        Ok(())
    } else {
        let norm_factor: f64 = (routers.len() * prefixes.len() * modifiers.len()) as f64;
        let magnitude = (num_fail as f64) / norm_factor;
        Err((magnitude, magnitude_per_step))
    }
}

pub fn do_router_based_random_reconfiguration_with_fail_magnitude(
    mut net: Network,
    modifiers: &Vec<ConfigModifier>,
    rng: &mut ThreadRng,
) -> Result<(), (f64, Vec<f64>)> {
    let mut num_fail: u64 = 0;
    let mut num_fail_per_step: u64;

    let mut magnitude_per_step: Vec<f64> = Vec::with_capacity(modifiers.len());

    let routers: Vec<_> = net.get_routers();
    let prefixes: Vec<_> = net.get_known_prefixes().iter().cloned().collect();

    let mut router_order = routers.clone();
    router_order.shuffle(rng);
    let mut router_modifiers: HashMap<RouterId, Vec<ConfigModifier>> = router_order
        .iter()
        .cloned()
        .zip(std::iter::repeat(Vec::new()))
        .collect();

    for m in modifiers {
        match match m {
            ConfigModifier::Insert(e) => (e, false, true),
            ConfigModifier::Remove(e) => (e, false, false),
            ConfigModifier::Update { to, .. } => (to, true, true),
        } {
            (ConfigExpr::IgpLinkWeight { source, .. }, _, _) => {
                router_modifiers
                    .get_mut(source)
                    .and_then(|v| Some(v.push(m.clone())));
            }
            (ConfigExpr::StaticRoute { router, .. }, _, _) => {
                router_modifiers.get_mut(router).unwrap().push(m.clone())
            }
            (ConfigExpr::BgpRouteMap { router, .. }, _, _) => {
                router_modifiers.get_mut(router).unwrap().push(m.clone())
            }
            (
                ConfigExpr::BgpSession {
                    source,
                    target,
                    session_type,
                },
                config_changed,
                config_remains,
            ) => {
                let source_pos = router_order.iter().position(|x| x == source).unwrap();
                let target_pos = router_order.iter().position(|x| x == target).unwrap();
                let first_router = if source_pos < target_pos {
                    source
                } else {
                    target
                };
                let last_router = if source_pos > target_pos {
                    source
                } else {
                    target
                };
                let source_internal = net.get_device(*source).is_external();
                match (
                    session_type,
                    config_changed,
                    config_remains,
                    source_internal,
                ) {
                    (BgpSessionType::EBgp, _, _, true) => {
                        router_modifiers.get_mut(source).unwrap().push(m.clone())
                    }
                    (BgpSessionType::EBgp, _, _, false) => {
                        router_modifiers.get_mut(target).unwrap().push(m.clone())
                    }
                    (_, _, false, _) => router_modifiers
                        .get_mut(first_router)
                        .unwrap()
                        .push(m.clone()),
                    (_, true, _, _) => router_modifiers.get_mut(source).unwrap().push(m.clone()),
                    (_, false, _, _) => router_modifiers
                        .get_mut(last_router)
                        .unwrap()
                        .push(m.clone()),
                }
            }
        }
    }

    let step_norm_factor: f64 = (routers.len() * prefixes.len()) as f64;

    let mut is_valid: bool = true;
    let num_patches = router_modifiers.len();

    for (_, mods) in router_modifiers {
        let patch = ConfigPatch { modifiers: mods };
        match net.apply_patch(&patch) {
            Ok(_) => {}
            Err(NetworkError::NoConvergence) => is_valid = false,
            Err(e) => panic!("Unrecoverable network error: {}", e),
        }
        num_fail_per_step = 0;
        let mut fw_state = net.get_forwarding_state();
        for r in routers.iter() {
            for p in prefixes.iter() {
                match fw_state.get_route(*r, *p) {
                    Ok(_) => {}
                    Err(_) => {
                        num_fail += 1;
                        num_fail_per_step += 1;
                        is_valid = false;
                    }
                }
            }
        }
        if num_fail_per_step > 0 {
            magnitude_per_step.push((num_fail_per_step as f64) / step_norm_factor);
        }
    }

    if is_valid {
        Ok(())
    } else {
        let norm_factor: f64 = (routers.len() * prefixes.len() * num_patches) as f64;
        let magnitude = (num_fail as f64) / norm_factor;
        Err((magnitude, magnitude_per_step))
    }
}

pub fn create_bins(data: &Vec<f64>, min: f64, max: f64, n_bins: usize) -> Vec<usize> {
    // compute bins
    let mut bins: Vec<usize> = std::iter::repeat(0).take(n_bins).collect();
    let delta: f64 = max - min;

    for elem in data.iter() {
        let mut bin = ((*elem - min) * (n_bins as f64) / delta).floor() as usize;
        if bin >= n_bins {
            bin = n_bins - 1
        }
        bins[bin] += 1;
    }

    bins
}

pub fn print_bins(data: &Vec<f64>, min: f64, max: f64, n_bins: usize) {
    let delta: f64 = max - min;
    let bin_step: f64 = delta / n_bins as f64;
    let bins = create_bins(data, min, max, n_bins);

    println!("Bins:");
    for b in 0..n_bins {
        let bf = b as f64;
        println!(
            "  [{:.4}, {:.4}]: {}",
            min + bin_step * bf,
            min + bin_step * (bf + 1.0),
            bins[b]
        );
    }
}

pub fn get_net_config(c: &TopoConfig) -> Result<(Network, Config), Box<dyn Error>> {
    let mut t = ZooTopology::new(&c.file, c.seed)?;

    let (net, config_b, hard_policy) = t.apply_scenario(
        c.scenario.clone().into(),
        c.random_root,
        100,
        if c.many_prefixes { 5 } else { 1 },
        if c.many_prefixes { 0.5 } else { 1.0 },
    )?;

    // check that config A and B works
    check_config(net.clone(), &config_b, hard_policy.clone())?;

    // try to find a valid ordering using the tree strategy
    snowcap::synthesize_parallel(
        net.clone(),
        net.current_config().clone(),
        config_b.clone(),
        hard_policy,
        std::time::Duration::from_secs(300),
        None,
    )?;

    Ok((net, config_b))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopoConfig {
    pub file: String,
    pub seed: u64,
    pub random_root: bool,
    pub many_prefixes: bool,
    pub scenario: Scenario,
}

#[derive(Clap, Debug, Clone, Serialize, Deserialize)]
pub enum Scenario {
    /// Scenario, where we start with a iBGP full mesh, and end up with a topology, where one single
    /// router is elected as a Route Reflectors, and all others pair with that router.
    #[clap(name = "FM2RR")]
    FullMesh2RouteReflector,
    /// Scenario, where we start with a topology, where one single router is elected as a Route
    /// Reflectors, and all others pair with that router, and we end up wiht an iBGP full mesh.
    #[clap(name = "RR2FM")]
    RouteReflector2FullMesh,
    /// Scenario, where every IGP weight is doubled
    #[clap(name = "IGPx2")]
    DoubleIgpWeight,
    /// Scenario, where every IGP weight is halved
    #[clap(name = "IGPdiv2")]
    HalveIgpWeight,
    /// Scenario, where every loacl pref is doubled
    #[clap(name = "LPx2")]
    DoubleLocalPref,
    /// Scenario, where every local pref is halved
    #[clap(name = "LPdiv2")]
    HalveLocalPref,
    /// Scenario, where we start with a single Route-Reflector, to which all other routers pair, and
    /// end with a second Route-Reflector as a backup, where all other routers have a session to
    /// both reflectors, and the two reflectors are connected with a peer.
    #[clap(name = "add2ndRR")]
    IntroduceSecondRouteReflector,
    /// Scenario, where we start with a second Route-Reflector as a backup, where all other routers
    /// have a session to both reflectors, and the two reflectors are connected with a peer, and end
    /// with a single Route-Reflector, to which all other routers pair.
    #[clap(name = "del2ndRR")]
    RemoveSecondRouteReflector,
    /// Scenario, where we start with two different connected components, both having connection to
    /// the outside world, and we merge them by adding the links in between.
    #[clap(name = "NetAcq")]
    NetworkAcquisition,
    /// Reverse scenario of the Network Acquisition
    #[clap(name = "NetSplit")]
    NetworkSplit,
    /// Disconnect a random non-border router form the network by setting all of its link weights to
    /// infinity. The IBGP topoogy will be a Route-Reflector topology, and the router disabled will
    /// not be selected as root!
    #[clap(name = "DiscR")]
    DisconnectRouter,
    /// Connect a random non-border router to the network by setting all of its link weights to a
    /// normal number. The IBGP topoogy will be a Route-Reflector topology, and the router disabled
    /// will not be selected as root!
    #[clap(name = "ConnR")]
    ConnectRouter,
}

impl Into<topology_zoo::Scenario> for Scenario {
    fn into(self) -> topology_zoo::Scenario {
        match self {
            Scenario::FullMesh2RouteReflector => topology_zoo::Scenario::FullMesh2RouteReflector,
            Scenario::RouteReflector2FullMesh => topology_zoo::Scenario::RouteReflector2FullMesh,
            Scenario::DoubleIgpWeight => topology_zoo::Scenario::DoubleIgpWeight,
            Scenario::HalveIgpWeight => topology_zoo::Scenario::HalveIgpWeight,
            Scenario::DoubleLocalPref => topology_zoo::Scenario::DoubleLocalPref,
            Scenario::HalveLocalPref => topology_zoo::Scenario::HalveLocalPref,
            Scenario::IntroduceSecondRouteReflector => {
                topology_zoo::Scenario::IntroduceSecondRouteReflector
            }
            Scenario::RemoveSecondRouteReflector => {
                topology_zoo::Scenario::RemoveSecondRouteReflector
            }
            Scenario::NetworkAcquisition => topology_zoo::Scenario::NetworkAcquisition,
            Scenario::NetworkSplit => topology_zoo::Scenario::NetworkSplit,
            Scenario::DisconnectRouter => topology_zoo::Scenario::DisconnectRouter,
            Scenario::ConnectRouter => topology_zoo::Scenario::ConnectRouter,
        }
    }
}

impl TopoConfig {
    pub fn html_description(&self) -> String {
        let mut html = String::new();
        html.push_str("<h3>Network Description</h3>\n");
        html.push_str("<table style=\"width: 100%\">\n");
        html.push_str(&format!(
            "<tr><th>Filename</th><td>{}</td></tr>\n",
            self.file.split('/').last().unwrap(),
        ));
        html.push_str(&format!(
            "<tr><th>Scenario</th><td>{}</td></tr>\n",
            match self.scenario {
                Scenario::FullMesh2RouteReflector => "Full-Mesh to Route-Reflector",
                Scenario::RouteReflector2FullMesh => "Route-Reflector to Full-Mesh",
                Scenario::DoubleIgpWeight => "Double IGP weights",
                Scenario::HalveIgpWeight => "Halve IGP weights",
                Scenario::DoubleLocalPref => "Double LocalPref",
                Scenario::HalveLocalPref => "Halve LocalPref",
                Scenario::IntroduceSecondRouteReflector => "Introduce second route reflector",
                Scenario::RemoveSecondRouteReflector => "Remove second route reflector",
                Scenario::NetworkAcquisition => "Network Acquisition",
                Scenario::NetworkSplit => "Network Split",
                Scenario::DisconnectRouter => "Disconnect Router",
                Scenario::ConnectRouter => "Connect Router",
            }
        ));
        html.push_str(&format!(
            "<tr><th>Prefixes</th><td>{}</td></tr>\n",
            if self.many_prefixes {
                "5 different prefixes, each of them advertised on every external router with probability 0.5"
            } else {
                "1 prefix, advertised on all external routers"
            }
        ));
        html.push_str(&format!(
            "<tr><th>Choice of RR</th><td>{}</td></tr>\n",
            if self.random_root {
                "Route Reflector chosen at random"
            } else {
                "Route Reflector chosen to be the router with the most links to other internal routers"
            }
        ));
        html.push_str(&format!("<tr><th>Seed</th><td>{}</td></tr>\n", self.seed));
        html.push_str("</table>\n");
        html
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StrategyResult {
    pub trials: usize,
    pub success: usize,
    pub failures: usize,
    pub success_rate: f64,
}

impl StrategyResult {
    pub fn new(success: usize, failures: usize) -> Self {
        Self {
            trials: success + failures,
            success,
            failures,
            success_rate: (success as f64) / ((success + failures) as f64),
        }
    }
    pub fn summary(&self, title: impl AsRef<str>) {
        println!("Summary of {}:", title.as_ref());
        println!(
            "  Success rate: {}% ({} of {})",
            self.success_rate * 100.0,
            self.success,
            self.trials
        );
    }

    pub fn summary_to_html(&self, title: &str) -> String {
        let mut html = String::new();
        html.push_str(&format!(
            "<tr><th>{}</th><td>Success Rate</td><td><b>{:.2}%</b> ({} of {})</td></tr>\n",
            title,
            self.success_rate * 100.0,
            self.success,
            self.trials
        ));
        html
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OptimizerResult {
    pub trials: usize,
    pub success: usize,
    pub failures: usize,
    pub success_rate: f64,
    pub cost: StatisticsResult<f64>,
}

impl OptimizerResult {
    pub fn new(success: usize, failures: usize, cost_values: Vec<f64>) -> Self {
        Self {
            trials: success + failures,
            success,
            failures,
            success_rate: (success as f64) / ((success + failures) as f64),
            cost: StatisticsResult::new(cost_values),
        }
    }

    pub fn summary(&self, title: impl AsRef<str>) {
        println!("Summary of {}:", title.as_ref());
        println!(
            "  Success rate: {:.2}% ({} of {})",
            self.success_rate * 100.0,
            self.success,
            self.trials
        );
        println!("  cost: {:.4} +- {:.4}", self.cost.mean, self.cost.std);
        println!(
            "        min: {:.4}, median: {:.4}, max: {:.4}",
            self.cost.min, self.cost.median, self.cost.max
        );
    }

    pub fn summary_to_html(&self, title: &str) -> String {
        let mut html = String::new();
        html.push_str(&format!(
            "<tr><th>{}</th><td>Success Rate</td><td><b>{:.2}%</b> ({} of {})</td></tr>\n",
            title,
            self.success_rate * 100.0,
            self.success,
            self.trials
        ));
        html.push_str(&format!(
            "<tr><th></th><td>Cost</td><td><b>{:.4}</b> +- {:.4} (min: {:.4}, median: {:.4}, max: {:.4})</td></tr>\n",
            self.cost.mean, self.cost.std, self.cost.min, self.cost.median, self.cost.max,
        ));
        html
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatisticsResult<T>
where
    T: Clone + PartialOrd + Into<f64> + std::ops::Add<Output = T> + Default,
{
    pub min: T,
    pub max: T,
    pub mean: f64,
    pub median: f64,
    pub std: f64,
    pub values: Vec<T>,
}

impl<T> StatisticsResult<T>
where
    T: Clone + PartialOrd + Into<f64> + std::ops::Add<Output = T> + Default,
{
    /// Prepares the statistical result
    pub fn new(values: Vec<T>) -> StatisticsResult<T> {
        if values.len() == 0 {
            return Self {
                min: T::default(),
                max: T::default(),
                mean: 0.0,
                median: 0.0,
                std: 0.0,
                values: Vec::new(),
            };
        }
        let first_elem: T = values.get(0).unwrap().clone();
        let min: T = values.iter().skip(1).fold(first_elem.clone(), |min, elem| {
            if min > *elem {
                elem.clone()
            } else {
                min
            }
        });
        let max: T = values.iter().skip(1).fold(first_elem.clone(), |max, elem| {
            if max < *elem {
                elem.clone()
            } else {
                max
            }
        });
        let mut float_values: Vec<f64> = values.iter().map(|x| x.clone().into()).collect();
        float_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mean: f64 = float_values.iter().fold(0.0, |a, b| a + b) / (float_values.len() as f64);
        let median: f64 = if float_values.len() % 2 == 1 {
            let pos = float_values.len() / 2;
            float_values[pos]
        } else {
            let pos = float_values.len() / 2;
            (float_values[pos] + float_values[pos + 1]) / 2.0
        };
        let std: f64 = (float_values
            .iter()
            .fold(0.0, |sum, x| sum + (x - mean) * (x - mean))
            / (float_values.len() as f64))
            .sqrt();

        Self {
            min,
            max,
            mean,
            median,
            std,
            values,
        }
    }
}

impl StatisticsResult<f64> {
    pub fn summary(&self, title: impl AsRef<str>) {
        println!(
            "{}: {:.4} +- {:.4} (min: {:.4}, median: {:.4}, max: {:.4})",
            title.as_ref(),
            self.mean,
            self.std,
            self.min,
            self.median,
            self.max
        );
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CostResult {
    pub scenario: TopoConfig,
    pub ideal_cost: f64,
    pub random_permutations: OptimizerResult,
    pub optimizer: OptimizerResult,
}

impl CostResult {
    pub fn summary_to_html(&self) -> String {
        let mut html = String::new();
        html.push_str(&format!(
            "<p>Ideal cost of the network: <b>{:.4}</b></p><br />",
            self.ideal_cost
        ));
        html.push_str("<table style=\"width: 100%\">\n");
        html.push_str(
            &self
                .random_permutations
                .summary_to_html("Random permutations"),
        );
        html.push_str("<tr></tr>\n");
        html.push_str(&self.optimizer.summary_to_html("Tree optimizer"));
        html.push_str("</table>\n");
        html
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProblemSeverityResult {
    pub scenario: TopoConfig,
    pub random_permutations: StrategySeverity,
    pub random_router_order: StrategySeverity,
    pub insert_before_order: StrategySeverity,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StrategySeverity {
    pub result: StrategyResult,
    pub total_severity: StatisticsResult<f64>,
    pub per_step_severity: StatisticsResult<f64>,
}

impl StrategySeverity {
    pub fn summary_to_html(&self) -> String {
        let mut html = String::new();
        html.push_str("<table style=\"width: 100%\">\n");
        html.push_str(&format!(
            "<tr><th>Success rate</th><td><b>{:.2}% ({} of {})</td></tr>",
            self.result.success_rate * 100.0,
            self.result.success,
            self.result.trials,
        ));
        html.push_str(&format!(
            "<tr><th>Total severity</th><td><b>{:.4}</b> +- {:.4} (min: {:.4}, median: {:.4}, max: {:.4})</td></tr>",
            self.total_severity.mean,
            self.total_severity.std,
            self.total_severity.min,
            self.total_severity.median,
            self.total_severity.max,
        ));
        html.push_str(&format!(
            "<tr><th>Per-step severity</th><td><b>{:.4}</b> +- {:.4} (min: {:.4}, median: {:.4}, max: {:.4})</td></tr>",
            self.per_step_severity.mean,
            self.per_step_severity.std,
            self.per_step_severity.min,
            self.per_step_severity.median,
            self.per_step_severity.max,
        ));
        html.push_str("</table>\n");
        html
    }
}
