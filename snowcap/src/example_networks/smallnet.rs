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

//! Smallnet

use super::ExampleNetwork;
use crate::hard_policies::HardPolicy;
use crate::netsim::config::{Config, ConfigExpr::*};
use crate::netsim::{AsId, BgpSessionType::*, Network, Prefix};

/// # Smallnet
///
/// ![SmallNet](https://n.ethz.ch/~sctibor/images/SmallNet.svg)
pub struct SmallNet;

impl ExampleNetwork for SmallNet {
    /// Get raw network without configuration
    fn net(initial_variant: usize) -> Network {
        let mut net = Network::new();

        // add routers
        let b1 = net.add_router("b1");
        let b2 = net.add_router("b2");
        let b3 = net.add_router("b3");
        let b4 = net.add_router("b4");
        let r1 = net.add_router("r1");
        let r2 = net.add_router("r2");
        let rr = net.add_router("rr");
        let e1 = net.add_external_router("e1", AsId(65101));
        let e2 = net.add_external_router("e2", AsId(65102));
        let e3 = net.add_external_router("e3", AsId(65103));
        let e4 = net.add_external_router("e4", AsId(65104));

        // add links
        net.add_link(e1, b1);
        net.add_link(e2, b2);
        net.add_link(e3, b3);
        net.add_link(e4, b4);
        net.add_link(b1, b2);
        net.add_link(b1, r1);
        net.add_link(b2, b3);
        net.add_link(b2, r1);
        net.add_link(b3, r2);
        net.add_link(b4, r2);
        net.add_link(b4, rr);
        net.add_link(r1, rr);
        net.add_link(r2, rr);

        let cf = Self::initial_config(&net, initial_variant);
        net.set_config(&cf).unwrap();

        net.advertise_external_route(e1, Prefix(0), vec![AsId(65101), AsId(65200)], None, None)
            .unwrap();
        net.advertise_external_route(e2, Prefix(0), vec![AsId(65102), AsId(65200)], None, None)
            .unwrap();
        net.advertise_external_route(e3, Prefix(0), vec![AsId(65103), AsId(65200)], None, None)
            .unwrap();
        net.advertise_external_route(e4, Prefix(0), vec![AsId(65104), AsId(65200)], None, None)
            .unwrap();

        net
    }

    /// Get the initial configuration
    ///
    /// # Variant 0: FullMesh
    /// - Link weights (see main documentation)
    /// - The following BGP sessions:
    ///   - e\[i\] --> b\[i\] (eBGP)
    ///   - full mesh for all b\[i\], r\[i\], rr (iBGP Peer)
    ///
    /// # Variant 1: minimal reasonable
    /// - Link weights (see main documentation)
    /// - The following BGP sessions:
    ///   - e\[i\] --> b\[i\] (eBGP)
    ///   - b2 --- r1 (iBGP Peer)
    ///   - b3 --- r2 (iBGP Peer)
    ///   - b4 --- rr (iBGP Peer)
    ///
    /// # Variant 1: minimal unreasonable
    /// - Link weights (see main documentation)
    /// - The following BGP sessions:
    ///   - e\[i\] --> b\[i\] (eBGP)
    ///   - b2 --- r2 (iBGP Peer)
    ///   - b3 --- rr (iBGP Peer)
    ///   - b4 --- r1 (iBGP Peer)
    fn initial_config(net: &Network, variant: usize) -> Config {
        let mut c = Config::new();

        let b1 = net.get_router_id("b1").unwrap();
        let b2 = net.get_router_id("b2").unwrap();
        let b3 = net.get_router_id("b3").unwrap();
        let b4 = net.get_router_id("b4").unwrap();
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let rr = net.get_router_id("rr").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e2 = net.get_router_id("e2").unwrap();
        let e3 = net.get_router_id("e3").unwrap();
        let e4 = net.get_router_id("e4").unwrap();

        c.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b2, target: e2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b3, target: e3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b4, target: e4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b1, target: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b1, target: r1, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { source: b2, target: b3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b2, target: r1, weight: 6.0 }).unwrap();
        c.add(IgpLinkWeight { source: b3, target: r2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b4, target: r2, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { source: b4, target: rr, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r1, target: rr, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: rr, weight: 7.0 }).unwrap();
        // symmetric weights
        c.add(IgpLinkWeight { target: b1, source: e1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b2, source: e2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b3, source: e3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b4, source: e4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b1, source: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b1, source: r1, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { target: b2, source: b3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b2, source: r1, weight: 6.0 }).unwrap();
        c.add(IgpLinkWeight { target: b3, source: r2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b4, source: r2, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { target: b4, source: rr, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r1, source: rr, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: rr, weight: 7.0 }).unwrap();

        c.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b3, target: e3, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b4, target: e4, session_type: EBgp }).unwrap();
        if variant == 0 {
            c.add(BgpSession { source: b1, target: b2, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b1, target: b3, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b1, target: b4, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b1, target: r1, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b1, target: r2, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b1, target: rr, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b2, target: b3, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b2, target: b4, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b2, target: r1, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b2, target: r2, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b2, target: rr, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b3, target: b4, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b3, target: r1, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b3, target: r2, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b3, target: rr, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b4, target: r1, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b4, target: r2, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b4, target: rr, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: r1, target: r2, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: r1, target: rr, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: r2, target: rr, session_type: IBgpPeer }).unwrap();
        } else if variant == 1 {
            c.add(BgpSession { source: b2, target: r1, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b3, target: r2, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b4, target: rr, session_type: IBgpPeer }).unwrap();
        } else if variant == 2 {
            c.add(BgpSession { source: b2, target: r2, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b3, target: rr, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: b4, target: r1, session_type: IBgpPeer }).unwrap();
        } else {
            panic!("Invalid variant number!")
        }

        c
    }

