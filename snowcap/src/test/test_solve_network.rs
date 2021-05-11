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

#![cfg(test)]
use crate::example_networks::repetitions::*;
use crate::example_networks::*;

use crate::hard_policies::*;
use crate::modifier_ordering::*;
use crate::netsim::printer;
use crate::permutators::*;
use crate::strategies::*;
use crate::{Error, Stopper};

use std::time::Duration;

fn test_net<S, N>(initial_variant: usize, final_variant: usize)
where
    S: Strategy,
    N: ExampleNetwork,
{
    eprintln!("initial variant: {}, final variant: {}", initial_variant, final_variant);
    let net = N::net(initial_variant);
    let ca = net.current_config();
    let cf = N::final_config(&net, final_variant);

    let mut modifiers = ca.get_diff(&cf).modifiers;

    let hard_policy =
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter());

    // create instance
    let result = S::synthesize(net, cf, hard_policy, Some(Duration::from_secs(60)), Stopper::new());

    assert!(result.is_ok());

    // check the sequence
    let sequence = result.unwrap();
    assert_eq!(sequence.len(), modifiers.len());

    // check that every modifier is present exactly once
    for m in sequence.iter() {
        let pos = modifiers.iter().position(|x| m == x).unwrap();
        modifiers.remove(pos);
    }
}

fn test_net_no_solution<S, N>(initial_variant: usize, final_variant: usize)
where
    S: Strategy,
    N: ExampleNetwork,
{
    let net = N::net(initial_variant);
    let net_cloned = net.clone();
    let cf = N::final_config(&net, final_variant);

    let hard_policy =
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter());

    // create instance
    let result = S::synthesize(net, cf, hard_policy, Some(Duration::from_secs(60)), Stopper::new());

    match result {
        Ok(r) => panic!(
            "Solution was found!\n{:#?}",
            r.iter()
                .map(|m| printer::config_modifier(&net_cloned, m).unwrap())
                .collect::<Vec<String>>()
        ),
        Err(Error::NoSafeOrdering) => {}
        Err(e) => panic!("Unexpected error: {}", e),
    }
}

fn test_net_bad_policy<S>()
where
    S: Strategy,
{
    let net = SimpleNet::net(0);
    let cf = SimpleNet::final_config(&net, 0);

    // Hard Policy: G false
    let hard_policy = HardPolicy::new(vec![], LTLModal::Globally(Box::new(false)));
    match S::synthesize(
        net.clone(),
        cf.clone(),
        hard_policy,
        Some(Duration::from_secs(60)),
        Stopper::new(),
    ) {
        Ok(r) => panic!(
            "Solution was found!\n{:#?}",
            r.iter().map(|m| printer::config_modifier(&net, m).unwrap()).collect::<Vec<String>>()
        ),
        Err(Error::InvalidInitialState) => {}
        Err(e) => panic!("Unexpected error: {}", e),
    }

    // Hard Policy: false M G reachability
    let tmp_policy =
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter());
    let hard_policy = HardPolicy::new(
        tmp_policy.prop_vars,
        LTLModal::StrongRelease(Box::new(false), Box::new(tmp_policy.expr)),
    );
    match S::synthesize(
        net.clone(),
        cf.clone(),
        hard_policy,
        Some(Duration::from_secs(10)),
        Stopper::new(),
    ) {
        Ok(r) => panic!(
            "Solution was found!\n{:#?}",
            r.iter().map(|m| printer::config_modifier(&net, m).unwrap()).collect::<Vec<String>>()
        ),
        Err(Error::NoSafeOrdering) | Err(Error::ProbablyNoSafeOrdering) => {}
        Err(e) => panic!("Unexpected error: {}", e),
    }
}

fn test_firewall_net<S>(variant: usize)
where
    S: Strategy,
{
    let net = FirewallNet::net(variant);
    let cf = FirewallNet::final_config(&net, variant);
    let hard_policy = FirewallNet::get_policy(&net, variant);
    assert!(
        S::synthesize(net, cf, hard_policy, Some(Duration::from_secs(60)), Stopper::new()).is_ok()
    );
}

