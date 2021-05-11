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

//! # Optimizer
//!
//! Optimizers try to solve the problem of reconfiguraiton, by always requiring the (hard) policies
//! to hold, while trying to minimize the cost of soft policies. The following optimizers exist:
//!
//! - **One Optimizer To Rule Them All** ([`OptimizerTRTA`]): This optimizer combines the best of
//!   both the simple, and soft-policy oriented [`TreeOptimizer`], and the more complex, hard-policy
//!   based [`DepGroupsOptimizer`]. It does this by exploring the search space similar to the tree
//!   optimizer. But as soon as we would need to backtrace, we try to find a single dependency. If
//!   it succeeds, we repeat the tree traversal using the new dependency. This results in much
//!   better results interms of minimizing cost, while keeping the benefits of searching actively
//!   for dependencies. Additionally, we only search for the dependencies, which are not solvable by
//!   using the naive Tree strategy.
//!
//! - **[`TreeOptimizer`]**: This optimizer is similar to the
//!   [`TreeStrategy`](crate::strategies::TreeStrategy). However, the most important difference is
//!   that on ever step, the tree is traversed into the direction, which will (locally) keep the
//!   cost of the soft policy to a minimum. This means, that before any modifier is taken, this
//!   strategy will try all possible modifiers, and check which of them will reduce the cost
//!   locally.
//!
//! - **[`GlobalOptimizer`]**: This optimizer computes the cost of every possible ordering, which is
//!   valid under the given hard constraints. Then, it returns the one which minimizes the cost.
//!   This optimizer will always return the global minimum, however, it is no longer feasible to
//!   compute with 10 or more modifiers.
//!
//! - **[`DepGroupsOptimizer`]**: This optimizer is similar to the
//!   [`DebGroupsStrategy`](crate::strategies::DepGroupsStrategy), as it searches for dependencies
//!   actively by building groups. Once a valid solution is found, we store the ordering and the
//!   cost, and continue the iteration, until we have either not improved our solution in the last
//!   10 iterations, or until we have exceeded the time budget.
//!
//! - **[`NaiveRandomOptimizer`]**: This optimizer is only used for evaluation purpose. It simply
//!   tries random orderings, until it finds a valid ordering, which will then be returned.
//!
//! - **[`NaiveRandomIBROptimizer`]**: This optimizer is only used for evaluation purpose. It simply
//!   tries random orderings, until it finds a valid ordering, which will then be returned. However,
//!   the sequence will always first insert, then modify, and finally, remove configuration.

mod tree;
pub use tree::TreeOptimizer;

mod global;
pub use global::GlobalOptimizer;

mod naive_random;
pub use naive_random::NaiveRandomOptimizer;

mod naive_random_ibr;
pub use naive_random_ibr::NaiveRandomIBROptimizer;

mod naive_most_important_first;
#[cfg(feature = "strawman-strategies")]
pub use naive_most_important_first::NaiveMostImportantFirst;

mod naive_most_important_last;
#[cfg(feature = "strawman-strategies")]
pub use naive_most_important_last::NaiveMostImportantLast;

pub use crate::dep_groups::optimizer::DepGroupsOptimizer;
pub use crate::dep_groups::optimizer_trta::OptimizerTRTA;

use crate::hard_policies::HardPolicy;
use crate::netsim::config::{Config, ConfigModifier};
use crate::netsim::Network;
use crate::soft_policies::SoftPolicy;
use crate::{Error, Stopper};

use std::time::Duration;

use log::*;

/// Infterface for all Optimizers
pub trait Optimizer<P>
where
    P: SoftPolicy,
{
    /// Wrapper, that creates the optimizer and synthesizes the network update order.
    fn synthesize(
        net: Network,
        end_config: Config,
        hard_policy: HardPolicy,
        soft_policy: P,
        time_budget: Option<Duration>,
        abort: Stopper,
    ) -> Result<(Vec<ConfigModifier>, f64), Error> {
        let start_config = net.current_config().clone();
        let patch = start_config.get_diff(&end_config);
        let modifiers: Vec<ConfigModifier> = patch.modifiers;
        let mut optimizer = match Self::new(net, modifiers, hard_policy, soft_policy, time_budget) {
            Ok(o) => {
                info!("Initial configuration is valid!");
                o
            }
            Err(Error::InvalidInitialState) => {
                error!("Invalid initial state");
                return Err(Error::InvalidInitialState);
            }
            Err(e) => {
                error!("Unexpected error while setting up the strategy: {}", e);
                return Err(e);
            }
        };
        optimizer.work(abort)
    }

    /// Create the strategy
    fn new(
        net: Network,
        modifiers: Vec<ConfigModifier>,
        hard_policy: HardPolicy,
        soft_policy: P,
        time_budget: Option<Duration>,
    ) -> Result<Box<Self>, Error>;

    /// Main function to find a valid reconfiguration sequence (if it exists) and return it.
    /// The function also returns the cost of the sequence.
    fn work(&mut self, abort: Stopper) -> Result<(Vec<ConfigModifier>, f64), Error>;
    /// Returns the number of states explored by the strategy.
    ///
    /// *This method is only available if the `"count-states"` feature is enabled!*
    #[cfg(feature = "count-states")]
    fn num_states(&self) -> usize;
}
