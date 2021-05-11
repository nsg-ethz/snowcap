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

//! # Evil Twin Gadget

use super::ExampleNetwork;
use crate::hard_policies::HardPolicy;
use crate::netsim::config::{Config, ConfigExpr::*};
use crate::netsim::{AsId, BgpSessionType::*, Network, Prefix};

/// # Evil Twin Gadget
/// Example from [Laurent Vanbever et. al.](https://ieeexplore.ieee.org/document/6567025), Figure 4
/// on page 5. It contains four top-level route reflectors (`r1`, `r2`, `r3` and `r4`) connected in
/// an iBGP full-mesh, two second-level route reflectors (`ra` and `rb`), and five border routers
/// (`b1`, `bx`, `b2`, `b3`, `b4`), where two different prefixes are advertised by 5 external
/// routers (`e1`, `ex`, `e2`, `e3`, `e4`).
pub struct EvilTwinGadget {}

impl ExampleNetwork for EvilTwinGadget {
    fn net(initial_variant: usize) -> Network {
        let mut net = Network::new();
        net.set_msg_limit(Some(10_000));

        let r1 = net.add_router("r1");
        let r2 = net.add_router("r2");
        let r3 = net.add_router("r3");
        let r4 = net.add_router("r4");
        let ra = net.add_router("ra");
        let rb = net.add_router("rb");
        let b1 = net.add_router("b1");
        let bx = net.add_router("bx");
        let b2 = net.add_router("b2");
        let b3 = net.add_router("b3");
        let b4 = net.add_router("b4");
        let e1 = net.add_external_router("e1", AsId(65101));
        let ex = net.add_external_router("ex", AsId(65100));
        let e2 = net.add_external_router("e2", AsId(65102));
        let e3 = net.add_external_router("e3", AsId(65103));
        let e4 = net.add_external_router("e4", AsId(65104));
        let er = net.add_external_router("er", AsId(65105));

        net.add_link(r1, b1);
        net.add_link(r1, b2);
        net.add_link(ra, b1);
        net.add_link(ra, bx);
        net.add_link(ra, b2);
        net.add_link(r2, bx);
        net.add_link(r2, b2);
        net.add_link(r2, b3);
        net.add_link(r2, b4);
        net.add_link(rb, b1);
        net.add_link(rb, b3);
        net.add_link(rb, b4);
        net.add_link(r3, b1);
        net.add_link(r3, bx);
        net.add_link(r3, b3);
        net.add_link(r4, b1);
        net.add_link(r4, b4);
        net.add_link(b1, e1);
        net.add_link(bx, ex);
        net.add_link(b2, e2);
        net.add_link(b3, e3);
        net.add_link(b4, e4);
        net.add_link(r1, er);

        let ca = Self::initial_config(&net, initial_variant);
        net.set_config(&ca).unwrap();

        net.advertise_external_route(er, Prefix(2), vec![AsId(65105), AsId(65202)], None, None)
            .unwrap();
        net.advertise_external_route(ex, Prefix(1), vec![AsId(65100), AsId(65201)], None, None)
            .unwrap();
        net.advertise_external_route(ex, Prefix(2), vec![AsId(65100), AsId(65202)], None, None)
            .unwrap();
        net.advertise_external_route(e1, Prefix(1), vec![AsId(65101), AsId(65201)], None, None)
            .unwrap();
        net.advertise_external_route(e1, Prefix(2), vec![AsId(65101), AsId(65202)], None, None)
            .unwrap();
        net.advertise_external_route(e2, Prefix(1), vec![AsId(65102), AsId(65201)], None, None)
            .unwrap();
        net.advertise_external_route(e3, Prefix(1), vec![AsId(65103), AsId(65201)], None, None)
            .unwrap();
        net.advertise_external_route(e4, Prefix(2), vec![AsId(65104), AsId(65202)], None, None)
            .unwrap();

        net
    }

