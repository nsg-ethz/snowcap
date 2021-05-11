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

//! # Difficult Gadget
//! This example topology is designed deliberately to be hard to solve. It contains two
//! configuration changes, which need to be executed in order. The problem manifests itself only
//! after the third configuration change is introduced.
//!
//! This module contains two different networks, the `DifficultGadgetMinimal` and the
//! `DifficultGadgetComplete`, which contains additional configuration to amplify the problem.

use super::{
    repetitions::{Repetition3, Repetitions},
    ExampleNetwork,
};
use crate::hard_policies::HardPolicy;
use crate::netsim::config::{Config, ConfigExpr::*};
use crate::netsim::{AsId, BgpSessionType::*, Network, Prefix};
use std::marker::PhantomData;

/// # Difficult Gadget Minimal
/// At the hart of this gadget, we have the unstable gadget, where the order of advertisement
/// influences the final result:
///
/// ![UnstableGadget](https://n.ethz.ch/~sctibor/images/UnstableGadget.svg)
///
/// In this topology, `rx` and `ry` have a iBGP peer session, while `rx` and `ry` are route
/// reflectors for `r1` and `r2` respectively. If `r1` sends the advertisement before `r2`, then
/// both `rx` and `ry` will select `r1`, even after `r2` has sent its advertisement.
///
/// We extend this topology by introducing several other routers. First, we add two routers `t1`
/// `t2`, which replace `r1` and `r2` in their role to send the advertisement to `rx` and `ry`,
/// without being directly connected to them. Then, we introduce a new router `bx` connected to `rx`
/// and `ry`, which will make sure that `r2` selects its next-hop for the prefix such that it sends
/// the packets back to `rx`. However, this session is only "activated" by removing another link,
/// thus causing the problem.
///
/// ![DifficultGadgetMinimal](https://n.ethz.ch/~sctibor/images/DifficultGadgetMinimal.svg)
///
/// All link weights are set to 1, except the links between `bx` and `rx` and `ry` are set to 10.
/// `e1`, `e2` and `ex` all avertise the same prefix. In the initial configuration, we have the
/// following BGP sessions:
/// - `bx` --> `ex` (eBGP)
/// - `b1` --> `e1` (eBGP)
/// - `b2` --> `e2` (eBGP)
/// - `r1` --> `b1` (iBGP RR --> Client)
/// - `r2` --> `b2` (iBGP RR --> Client)
/// - `t1` --> `b1` (iBGP RR --> Client)
/// - `t2` --> `b2` (iBGP RR --> Client)
/// - `rx` --> `bx` (iBGP RR --> Client)
/// - `ry` --> `bx` (iBGP RR --> Client)
/// - `rx` --> `ry` (iBGP Peer)
/// - `r2` --> `bx` (iBGP RR -> Client) (the conection which causes the problem)
///
/// We apply the following modifications (The order is in which it works)
/// 1. ADD `rx` --> `t1` (iBGP RR --> Client)
/// 2. ADD `ry` --> `t2` (iBGP RR --> Client)
/// 3. DEL `r2` --> `b2` (iBGP RR --> Client)
pub struct DifficultGadgetMinimal {}

