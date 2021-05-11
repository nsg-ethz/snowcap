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

//! # Chain Gadget
//! This topology consists of a single chain (line) of `N` routers. on each end of the line, we have
//! two border routers, which both receive the same advertisement. The reconfiguration requires the
//! routers to be reconfigured from the right to the left. There is only one single valid solution.

use super::{
    repetitions::{Repetition3, Repetitions},
    ExampleNetwork,
};
use crate::hard_policies::HardPolicy;
use crate::netsim::config::{Config, ConfigExpr::*};
use crate::netsim::route_map::*;
use crate::netsim::{AsId, BgpSessionType::*, Network, Prefix};
use std::marker::PhantomData;

/// # Chain Gadget
/// This gadget consists of N+2 internal routers, and two external routers, arranged as follows:
///
/// ![ChainGadget](https://n.ethz.ch/~sctibor/images/ChainGadget.svg)
///
/// All links weights are set to 1. Both `e0` and `e1` are external routers, and advertise the same
/// prefix to their neighbors. `b0` sets the `local_pref` to 50, while `b1` keeps its `local_pref`
/// at 100.
///
/// In the initial configuration, all routers ri, (from `r0` up to `r(n-1)`) have iBGP peer
/// connections to `b0`, and in the final configuration, all routers have an iBGP peer connection
/// to `b1`. To avoid loops, the router `r(N-1)` must be configured before `r(N-2)`, and so on.
/// There exists only one single valid solution to this problem.
pub struct ChainGadget<R = Repetition3> {
    phantom: PhantomData<R>,
}

