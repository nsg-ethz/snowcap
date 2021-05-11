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

use snowcap::example_networks::{self, ExampleNetwork};
use snowcap::hard_policies::*;
use snowcap::netsim::{
    config::{ConfigExpr, ConfigModifier},
    Network, NetworkError, Prefix,
};
use snowcap::topology_zoo::*;

use core::ops::AddAssign;
use std::error::Error;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{spawn, JoinHandle};

pub fn transient_violation(n_iter: usize, variant: usize) -> Result<(), Box<dyn Error>> {
    assert!(variant == 0 || variant == 1);

    // get the abilene network
    let net = example_networks::AbileneNetwork::net(variant);

    let sv = net.get_router_id("Sunnyvale").unwrap();
    let se = net.get_router_id("Seattle").unwrap();
    let dv = net.get_router_id("Denver").unwrap();
    let la = net.get_router_id("Los Angeles").unwrap();
    let hs = net.get_router_id("Huston").unwrap();
    let ks = net.get_router_id("Kansas City").unwrap();
    let ip = net.get_router_id("Indianapolis").unwrap();
    let at = net.get_router_id("Atlanta").unwrap();
    let dc = net.get_router_id("Washington DC").unwrap();
    let ny = net.get_router_id("New York").unwrap();
    let ch = net.get_router_id("Chicago").unwrap();

    let p = Prefix(0);

    let commands = [ConfigModifier::Update {
        from: ConfigExpr::IgpLinkWeight {
            source: dv,
            target: sv,
            weight: 10.0,
        },
        to: ConfigExpr::IgpLinkWeight {
            source: dv,
            target: sv,
            weight: 100.0,
        },
    }];

    for &router in &[se, dv, ks, ip, ch, ny, dc, at, hs] {
        println!("- Router: {}", net.get_router_name(router)?);

        let path_cond = PathCondition::Or(vec![
            PathCondition::Edge(dv, sv),
            PathCondition::Edge(hs, la),
        ]);

        let transient_conds = [Condition::Reachable(router, p, Some(path_cond.clone()))];
        let policy =
            HardPolicy::globally(vec![Condition::TransientPath(router, p, path_cond.clone())]);

        perform_sequence_check_condition(&net, &commands, &policy, &transient_conds, n_iter)?;
    }

    Ok(())
}

pub fn transient_violation_topologyzoo(
    gml_file: String,
    seed: u64,
    n_seeds: usize,
    n_iter: usize,
    n_threads: Option<usize>,
    reverse: bool,
) -> Result<(), Box<dyn Error>> {
    let n_threads = n_threads.unwrap_or_else(|| num_cpus::get());

    let current_seed = Arc::new(Mutex::new(seed));
    let remaining_seeds = Arc::new(Mutex::new(n_seeds));

    let (sender, receiver) = channel::<SeedResult>();

    let mut result = SeedResult::default();

    let workers: Vec<JoinHandle<_>> = (0..n_threads)
        .map(|_| {
            let tx = sender.clone();
            let s = current_seed.clone();
            let r = remaining_seeds.clone();
            let file = gml_file.clone();
            spawn(move || transient_violation_topologyzoo_thread(reverse, file, n_iter, s, r, tx))
        })
        .collect();

    for _ in 0..n_seeds {
        let sample = receiver.recv()?;
        result += sample;
    }

    workers.into_iter().for_each(|w| w.join().unwrap());

    let total = result.total() as f64;

    println!("\nResults:");
    println!(
        "    True Positive:  {} ({:.2}%)",
        result.true_positive,
        (result.true_positive as f64) / total * 100.0
    );
    println!(
        "    True Negative:  {} ({:.2}%)",
        result.true_negative,
        (result.true_negative as f64) / total * 100.0
    );
    println!(
        "    False Positive: {} ({:.2}%)",
        result.false_positive,
        (result.false_positive as f64) / total * 100.0
    );
    println!(
        "    False Negative: {} ({:.2}%)",
        result.false_negative,
        (result.false_negative as f64) / total * 100.0
    );

    Ok(())
}