impl ExampleNetwork for DifficultGadgetMinimal {
    fn net(initial_variant: usize) -> Network {
        let mut net = Network::new();

        let ry = net.add_router("ry");
        let r1 = net.add_router("r1");
        let r2 = net.add_router("r2");
        let b1 = net.add_router("b1");
        let b2 = net.add_router("b2");
        let t1 = net.add_router("t1");
        let t2 = net.add_router("t2");
        let bx = net.add_router("bx");
        let ex = net.add_external_router("ex", AsId(65100));
        let e1 = net.add_external_router("e1", AsId(65101));
        let e2 = net.add_external_router("e2", AsId(65102));
        let rx = net.add_router("rx"); // this needs to be last, because we want this update to be after the ry update

        net.add_link(ex, bx);
        net.add_link(bx, rx);
        net.add_link(bx, ry);
        net.add_link(rx, r2);
        net.add_link(ry, r1);
        net.add_link(r1, b1);
        net.add_link(r2, b2);
        net.add_link(t1, b1);
        net.add_link(t2, b2);
        net.add_link(b1, e1);
        net.add_link(b2, e2);
        net.add_link(rx, ry);

        // apply initial config
        let cf = Self::initial_config(&net, initial_variant);
        net.set_config(&cf).unwrap();

        // advertise prefixes
        net.advertise_external_route(ex, Prefix(0), vec![AsId(65100), AsId(65200)], None, None)
            .unwrap();
        net.advertise_external_route(e1, Prefix(0), vec![AsId(65101), AsId(65200)], None, None)
            .unwrap();
        net.advertise_external_route(e2, Prefix(0), vec![AsId(65102), AsId(65200)], None, None)
            .unwrap();

        net
    }