    fn initial_config(net: &Network, _variant: usize) -> Config {
        let mut c = Config::new();

        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let ra = net.get_router_id("ra").unwrap();
        let rb = net.get_router_id("rb").unwrap();
        let b1 = net.get_router_id("b1").unwrap();
        let bx = net.get_router_id("bx").unwrap();
        let b2 = net.get_router_id("b2").unwrap();
        let b3 = net.get_router_id("b3").unwrap();
        let b4 = net.get_router_id("b4").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let ex = net.get_router_id("ex").unwrap();
        let e2 = net.get_router_id("e2").unwrap();
        let e3 = net.get_router_id("e3").unwrap();
        let e4 = net.get_router_id("e4").unwrap();
        let er = net.get_router_id("er").unwrap();

        // link weights
        c.add(IgpLinkWeight { source: r1, target: b1, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { source: r1, target: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: ra, target: b1, weight: 4.0 }).unwrap();
        c.add(IgpLinkWeight { source: ra, target: bx, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { source: ra, target: b2, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: bx, weight: 4.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: b2, weight: 6.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: b3, weight: 5.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: b4, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { source: rb, target: b1, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { source: rb, target: b3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: rb, target: b4, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { source: r3, target: b1, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { source: r3, target: bx, weight: 7.0 }).unwrap();
        c.add(IgpLinkWeight { source: r3, target: b3, weight: 9.0 }).unwrap();
        c.add(IgpLinkWeight { source: r4, target: b1, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { source: r4, target: b4, weight: 9.0 }).unwrap();
        c.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: bx, target: ex, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b2, target: e2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b3, target: e3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b4, target: e4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r1, target: er, weight: 1.0 }).unwrap();
        // reverse weights
        c.add(IgpLinkWeight { target: r1, source: b1, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { target: r1, source: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: ra, source: b1, weight: 4.0 }).unwrap();
        c.add(IgpLinkWeight { target: ra, source: bx, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { target: ra, source: b2, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: bx, weight: 4.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: b2, weight: 6.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: b3, weight: 5.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: b4, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { target: rb, source: b1, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { target: rb, source: b3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: rb, source: b4, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { target: r3, source: b1, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { target: r3, source: bx, weight: 7.0 }).unwrap();
        c.add(IgpLinkWeight { target: r3, source: b3, weight: 9.0 }).unwrap();
        c.add(IgpLinkWeight { target: r4, source: b1, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { target: r4, source: b4, weight: 9.0 }).unwrap();
        c.add(IgpLinkWeight { target: b1, source: e1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: bx, source: ex, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b2, source: e2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b3, source: e3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b4, source: e4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r1, source: er, weight: 1.0 }).unwrap();

        // bgp sessions
        c.add(BgpSession { source: r1, target: r2, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r1, target: r3, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r1, target: r4, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r2, target: r3, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r2, target: r4, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r3, target: r4, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r1, target: b1, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r1, target: bx, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r2, target: ra, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r2, target: b2, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r3, target: rb, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r3, target: b3, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r4, target: b4, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: ra, target: b1, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: ra, target: bx, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: ra, target: b2, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: rb, target: b1, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: rb, target: b3, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: rb, target: b4, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: bx, target: ex, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b3, target: e3, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b4, target: e4, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: r1, target: er, session_type: EBgp }).unwrap();

        c
    }

    fn final_config(net: &Network, _variant: usize) -> Config {
        let mut c = Config::new();

        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let ra = net.get_router_id("ra").unwrap();
        let rb = net.get_router_id("rb").unwrap();
        let b1 = net.get_router_id("b1").unwrap();
        let bx = net.get_router_id("bx").unwrap();
        let b2 = net.get_router_id("b2").unwrap();
        let b3 = net.get_router_id("b3").unwrap();
        let b4 = net.get_router_id("b4").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let ex = net.get_router_id("ex").unwrap();
        let e2 = net.get_router_id("e2").unwrap();
        let e3 = net.get_router_id("e3").unwrap();
        let e4 = net.get_router_id("e4").unwrap();
        let er = net.get_router_id("er").unwrap();

        // link weights
        c.add(IgpLinkWeight { source: r1, target: b1, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { source: r1, target: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: ra, target: b1, weight: 4.0 }).unwrap();
        c.add(IgpLinkWeight { source: ra, target: bx, weight: 5.0 }).unwrap();
        c.add(IgpLinkWeight { source: ra, target: b2, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: bx, weight: 4.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: b2, weight: 6.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: b3, weight: 5.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: b4, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { source: rb, target: b1, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { source: rb, target: b3, weight: 4.0 }).unwrap();
        c.add(IgpLinkWeight { source: rb, target: b4, weight: 5.0 }).unwrap();
        c.add(IgpLinkWeight { source: r3, target: b1, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { source: r3, target: bx, weight: 7.0 }).unwrap();
        c.add(IgpLinkWeight { source: r3, target: b3, weight: 9.0 }).unwrap();
        c.add(IgpLinkWeight { source: r4, target: b1, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { source: r4, target: b4, weight: 9.0 }).unwrap();
        c.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: bx, target: ex, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b2, target: e2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b3, target: e3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b4, target: e4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r1, target: er, weight: 1.0 }).unwrap();
        // reverse weights
        c.add(IgpLinkWeight { target: r1, source: b1, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { target: r1, source: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: ra, source: b1, weight: 4.0 }).unwrap();
        c.add(IgpLinkWeight { target: ra, source: bx, weight: 5.0 }).unwrap();
        c.add(IgpLinkWeight { target: ra, source: b2, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: bx, weight: 4.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: b2, weight: 6.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: b3, weight: 5.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: b4, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { target: rb, source: b1, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { target: rb, source: b3, weight: 4.0 }).unwrap();
        c.add(IgpLinkWeight { target: rb, source: b4, weight: 5.0 }).unwrap();
        c.add(IgpLinkWeight { target: r3, source: b1, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { target: r3, source: bx, weight: 7.0 }).unwrap();
        c.add(IgpLinkWeight { target: r3, source: b3, weight: 9.0 }).unwrap();
        c.add(IgpLinkWeight { target: r4, source: b1, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { target: r4, source: b4, weight: 9.0 }).unwrap();
        c.add(IgpLinkWeight { target: b1, source: e1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: bx, source: ex, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b2, source: e2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b3, source: e3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b4, source: e4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r1, source: er, weight: 1.0 }).unwrap();

        // bgp sessions
        c.add(BgpSession { source: r1, target: r2, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r1, target: r3, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r1, target: r4, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r2, target: r3, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r2, target: r4, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r3, target: r4, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r1, target: b1, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r1, target: bx, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r2, target: ra, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r2, target: b2, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r3, target: rb, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r3, target: b3, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r4, target: b4, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: ra, target: b1, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: ra, target: bx, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: ra, target: b2, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: rb, target: b1, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: rb, target: b3, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: rb, target: b4, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: bx, target: ex, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b3, target: e3, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b4, target: e4, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: r1, target: er, session_type: EBgp }).unwrap();

        c
    }

    fn get_policy(net: &Network, _variant: usize) -> HardPolicy {
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter())
    }
}