fn transient_violation_topologyzoo_thread(
    reverse: bool,
    gml_file: String,
    n_iter: usize,
    seed: Arc<Mutex<u64>>,
    remaining: Arc<Mutex<usize>>,
    sender: Sender<SeedResult>,
) -> () {
    'job_loop: loop {
        // check if there is something remaining to do
        let rem = {
            let mut rem_lock = remaining.lock().unwrap();
            let r = *rem_lock;
            if *rem_lock > 0 {
                *rem_lock -= 1;
            } else {
                return;
            }
            r
        };

        // loop until we get a good seed
        'seed_loop: loop {
            let seed = {
                let mut seed_lock = seed.lock().unwrap();
                let s = *seed_lock;
                *seed_lock += 1;
                s
            };

            println!("Seed {} ({} remaining)", seed, rem);

            // generate the topology
            let mut topo = ZooTopology::new(&gml_file, seed).unwrap();
            let net = topo.get_net();
            let (net, final_config, _) = topo
                .apply_transient_condition_scenario(
                    net,
                    100,
                    reverse,
                    Some(("BelWue", "TIX", "GEANT2")),
                )
                .unwrap();

            let mut final_net = net.clone();
            final_net.set_config(&final_config).unwrap();

            let init_fws = net.get_forwarding_state();
            let final_fws = final_net.get_forwarding_state();
            let external_routers = net.get_external_routers();

            let p = Prefix(0);
            let commands = net.current_config().get_diff(&final_config).modifiers;

            let mut result = SeedResult::default();

            'router_loop: for node in net.get_routers() {
                let nh_before = init_fws.get_next_hop(node, p).unwrap().unwrap();
                let nh_after = final_fws.get_next_hop(node, p).unwrap().unwrap();

                // skip routers that are directly connected with one of the external routers.
                if external_routers.contains(&nh_before) || external_routers.contains(&nh_after) {
                    continue 'router_loop;
                }

                // for all others, generate the transient policy
                let path_cond = PathCondition::Or(vec![
                    PathCondition::Edge(node, nh_before),
                    PathCondition::Edge(node, nh_after),
                ]);

                let transient_conds = [Condition::Reachable(node, p, Some(path_cond.clone()))];
                let policy = HardPolicy::globally(vec![Condition::TransientPath(
                    node,
                    p,
                    path_cond.clone(),
                )]);

                match perform_sequence_check_condition(
                    &net,
                    &commands,
                    &policy,
                    &transient_conds,
                    n_iter,
                )
                .unwrap()
                {
                    ConditionResult::TruePositive => result.true_positive += 1,
                    ConditionResult::TrueNegative(_) => result.true_negative += 1,
                    ConditionResult::FalsePositive(_) => result.false_positive += 1,
                    ConditionResult::FalseNegative => result.false_negative += 1,
                    ConditionResult::NothingToReorder => continue 'seed_loop,
                }
            }

            sender.send(result).unwrap();
            continue 'job_loop;
        }
    }
}

fn perform_sequence_check_condition(
    net: &Network,
    modifiers: &[ConfigModifier],
    policy: &HardPolicy,
    transient_conds: &[Condition],
    n_iter: usize,
) -> Result<ConditionResult, Box<dyn Error>> {
    let mut policy = policy.clone();
    let mut net = net.clone();

    policy.set_num_mods_if_none(2);
    let mut fw_state = net.get_forwarding_state();
    policy.step(&mut net, &mut fw_state)?;

    assert_eq!(modifiers.len(), 1);

    let command = modifiers.get(0).unwrap();

    let num_correct = match net.apply_modifier_check_transient(&command, transient_conds, n_iter) {
        Ok(n) => n,
        Err(NetworkError::NoEventsToReorder) => return Ok(ConditionResult::NothingToReorder),
        Err(e) => return Err(e.into()),
    };
    let fail_prob = (1.0 - (num_correct as f64 / n_iter as f64)) * 100.0;

    let mut fw_state = net.get_forwarding_state();
    policy.step(&mut net, &mut fw_state)?;
    if policy.check() {
        if num_correct != n_iter {
            println!("!!!ERROR!!!");
            Ok(ConditionResult::FalsePositive(fail_prob))
        } else {
            Ok(ConditionResult::TruePositive)
        }
    } else {
        if num_correct != n_iter {
            Ok(ConditionResult::TrueNegative(fail_prob))
        } else {
            Ok(ConditionResult::FalseNegative)
        }
    }
}

enum ConditionResult {
    TruePositive,
    TrueNegative(f64),
    FalsePositive(f64),
    FalseNegative,
    NothingToReorder,
}

#[derive(Clone, PartialEq, Eq, Default)]
struct SeedResult {
    pub true_positive: usize,
    pub true_negative: usize,
    pub false_positive: usize,
    pub false_negative: usize,
}

impl AddAssign for SeedResult {
    fn add_assign(&mut self, other: Self) {
        *self = Self {
            true_positive: self.true_positive + other.true_positive,
            true_negative: self.true_negative + other.true_negative,
            false_positive: self.false_positive + other.false_positive,
            false_negative: self.false_negative + other.false_negative,
        }
    }
}

impl SeedResult {
    pub fn total(&self) -> usize {
        self.true_positive + self.true_negative + self.false_positive + self.false_negative
    }
}