    fn initial_config(net: &Network, _variant: usize) -> Config {
        let mut config = Config::new();

        let rx = net.get_router_id("rx").unwrap();
        let ry = net.get_router_id("ry").unwrap();
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let b1 = net.get_router_id("b1").unwrap();
        let b2 = net.get_router_id("b2").unwrap();
        let t1 = net.get_router_id("t1").unwrap();
        let t2 = net.get_router_id("t2").unwrap();
        let bx = net.get_router_id("bx").unwrap();
        let ex = net.get_router_id("ex").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e2 = net.get_router_id("e2").unwrap();

        config.add(IgpLinkWeight { source: ex, target: bx, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: bx, target: rx, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { source: bx, target: ry, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { source: rx, target: r2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: ry, target: r1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: r1, target: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: r2, target: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: t1, target: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: t2, target: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: b2, target: e2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: rx, target: ry, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: ex, source: bx, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: bx, source: rx, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { target: bx, source: ry, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { target: rx, source: r2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: ry, source: r1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: r1, source: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: r2, source: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: t1, source: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: t2, source: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: b1, source: e1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: b2, source: e2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: rx, source: ry, weight: 1.0 }).unwrap();

        config.add(BgpSession { source: bx, target: ex, session_type: EBgp }).unwrap();
        config.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
        config.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();
        config.add(BgpSession { source: rx, target: ry, session_type: IBgpPeer }).unwrap();
        config.add(BgpSession { source: rx, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: ry, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: r1, target: b1, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: r2, target: b2, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: t1, target: b1, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: t2, target: b2, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: r2, target: bx, session_type: IBgpClient }).unwrap();

        config
    }

    fn final_config(net: &Network, _variant: usize) -> Config {
        let mut config = Config::new();

        let rx = net.get_router_id("rx").unwrap();
        let ry = net.get_router_id("ry").unwrap();
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let b1 = net.get_router_id("b1").unwrap();
        let b2 = net.get_router_id("b2").unwrap();
        let t1 = net.get_router_id("t1").unwrap();
        let t2 = net.get_router_id("t2").unwrap();
        let bx = net.get_router_id("bx").unwrap();
        let ex = net.get_router_id("ex").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e2 = net.get_router_id("e2").unwrap();

        config.add(IgpLinkWeight { source: ex, target: bx, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: bx, target: rx, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { source: bx, target: ry, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { source: rx, target: r2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: ry, target: r1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: r1, target: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: r2, target: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: t1, target: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: t2, target: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: b2, target: e2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: rx, target: ry, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: ex, source: bx, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: bx, source: rx, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { target: bx, source: ry, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { target: rx, source: r2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: ry, source: r1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: r1, source: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: r2, source: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: t1, source: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: t2, source: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: b1, source: e1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: b2, source: e2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: rx, source: ry, weight: 1.0 }).unwrap();

        config.add(BgpSession { source: bx, target: ex, session_type: EBgp }).unwrap();
        config.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
        config.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();
        config.add(BgpSession { source: rx, target: ry, session_type: IBgpPeer }).unwrap();
        config.add(BgpSession { source: rx, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: ry, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: r1, target: b1, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: t1, target: b1, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: t2, target: b2, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: r2, target: bx, session_type: IBgpClient }).unwrap();
        //new config
        config.add(BgpSession { source: rx, target: t1, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: ry, target: t2, session_type: IBgpClient }).unwrap();
        // old removed config
        //config.add(BgpSession { source: r2, target: b2, session_type: IBgpClient }).unwrap();

        config
    }

    fn get_policy(net: &Network, _variant: usize) -> HardPolicy {
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter())
    }
}

/// # DifficultGadgetComplete
/// This is essentially the same network as the minimal difficult gadget, but we have added
/// N+1 other routers in the following way (with N=5)
///
/// ![DifficultGadgetComplete](https://n.ethz.ch/~sctibor/images/DifficultGadgetComplete.svg)
///
/// These routers are initially configured such that we have RR-Client sessions from `ni` --> `n0`,
/// and one from `n0` --> `bx`. All link weights are 1. After reconfiguration, we add additional
/// RR-Client bgp sessions from `ni` directly to `bx`. This will make sure that the ordering will
/// will still cause the problem, while not messing with anything that we had before.
pub struct DifficultGadgetComplete {}

impl ExampleNetwork for DifficultGadgetComplete {
    fn net(initial_variant: usize) -> Network {
        let mut net = Network::new();

        let ry = net.add_router("ry");
        let r1 = net.add_router("r1");
        let r2 = net.add_router("r2");
        let b1 = net.add_router("b1");
        let b2 = net.add_router("b2");
        let t1 = net.add_router("t1");
        let t2 = net.add_router("t2");
        let bx = net.add_router("bx");
        let ex = net.add_external_router("ex", AsId(65100));
        let e1 = net.add_external_router("e1", AsId(65101));
        let e2 = net.add_external_router("e2", AsId(65102));
        let n0 = net.add_router("n0");
        let n1 = net.add_router("n1");
        let n2 = net.add_router("n2");
        let n3 = net.add_router("n3");
        let n4 = net.add_router("n4");
        let n5 = net.add_router("n5");
        let n6 = net.add_router("n6");
        let rx = net.add_router("rx"); // this needs to be last, because we want this update to be after the ry update

        net.add_link(ex, bx);
        net.add_link(bx, rx);
        net.add_link(bx, ry);
        net.add_link(rx, r2);
        net.add_link(ry, r1);
        net.add_link(r1, b1);
        net.add_link(r2, b2);
        net.add_link(t1, b1);
        net.add_link(t2, b2);
        net.add_link(b1, e1);
        net.add_link(b2, e2);
        net.add_link(rx, ry);
        // additional links
        net.add_link(bx, n0);
        net.add_link(n0, n1);
        net.add_link(n0, n2);
        net.add_link(n0, n3);
        net.add_link(n0, n4);
        net.add_link(n0, n5);
        net.add_link(n0, n6);

        // apply initial config
        let cf = Self::initial_config(&net, initial_variant);
        net.set_config(&cf).unwrap();

        // advertise prefixes
        net.advertise_external_route(ex, Prefix(0), vec![AsId(65100), AsId(65200)], None, None)
            .unwrap();
        net.advertise_external_route(e1, Prefix(0), vec![AsId(65101), AsId(65200)], None, None)
            .unwrap();
        net.advertise_external_route(e2, Prefix(0), vec![AsId(65102), AsId(65200)], None, None)
            .unwrap();

        net
    }

    fn initial_config(net: &Network, _variant: usize) -> Config {
        let mut config = Config::new();

        let rx = net.get_router_id("rx").unwrap();
        let ry = net.get_router_id("ry").unwrap();
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let b1 = net.get_router_id("b1").unwrap();
        let b2 = net.get_router_id("b2").unwrap();
        let t1 = net.get_router_id("t1").unwrap();
        let t2 = net.get_router_id("t2").unwrap();
        let bx = net.get_router_id("bx").unwrap();
        let ex = net.get_router_id("ex").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e2 = net.get_router_id("e2").unwrap();
        // additional routers
        let n0 = net.get_router_id("n0").unwrap();
        let n1 = net.get_router_id("n1").unwrap();
        let n2 = net.get_router_id("n2").unwrap();
        let n3 = net.get_router_id("n3").unwrap();
        let n4 = net.get_router_id("n4").unwrap();
        let n5 = net.get_router_id("n5").unwrap();
        let n6 = net.get_router_id("n6").unwrap();

        config.add(IgpLinkWeight { source: ex, target: bx, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: bx, target: rx, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { source: bx, target: ry, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { source: rx, target: r2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: ry, target: r1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: r1, target: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: r2, target: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: t1, target: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: t2, target: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: b2, target: e2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: rx, target: ry, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: ex, source: bx, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: bx, source: rx, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { target: bx, source: ry, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { target: rx, source: r2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: ry, source: r1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: r1, source: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: r2, source: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: t1, source: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: t2, source: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: b1, source: e1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: b2, source: e2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: rx, source: ry, weight: 1.0 }).unwrap();
        // additional link weights
        config.add(IgpLinkWeight { source: bx, target: n0, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: n0, target: n1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: n0, target: n2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: n0, target: n3, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: n0, target: n4, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: n0, target: n5, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: n0, target: n6, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: bx, source: n0, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: n0, source: n1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: n0, source: n2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: n0, source: n3, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: n0, source: n4, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: n0, source: n5, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: n0, source: n6, weight: 1.0 }).unwrap();

        config.add(BgpSession { source: bx, target: ex, session_type: EBgp }).unwrap();
        config.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
        config.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();
        config.add(BgpSession { source: rx, target: ry, session_type: IBgpPeer }).unwrap();
        config.add(BgpSession { source: rx, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: ry, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: r1, target: b1, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: r2, target: b2, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: t1, target: b1, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: t2, target: b2, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: r2, target: bx, session_type: IBgpClient }).unwrap();
        // additional sessions
        config.add(BgpSession { source: n0, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n1, target: n0, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n2, target: n0, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n3, target: n0, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n4, target: n0, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n5, target: n0, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n6, target: n0, session_type: IBgpClient }).unwrap();

        config
    }

    fn final_config(net: &Network, _variant: usize) -> Config {
        let mut config = Config::new();

        let rx = net.get_router_id("rx").unwrap();
        let ry = net.get_router_id("ry").unwrap();
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let b1 = net.get_router_id("b1").unwrap();
        let b2 = net.get_router_id("b2").unwrap();
        let t1 = net.get_router_id("t1").unwrap();
        let t2 = net.get_router_id("t2").unwrap();
        let bx = net.get_router_id("bx").unwrap();
        let ex = net.get_router_id("ex").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e2 = net.get_router_id("e2").unwrap();
        // additional routers
        let n0 = net.get_router_id("n0").unwrap();
        let n1 = net.get_router_id("n1").unwrap();
        let n2 = net.get_router_id("n2").unwrap();
        let n3 = net.get_router_id("n3").unwrap();
        let n4 = net.get_router_id("n4").unwrap();
        let n5 = net.get_router_id("n5").unwrap();
        let n6 = net.get_router_id("n6").unwrap();

        config.add(IgpLinkWeight { source: ex, target: bx, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: bx, target: rx, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { source: bx, target: ry, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { source: rx, target: r2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: ry, target: r1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: r1, target: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: r2, target: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: t1, target: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: t2, target: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: b2, target: e2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: rx, target: ry, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: ex, source: bx, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: bx, source: rx, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { target: bx, source: ry, weight: 10.0 }).unwrap();
        config.add(IgpLinkWeight { target: rx, source: r2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: ry, source: r1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: r1, source: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: r2, source: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: t1, source: b1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: t2, source: b2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: b1, source: e1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: b2, source: e2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: rx, source: ry, weight: 1.0 }).unwrap();
        // additional link weights
        config.add(IgpLinkWeight { source: bx, target: n0, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: n0, target: n1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: n0, target: n2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: n0, target: n3, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: n0, target: n4, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: n0, target: n5, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { source: n0, target: n6, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: bx, source: n0, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: n0, source: n1, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: n0, source: n2, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: n0, source: n3, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: n0, source: n4, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: n0, source: n5, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: n0, source: n6, weight: 1.0 }).unwrap();

        config.add(BgpSession { source: bx, target: ex, session_type: EBgp }).unwrap();
        config.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
        config.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();
        config.add(BgpSession { source: rx, target: ry, session_type: IBgpPeer }).unwrap();
        config.add(BgpSession { source: rx, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: ry, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: r1, target: b1, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: t1, target: b1, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: t2, target: b2, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: r2, target: bx, session_type: IBgpClient }).unwrap();
        // additional sessions
        config.add(BgpSession { source: n0, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n1, target: n0, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n2, target: n0, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n3, target: n0, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n4, target: n0, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n5, target: n0, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n6, target: n0, session_type: IBgpClient }).unwrap();
        //new config
        config.add(BgpSession { source: rx, target: t1, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: ry, target: t2, session_type: IBgpClient }).unwrap();
        // old removed config
        //config.add(BgpSession { source: r2, target: b2, session_type: IBgpClient }).unwrap();
        // additional new sessions
        config.add(BgpSession { source: n1, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n2, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n3, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n4, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n5, target: bx, session_type: IBgpClient }).unwrap();
        config.add(BgpSession { source: n6, target: bx, session_type: IBgpClient }).unwrap();

        config
    }

    fn get_policy(net: &Network, _variant: usize) -> HardPolicy {
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter())
    }
}

/// # Difficult Gadget Repeated
/// This is the `DifficultGadget`, but it repeats itself multiple times. They all connect at the
/// router `bx`, and both `bx`and `ex` are shared amongst all n copies.
///
/// ![DifficultGadgetRepeated](https://n.ethz.ch/~sctibor/images/DifficultGadgetRepeated.svg)
pub struct DifficultGadgetRepeated<R = Repetition3> {
    phantom: PhantomData<R>,
}

impl<R> ExampleNetwork for DifficultGadgetRepeated<R>
where
    R: Repetitions,
{
    fn net(initial_variant: usize) -> Network {
        let mut net = Network::new();

        // create the shared routers which fonnect all the n copies
        let bx = net.add_router(String::from("s_bx"));
        let ex = net.add_external_router(String::from("s_ex"), AsId(65100));

        net.add_link(ex, bx);

        for i in 0..R::get_count() {
            let ry = net.add_router(format!("{:02}_ry", i));
            let r1 = net.add_router(format!("{:02}_r1", i));
            let r2 = net.add_router(format!("{:02}_r2", i));
            let b1 = net.add_router(format!("{:02}_b1", i));
            let b2 = net.add_router(format!("{:02}_b2", i));
            let t1 = net.add_router(format!("{:02}_t1", i));
            let t2 = net.add_router(format!("{:02}_t2", i));
            let e1 = net.add_external_router(format!("{:02}_e1", i), AsId(65101));
            let e2 = net.add_external_router(format!("{:02}_e2", i), AsId(65102));
            let rx = net.add_router(format!("{:02}_rx", i)); // this needs to be last, because we want this update to be after the ry update

            net.add_link(bx, rx);
            net.add_link(bx, ry);
            net.add_link(rx, r2);
            net.add_link(ry, r1);
            net.add_link(r1, b1);
            net.add_link(r2, b2);
            net.add_link(t1, b1);
            net.add_link(t2, b2);
            net.add_link(b1, e1);
            net.add_link(b2, e2);
            net.add_link(rx, ry);
        }

        // apply initial config
        let cf = Self::initial_config(&net, initial_variant);
        net.set_config(&cf).unwrap();

        // advertise prefixes
        net.advertise_external_route(ex, Prefix(0), vec![AsId(65100), AsId(65200)], None, None)
            .unwrap();

        for i in 0..R::get_count() {
            let e1 = net.get_router_id(&format!("{:02}_e1", i)).unwrap();
            let e2 = net.get_router_id(&format!("{:02}_e2", i)).unwrap();
            net.advertise_external_route(e1, Prefix(0), vec![AsId(65101), AsId(65200)], None, None)
                .unwrap();
            net.advertise_external_route(e2, Prefix(0), vec![AsId(65102), AsId(65200)], None, None)
                .unwrap();
        }

        net
    }

    fn initial_config(net: &Network, _variant: usize) -> Config {
        let mut config = Config::new();

        let bx = net.get_router_id("s_bx").unwrap();
        let ex = net.get_router_id("s_ex").unwrap();

        // add the setting for bx and ex
        config.add(IgpLinkWeight { source: ex, target: bx, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: ex, source: bx, weight: 1.0 }).unwrap();
        config.add(BgpSession { source: bx, target: ex, session_type: EBgp }).unwrap();

        for i in 0..R::get_count() {
            let rx = net.get_router_id(&format!("{:02}_rx", i)).unwrap();
            let ry = net.get_router_id(&format!("{:02}_ry", i)).unwrap();
            let r1 = net.get_router_id(&format!("{:02}_r1", i)).unwrap();
            let r2 = net.get_router_id(&format!("{:02}_r2", i)).unwrap();
            let b1 = net.get_router_id(&format!("{:02}_b1", i)).unwrap();
            let b2 = net.get_router_id(&format!("{:02}_b2", i)).unwrap();
            let t1 = net.get_router_id(&format!("{:02}_t1", i)).unwrap();
            let t2 = net.get_router_id(&format!("{:02}_t2", i)).unwrap();
            let e1 = net.get_router_id(&format!("{:02}_e1", i)).unwrap();
            let e2 = net.get_router_id(&format!("{:02}_e2", i)).unwrap();

            config.add(IgpLinkWeight { source: bx, target: rx, weight: 10.0 }).unwrap();
            config.add(IgpLinkWeight { source: bx, target: ry, weight: 10.0 }).unwrap();
            config.add(IgpLinkWeight { source: rx, target: r2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: ry, target: r1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: r1, target: b1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: r2, target: b2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: t1, target: b1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: t2, target: b2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: b2, target: e2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: rx, target: ry, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: bx, source: rx, weight: 10.0 }).unwrap();
            config.add(IgpLinkWeight { target: bx, source: ry, weight: 10.0 }).unwrap();
            config.add(IgpLinkWeight { target: rx, source: r2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: ry, source: r1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: r1, source: b1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: r2, source: b2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: t1, source: b1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: t2, source: b2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: b1, source: e1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: b2, source: e2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: rx, source: ry, weight: 1.0 }).unwrap();

            config.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
            config.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();
            config.add(BgpSession { source: rx, target: ry, session_type: IBgpPeer }).unwrap();
            config.add(BgpSession { source: rx, target: bx, session_type: IBgpClient }).unwrap();
            config.add(BgpSession { source: ry, target: bx, session_type: IBgpClient }).unwrap();
            config.add(BgpSession { source: r1, target: b1, session_type: IBgpClient }).unwrap();
            config.add(BgpSession { source: r2, target: b2, session_type: IBgpClient }).unwrap();
            config.add(BgpSession { source: t1, target: b1, session_type: IBgpClient }).unwrap();
            config.add(BgpSession { source: t2, target: b2, session_type: IBgpClient }).unwrap();
            config.add(BgpSession { source: r2, target: bx, session_type: IBgpClient }).unwrap();
        }

        config
    }

    fn final_config(net: &Network, _variant: usize) -> Config {
        let mut config = Config::new();

        let bx = net.get_router_id("s_bx").unwrap();
        let ex = net.get_router_id("s_ex").unwrap();

        // add the setting for bx and ex
        config.add(IgpLinkWeight { source: ex, target: bx, weight: 1.0 }).unwrap();
        config.add(IgpLinkWeight { target: ex, source: bx, weight: 1.0 }).unwrap();
        config.add(BgpSession { source: bx, target: ex, session_type: EBgp }).unwrap();

        for i in 0..R::get_count() {
            let rx = net.get_router_id(&format!("{:02}_rx", i)).unwrap();
            let ry = net.get_router_id(&format!("{:02}_ry", i)).unwrap();
            let r1 = net.get_router_id(&format!("{:02}_r1", i)).unwrap();
            let r2 = net.get_router_id(&format!("{:02}_r2", i)).unwrap();
            let b1 = net.get_router_id(&format!("{:02}_b1", i)).unwrap();
            let b2 = net.get_router_id(&format!("{:02}_b2", i)).unwrap();
            let t1 = net.get_router_id(&format!("{:02}_t1", i)).unwrap();
            let t2 = net.get_router_id(&format!("{:02}_t2", i)).unwrap();
            let e1 = net.get_router_id(&format!("{:02}_e1", i)).unwrap();
            let e2 = net.get_router_id(&format!("{:02}_e2", i)).unwrap();

            config.add(IgpLinkWeight { source: bx, target: rx, weight: 10.0 }).unwrap();
            config.add(IgpLinkWeight { source: bx, target: ry, weight: 10.0 }).unwrap();
            config.add(IgpLinkWeight { source: rx, target: r2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: ry, target: r1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: r1, target: b1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: r2, target: b2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: t1, target: b1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: t2, target: b2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: b2, target: e2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { source: rx, target: ry, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: bx, source: rx, weight: 10.0 }).unwrap();
            config.add(IgpLinkWeight { target: bx, source: ry, weight: 10.0 }).unwrap();
            config.add(IgpLinkWeight { target: rx, source: r2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: ry, source: r1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: r1, source: b1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: r2, source: b2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: t1, source: b1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: t2, source: b2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: b1, source: e1, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: b2, source: e2, weight: 1.0 }).unwrap();
            config.add(IgpLinkWeight { target: rx, source: ry, weight: 1.0 }).unwrap();

            config.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
            config.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();
            config.add(BgpSession { source: rx, target: ry, session_type: IBgpPeer }).unwrap();
            config.add(BgpSession { source: rx, target: bx, session_type: IBgpClient }).unwrap();
            config.add(BgpSession { source: ry, target: bx, session_type: IBgpClient }).unwrap();
            config.add(BgpSession { source: r1, target: b1, session_type: IBgpClient }).unwrap();
            config.add(BgpSession { source: t1, target: b1, session_type: IBgpClient }).unwrap();
            config.add(BgpSession { source: t2, target: b2, session_type: IBgpClient }).unwrap();
            config.add(BgpSession { source: r2, target: bx, session_type: IBgpClient }).unwrap();
            //new config
            config.add(BgpSession { source: rx, target: t1, session_type: IBgpClient }).unwrap();
            config.add(BgpSession { source: ry, target: t2, session_type: IBgpClient }).unwrap();
            // old removed config
            //config.add(BgpSession { source: r2, target: b2, session_type: IBgpClient }).unwrap();
        }

        config
    }

    fn get_policy(net: &Network, _variant: usize) -> HardPolicy {
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter())
    }
}