#[test]
fn permutator_heaps_unordered() {
    test_net::<PermutationStrategy<HeapsPermutator<NoOrdering>>, SimpleNet>(0, 0);
    test_net::<PermutationStrategy<HeapsPermutator<NoOrdering>>, SimpleNet>(1, 0);
}

#[test]
fn permutator_heaps_random() {
    test_net::<PermutationStrategy<HeapsPermutator<RandomOrdering>>, SimpleNet>(0, 0);
    test_net::<PermutationStrategy<HeapsPermutator<RandomOrdering>>, SimpleNet>(1, 0);
}

#[test]
fn permutator_heaps_ordered() {
    test_net::<PermutationStrategy<HeapsPermutator<SimpleOrdering>>, SimpleNet>(0, 0);
    test_net::<PermutationStrategy<HeapsPermutator<SimpleOrdering>>, SimpleNet>(1, 0);
}

#[test]
fn permutator_heaps_reversed() {
    test_net::<PermutationStrategy<HeapsPermutator<SimpleReverseOrdering>>, SimpleNet>(0, 0);
    test_net::<PermutationStrategy<HeapsPermutator<SimpleReverseOrdering>>, SimpleNet>(1, 0);
}

#[test]
fn permutator_lexicographic() {
    test_net::<PermutationStrategy<LexicographicPermutator<SimpleOrdering>>, SimpleNet>(0, 0);
    test_net::<PermutationStrategy<LexicographicPermutator<SimpleOrdering>>, SimpleNet>(1, 0);
    test_net::<PermutationStrategy<LexicographicPermutator<SimpleOrdering>>, SmallNet>(0, 1);
    test_net::<PermutationStrategy<LexicographicPermutator<SimpleOrdering>>, SmallNet>(1, 1);
    test_net::<PermutationStrategy<LexicographicPermutator<SimpleOrdering>>, SmallNet>(2, 1);
    test_net::<PermutationStrategy<LexicographicPermutator<SimpleOrdering>>, MediumNet>(0, 0);
    test_net::<PermutationStrategy<LexicographicPermutator<SimpleOrdering>>, MediumNet>(0, 1);
    // this version does take way too long!
    //test_net::<PermutationStrategy<LexicographicPermutator<SimpleOrdering>>, MediumNet>(0, 2);
    //test_net::<PermutationStrategy<LexicographicPermutator<SimpleOrdering>>, MediumNet>(0, 3);
    test_net::<PermutationStrategy<LexicographicPermutator<SimpleOrdering>>, MediumNet>(1, 0);
    test_net::<PermutationStrategy<LexicographicPermutator<SimpleOrdering>>, MediumNet>(1, 1);
    test_net::<PermutationStrategy<LexicographicPermutator<SimpleOrdering>>, MediumNet>(1, 2);
    test_net::<PermutationStrategy<LexicographicPermutator<SimpleOrdering>>, MediumNet>(1, 3);
}

#[test]
fn permutator_lexicographic_reversed() {
    test_net::<PermutationStrategy<LexicographicPermutator<SimpleReverseOrdering>>, SimpleNet>(
        0, 0,
    );
    test_net::<PermutationStrategy<LexicographicPermutator<SimpleReverseOrdering>>, SimpleNet>(
        1, 0,
    );
}

#[test]
fn tree_random() {
    test_net::<TreeStrategy<RandomOrdering>, SimpleNet>(0, 0);
    test_net::<TreeStrategy<RandomOrdering>, SimpleNet>(1, 0);
    test_net::<TreeStrategy<RandomOrdering>, SmallNet>(0, 1);
    test_net::<TreeStrategy<RandomOrdering>, SmallNet>(1, 1);
    test_net::<TreeStrategy<RandomOrdering>, SmallNet>(2, 1);
    test_net::<TreeStrategy<RandomOrdering>, MediumNet>(0, 0);
    test_net::<TreeStrategy<RandomOrdering>, MediumNet>(0, 1);
    test_net::<TreeStrategy<RandomOrdering>, MediumNet>(0, 2);
    test_net::<TreeStrategy<RandomOrdering>, MediumNet>(0, 3);
    test_net::<TreeStrategy<RandomOrdering>, MediumNet>(1, 0);
    test_net::<TreeStrategy<RandomOrdering>, MediumNet>(1, 1);
    test_net::<TreeStrategy<RandomOrdering>, MediumNet>(1, 2);
    test_net::<TreeStrategy<RandomOrdering>, MediumNet>(1, 3);
}

