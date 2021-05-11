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

//! # Soft Policies
//!
//! Soft policies are expressed as cost functions, the smaller the result fo the cost functions, the
//! better is the solution which is found.

use crate::netsim::config::ConfigModifier;
use crate::netsim::{ForwardingState, Network, NetworkError};

mod minimize_traffic_shift;
pub use minimize_traffic_shift::MinimizeTrafficShift;

/// Trait for creating soft policies.
pub trait SoftPolicy {
    /// Crates a new soft policy and initializes it with correct initial values.
    fn new(state: &mut ForwardingState, net: &Network) -> Self;

    /// Update the information in the SoftPolicy. This function must be called after every modifier
    /// is applied.
    fn update(&mut self, state: &mut ForwardingState, net: &Network);

    /// Compute the score based on the information gathered by several calls to update. The output
    /// of this funciton is between 0 and 1, and lower is better.
    fn cost(&self) -> f64;
}

/// Compute the overall cost of a migration, given by a vector of all ordered modifications. If the
/// sequence cannot be applied due to some network errors, the error is returned.
pub fn compute_cost<P: SoftPolicy>(
    net: &Network,
    modifiers: &[ConfigModifier],
) -> Result<f64, NetworkError> {
    let mut net = net.clone();
    let mut cost: f64 = 0.0;
    let mut p = P::new(&mut net.get_forwarding_state(), &net);

    for m in modifiers.iter() {
        net.apply_modifier(m)?;
        p.update(&mut net.get_forwarding_state(), &net);
        cost += p.cost();
    }

    Ok(cost)
}
