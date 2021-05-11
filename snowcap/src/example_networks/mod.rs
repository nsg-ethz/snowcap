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

//! Networks for testing

use crate::hard_policies::HardPolicy;
use crate::netsim::config::Config;
use crate::netsim::Network;

mod simplenet;
pub use simplenet::SimpleNet;

mod smallnet;
pub use smallnet::SmallNet;

mod mediumnet;
pub use mediumnet::MediumNet;

mod carousel_gadget;
pub use carousel_gadget::CarouselGadget;

mod evil_twin_gadget;
pub use evil_twin_gadget::EvilTwinGadget;

mod difficult_gadget;
pub use difficult_gadget::{
    DifficultGadgetComplete, DifficultGadgetMinimal, DifficultGadgetRepeated,
};

mod bipartite_gadget;
pub use bipartite_gadget::BipartiteGadget;

mod bipartite_carousel_fusion;
pub use bipartite_carousel_fusion::BipartiteCarouselFusion;

mod firewallnet;
pub use firewallnet::FirewallNet;

pub mod repetitions;

mod chain_gadget;
pub use chain_gadget::{ChainGadget, StateSpecificChainGadget};

mod abilene_net;
pub use abilene_net::AbileneNetwork;

mod variable_abilene_net;
pub use variable_abilene_net::VariableAbileneNetwork;

/// Trait for easier access to example networks.
pub trait ExampleNetwork {
    /// Get the network configured with the chosen initial variant.
    fn net(initial_variant: usize) -> Network;
    /// Get the initial configuration
    fn initial_config(net: &Network, variant: usize) -> Config;
    /// Get the final configuration
    fn final_config(net: &Network, variant: usize) -> Config;
    /// Get the hard policies.
    fn get_policy(net: &Network, variant: usize) -> HardPolicy;
}
