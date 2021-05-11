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

//! # Strategies
//!
//! This module contains the source codes for different strategies for finding a valid update
//! sequence. It contains the trait definition for `Strategy`, which `Snowcap` will use.
//!
//! ## Modifier Dependency Properties
//!
//! - **Statically Determinable**: If a dependency can be identified using domain-specific
//!   knowledge, without simulating it, it is called *statically determinable*.
//!
//!   *Example*: A router has a single BGP session before reconfiguration, and a singe, but
//!   different session after reconfiguration, the new session must be added before the old one can
//!   be removed.
//!
//! - **Immediate Effect**: If the order of the dependency is incorrect, and the problem is visible
//!   immediately after the wrong modifier is applied, then it has an *immediate effect*. This
//!   property must hold for all possible orderings, in order for a dependency to have an
//!   *immediate effect*.
//!
//!   *Example*: [`ChainGadget`](crate::example_networks::ChainGadget) has an *immediate effect*,
//!   since every wrong modifier will immediately cause a forwarding loop. The
//!   [`DifficultGadget`](crate::example_networks::DifficultGadgetMinimal) has *no immediate*
//!   *effect*, since the problem only arises when a specific modifier is applied.
//!
//! - **Sparse Solution**: A modifier dependency has *sparse solutions* if the number of correct
//!   solutions is very small compared to the total possible orderings of this dependency. If
//!   there exists one or two valid sequences, we call this dependency to have a *sparse solution*.
//!
//!   *Example*: [`ChainGadget`](crate::example_networks::ChainGadget) has a *sparse solution*,
//!   since only a single ordering is valid. The
//!   [`BipartiteGadget`](crate::example_networks::BipartiteGadget) with `N >> 2` has *no sparse*
//!   *solution*, since the only requirement is that one single modifier is applied first.
//!
//! - **State Specific**: If a modifier dependency is only problematic at a specific state of the
//!   network, it is called state specific. It might only be a problem when it is applied at the
//!   beginning of the sequence, or at the end, or only in between.
//!
//!   *Example*:
//!   [`StateSpecificChainGadget`](crate::example_networks::StateSpecificChainGadget) has a
//!   *sparse*, *state specific* dependency with *immediate effect*. The immediate effect is only
//!   the case in this special state, but neither in the initial and the final state of the network.
//!   In the initial and final configuration, the dependency is not actually there.
//!
//!   *Note*: This can also be modelled by extending the modifier dependency to include other
//!   modifiers.
//!
//! ## Strategies
//!
//! - **One Strategy To Rule Them All** ([`StrategyTRTA`]): This strategy combines the best of both
//!   the simple [`TreeStrategy`], and the more complex, [`DepGroupsStrategy`]. It does this by
//!   exploring the search space similar to the tree strategy. But as soon as we would need to
//!   backtrace, we try to find a single dependency. If it succeeds, we repeat the tree traversal
//!   using the new dependency. Using this approach, we only search for the dependencies, which are
//!   not solvable by using the naive Tree strategy.
//!
//!   *Type Arguments*: None, this algorithm is as good as it gets (using this approach)
//!
//! - **[`PermutationStrategy`]**: This is the simplest strategy, naively checking every single
//!   permutation one after the other. It does benefit from dependencies, which have an *immediate*
//!   *effect* (only if the permutator makes use of the feedback mechanism, when the function
//!   `fail_pos` is reimplemented), and has problems if the problem has a *sparse* *solution*. This
//!   strategy is an [`ExhaustiveStrategy`].
//!
//!   *Type Arguments:* The first type argument `P` is the chosen
//!   [`Permutator`](crate::permutators), with an ordering of your choice.
//!
//! - **[`TreeStrategy`]**: This is a strategy trying out all possible
//!   permutations, in a similar fashion to the
//!   [`LexicographicPermutator`](crate::permutators::LexicographicPermutator). However, as soon as
//!   a problem was found for a given ordering up to a specific modifier, it does not need to check
//!   again all permutations starting with the same modifiers in the same ordering. Thus, this
//!   strategy benefits from from dependencies with an *immediate effect*, and is able to solve the
//!   [`ChainGadget`](crate::example_networks::ChainGadget) in `O(n^3)` time. This strategy is an
//!   [`ExhaustiveStrategy`].
//!
//!   *Type Arguments*: The first type argument `O` represents the chosen
//!   [`ModifierOrdering`](crate::modifier_ordering), which is used to order the modifiers before
//!   the tree algorithm starts.
//!
//! - **[`PushBackTreeStrategy`]**: This is a strategy very similar to the [`TreeStrategy`].
//!   However, once a modifier is causing an error, it is pushed back in the queue of remaining
//!   modifiers, and another is tried. As the `TreeStrategy`, the `PushBackTreeStrategy` benefits
//!   from dependencies with an *immediate* *effect*, and is an [`ExhaustiveStrategy`].
//!   Additionally, the `PushBackTreeStrategy` implements [`GroupStrategy`].
//!
//!   *Type Arguments*: The first type argument `O` represents the chosen
//!   [`ModifierOrdering`](crate::modifier_ordering), which is used to order the modifiers before
//!   the tree algorithm starts.
//!
//! - **[`DepGroupsStrategy`]**: This is a sophisticated algoirthm. It builds a set of groups of
//!   dependencies, which are solvable by their own. Then, it tries to use these to either build
//!   larger dependency groups, or find a solution to the entire problem. This strategy benefits
//!   from dependencies with an *immediate effect*, and having a *sparse* *solution*. However,
//!   this strategy cannot capture *state specific* dependencies, and may build larger dependencies
//!   than are actually needed. By adding more groups, the strategy scales with `O(g^4)`, but
//!   increasing each group size (with *no immediate effect*) scales with `O(n!)`. This strategy is
//!   not exhaustive.
//!
//!   *Type Arguments*: The first type argument is a [`GroupStrategy`], used to solve a smaller
//!   problem with the group information learned before. The last type argument `P` is a
//!   [`Permutator<usize>`](crate::permutators), used to generate all permutations of the groups.
//!   As soon as a new group is formed, the permutator is reset.
//!
//! - **[`NaiveRandomStrategy`]**: This strategy just exists for evaluation purpose. It simply
//!   shuffles the sequence and checks if this sequence is correct.
//!
//! - **[`NaiveRandomIBRStrategy`]**: This strategy is similar to the random strategy, but it always
//!   schedules insert before modify before remove commands.

