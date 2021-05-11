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

//! # Simplenet Network

use super::ExampleNetwork;
use crate::hard_policies::HardPolicy;
use crate::netsim::config::{Config, ConfigExpr::*};
use crate::netsim::{AsId, BgpSessionType::*, Network, Prefix};

/// # Simplenet
///
/// ![SimpleNet](https://n.ethz.ch/~sctibor/images/SimpleNet.svg)
pub struct SimpleNet {}

impl ExampleNetwork for SimpleNet {
    /// Get raw network without configuration
    fn net(initial_variant: usize) -> Network {
        let mut net = Network::new();

        // add routers
        let r1 = net.add_router("r1");
        let r2 = net.add_router("r2");
        let r3 = net.add_router("r3");
        let r4 = net.add_router("r4");
        let e1 = net.add_external_router("e1", AsId(65101));
        let e4 = net.add_external_router("e4", AsId(65104));

        // add links
        net.add_link(r1, r2);
        net.add_link(r1, r3);
        net.add_link(r2, r3);
        net.add_link(r2, r4);
        net.add_link(r3, r4);
        net.add_link(r1, e1);
        net.add_link(r4, e4);

        let cf = Self::initial_config(&net, initial_variant);
        net.set_config(&cf).unwrap();

        // advertise prefixes
        net.advertise_external_route(e1, Prefix(0), vec![AsId(65101), AsId(65200)], None, None)
            .unwrap();
        net.advertise_external_route(e4, Prefix(0), vec![AsId(65104), AsId(65200)], None, None)
            .unwrap();

        net
    }

    /// Get the initial configuration
    ///
    /// # Variant 0:
    /// - All link weights are set to 1
    /// - The following bgp sessions are set:
    ///   - e1 --> r1 (eBGP)
    ///   - r1 --- r2 (iBGP Peer)
    ///   - r1 --- r3 (iBGP Peer)
    ///   - e4 --> r4 (eBGP)
    ///
    /// # Variant 1
    /// - All link weights are set to 1
    /// - The following bgp sessions are set:
    ///   - e1 --> r1 (eBGP)
    ///   - r1 --- r2 (iBGP Peer)
    ///   - r1 --- r3 (iBGP Peer)
    ///   - r1 --- r4 (iBGP Peer)
    ///
    /// # Variant 2
    /// - All link weights are set to 1
    /// - The following bgp sessions are set:
    ///   - e1 --> r1 (eBGP)
    ///   - e4 --> r4 (eBGP)
    ///   - r1 --- r2 (iBGP Peer)
    ///   - r1 --- r3 (iBGP Peer)
    ///   - r1 --- r4 (iBGP Peer)
    ///   - r2 --- r4 (iBGP Peer)
    ///   - r3 --- r4 (iBGP Peer)
    fn initial_config(net: &Network, variant: usize) -> Config {
        let mut c = Config::new();

        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e4 = net.get_router_id("e4").unwrap();

        c.add(IgpLinkWeight { source: r1, target: r2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r1, target: r3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: r3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: r4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r3, target: r4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r1, target: e1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r4, target: e4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r1, source: r2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r1, source: r3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: r3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: r4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r3, source: r4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r1, source: e1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r4, source: e4, weight: 1.0 }).unwrap();

        c.add(BgpSession { source: r1, target: e1, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: r1, target: r2, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r1, target: r3, session_type: IBgpPeer }).unwrap();

        if variant == 0 {
            c.add(BgpSession { source: r4, target: e4, session_type: EBgp }).unwrap();
        } else if variant == 1 {
            c.add(BgpSession { source: r1, target: r4, session_type: IBgpPeer }).unwrap();
        } else if variant == 2 {
            c.add(BgpSession { source: r1, target: r4, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: r2, target: r4, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: r3, target: r4, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: r4, target: e4, session_type: EBgp }).unwrap();
        } else {
            panic!("Invalid variant number");
        }

        c
    }

    /// Get the end configuration
    ///
    /// # Variant 0
    /// - All link weights are set to 1
    /// - The following bgp sessions are set:
    ///   - r4 --- r1 (iBGP Peer)
    ///   - r4 --- r2 (iBGP Peer)
    ///   - r4 --- r3 (iBGP Peer)
    ///   - e4 --> r4 (eBGP)
    fn final_config(net: &Network, variant: usize) -> Config {
        let mut c = Config::new();

        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e4 = net.get_router_id("e4").unwrap();

        c.add(IgpLinkWeight { source: r1, target: r2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r1, target: r3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: r3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: r4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r3, target: r4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r1, target: e1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r4, target: e4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r1, source: r2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r1, source: r3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: r3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: r4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r3, source: r4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r1, source: e1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r4, source: e4, weight: 1.0 }).unwrap();

        c.add(BgpSession { source: r4, target: e4, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: r4, target: r1, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r4, target: r2, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r4, target: r3, session_type: IBgpPeer }).unwrap();

        if variant != 0 {
            panic!("Invalid variant number");
        }

        c
    }

    fn get_policy(net: &Network, _variant: usize) -> HardPolicy {
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter())
    }
}