#[test]
fn tree_ordered() {
    test_net::<TreeStrategy<SimpleOrdering>, SimpleNet>(0, 0);
    test_net::<TreeStrategy<SimpleOrdering>, SimpleNet>(1, 0);
    test_net::<TreeStrategy<SimpleOrdering>, SmallNet>(0, 1);
    test_net::<TreeStrategy<SimpleOrdering>, SmallNet>(1, 1);
    test_net::<TreeStrategy<SimpleOrdering>, SmallNet>(2, 1);
    test_net::<TreeStrategy<SimpleOrdering>, MediumNet>(0, 0);
    test_net::<TreeStrategy<SimpleOrdering>, MediumNet>(0, 1);
    test_net::<TreeStrategy<SimpleOrdering>, MediumNet>(0, 2);
    test_net::<TreeStrategy<SimpleOrdering>, MediumNet>(0, 3);
    test_net::<TreeStrategy<SimpleOrdering>, MediumNet>(1, 0);
    test_net::<TreeStrategy<SimpleOrdering>, MediumNet>(1, 1);
    test_net::<TreeStrategy<SimpleOrdering>, MediumNet>(1, 2);
    test_net::<TreeStrategy<SimpleOrdering>, MediumNet>(1, 3);
}

#[test]
fn tree_reversed() {
    test_net::<TreeStrategy<SimpleReverseOrdering>, SimpleNet>(0, 0);
    test_net::<TreeStrategy<SimpleReverseOrdering>, SimpleNet>(1, 0);
    test_net::<TreeStrategy<SimpleReverseOrdering>, SmallNet>(0, 1);
    test_net::<TreeStrategy<SimpleReverseOrdering>, SmallNet>(1, 1);
    test_net::<TreeStrategy<SimpleReverseOrdering>, SmallNet>(2, 1);
    test_net::<TreeStrategy<SimpleReverseOrdering>, MediumNet>(0, 0);
    test_net::<TreeStrategy<SimpleReverseOrdering>, MediumNet>(0, 1);
    test_net::<TreeStrategy<SimpleReverseOrdering>, MediumNet>(0, 2);
    test_net::<TreeStrategy<SimpleReverseOrdering>, MediumNet>(0, 3);
    test_net::<TreeStrategy<SimpleReverseOrdering>, MediumNet>(1, 0);
    test_net::<TreeStrategy<SimpleReverseOrdering>, MediumNet>(1, 1);
    test_net::<TreeStrategy<SimpleReverseOrdering>, MediumNet>(1, 2);
    test_net::<TreeStrategy<SimpleReverseOrdering>, MediumNet>(1, 3);
}

#[test]
fn pbtree_random() {
    test_net::<PushBackTreeStrategy<RandomOrdering>, SimpleNet>(0, 0);
    test_net::<PushBackTreeStrategy<RandomOrdering>, SimpleNet>(1, 0);
    test_net::<PushBackTreeStrategy<RandomOrdering>, SmallNet>(0, 1);
    test_net::<PushBackTreeStrategy<RandomOrdering>, SmallNet>(1, 1);
    test_net::<PushBackTreeStrategy<RandomOrdering>, SmallNet>(2, 1);
    test_net::<PushBackTreeStrategy<RandomOrdering>, MediumNet>(0, 0);
    test_net::<PushBackTreeStrategy<RandomOrdering>, MediumNet>(0, 1);
    test_net::<PushBackTreeStrategy<RandomOrdering>, MediumNet>(0, 2);
    test_net::<PushBackTreeStrategy<RandomOrdering>, MediumNet>(0, 3);
    test_net::<PushBackTreeStrategy<RandomOrdering>, MediumNet>(1, 0);
    test_net::<PushBackTreeStrategy<RandomOrdering>, MediumNet>(1, 1);
    test_net::<PushBackTreeStrategy<RandomOrdering>, MediumNet>(1, 2);
    test_net::<PushBackTreeStrategy<RandomOrdering>, MediumNet>(1, 3);
}

