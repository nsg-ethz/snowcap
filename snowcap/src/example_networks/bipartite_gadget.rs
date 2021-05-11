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

//! # Bipartite Gadget
//! Generalized version of the difficult gadget

use super::{
    repetitions::{Repetition1, Repetitions},
    ExampleNetwork,
};
use crate::hard_policies::HardPolicy;
use crate::netsim::config::{Config, ConfigExpr::*};
use crate::netsim::{AsId, BgpSessionType::*, Network, Prefix, RouterId};
use std::marker::PhantomData;

/// # Bipartite Gadget
/// This gadget is a generalized version of [difficult gadget](super::DifficultGadgetMinimal).
/// Here, instead of having two branches, we can set the number dynamically. As for the repeated
/// difficult gadget, the groups can also be repeated `R` times. In each group, there are `2N - 1`
/// different modifiers. The routers of one group are named in the following convention:
///
/// ![BipartiteGadget](https://n.ethz.ch/~sctibor/images/BipartiteGadget.svg)
///
/// The idea is the same as for the difficult gadget. `x1` must advertise the route first to `t1`,
/// such that all `tI` choose `b1` as next hop for the prefix. Then, no matter in what order the
/// remaining `xI` (for `I` in `[1..N)`) advertise their prefix to `tI`, it will not change anything
/// in the forwarding state. Then, to cause the error, we remove the bgp sessions from `rI` to `bI`
/// for `I` in `[1..N)`, such that all those routers choose `rx` as next hop, thus causing a
/// forwarding loop if `xI` has advertised the prefix before `x1`.
///
/// This means, that every group only has one requirement, that the BGP session `t1` --> `x1` is
/// established before any other `tI` -> `xI`, for all `I` in `[1..N)`. If `xI` has advertised
/// first, then the problem arises when the session `rI` --> `bI` is removed. Thus, the group is
/// more complicated than for the difficult gadget, because there are more valid solutions than
/// just one, but the single dependency group is still larger.
///
/// The parameter `N` is set with the variant parameter. the number of repetitions is set with the
/// type argument.
///
/// ## IGP Topology
/// - `bI --- eI` (weight 1), for all `I` in `[x, 0..N)`.
/// - `bI --- xI` (weight 1), for all `I` in `[0..N)`.
/// - `bI --- rI` (weight 1), for all `I` in `[0..N)`.
/// - `tI --- bx` (weight 10), for all `I` in `[0..N)`.
/// - `rI --- tJ` (weight 1) for all `I` in [0..N), `J` in `[0..N)` and `I != J`
/// - `tI --- tJ` (weight 1), for all `I` in `[0..N)`, `J` in `[0..N)` and `I != J` (full-mesh).
///
/// ## BGP Topology before reconfiguration
/// - `bI --> eI` (eBGP), for all `I` in [`x, 1..N)`.
/// - `tI --> bx` (iBGP RR), for all `I` in `[0..N)`.
/// - `rI --> bI` (iBGP RR), for all `I` in `[0..N)`.
/// - `xI --> bI` (iBGP RR), for all `I` in `[0..N)`.
/// - `xI --> bx` (iBGP RR), for all `I` in `[1..N)`.
/// - `tI --- tJ` (iBGP Peer), for all `I` in `[0..N)`, `J` in `[0..N)` and `I != J` (full-mesh).
///
/// ## Reconfiguration
/// - Add BGP session `tI --> xI` (iBGP RR), for all `I` in `[0..N)`
/// - remove BGP session `rI --> bI` (iBGP RR) for all `I` in `[1..N)`
///
/// ## Repetitions and Naming Convention
/// Each repetition is connected to the same `ex`. The routers of each repetition are named in the
/// following way:
/// ```text
/// {id:02}_[b|e|r|t|x]{num:02}
/// ```
pub struct BipartiteGadget<R = Repetition1> {
    phantom: PhantomData<R>,
}

