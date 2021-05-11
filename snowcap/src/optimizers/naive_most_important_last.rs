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

#![cfg(feature = "strawman-strategies")]
//! Strawman Strategy

use super::Optimizer;
use crate::hard_policies::HardPolicy;
use crate::netsim::{
    config::{ConfigExprKey, ConfigModifier},
    Network, RouterId,
};
use crate::soft_policies::SoftPolicy;
use crate::{Error, Stopper};

use log::*;
use rand::prelude::*;
use std::time::Duration;

/// # Strawman Strategy: Most Important Last
///
/// *This strategy is only available if the `"strawman-strategies"` feature is enabled!*
///
/// This strategy orders the modifiers based on their importance. It works as follows:
///
/// 1. If the modifier is changing a link weight, then its weight will be the number of flows that
///    traverse this link (in the given direction).
/// 2. If the modifier is changing the route map of a router, or modify a static route, then its
///    weight will be the number of flows that traverse this router.
/// 3. If the modifier is changing a BGP session, then the its weight will be the number of flows
///    that traverse at least one of the two routers.
///
/// Then, for each of these three different cost types, they are normalized such that 1 is the
/// highest cost that occurs in this group. Finally, all modifiers are sorted based on their cost,
/// in ascending order.
///
/// **Warning**: This strategy only checks one single ordering, and aborts afterwards!
pub struct NaiveMostImportantLast<P: SoftPolicy + Clone> {
    net: Network,
    modifiers: Vec<ConfigModifier>,
    hard_policy: HardPolicy,
    soft_policy: P,
}

impl<P: SoftPolicy + Clone> Optimizer<P> for NaiveMostImportantLast<P> {
    fn new(
        mut net: Network,
        modifiers: Vec<ConfigModifier>,
        mut hard_policy: HardPolicy,
        soft_policy: P,
        _time_budget: Option<Duration>,
    ) -> Result<Box<Self>, Error> {
        let mut fw_state = net.get_forwarding_state();
        hard_policy.set_num_mods_if_none(modifiers.len());
        hard_policy.step(&mut net, &mut fw_state)?;
        if !hard_policy.check() {
            error!(
                "Initial state errors: \n    {}",
                hard_policy
                    .last_errors()
                    .into_iter()
                    .map(|e| e.repr_with_name(&net))
                    .collect::<Vec<_>>()
                    .join("\n    "),
            );
            return Err(Error::InvalidInitialState);
        }
        Ok(Box::new(Self { net, modifiers, hard_policy, soft_policy }))
    }

    fn work(&mut self, _abort: Stopper) -> Result<(Vec<ConfigModifier>, f64), Error> {
        let flows =
            self.net.get_forwarding_state().into_iter().map(|(_, _, p)| p).collect::<Vec<_>>();

        // sort the sequence
        let weights = self.modifiers.iter().map(|m| modifier_cost(m, &flows)).collect::<Vec<_>>();
        let max_link = weights
            .iter()
            .filter(|(g, _)| *g == ModifierGroup::Link)
            .map(|(_, c)| c)
            .max()
            .cloned()
            .unwrap_or(1);
        let max_pair = weights
            .iter()
            .filter(|(g, _)| *g == ModifierGroup::Pair)
            .map(|(_, c)| c)
            .max()
            .cloned()
            .unwrap_or(1);
        let max_node = weights
            .iter()
            .filter(|(g, _)| *g == ModifierGroup::Node)
            .map(|(_, c)| c)
            .max()
            .cloned()
            .unwrap_or(1);

        let mut weighted_sequence = self
            .modifiers
            .clone()
            .into_iter()
            .zip(weights.into_iter().map(|(g, x)| g.scale(x, max_link, max_pair, max_node)))
            .collect::<Vec<_>>();

        weighted_sequence.shuffle(&mut thread_rng());
        weighted_sequence.sort_by(|(_, a), (_, b)| a.cmp(b));
        let order = weighted_sequence.into_iter().map(|(m, _)| m).collect::<Vec<_>>();

        // check the sequence
        match self.check_sequence(&order) {
            Some(cost) => Ok((order, cost)),
            None => Err(Error::ProbablyNoSafeOrdering),
        }
    }

    #[cfg(feature = "count-states")]
    fn num_states(&self) -> usize {
        self.modifiers.len()
    }
}

impl<P: SoftPolicy + Clone> NaiveMostImportantLast<P> {
    fn check_sequence(&self, patch_seq: &[ConfigModifier]) -> Option<f64> {
        let mut net = self.net.clone();
        let mut hard_policy = self.hard_policy.clone();
        let mut soft_policy = self.soft_policy.clone();

        let mut cost = 0.0;

        // apply every step in sequence
        for modifier in patch_seq.iter() {
            net.apply_modifier(modifier).ok()?;
            let mut fw_state = net.get_forwarding_state();
            hard_policy.step(&mut net, &mut fw_state).ok()?;
            if !hard_policy.check() {
                return None;
            }
            soft_policy.update(&mut fw_state, &net);
            cost += soft_policy.cost();
        }

        Some(cost)
    }
}

fn modifier_cost(m: &ConfigModifier, flows: &[Vec<RouterId>]) -> (ModifierGroup, usize) {
    match m.key() {
        ConfigExprKey::IgpLinkWeight { source, target } => (
            ModifierGroup::Link,
            flows.iter().filter(|f| path_contains_edge(source, target, f).is_some()).count(),
        ),
        ConfigExprKey::BgpSession { speaker_a, speaker_b } => (
            ModifierGroup::Pair,
            flows.iter().filter(|f| f.contains(&speaker_a) || f.contains(&speaker_b)).count(),
        ),
        ConfigExprKey::BgpRouteMap { router, .. } | ConfigExprKey::StaticRoute { router, .. } => {
            (ModifierGroup::Node, flows.iter().filter(|f| f.contains(&router)).count())
        }
    }
}

fn path_contains_edge(a: RouterId, b: RouterId, path: &[RouterId]) -> Option<usize> {
    let mut i = path.iter().enumerate().peekable();
    loop {
        let (pos, x) = i.next()?;
        let y = i.peek()?.1;
        if *x == a && *y == b {
            return Some(pos);
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum ModifierGroup {
    Link,
    Node,
    Pair,
}

impl ModifierGroup {
    fn scale(&self, x: usize, max_link: usize, max_pair: usize, max_node: usize) -> u64 {
        match self {
            ModifierGroup::Link => ((x as f64) / (max_link as f64) * (u64::MAX as f64)) as u64,
            ModifierGroup::Pair => ((x as f64) / (max_pair as f64) * (u64::MAX as f64)) as u64,
            ModifierGroup::Node => ((x as f64) / (max_node as f64) * (u64::MAX as f64)) as u64,
        }
    }
}