impl<R> ExampleNetwork for ChainGadget<R>
where
    R: Repetitions,
{
    fn net(initial_variant: usize) -> Network {
        let mut net = Network::new();

        let e0 = net.add_external_router(String::from("e00"), AsId(65100));
        let e1 = net.add_external_router(String::from("e01"), AsId(65101));
        let b0 = net.add_router(String::from("b00"));
        let b1 = net.add_router(String::from("b01"));

        net.add_link(e0, b0);
        net.add_link(e1, b1);

        let mut current_r = net.add_router(String::from("r00"));
        let mut last_r = b0;
        net.add_link(current_r, last_r);

        for i in 1..R::get_count() {
            last_r = current_r;
            current_r = net.add_router(format!("r{:02}", i));
            net.add_link(current_r, last_r);
        }

        net.add_link(current_r, b1);

        // apply initial config
        let cf = Self::initial_config(&net, initial_variant);
        net.set_config(&cf).unwrap();

        // advertise prefixes
        net.advertise_external_route(e0, Prefix(0), vec![AsId(65100), AsId(65200)], None, None)
            .unwrap();
        net.advertise_external_route(e1, Prefix(0), vec![AsId(65101), AsId(65200)], None, None)
            .unwrap();

        net
    }

    fn initial_config(net: &Network, _variant: usize) -> Config {
        let mut c = Config::new();

        let e0 = net.get_router_id("e00").unwrap();
        let e1 = net.get_router_id("e01").unwrap();
        let b0 = net.get_router_id("b00").unwrap();
        let b1 = net.get_router_id("b01").unwrap();

        // add the sessions and the link weights of the tail routers
        c.add(IgpLinkWeight { source: e0, target: b0, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b0, target: e0, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e1, target: b1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
        c.add(BgpSession { source: e0, target: b0, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: e1, target: b1, session_type: EBgp }).unwrap();

        // set the local pref
        c.add(BgpRouteMap {
            router: b0,
            direction: RouteMapDirection::Incoming,
            map: RouteMapBuilder::new()
                .order(10)
                .allow()
                .match_neighbor(e0)
                .set_local_pref(50)
                .build(),
        })
        .unwrap();

        // add all the other link weights
        let mut current_r = net.get_router_id(String::from("r00")).unwrap();
        let mut last_r = b0;
        c.add(IgpLinkWeight { source: current_r, target: last_r, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: current_r, source: last_r, weight: 1.0 }).unwrap();

        for i in 1..R::get_count() {
            last_r = current_r;
            current_r = net.get_router_id(&format!("r{:02}", i)).unwrap();
            c.add(IgpLinkWeight { source: current_r, target: last_r, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: current_r, source: last_r, weight: 1.0 }).unwrap();
        }

        last_r = current_r;
        current_r = b1;
        c.add(IgpLinkWeight { source: current_r, target: last_r, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: current_r, source: last_r, weight: 1.0 }).unwrap();

        // set all other bgp sessions
        for i in 0..R::get_count() {
            let r = net.get_router_id(&format!("r{:02}", i)).unwrap();
            c.add(BgpSession { source: r, target: b0, session_type: IBgpPeer }).unwrap();
        }

        c
    }

    fn final_config(net: &Network, _variant: usize) -> Config {
        let mut c = Config::new();

        let e0 = net.get_router_id("e00").unwrap();
        let e1 = net.get_router_id("e01").unwrap();
        let b0 = net.get_router_id("b00").unwrap();
        let b1 = net.get_router_id("b01").unwrap();

        // add the sessions and the link weights of the tail routers
        c.add(IgpLinkWeight { source: e0, target: b0, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b0, target: e0, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e1, target: b1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
        c.add(BgpSession { source: e0, target: b0, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: e1, target: b1, session_type: EBgp }).unwrap();

        // set the local pref
        c.add(BgpRouteMap {
            router: b0,
            direction: RouteMapDirection::Incoming,
            map: RouteMapBuilder::new()
                .order(10)
                .allow()
                .match_neighbor(e0)
                .set_local_pref(50)
                .build(),
        })
        .unwrap();

        // add all the other link weights
        let mut current_r = net.get_router_id(String::from("r00")).unwrap();
        let mut last_r = b0;
        c.add(IgpLinkWeight { source: current_r, target: last_r, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: current_r, source: last_r, weight: 1.0 }).unwrap();

        for i in 1..R::get_count() {
            last_r = current_r;
            current_r = net.get_router_id(&format!("r{:02}", i)).unwrap();
            c.add(IgpLinkWeight { source: current_r, target: last_r, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: current_r, source: last_r, weight: 1.0 }).unwrap();
        }

        last_r = current_r;
        current_r = b1;
        c.add(IgpLinkWeight { source: current_r, target: last_r, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: current_r, source: last_r, weight: 1.0 }).unwrap();

        // set all other bgp sessions
        for i in 0..R::get_count() {
            let r = net.get_router_id(&format!("r{:02}", i)).unwrap();
            c.add(BgpSession { source: r, target: b0, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: r, target: b1, session_type: IBgpPeer }).unwrap();
        }

        c
    }

    fn get_policy(net: &Network, _variant: usize) -> HardPolicy {
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter())
    }
}

/// # State Specific Chain Gadget
///
/// This is the same as the [`ChainGadget`], with a few differences. The first difference is that
/// the link from `r(N-1)` to `b1` is set to a really high value, such that the routers prefer
/// routes via `b1` in the normal case. During reconfiguration, the local-pref of both routers is
/// increased to 200. If `b1` is reconfigured before `b0`, then the traffic temporarily shifts
/// towards `b1`, which may cause forwarding loops. When `b0` is finally reconfigured, the problem
/// is solved.
///
/// ![StateSpecificChainGadget](https://n.ethz.ch/~sctibor/images/StateSpecificChainGadget.svg)
pub struct StateSpecificChainGadget<R = Repetition3> {
    phantom: PhantomData<R>,
}

impl<R> ExampleNetwork for StateSpecificChainGadget<R>
where
    R: Repetitions,
{
    fn net(initial_variant: usize) -> Network {
        let mut net = Network::new();

        let e0 = net.add_external_router(String::from("e00"), AsId(65100));
        let e1 = net.add_external_router(String::from("e01"), AsId(65101));
        let b0 = net.add_router(String::from("b00"));
        let b1 = net.add_router(String::from("b01"));

        net.add_link(e0, b0);
        net.add_link(e1, b1);

        let mut current_r = net.add_router(String::from("r00"));
        let mut last_r = b0;
        net.add_link(current_r, last_r);

        for i in 1..R::get_count() {
            last_r = current_r;
            current_r = net.add_router(format!("r{:02}", i));
            net.add_link(current_r, last_r);
        }

        net.add_link(current_r, b1);

        // apply initial config
        let cf = Self::initial_config(&net, initial_variant);
        net.set_config(&cf).unwrap();

        // advertise prefixes
        net.advertise_external_route(e0, Prefix(0), vec![AsId(65100), AsId(65200)], None, None)
            .unwrap();
        net.advertise_external_route(e1, Prefix(0), vec![AsId(65101), AsId(65200)], None, None)
            .unwrap();

        net
    }

    fn initial_config(net: &Network, _variant: usize) -> Config {
        let mut c = Config::new();

        let e0 = net.get_router_id("e00").unwrap();
        let e1 = net.get_router_id("e01").unwrap();
        let b0 = net.get_router_id("b00").unwrap();
        let b1 = net.get_router_id("b01").unwrap();

        // add the sessions and the link weights of the tail routers
        c.add(IgpLinkWeight { source: e0, target: b0, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b0, target: e0, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e1, target: b1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
        c.add(BgpSession { source: e0, target: b0, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: e1, target: b1, session_type: EBgp }).unwrap();

        // add all the other link weights
        let mut current_r = net.get_router_id(String::from("r00")).unwrap();
        let mut last_r = b0;
        c.add(IgpLinkWeight { source: current_r, target: last_r, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: current_r, source: last_r, weight: 1.0 }).unwrap();

        for i in 1..R::get_count() {
            last_r = current_r;
            current_r = net.get_router_id(&format!("r{:02}", i)).unwrap();
            c.add(IgpLinkWeight { source: current_r, target: last_r, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: current_r, source: last_r, weight: 1.0 }).unwrap();
        }

        last_r = current_r;
        current_r = b1;
        c.add(IgpLinkWeight { source: current_r, target: last_r, weight: 1000.0 }).unwrap();
        c.add(IgpLinkWeight { target: current_r, source: last_r, weight: 1000.0 }).unwrap();

        // set all other bgp sessions
        for i in 0..R::get_count() {
            let r = net.get_router_id(&format!("r{:02}", i)).unwrap();
            c.add(BgpSession { source: r, target: b0, session_type: IBgpPeer }).unwrap();
        }

        c
    }

    fn final_config(net: &Network, _variant: usize) -> Config {
        let mut c = Config::new();

        let e0 = net.get_router_id("e00").unwrap();
        let e1 = net.get_router_id("e01").unwrap();
        let b0 = net.get_router_id("b00").unwrap();
        let b1 = net.get_router_id("b01").unwrap();

        // add the sessions and the link weights of the tail routers
        c.add(IgpLinkWeight { source: e0, target: b0, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b0, target: e0, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e1, target: b1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
        c.add(BgpSession { source: e0, target: b0, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: e1, target: b1, session_type: EBgp }).unwrap();

        // set the local pref
        c.add(BgpRouteMap {
            router: b0,
            direction: RouteMapDirection::Incoming,
            map: RouteMapBuilder::new()
                .order(10)
                .allow()
                .match_neighbor(e0)
                .set_local_pref(200)
                .build(),
        })
        .unwrap();
        c.add(BgpRouteMap {
            router: b1,
            direction: RouteMapDirection::Incoming,
            map: RouteMapBuilder::new()
                .order(10)
                .allow()
                .match_neighbor(e1)
                .set_local_pref(200)
                .build(),
        })
        .unwrap();

        // add all the other link weights
        let mut current_r = net.get_router_id(String::from("r00")).unwrap();
        let mut last_r = b0;
        c.add(IgpLinkWeight { source: current_r, target: last_r, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: current_r, source: last_r, weight: 1.0 }).unwrap();

        for i in 1..R::get_count() {
            last_r = current_r;
            current_r = net.get_router_id(&format!("r{:02}", i)).unwrap();
            c.add(IgpLinkWeight { source: current_r, target: last_r, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: current_r, source: last_r, weight: 1.0 }).unwrap();
        }

        last_r = current_r;
        current_r = b1;
        c.add(IgpLinkWeight { source: current_r, target: last_r, weight: 1000.0 }).unwrap();
        c.add(IgpLinkWeight { target: current_r, source: last_r, weight: 1000.0 }).unwrap();

        // set all other bgp sessions
        for i in 0..R::get_count() {
            let r = net.get_router_id(&format!("r{:02}", i)).unwrap();
            c.add(BgpSession { source: r, target: b0, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: r, target: b1, session_type: IBgpPeer }).unwrap();
        }

        c
    }

    fn get_policy(net: &Network, _variant: usize) -> HardPolicy {
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::netsim::config::ConfigModifier::Insert;
    #[test]
    fn state_specific_chain_gadget() {
        let mut net = StateSpecificChainGadget::<Repetition3>::net(0);
        let e1 = net.get_router_id("e01").unwrap();
        let b1 = net.get_router_id("b01").unwrap();
        let r1 = net.get_router_id("r01").unwrap();
        net.apply_modifier(&Insert(BgpSession { source: r1, target: b1, session_type: IBgpPeer }))
            .unwrap();
        net.apply_modifier(&Insert(BgpRouteMap {
            router: b1,
            direction: RouteMapDirection::Incoming,
            map: RouteMapBuilder::new()
                .order(10)
                .allow()
                .match_neighbor(e1)
                .set_local_pref(200)
                .build(),
        }))
        .unwrap();
        assert!(net.get_route(r1, Prefix(0)).is_err());
    }
}