#[test]
fn pbtree_ordered() {
    test_net::<PushBackTreeStrategy<SimpleOrdering>, SimpleNet>(0, 0);
    test_net::<PushBackTreeStrategy<SimpleOrdering>, SimpleNet>(1, 0);
    test_net::<PushBackTreeStrategy<SimpleOrdering>, SmallNet>(0, 1);
    test_net::<PushBackTreeStrategy<SimpleOrdering>, SmallNet>(1, 1);
    test_net::<PushBackTreeStrategy<SimpleOrdering>, SmallNet>(2, 1);
    test_net::<PushBackTreeStrategy<SimpleOrdering>, MediumNet>(0, 0);
    test_net::<PushBackTreeStrategy<SimpleOrdering>, MediumNet>(0, 1);
    test_net::<PushBackTreeStrategy<SimpleOrdering>, MediumNet>(0, 2);
    test_net::<PushBackTreeStrategy<SimpleOrdering>, MediumNet>(0, 3);
    test_net::<PushBackTreeStrategy<SimpleOrdering>, MediumNet>(1, 0);
    test_net::<PushBackTreeStrategy<SimpleOrdering>, MediumNet>(1, 1);
    test_net::<PushBackTreeStrategy<SimpleOrdering>, MediumNet>(1, 2);
    test_net::<PushBackTreeStrategy<SimpleOrdering>, MediumNet>(1, 3);
}

#[test]
fn pbtree_reversed() {
    test_net::<PushBackTreeStrategy<SimpleReverseOrdering>, SimpleNet>(0, 0);
    test_net::<PushBackTreeStrategy<SimpleReverseOrdering>, SimpleNet>(1, 0);
    test_net::<PushBackTreeStrategy<SimpleReverseOrdering>, SmallNet>(0, 1);
    test_net::<PushBackTreeStrategy<SimpleReverseOrdering>, SmallNet>(1, 1);
    test_net::<PushBackTreeStrategy<SimpleReverseOrdering>, SmallNet>(2, 1);
    test_net::<PushBackTreeStrategy<SimpleReverseOrdering>, MediumNet>(0, 0);
    test_net::<PushBackTreeStrategy<SimpleReverseOrdering>, MediumNet>(0, 1);
    test_net::<PushBackTreeStrategy<SimpleReverseOrdering>, MediumNet>(0, 2);
    test_net::<PushBackTreeStrategy<SimpleReverseOrdering>, MediumNet>(0, 3);
    test_net::<PushBackTreeStrategy<SimpleReverseOrdering>, MediumNet>(1, 0);
    test_net::<PushBackTreeStrategy<SimpleReverseOrdering>, MediumNet>(1, 1);
    test_net::<PushBackTreeStrategy<SimpleReverseOrdering>, MediumNet>(1, 2);
    test_net::<PushBackTreeStrategy<SimpleReverseOrdering>, MediumNet>(1, 3);
}

#[test]
fn dep_groups_builder() {
    test_net::<DepGroupsStrategy, SimpleNet>(0, 0);
    test_net::<DepGroupsStrategy, SimpleNet>(1, 0);
    test_net::<DepGroupsStrategy, SmallNet>(0, 1);
    test_net::<DepGroupsStrategy, DifficultGadgetRepeated<Repetition1>>(0, 0);
    test_net::<DepGroupsStrategy, DifficultGadgetRepeated<Repetition2>>(0, 0);
    test_net::<DepGroupsStrategy, DifficultGadgetRepeated<Repetition3>>(0, 0);
}

