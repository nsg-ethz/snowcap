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

//! Soft Policy to minimize traffic shit

use super::SoftPolicy;
use crate::netsim::{ForwardingState, Network, Prefix, RouterId};

/// # Soft Policy: Minimize Traffic Shift
///
/// This is a soft policy trying to minimize the number of traffic shifts during reconfiguration.
/// Traffic shifts are counted in the following way: For every router and every prefix, if the next
/// hop changes from the previous state to the current state, then increase the count by 1.
#[derive(Clone, Debug)]
pub struct MinimizeTrafficShift {
    current_next_hops: Vec<Option<RouterId>>,
    prefix_lookup: Vec<(Prefix, usize)>,
    num_prefixes: usize,
    num_routers: usize,
    num_different: usize,
}

impl SoftPolicy for MinimizeTrafficShift {
    fn new(state: &mut ForwardingState, net: &Network) -> Self {
        let prefix_lookup: Vec<(Prefix, usize)> =
            net.get_known_prefixes().iter().cloned().enumerate().map(|(i, p)| (p, i)).collect();
        let num_prefixes = prefix_lookup.len();
        let num_routers = net.get_routers().len();

        assert!(num_prefixes > 0);
        assert!(num_routers > 0);

        let mut current_next_hops: Vec<Option<RouterId>> =
            std::iter::repeat(None).take(net.num_devices() * num_prefixes).collect();
        for r in net.get_routers() {
            for (p, pid) in prefix_lookup.iter() {
                let idx = get_idx(r.index(), *pid, num_prefixes);
                current_next_hops[idx] = state.get_next_hop(r, *p).unwrap();
            }
        }

        Self { current_next_hops, prefix_lookup, num_prefixes, num_routers, num_different: 0 }
    }

    fn update(&mut self, state: &mut ForwardingState, net: &Network) {
        let mut count: usize = 0;
        for router in net.get_routers() {
            for (p, pid) in self.prefix_lookup.iter() {
                let idx = get_idx(router.index(), *pid, self.num_prefixes);
                let new_next_hop = state.get_next_hop(router, *p).unwrap();
                let old_next_hop = self.current_next_hops[idx];
                if new_next_hop.is_some() && old_next_hop.is_some() && new_next_hop != old_next_hop
                {
                    count += 1;
                }
                self.current_next_hops[idx] = new_next_hop;
            }
        }
        self.num_different = count;
    }

    fn cost(&self) -> f64 {
        let total_next_hops = self.num_routers * self.num_prefixes;
        (self.num_different as f64) / (total_next_hops as f64)
    }
}

fn get_idx(rid: usize, pid: usize, n_prefixes: usize) -> usize {
    rid * n_prefixes + pid
}