mod permutation;
pub use permutation::PermutationStrategy;

mod tree;
pub use tree::TreeStrategy;

mod push_back_tree;
pub use push_back_tree::PushBackTreeStrategy;

mod naive_random;
pub use naive_random::NaiveRandomStrategy;

mod naive_random_ibr;
pub use naive_random_ibr::NaiveRandomIBRStrategy;

// dep_pairs_builder is very bad! Therefore, we do not re-export the name!
//mod dep_pairs_builder;
//pub use dep_pairs_builder::DepPairsBuilder;

// the DepGroupsStrategy is in a different module. Just re-export it from here
pub use crate::dep_groups::strategy::DepGroupsStrategy;
pub use crate::dep_groups::strategy_trta::StrategyTRTA;

use crate::hard_policies::HardPolicy;
use crate::netsim::config::{Config, ConfigModifier};
use crate::netsim::Network;
use crate::{Error, Stopper};

use std::time::Duration;

use log::*;

/// Infterface for all strategies
pub trait Strategy {
    /// Wrapper, that creates the strategy and synthesizes the network update order.
    fn synthesize(
        net: Network,
        end_config: Config,
        hard_policy: HardPolicy,
        time_budget: Option<Duration>,
        abort: Stopper,
    ) -> Result<Vec<ConfigModifier>, Error> {
        let start_config = net.current_config().clone();
        let patch = start_config.get_diff(&end_config);
        let modifiers: Vec<ConfigModifier> = patch.modifiers;
        let mut strategy = match Self::new(net, modifiers, hard_policy, time_budget) {
            Ok(s) => {
                info!("Initial configuration is valid!");
                s
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
        strategy.work(abort)
    }

    /// Create the strategy
    fn new(
        net: Network,
        modifiers: Vec<ConfigModifier>,
        hard_policy: HardPolicy,
        time_budget: Option<Duration>,
    ) -> Result<Box<Self>, Error>;

    /// Main function to find a valid reconfiguration sequence (if it exists) and return it.
    /// The function also returns the number of sequences that were tested.
    fn work(&mut self, abort: Stopper) -> Result<Vec<ConfigModifier>, Error>;
    /// Returns the number of states explored by the strategy.
    ///
    /// *This method is only available if the `"count-states"` feature is enabled!*
    #[cfg(feature = "count-states")]
    fn num_states(&self) -> usize;
}

/// Trait for a strategy being able to solve groups of modifiers
pub trait GroupStrategy: Strategy {
    /// Generate a GroupStrategy from a nested vector of ConfigModifiers.
    fn from_groups(
        net: Network,
        groups_idx: Vec<Vec<ConfigModifier>>,
        hard_policy: HardPolicy,
        time_budget: Option<Duration>,
    ) -> Result<Box<Self>, Error>;
}

/// Marking to tell that this strategy is exhaustive.
pub trait ExhaustiveStrategy: Strategy {}