impl<R> ExampleNetwork for BipartiteGadget<R>
where
    R: Repetitions,
{
    fn net(initial_variant: usize) -> Network {
        assert!(initial_variant > 1);

        let mut net = Network::new();

        // create the shared routers which fonnect all the n copies
        let bx = net.add_router(String::from("bx"));
        let ex = net.add_external_router(String::from("ex"), AsId(65100));

        net.add_link(ex, bx);

        // this is a vector storing all router ids for advertising the prefix later.
        let mut all_e: Vec<RouterId> = Vec::new();

        // generate all groups
        for i in 0..R::get_count() {
            let mut all_t: Vec<RouterId> = Vec::new();
            let mut all_r: Vec<RouterId> = Vec::new();
            for n in (0..initial_variant).rev() {
                // generate the routers
                let router_t = net.add_router(format!("{:02}_t{:02}", i, n));
                let router_r = net.add_router(format!("{:02}_r{:02}", i, n));
                let router_b = net.add_router(format!("{:02}_b{:02}", i, n));
                let router_x = net.add_router(format!("{:02}_x{:02}", i, n));
                let router_e = net
                    .add_external_router(format!("{:02}_e{:02}", i, n), AsId((n * 100 + i) as u32));

                // add all non-fullmesh
                net.add_link(router_t, bx);
                net.add_link(router_r, router_b);
                net.add_link(router_x, router_b);
                net.add_link(router_b, router_e);

                // add full-mesh links of tI, and between tI and rJ
                all_t.iter().for_each(|t| net.add_link(*t, router_t));
                all_t.iter().for_each(|t| net.add_link(*t, router_r));
                all_r.iter().for_each(|r| net.add_link(*r, router_t));

                // add router_r and router_t to the already existing vector
                all_t.push(router_t);
                all_r.push(router_r);
                all_e.push(router_e);
            }
        }

        // apply configuration
        let cf = Self::initial_config(&net, initial_variant);
        net.set_config(&cf).unwrap();

        // advertise prefixes
        net.advertise_external_route(ex, Prefix(0), vec![AsId(65100), AsId(65200)], None, None)
            .unwrap();

        for e in all_e {
            let e_as = net.get_device(e).unwrap_external().as_id();
            net.advertise_external_route(e, Prefix(0), vec![e_as, AsId(65535)], None, None)
                .unwrap();
        }

        net
    }

    fn initial_config(net: &Network, variant: usize) -> Config {
        // check that we have the correct variant, by checking the number of routers
        assert!(variant > 1);
        assert!(
            net.get_routers().len() + net.get_external_routers().len()
                == 2 + R::get_count() * variant * 5
        );

        let mut c = Config::new();

        let bx = net.get_router_id("bx").unwrap();
        let ex = net.get_router_id("ex").unwrap();

        // add the setting for bx and ex
        c.add(IgpLinkWeight { source: ex, target: bx, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: ex, source: bx, weight: 1.0 }).unwrap();
        c.add(BgpSession { source: bx, target: ex, session_type: EBgp }).unwrap();

        // configure all groups
        for i in 0..R::get_count() {
            let mut all_t: Vec<RouterId> = Vec::new();
            let mut all_r: Vec<RouterId> = Vec::new();
            // go through all branches in the group
            for n in (0..variant).rev() {
                // generate the routers
                let rt = net.get_router_id(format!("{:02}_t{:02}", i, n)).unwrap();
                let rr = net.get_router_id(format!("{:02}_r{:02}", i, n)).unwrap();
                let rb = net.get_router_id(format!("{:02}_b{:02}", i, n)).unwrap();
                let rx = net.get_router_id(format!("{:02}_x{:02}", i, n)).unwrap();
                let re = net.get_router_id(format!("{:02}_e{:02}", i, n)).unwrap();

                // add all non-fullmesh link weights
                c.add(IgpLinkWeight { source: rt, target: bx, weight: 10.0 }).unwrap();
                c.add(IgpLinkWeight { target: rt, source: bx, weight: 10.0 }).unwrap();
                c.add(IgpLinkWeight { source: rr, target: rb, weight: 1.0 }).unwrap();
                c.add(IgpLinkWeight { target: rr, source: rb, weight: 1.0 }).unwrap();
                c.add(IgpLinkWeight { source: rx, target: rb, weight: 1.0 }).unwrap();
                c.add(IgpLinkWeight { target: rx, source: rb, weight: 1.0 }).unwrap();
                c.add(IgpLinkWeight { source: rb, target: re, weight: 1.0 }).unwrap();
                c.add(IgpLinkWeight { target: rb, source: re, weight: 1.0 }).unwrap();

                // add full-mesh link weights of tI, and between tI and rJ
                all_t.iter().for_each(|t| {
                    c.add(IgpLinkWeight { source: *t, target: rt, weight: 1.0 }).unwrap();
                    c.add(IgpLinkWeight { target: *t, source: rt, weight: 1.0 }).unwrap();
                });
                all_t.iter().for_each(|t| {
                    c.add(IgpLinkWeight { source: *t, target: rr, weight: 1.0 }).unwrap();
                    c.add(IgpLinkWeight { target: *t, source: rr, weight: 1.0 }).unwrap();
                });
                all_r.iter().for_each(|r| {
                    c.add(IgpLinkWeight { source: *r, target: rt, weight: 1.0 }).unwrap();
                    c.add(IgpLinkWeight { target: *r, source: rt, weight: 1.0 }).unwrap();
                });

                // add BGP sessions
                c.add(BgpSession { source: rb, target: re, session_type: EBgp }).unwrap();
                c.add(BgpSession { source: rx, target: rb, session_type: IBgpClient }).unwrap();
                c.add(BgpSession { source: rr, target: rb, session_type: IBgpClient }).unwrap();
                c.add(BgpSession { source: rt, target: bx, session_type: IBgpClient }).unwrap();

                // add BGP full mesh for tI
                all_t.iter().for_each(|t| {
                    c.add(BgpSession { source: rt, target: *t, session_type: IBgpPeer }).unwrap()
                });

                // add the problematic session only for n > 0
                if n > 0 {
                    c.add(BgpSession { source: rr, target: bx, session_type: IBgpClient }).unwrap();
                }

                // add router_r and router_t to the already existing vector
                all_t.push(rt);
                all_r.push(rr);
            }
        }

        c
    }

    fn final_config(net: &Network, variant: usize) -> Config {
        // check that we have the correct variant, by checking the number of routers
        assert!(variant > 1);
        assert!(
            net.get_routers().len() + net.get_external_routers().len()
                == 2 + R::get_count() * variant * 5
        );

        let mut c = Config::new();

        let bx = net.get_router_id("bx").unwrap();
        let ex = net.get_router_id("ex").unwrap();

        // add the setting for bx and ex
        c.add(IgpLinkWeight { source: ex, target: bx, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: ex, source: bx, weight: 1.0 }).unwrap();
        c.add(BgpSession { source: bx, target: ex, session_type: EBgp }).unwrap();

        // configure all groups
        for i in 0..R::get_count() {
            let mut all_t: Vec<RouterId> = Vec::new();
            let mut all_r: Vec<RouterId> = Vec::new();
            // go through all branches in the group
            for n in (0..variant).rev() {
                // generate the routers
                let rt = net.get_router_id(format!("{:02}_t{:02}", i, n)).unwrap();
                let rr = net.get_router_id(format!("{:02}_r{:02}", i, n)).unwrap();
                let rb = net.get_router_id(format!("{:02}_b{:02}", i, n)).unwrap();
                let rx = net.get_router_id(format!("{:02}_x{:02}", i, n)).unwrap();
                let re = net.get_router_id(format!("{:02}_e{:02}", i, n)).unwrap();

                // add all non-fullmesh link weights
                c.add(IgpLinkWeight { source: rt, target: bx, weight: 10.0 }).unwrap();
                c.add(IgpLinkWeight { target: rt, source: bx, weight: 10.0 }).unwrap();
                c.add(IgpLinkWeight { source: rr, target: rb, weight: 1.0 }).unwrap();
                c.add(IgpLinkWeight { target: rr, source: rb, weight: 1.0 }).unwrap();
                c.add(IgpLinkWeight { source: rx, target: rb, weight: 1.0 }).unwrap();
                c.add(IgpLinkWeight { target: rx, source: rb, weight: 1.0 }).unwrap();
                c.add(IgpLinkWeight { source: rb, target: re, weight: 1.0 }).unwrap();
                c.add(IgpLinkWeight { target: rb, source: re, weight: 1.0 }).unwrap();

                // add full-mesh link weights of tI, and between tI and rJ
                all_t.iter().for_each(|t| {
                    c.add(IgpLinkWeight { source: *t, target: rt, weight: 1.0 }).unwrap();
                    c.add(IgpLinkWeight { target: *t, source: rt, weight: 1.0 }).unwrap();
                });
                all_t.iter().for_each(|t| {
                    c.add(IgpLinkWeight { source: *t, target: rr, weight: 1.0 }).unwrap();
                    c.add(IgpLinkWeight { target: *t, source: rr, weight: 1.0 }).unwrap();
                });
                all_r.iter().for_each(|r| {
                    c.add(IgpLinkWeight { source: *r, target: rt, weight: 1.0 }).unwrap();
                    c.add(IgpLinkWeight { target: *r, source: rt, weight: 1.0 }).unwrap();
                });

                // add BGP sessions
                c.add(BgpSession { source: rb, target: re, session_type: EBgp }).unwrap();
                c.add(BgpSession { source: rx, target: rb, session_type: IBgpClient }).unwrap();
                c.add(BgpSession { source: rt, target: bx, session_type: IBgpClient }).unwrap();

                // Differences from the initial configuration:
                // this session is added while reconfiguration
                c.add(BgpSession { source: rt, target: rx, session_type: IBgpClient }).unwrap();
                // this session is removed while reconfiguration, except for n == 0
                // c.add(BgpSession { source: rr, target: rb, session_type: IBgpClient }).unwrap();
                if n == 0 {
                    c.add(BgpSession { source: rr, target: rb, session_type: IBgpClient }).unwrap();
                }

                // add BGP full mesh for tI
                all_t.iter().for_each(|t| {
                    c.add(BgpSession { source: rt, target: *t, session_type: IBgpPeer }).unwrap()
                });

                // add the problematic session only for n > 0
                if n > 0 {
                    c.add(BgpSession { source: rr, target: bx, session_type: IBgpClient }).unwrap();
                }

                // add router_r and router_t to the already existing vector
                all_t.push(rt);
                all_r.push(rr);
            }
        }

        c
    }

    fn get_policy(net: &Network, _variant: usize) -> HardPolicy {
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter())
    }
}