    /// Get the end configuration
    ///
    /// # Variant 0: Hierarchy
    /// - All link weights are set to 1
    /// - The following bgp sessions are set:
    ///   - r1 --> b1 (iBGP Client)
    ///   - r1 --> b2 (iBGP Client)
    ///   - r2 --> b3 (iBGP Client)
    ///   - e2 --> b4 (iBGP Client)
    ///   - rr --> r1 (iBGP Client)
    ///   - rr --> r2 (iBGP Client)
    ///   - rr --> b4 (iBGP Client)
    ///
    /// # Variant 1: Hierarchy with missing eBGP sessions
    /// - The same as variant 0, but the following eBGP sessions are missing:
    ///   - e2 --> b2
    ///   - e4 --> b4
    fn final_config(net: &Network, variant: usize) -> Config {
        let mut c = Config::new();

        let b1 = net.get_router_id("b1").unwrap();
        let b2 = net.get_router_id("b2").unwrap();
        let b3 = net.get_router_id("b3").unwrap();
        let b4 = net.get_router_id("b4").unwrap();
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let rr = net.get_router_id("rr").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e2 = net.get_router_id("e2").unwrap();
        let e3 = net.get_router_id("e3").unwrap();
        let e4 = net.get_router_id("e4").unwrap();

        c.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b2, target: e2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b3, target: e3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b4, target: e4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b1, target: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b1, target: r1, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { source: b2, target: b3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b2, target: r1, weight: 6.0 }).unwrap();
        c.add(IgpLinkWeight { source: b3, target: r2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b4, target: r2, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { source: b4, target: rr, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r1, target: rr, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: rr, weight: 7.0 }).unwrap();
        // symmetric weights
        c.add(IgpLinkWeight { target: b1, source: e1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b2, source: e2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b3, source: e3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b4, source: e4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b1, source: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b1, source: r1, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { target: b2, source: b3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b2, source: r1, weight: 6.0 }).unwrap();
        c.add(IgpLinkWeight { target: b3, source: r2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b4, source: r2, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { target: b4, source: rr, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r1, source: rr, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: rr, weight: 7.0 }).unwrap();

        c.add(BgpSession { source: rr, target: r1, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: rr, target: r2, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: rr, target: b4, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r1, target: b1, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r1, target: b2, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r2, target: b3, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r2, target: b4, session_type: IBgpClient }).unwrap();

        if variant == 0 {
            c.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: b3, target: e3, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: b4, target: e4, session_type: EBgp }).unwrap();
        } else if variant == 1 {
            c.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: b3, target: e3, session_type: EBgp }).unwrap();
        } else {
            panic!("Invalid variant number!");
        }

        c
    }

    fn get_policy(net: &Network, _variant: usize) -> HardPolicy {
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter())
    }
}
