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

#![deny(missing_docs)]

//! # Snowcap: Synthesizing NetwOrk-Wide ConfigurAtion uPdates
//! This is a library for comparing two network configurations and generating a sequence of small
//! configuration updates, which can be applied without producing anomalies.
//!
//! This library was created during the Master Thesis: "Synthesizing Network-Wide Configuration
//! Updates" by Tibor Schneider, supervised by Laurent Vanbever and Rüdiker Birkener.
//!
//! ## Problem Statement
//! Given
//! - an initial network configuration $A$ and a target configuration $B$,
//! - a set of hard-constraints, which are satisfied by both configuration $A$ and $B$
//! - a set of soft-constraints
//!
//! find an ordered set of configuration changes, such that the hard-constraints are satisfied
//! throughout the entire reconfiguration process, and the soft-constraints are optimized.
//!
//! ## Structure
//!
//! This library is structured in the following way:
//!
//! - **[`NetSim`](netsim)**: Network Simulator used in this project. See the main structure
//!   [`Network`](netsim::Network).
//!
//! - **[`Strategies`](strategies)**: Collection of different strategies, which `Snowcap` shold
//!   use. Strategies can be described using different [`Permutators`](permutators::Permutator),
//!   [`ModifierOrderings`](modifier_ordering::ModifierOrdering), or even other
//!   [`Strategies`](strategies::Strategy). Strategies can either be exhaustive (when they implement
//!   [`ExhaustiveStrategy`](strategies::ExhaustiveStrategy)), or non-exhaustive (when they don't
//!   implemeht that trait). Also, strategies can implement
//!   [`GroupStrategy`](strategies::GroupStrategy), which means that the strategy can use group
//!   information which was extracted previously.
//!
//! - **[`Optimizers`](optimizers)**: Collection of different optimization strategies for
//!   synthesizing network updates, while always requiring the hard policies to hold, and the cost of
//!   the soft policies to be minimized. Optimizers are very similar to strategies. They can also
//!   implement `ExhaustiveStrategy`, to tell that a given optimizer will check every possible
//!   combination.
//!
//! - **[`Permutators`](permutators)**: Collection of different ways to iterate over all possible
//!   permutations of reconfiguration orderings, or over all possible permutations of any vector.
//!   Usually, they take a type argument [`ModifierOrdering`](modifier_ordering::ModifierOrdering),
//!   to configure how they are ordered. All `Permutators` return every possible permutation exactly
//!   once, except in the following case: Some permutators have the possibility of getting feedback
//!   from the strategy. The idea is that permutations don't need to be checked if an earlier
//!   permutation with the same beginning already failed at a position where both permutations are
//!   still the same (they only differ after the failed position). In this case, the permutator may
//!   skip all these permutations, resulting in a reduced number of permutations to try.
//!
//! - **[`ModifierOrdering`](modifier_ordering)**: Collection of different orerings for modifiers.
//!   Some of these orderings are [`CompleteOrderings`](modifier_ordering::CompleteOrdering), which
//!   means that it only returns `Ordering::Equal` for modifiers which are actually equal. This
//!   ordering is used for several strategies, and permutators. Note, that there also exist
//!   `ModifierOrderings` which do not only apply to
//!   [`ConfigModifiers`](netsim::config::ConfigModifier), but also to any other type which can be
//!   ordered.
//!
//! - **[`HardPolicy`](hard_policies)**: Collection of [Conditions](hard_policies::Condition), which
//!   can be combined with a [LTL Formula](hard_policies::LTLModal) to generate a
//!   [Hard Policy](hard_policies::HardPolicy) for checking the policies while exploring the problem
//!   space.
//!
//! - **[`SoftPolicy`](soft_policies)**: Collection of different soft policies which allows the
//!   synthetisized configuration update to minimize a cost function.
//!
//! - **[`ExampleNetworks`](example_networks)**: Collection of prepared networks and reconfiguration
//!   scenarios to test different strategies. Some of these networks can be scaled to arbitrary
//!   size.
//!
//! - **[`TopologyZoo`](topology_zoo::ZooTopology)**: Functions to generate a network from a
//!   topology downloaded from [TopologyZoo](http://www.topology-zoo.org/dataset.html) (as `GML`
//!   files). The configuration can be generated randomly.
//!
//! ## Features
//!
//! - *`count-states`*: If this feature is enabled, then [strategies](strategies::Strategy) and
//!   [optimizers](optimizers::Optimizer) will contain the method `num_states`, to get the number
//!   of network states that have been explored.
//!
//! ## Usage
//!
//! To use this module, you need to do first prepare your [network](netsim::Network) to
//! include all routers, the initial configuration, and you need to make sure that all routes are
//! advertised by some routers. Then, you need to prepare the final configuration. Next, express
//! your hard policies as [constraints](hard_policies::Condition), and choose your
//! [strategy](strategies). Finally, call the `synthesize` method on the strategy.
//!
//! ```
//! use snowcap::hard_policies::*;
//! use snowcap::synthesize;
//! use snowcap::Error;
//! use snowcap::netsim::Network;
//! use snowcap::netsim::config::Config;
//! # use snowcap::example_networks::*;
//!
//! fn main() -> Result<(), Error> {
//!     // prepare the network
//!     // let net = ...
//!     // let initial_config = ...
//!     // let final_config = ...
//! # let net = SimpleNet::net(0);
//! # let initial_config = net.current_config().clone();
//! # let final_config = SimpleNet::final_config(&net, 0);
//!
//!     // prepare the policies
//!     // let hard_policy = ...
//! # let hard_policy = HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter());
//!
//!     // synthesize the reconfiguration
//!     let sequence = synthesize(net, initial_config, final_config, hard_policy, None)?;
//!
//!     // Do something with the result
//!     println!("{:#?}", sequence);
//!
//!     Ok(())
//! }
//! ```
// test modules
pub mod example_networks;
mod test;
pub mod topology_zoo;

mod dep_groups;
mod error;
pub mod hard_policies;
pub mod modifier_ordering;
pub mod netsim;
pub mod optimizers;
pub mod permutators;
pub mod soft_policies;
//pub mod static_analysis;
pub mod strategies;
// TODO needs fixing
//pub mod transient_behavior;

mod synthesize;
pub use synthesize::{optimize, synthesize, synthesize_parallel};

pub use error::Error;

use std::sync::{Arc, RwLock};

/// Stopper, to check when to stop, or to send the stop command
#[derive(Clone, Debug)]
pub struct Stopper {
    b: Arc<RwLock<bool>>,
    c: usize,
}

impl Default for Stopper {
    fn default() -> Self {
        Self::new()
    }
}

impl Stopper {
    /// Create a new stopper
    pub fn new() -> Self {
        Self { b: Arc::new(RwLock::new(false)), c: 0 }
    }

    /// Send the stop command. This function will block until the write lock can be acquired.
    pub fn send_stop(&self) {
        *self.b.write().unwrap() = true;
    }

    /// Checks if the stop flag is set. This funciton will not block, just continue if the
    /// read-lock cannot be acquired.
    pub fn try_is_stop(&mut self) -> Option<bool> {
        self.c += 1;
        if self.c >= 9 {
            self.c = 0;
            self.b.try_read().map(|x| *x).ok()
        } else {
            None
        }
    }

    /// Checks if the stop flag is set. This funciton will block until the read lock can be
    /// acquired.
    pub fn is_stop(&self) -> bool {
        *self.b.read().unwrap()
    }
}