#[test]
fn one_strategy_to_rule_them_all() {
    test_net::<StrategyTRTA, SimpleNet>(0, 0);
    test_net::<StrategyTRTA, SimpleNet>(1, 0);
    test_net::<StrategyTRTA, SmallNet>(0, 1);
    test_net::<StrategyTRTA, SmallNet>(1, 1);
    test_net::<StrategyTRTA, SmallNet>(2, 1);
    test_net::<StrategyTRTA, MediumNet>(0, 0);
    test_net::<StrategyTRTA, MediumNet>(0, 1);
    test_net::<StrategyTRTA, MediumNet>(0, 2);
    test_net::<StrategyTRTA, MediumNet>(0, 3);
    test_net::<StrategyTRTA, MediumNet>(1, 0);
    test_net::<StrategyTRTA, MediumNet>(1, 1);
    test_net::<StrategyTRTA, MediumNet>(1, 2);
    test_net::<StrategyTRTA, MediumNet>(1, 3);
    test_net::<StrategyTRTA, DifficultGadgetRepeated<Repetition1>>(0, 0);
    test_net::<StrategyTRTA, DifficultGadgetRepeated<Repetition2>>(0, 0);
    test_net::<StrategyTRTA, DifficultGadgetRepeated<Repetition3>>(0, 0);
}

#[test]
fn firewall_net() {
    for variant in vec![0, 1] {
        test_firewall_net::<TreeStrategy<RandomOrdering>>(variant);
        test_firewall_net::<TreeStrategy<SimpleOrdering>>(variant);
        test_firewall_net::<TreeStrategy<SimpleReverseOrdering>>(variant);
        test_firewall_net::<PushBackTreeStrategy<RandomOrdering>>(variant);
        test_firewall_net::<PushBackTreeStrategy<SimpleOrdering>>(variant);
        test_firewall_net::<PushBackTreeStrategy<SimpleReverseOrdering>>(variant);
        test_firewall_net::<DepGroupsStrategy>(variant);
        test_firewall_net::<StrategyTRTA>(variant);
    }
}

#[test]
fn carousel_gadget() {
    test_net_no_solution::<PermutationStrategy<HeapsPermutator<RandomOrdering>>, CarouselGadget>(
        0, 0,
    );
    test_net_no_solution::<PermutationStrategy<TreePermutator<RandomOrdering, _>>, CarouselGadget>(
        0, 0,
    );
    test_net_no_solution::<TreeStrategy<SimpleOrdering>, CarouselGadget>(0, 0);
    test_net_no_solution::<PushBackTreeStrategy<SimpleOrdering>, CarouselGadget>(0, 0);
    test_net_no_solution::<DepGroupsStrategy, CarouselGadget>(0, 0);
    //test_net_no_solution::<StrategyTRTA, CarouselGadget>(0, 0);
}

#[test]
fn evil_twin_gadget() {
    //test_net_no_solution::<PushBackTreeStrategy<SimpleOrdering>, EvilTwinGadget>(0, 0);
    // It seems like this actually has a solution if the link weights can be changed inm a
    // non-symmetric way.
    test_net::<PushBackTreeStrategy<SimpleOrdering>, EvilTwinGadget>(0, 0);
}

#[test]
fn impossible_constraint() {
    eprintln!("Permutation (Heaps)");
    test_net_bad_policy::<PermutationStrategy<HeapsPermutator<RandomOrdering>>>();
    eprintln!("Permutation (Tree)");
    test_net_bad_policy::<PermutationStrategy<TreePermutator<RandomOrdering, _>>>();
    eprintln!("Tree");
    test_net_bad_policy::<TreeStrategy<RandomOrdering>>();
    eprintln!("Push-Back Tree");
    test_net_bad_policy::<PushBackTreeStrategy<RandomOrdering>>();
    eprintln!("DepGroups");
    test_net_bad_policy::<DepGroupsStrategy>();
    eprintln!("StrategyTRTA");
    test_net_bad_policy::<StrategyTRTA>();
}
