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

//! # Fusion of Bipartite Gadget and the Carousel Gadget
//! Generalized version of the difficult gadget

use super::{
    repetitions::{Repetition1, Repetitions},
    ExampleNetwork,
};
use crate::hard_policies::HardPolicy;
use crate::netsim::config::{Config, ConfigExpr::*};
use crate::netsim::route_map::*;
use crate::netsim::{AsId, BgpSessionType::*, Network, Prefix, RouterId};
use std::marker::PhantomData;

/// # Bipartite Gadget + Carousel Gadget
///
/// This is a fusion of the [Bipartite Gadget](super::BipartiteGadget) and the
/// [Carousel Gadget](super::CarouselGadget). The Carousel Gadget is connected to the rest of the
/// network at the router `bx` (which replaces router `rr`).
///
/// The repetitions are configured in the following way:
/// - `BipartiteR` (Type Argument): Number of (solvable) bipartite groups in the network.
/// - `CarouselR` (Type Argument): Number of (unsolvable) carousel groups in the network.
/// - `variant` (function argument): Size of each (solvable) bipartite group in the network.
pub struct BipartiteCarouselFusion<BipartiteR = Repetition1, CarouselR = Repetition1> {
    phantom: PhantomData<(BipartiteR, CarouselR)>,
}

impl<BipartiteR, CarouselR> ExampleNetwork for BipartiteCarouselFusion<BipartiteR, CarouselR>
where
    BipartiteR: Repetitions,
    CarouselR: Repetitions,
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

        // generate all groups of the bipartite gadget
        for i in 0..BipartiteR::get_count() {
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

        // generate all groups of the carousel gadget
        for i in BipartiteR::get_count()..(BipartiteR::get_count() + CarouselR::get_count()) {
            let r1 = net.add_router(format!("{:02}_r1", i));
            let r2 = net.add_router(format!("{:02}_r2", i));
            let r3 = net.add_router(format!("{:02}_r3", i));
            let r4 = net.add_router(format!("{:02}_r4", i));
            let b1 = net.add_router(format!("{:02}_b1", i));
            let b2 = net.add_router(format!("{:02}_b2", i));
            let b3 = net.add_router(format!("{:02}_b3", i));
            let b4 = net.add_router(format!("{:02}_b4", i));
            let e1 = net.add_external_router(format!("{:02}_e1", i), AsId(65101));
            let e2 = net.add_external_router(format!("{:02}_e2", i), AsId(65102));
            let e3 = net.add_external_router(format!("{:02}_e3", i), AsId(65103));
            let e4 = net.add_external_router(format!("{:02}_e4", i), AsId(65104));

            all_e.push(e1);
            all_e.push(e2);
            all_e.push(e3);
            all_e.push(e4);

            net.add_link(bx, r1);
            net.add_link(bx, r2);
            net.add_link(bx, r3);
            net.add_link(bx, r4);
            net.add_link(r1, r2);
            net.add_link(r1, b2);
            net.add_link(r1, b3);
            net.add_link(r2, b1);
            net.add_link(r3, b4);
            net.add_link(r4, r3);
            net.add_link(r4, b2);
            net.add_link(r4, b3);
            net.add_link(b1, e1);
            net.add_link(b2, e2);
            net.add_link(b3, e3);
            net.add_link(b4, e4);
        }

        // apply configuration
        let cf = Self::initial_config(&net, initial_variant);
        net.set_config(&cf).unwrap();

        // advertise prefixes
        net.advertise_external_route(ex, Prefix(0), vec![AsId(65100), AsId(65534)], None, None)
            .unwrap();
        net.advertise_external_route(ex, Prefix(1), vec![AsId(65100), AsId(65535)], None, None)
            .unwrap();

        for e in all_e {
            let e_as = net.get_device(e).unwrap_external().as_id();
            net.advertise_external_route(e, Prefix(0), vec![e_as, AsId(65534)], None, None)
                .unwrap();
            net.advertise_external_route(e, Prefix(1), vec![e_as, AsId(65535)], None, None)
                .unwrap();
        }

        net
    }

    fn initial_config(net: &Network, variant: usize) -> Config {
        // check that we have the correct variant, by checking the number of routers
        assert!(variant > 1);
        assert_eq!(
            net.get_routers().len() + net.get_external_routers().len(),
            2 + BipartiteR::get_count() * variant * 5 + CarouselR::get_count() * 12
        );

        let mut c = Config::new();

        let bx = net.get_router_id("bx").unwrap();
        let ex = net.get_router_id("ex").unwrap();

        // add the setting for bx and ex
        c.add(IgpLinkWeight { source: ex, target: bx, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: ex, source: bx, weight: 1.0 }).unwrap();
        c.add(BgpSession { source: bx, target: ex, session_type: EBgp }).unwrap();

        // configure all groups of the bipartite gadget
        for i in 0..BipartiteR::get_count() {
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

        // configure all groups of the carousel gadget
        for i in BipartiteR::get_count()..(BipartiteR::get_count() + CarouselR::get_count()) {
            let r1 = net.get_router_id(format!("{:02}_r1", i)).unwrap();
            let r2 = net.get_router_id(format!("{:02}_r2", i)).unwrap();
            let r3 = net.get_router_id(format!("{:02}_r3", i)).unwrap();
            let r4 = net.get_router_id(format!("{:02}_r4", i)).unwrap();
            let b1 = net.get_router_id(format!("{:02}_b1", i)).unwrap();
            let b2 = net.get_router_id(format!("{:02}_b2", i)).unwrap();
            let b3 = net.get_router_id(format!("{:02}_b3", i)).unwrap();
            let b4 = net.get_router_id(format!("{:02}_b4", i)).unwrap();
            let e1 = net.get_router_id(format!("{:02}_e1", i)).unwrap();
            let e2 = net.get_router_id(format!("{:02}_e2", i)).unwrap();
            let e3 = net.get_router_id(format!("{:02}_e3", i)).unwrap();
            let e4 = net.get_router_id(format!("{:02}_e4", i)).unwrap();

            // link weight
            c.add(IgpLinkWeight { source: bx, target: r1, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { source: bx, target: r2, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { source: bx, target: r3, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { source: bx, target: r4, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { source: r1, target: r2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: r1, target: b2, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { source: r1, target: b3, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: r2, target: b1, weight: 9.0 }).unwrap();
            c.add(IgpLinkWeight { source: r3, target: r4, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: r3, target: b4, weight: 9.0 }).unwrap();
            c.add(IgpLinkWeight { source: r4, target: b2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: r4, target: b3, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: b2, target: e2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: b3, target: e3, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: b4, target: e4, weight: 1.0 }).unwrap();
            // symmetric weight
            c.add(IgpLinkWeight { target: bx, source: r1, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { target: bx, source: r2, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { target: bx, source: r3, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { target: bx, source: r4, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { target: r1, source: r2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: r1, source: b2, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { target: r1, source: b3, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: r2, source: b1, weight: 9.0 }).unwrap();
            c.add(IgpLinkWeight { target: r3, source: r4, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: r3, source: b4, weight: 9.0 }).unwrap();
            c.add(IgpLinkWeight { target: r4, source: b2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: r4, source: b3, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { target: b1, source: e1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: b2, source: e2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: b3, source: e3, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: b4, source: e4, weight: 1.0 }).unwrap();

            // bgp sessions
            c.add(BgpSession { source: bx, target: r1, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: bx, target: r2, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: bx, target: r3, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: bx, target: r4, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r1, target: b1, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r1, target: b3, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r2, target: b1, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r2, target: b2, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r2, target: b3, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r3, target: b2, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r3, target: b3, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r3, target: b4, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r4, target: b2, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r4, target: b4, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: b3, target: e3, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: b4, target: e4, session_type: EBgp }).unwrap();

            // local pref setting
            c.add(BgpRouteMap {
                router: b2,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .order(10)
                    .allow()
                    .match_neighbor(e2)
                    .set_local_pref(50)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: b3,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .order(10)
                    .allow()
                    .match_neighbor(e3)
                    .set_local_pref(50)
                    .build(),
            })
            .unwrap();
        }

        c
    }

    fn final_config(net: &Network, variant: usize) -> Config {
        // check that we have the correct variant, by checking the number of routers
        assert!(variant > 1);
        assert_eq!(
            net.get_routers().len() + net.get_external_routers().len(),
            2 + BipartiteR::get_count() * variant * 5 + CarouselR::get_count() * 12
        );

        let mut c = Config::new();

        let bx = net.get_router_id("bx").unwrap();
        let ex = net.get_router_id("ex").unwrap();

        // add the setting for bx and ex
        c.add(IgpLinkWeight { source: ex, target: bx, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: ex, source: bx, weight: 1.0 }).unwrap();
        c.add(BgpSession { source: bx, target: ex, session_type: EBgp }).unwrap();

        // configure all groups
        for i in 0..BipartiteR::get_count() {
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
        // configure all groups of the carousel gadget
        for i in BipartiteR::get_count()..(BipartiteR::get_count() + CarouselR::get_count()) {
            let r1 = net.get_router_id(format!("{:02}_r1", i)).unwrap();
            let r2 = net.get_router_id(format!("{:02}_r2", i)).unwrap();
            let r3 = net.get_router_id(format!("{:02}_r3", i)).unwrap();
            let r4 = net.get_router_id(format!("{:02}_r4", i)).unwrap();
            let b1 = net.get_router_id(format!("{:02}_b1", i)).unwrap();
            let b2 = net.get_router_id(format!("{:02}_b2", i)).unwrap();
            let b3 = net.get_router_id(format!("{:02}_b3", i)).unwrap();
            let b4 = net.get_router_id(format!("{:02}_b4", i)).unwrap();
            let e1 = net.get_router_id(format!("{:02}_e1", i)).unwrap();
            let e2 = net.get_router_id(format!("{:02}_e2", i)).unwrap();
            let e3 = net.get_router_id(format!("{:02}_e3", i)).unwrap();
            let e4 = net.get_router_id(format!("{:02}_e4", i)).unwrap();

            // link weight
            c.add(IgpLinkWeight { source: bx, target: r1, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { source: bx, target: r2, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { source: bx, target: r3, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { source: bx, target: r4, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { source: r1, target: r2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: r1, target: b2, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { source: r1, target: b3, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: r2, target: b1, weight: 9.0 }).unwrap();
            c.add(IgpLinkWeight { source: r3, target: r4, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: r3, target: b4, weight: 9.0 }).unwrap();
            c.add(IgpLinkWeight { source: r4, target: b2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: r4, target: b3, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: b2, target: e2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: b3, target: e3, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: b4, target: e4, weight: 1.0 }).unwrap();
            // symmetric weight
            c.add(IgpLinkWeight { target: bx, source: r1, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { target: bx, source: r2, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { target: bx, source: r3, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { target: bx, source: r4, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { target: r1, source: r2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: r1, source: b2, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { target: r1, source: b3, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: r2, source: b1, weight: 9.0 }).unwrap();
            c.add(IgpLinkWeight { target: r3, source: r4, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: r3, source: b4, weight: 9.0 }).unwrap();
            c.add(IgpLinkWeight { target: r4, source: b2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: r4, source: b3, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { target: b1, source: e1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: b2, source: e2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: b3, source: e3, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: b4, source: e4, weight: 1.0 }).unwrap();

            // bgp sessions
            c.add(BgpSession { source: bx, target: r1, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: bx, target: r2, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: bx, target: r3, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: bx, target: r4, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r1, target: b1, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r1, target: b3, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r2, target: b1, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r2, target: b2, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r2, target: b3, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r3, target: b2, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r3, target: b3, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r3, target: b4, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r4, target: b2, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r4, target: b4, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: b3, target: e3, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: b4, target: e4, session_type: EBgp }).unwrap();
        }

        c
    }

    fn get_policy(net: &Network, _variant: usize) -> HardPolicy {
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter())
    }
}
