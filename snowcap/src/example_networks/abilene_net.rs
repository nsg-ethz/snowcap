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

//! [Abilene Network](http://topology-zoo.org/dataset.html)

use super::ExampleNetwork;
use crate::hard_policies::Condition::*;
use crate::hard_policies::PathCondition::*;
use crate::hard_policies::Waypoint::*;
use crate::hard_policies::*;
use crate::netsim::config::{Config, ConfigExpr::*};
use crate::netsim::route_map::*;
use crate::netsim::{AsId, BgpSessionType::*, Network, Prefix};
use itertools::iproduct;

/// # Abilene Network
///
/// This network is taken from [topology-zoo](http://topology-zoo.org/dataset.html), and consists of
/// 11 internal routers. Variant 0 and 1 are for moving all flows through a new firewall, while
/// variant 2 is used for generating more complex constraints.
///
/// ## Variant 0 + 1
///
/// The reconfiguration scenario is chosen such that only a single weight needs to be changed. In
/// the initial configuration, all traffic flows between Sunnyvale and Denver, and after the
/// reconfiguration, all traffic flows between Sunnyvale and Los Angeles. In the scenario, the old
/// firewall between Sunnyvale and Denver is replaced by a new one between Sunnyvale and Los
/// Angeles. The network operator expresses, that the traffic always traverses a firewall. There
/// are two different variants.
///
/// ### Variant 0
///
/// In this variant, Sunnyvale is the only place where the prefix is advertised. In the transient
/// state, every packet should either be dropped (or be in a forwarding loop), or traverse one of
/// the two firewalls.
///
/// ![Variant 0](https://n.ethz.ch/~sctibor/images/usa_0.svg)
///
/// ### Variant 1
///
/// In this variant, in addition to Sunnyvale, the same prefix is also advertised at Chicago, but
/// with a lower `local_pref` of 50. Here, in the transient state, there is a possibility that the
/// route via chicago is chosen, thus not traversing a firewall.
///
/// ![Variant 1](https://n.ethz.ch/~sctibor/images/usa_1.svg)
///
/// ## Variant 2 - 4
///
/// In the second variant, we assume that the network is connected to 5 external networks. Network
/// *A* and *B* are customers, network *C* is a peer, and networks *D* and *E* are providers, both
/// advertising every other prefix in the internet. The prefixes will be represented as 6 different
/// prefixes, one for all the networks, and one for the internet. The reconfiguration scenario
/// inolves changing the top-level iBGP topology from being 2-levels deep and having a single top-
/// level route reflector (Kansas City), to being 1-level deep, where all three route-reflectors
/// form a iBGP full-mesh. For the reconfiguration scenario, only the sessions between Denver,
/// Indianapoliy, Huston and Kansas City are changed.
///
/// ![Variant 2](https://n.ethz.ch/~sctibor/images/usa_2.svg)
///
/// This variant represents a more realistic network, where all neighbors have multiple points where
/// they are connected. The following table lists all paths in the network, before and after the
/// reconfiguration, including if the network changed or not.
///
/// ### Variant 2:
///
/// Only reachability is required:
///
/// $$\mathbf{G}\ \phi$$
///
/// ### Variant 3:
///
/// We require two things:
/// 1. Reachability
/// 2. For prefix *C*, every flow does never change its path!
///
/// $$\mathbf{G}\ \phi$$
///
/// ### Variant 4:
///
/// For every flow $i$, we require that the path switches from the initial to the final path exactly
/// once. If the path is the same, than this is equivalent to saying that the path is not allowed to
/// change.
///
/// $$\bigwedge \big( \phi_i\ \mathbf{U}\ \mathbf{G}\ \psi_i \big)$$
pub struct AbileneNetwork;

impl ExampleNetwork for AbileneNetwork {
    /// Get raw network without configuration
    fn net(initial_variant: usize) -> Network {
        let mut net = Network::new();

        // add routers
        let sv = net.add_router("Sunnyvale"); // 0
        let se = net.add_router("Seattle"); // 1
        let dv = net.add_router("Denver"); // 2
        let la = net.add_router("Los Angeles"); // 3
        let hs = net.add_router("Huston"); // 4
        let ks = net.add_router("Kansas City"); // 5
        let ip = net.add_router("Indianapolis"); // 6
        let at = net.add_router("Atlanta"); // 7
        let dc = net.add_router("Washington DC"); // 8
        let ny = net.add_router("New York"); // 9
        let ch = net.add_router("Chicago"); // 10

        // add links
        net.add_link(sv, se);
        net.add_link(sv, dv);
        net.add_link(sv, la);
        net.add_link(se, dv);
        net.add_link(dv, ks);
        net.add_link(la, hs);
        net.add_link(ks, hs);
        net.add_link(ks, ip);
        net.add_link(hs, at);
        net.add_link(ip, at);
        net.add_link(ip, ch);
        net.add_link(at, dc);
        net.add_link(ch, ny);
        net.add_link(dc, ny);

        if initial_variant == 0 || initial_variant == 1 {
            let e1 = net.add_external_router("Sunnyvale Ext", AsId(65101)); // 11
            let e2 = net.add_external_router("Los Angeles Ext", AsId(65101)); // 12
            let e3 = net.add_external_router("Chicago Ext", AsId(65103)); // 13

            net.add_link(sv, e1);
            net.add_link(la, e2);
            net.add_link(ch, e3);

            let cf = Self::initial_config(&net, initial_variant);
            net.set_config(&cf).unwrap();

            net.advertise_external_route(e1, Prefix(0), vec![AsId(65101), AsId(65200)], None, None)
                .unwrap();
            net.advertise_external_route(e2, Prefix(0), vec![AsId(65102), AsId(65200)], None, None)
                .unwrap();
            net.advertise_external_route(e3, Prefix(0), vec![AsId(65103), AsId(65200)], None, None)
                .unwrap();
        } else if initial_variant == 2
            || initial_variant == 3
            || initial_variant == 4
            || initial_variant == 5
            || initial_variant == 6
        {
            let p_a = Prefix(1);
            let p_b = Prefix(2);
            let p_c = Prefix(3);
            let p_d = Prefix(4);
            let p_e = Prefix(5);
            let p_i = Prefix(0);

            let as_a = AsId(65101);
            let as_b = AsId(65102);
            let as_c = AsId(65103);
            let as_d = AsId(65104);
            let as_e = AsId(65105);
            let as_i = AsId(1);

            let la_a = net.add_external_router("Ext A Los Angeles", as_a); // 11
            let at_a = net.add_external_router("Ext A Atlanta", as_a); // 12
            let se_b = net.add_external_router("Ext B Seattle", as_b); // 13
            let ch_b = net.add_external_router("Ext B Chicago", as_b); // 14
            let hs_c = net.add_external_router("Ext C Huston", as_c); // 15
            let at_c = net.add_external_router("Ext C Atlanta", as_c); // 16
            let sv_d = net.add_external_router("Ext D Sunnyvale", as_d); // 17
            let dc_d = net.add_external_router("Ext D Washington DC", as_d); // 18
            let dv_e = net.add_external_router("Ext E Denver", as_e); // 19
            let ny_e = net.add_external_router("Ext E New York", as_e); // 20

            net.add_link(la, la_a);
            net.add_link(at, at_a);
            net.add_link(se, se_b);
            net.add_link(ch, ch_b);
            net.add_link(hs, hs_c);
            net.add_link(at, at_c);
            net.add_link(sv, sv_d);
            net.add_link(dc, dc_d);
            net.add_link(dv, dv_e);
            net.add_link(ny, ny_e);

            let cf = Self::initial_config(&net, initial_variant);
            net.set_config(&cf).unwrap();

            net.advertise_external_route(la_a, p_a, vec![as_a], None, None).unwrap();
            net.advertise_external_route(at_a, p_a, vec![as_a], None, None).unwrap();
            net.advertise_external_route(se_b, p_b, vec![as_b], None, None).unwrap();
            net.advertise_external_route(ch_b, p_b, vec![as_b], None, None).unwrap();
            net.advertise_external_route(hs_c, p_c, vec![as_c], None, None).unwrap();
            net.advertise_external_route(at_c, p_c, vec![as_c], None, None).unwrap();
            net.advertise_external_route(sv_d, p_d, vec![as_d], None, None).unwrap();
            net.advertise_external_route(dc_d, p_d, vec![as_d], None, None).unwrap();
            net.advertise_external_route(dv_e, p_e, vec![as_e], None, None).unwrap();
            net.advertise_external_route(ny_e, p_e, vec![as_e], None, None).unwrap();
            net.advertise_external_route(sv_d, p_i, vec![as_d, as_i], None, None).unwrap();
            net.advertise_external_route(dc_d, p_i, vec![as_d, as_i], None, None).unwrap();
            net.advertise_external_route(dv_e, p_i, vec![as_e, as_i], None, None).unwrap();
            net.advertise_external_route(ny_e, p_i, vec![as_e, as_i], None, None).unwrap();
        }

        net
    }

    fn initial_config(net: &Network, variant: usize) -> Config {
        let mut c = Config::new();

        let sv = net.get_router_id("Sunnyvale").unwrap();
        let se = net.get_router_id("Seattle").unwrap();
        let dv = net.get_router_id("Denver").unwrap();
        let la = net.get_router_id("Los Angeles").unwrap();
        let hs = net.get_router_id("Huston").unwrap();
        let ks = net.get_router_id("Kansas City").unwrap();
        let ip = net.get_router_id("Indianapolis").unwrap();
        let at = net.get_router_id("Atlanta").unwrap();
        let dc = net.get_router_id("Washington DC").unwrap();
        let ny = net.get_router_id("New York").unwrap();
        let ch = net.get_router_id("Chicago").unwrap();

        if variant == 0 || variant == 1 {
            let e1 = net.get_router_id("Sunnyvale Ext").unwrap();
            let e2 = net.get_router_id("Los Angeles Ext").unwrap();
            let e3 = net.get_router_id("Chicago Ext").unwrap();

            c.add(IgpLinkWeight { source: sv, target: se, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { source: sv, target: dv, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: sv, target: la, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: se, target: dv, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: dv, target: ks, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: la, target: hs, weight: 50.0 }).unwrap();
            c.add(IgpLinkWeight { source: ks, target: hs, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: ks, target: ip, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: hs, target: at, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: ip, target: at, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: ip, target: ch, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: at, target: dc, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: ch, target: ny, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: dc, target: ny, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: sv, target: e1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: la, target: e2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: ch, target: e3, weight: 1.0 }).unwrap();

            c.add(IgpLinkWeight { target: sv, source: se, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { target: sv, source: dv, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: sv, source: la, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: se, source: dv, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: dv, source: ks, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: la, source: hs, weight: 50.0 }).unwrap();
            c.add(IgpLinkWeight { target: ks, source: hs, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: ks, source: ip, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: hs, source: at, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: ip, source: at, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: ip, source: ch, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: at, source: dc, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: ch, source: ny, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: dc, source: ny, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: sv, source: e1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: la, source: e2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: ch, source: e3, weight: 1.0 }).unwrap();

            c.add(BgpSession { source: dv, target: hs, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: dv, target: ip, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: hs, target: ip, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: dv, target: sv, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: dv, target: se, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: dv, target: ks, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: hs, target: la, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: hs, target: at, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: ip, target: ch, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: ip, target: ny, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: ip, target: dc, session_type: IBgpClient }).unwrap();

            c.add(BgpSession { source: sv, target: e1, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: la, target: e2, session_type: EBgp }).unwrap();

            if variant == 0 {
                // nothing to do
            } else if variant == 1 {
                c.add(BgpSession { source: ch, target: e3, session_type: EBgp }).unwrap();
                c.add(BgpRouteMap {
                    router: ch,
                    direction: RouteMapDirection::Incoming,
                    map: RouteMapBuilder::new()
                        .allow()
                        .order(10)
                        .match_neighbor(e3)
                        .set_local_pref(50)
                        .build(),
                })
                .unwrap();
            }
        } else if variant == 2 || variant == 3 || variant == 4 || variant == 5 || variant == 6 {
            let la_a = net.get_router_id("Ext A Los Angeles").unwrap();
            let at_a = net.get_router_id("Ext A Atlanta").unwrap();
            let se_b = net.get_router_id("Ext B Seattle").unwrap();
            let ch_b = net.get_router_id("Ext B Chicago").unwrap();
            let hs_c = net.get_router_id("Ext C Huston").unwrap();
            let at_c = net.get_router_id("Ext C Atlanta").unwrap();
            let sv_d = net.get_router_id("Ext D Sunnyvale").unwrap();
            let dc_d = net.get_router_id("Ext D Washington DC").unwrap();
            let dv_e = net.get_router_id("Ext E Denver").unwrap();
            let ny_e = net.get_router_id("Ext E New York").unwrap();

            // link weight
            c.add(IgpLinkWeight { source: sv, target: se, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { source: sv, target: dv, weight: 12.0 }).unwrap();
            c.add(IgpLinkWeight { source: sv, target: la, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { source: se, target: dv, weight: 8.0 }).unwrap();
            c.add(IgpLinkWeight { source: dv, target: ks, weight: 8.0 }).unwrap();
            c.add(IgpLinkWeight { source: la, target: hs, weight: 7.0 }).unwrap();
            c.add(IgpLinkWeight { source: ks, target: hs, weight: 12.0 }).unwrap();
            c.add(IgpLinkWeight { source: ks, target: ip, weight: 14.0 }).unwrap();
            c.add(IgpLinkWeight { source: hs, target: at, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { source: ip, target: at, weight: 16.0 }).unwrap();
            c.add(IgpLinkWeight { source: ip, target: ch, weight: 8.0 }).unwrap();
            c.add(IgpLinkWeight { source: at, target: dc, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { source: ch, target: ny, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { source: dc, target: ny, weight: 2.0 }).unwrap();
            c.add(IgpLinkWeight { source: la, target: la_a, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: at, target: at_a, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: se, target: se_b, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: ch, target: ch_b, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: hs, target: hs_c, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: at, target: at_c, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: sv, target: sv_d, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: dc, target: dc_d, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: dv, target: dv_e, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: ny, target: ny_e, weight: 1.0 }).unwrap();

            c.add(IgpLinkWeight { target: sv, source: se, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { target: sv, source: dv, weight: 12.0 }).unwrap();
            c.add(IgpLinkWeight { target: sv, source: la, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { target: se, source: dv, weight: 8.0 }).unwrap();
            c.add(IgpLinkWeight { target: dv, source: ks, weight: 8.0 }).unwrap();
            c.add(IgpLinkWeight { target: la, source: hs, weight: 7.0 }).unwrap();
            c.add(IgpLinkWeight { target: ks, source: hs, weight: 12.0 }).unwrap();
            c.add(IgpLinkWeight { target: ks, source: ip, weight: 14.0 }).unwrap();
            c.add(IgpLinkWeight { target: hs, source: at, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { target: ip, source: at, weight: 16.0 }).unwrap();
            c.add(IgpLinkWeight { target: ip, source: ch, weight: 8.0 }).unwrap();
            c.add(IgpLinkWeight { target: at, source: dc, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { target: ch, source: ny, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { target: dc, source: ny, weight: 2.0 }).unwrap();
            c.add(IgpLinkWeight { target: la, source: la_a, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: at, source: at_a, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: se, source: se_b, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: ch, source: ch_b, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: hs, source: hs_c, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: at, source: at_c, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: sv, source: sv_d, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: dc, source: dc_d, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: dv, source: dv_e, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: ny, source: ny_e, weight: 1.0 }).unwrap();

            // ebgp sessions
            c.add(BgpSession { source: la, target: la_a, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: at, target: at_a, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: se, target: se_b, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: ch, target: ch_b, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: hs, target: hs_c, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: at, target: at_c, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: sv, target: sv_d, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: dc, target: dc_d, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: dv, target: dv_e, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: ny, target: ny_e, session_type: EBgp }).unwrap();

            // LP settings
            c.add(BgpRouteMap {
                router: la,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(la_a)
                    .set_local_pref(50)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: at,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(at_a)
                    .set_local_pref(50)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: se,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(se_b)
                    .set_local_pref(50)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: ch,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(ch_b)
                    .set_local_pref(50)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: hs,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(hs_c)
                    .set_local_pref(100)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: at,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(20)
                    .match_neighbor(at_c)
                    .set_local_pref(100)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: sv,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(sv_d)
                    .set_local_pref(200)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: dc,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(dc_d)
                    .set_local_pref(200)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: dv,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(dv_e)
                    .set_local_pref(200)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: ny,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(ny_e)
                    .set_local_pref(200)
                    .build(),
            })
            .unwrap();

            // full-mesh
            c.add(BgpSession { source: ks, target: dv, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: ks, target: hs, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: ks, target: ip, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: dv, target: sv, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: dv, target: se, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: hs, target: la, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: hs, target: at, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: ip, target: ch, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: ip, target: ny, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: ip, target: dc, session_type: IBgpClient }).unwrap();
        }

        c
    }

    fn final_config(net: &Network, variant: usize) -> Config {
        let mut c = Config::new();

        let sv = net.get_router_id("Sunnyvale").unwrap();
        let se = net.get_router_id("Seattle").unwrap();
        let dv = net.get_router_id("Denver").unwrap();
        let la = net.get_router_id("Los Angeles").unwrap();
        let hs = net.get_router_id("Huston").unwrap();
        let ks = net.get_router_id("Kansas City").unwrap();
        let ip = net.get_router_id("Indianapolis").unwrap();
        let at = net.get_router_id("Atlanta").unwrap();
        let dc = net.get_router_id("Washington DC").unwrap();
        let ny = net.get_router_id("New York").unwrap();
        let ch = net.get_router_id("Chicago").unwrap();

        if variant == 0 || variant == 1 {
            let e1 = net.get_router_id("Sunnyvale Ext").unwrap();
            let e2 = net.get_router_id("Los Angeles Ext").unwrap();
            let e3 = net.get_router_id("Chicago Ext").unwrap();

            c.add(IgpLinkWeight { source: sv, target: se, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { source: sv, target: dv, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { source: sv, target: la, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: se, target: dv, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: dv, target: ks, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: la, target: hs, weight: 50.0 }).unwrap();
            c.add(IgpLinkWeight { source: ks, target: hs, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: ks, target: ip, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: hs, target: at, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: ip, target: at, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: ip, target: ch, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: at, target: dc, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: ch, target: ny, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: dc, target: ny, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { source: sv, target: e1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: la, target: e2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: ch, target: e3, weight: 1.0 }).unwrap();

            c.add(IgpLinkWeight { target: sv, source: se, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { target: sv, source: dv, weight: 100.0 }).unwrap();
            c.add(IgpLinkWeight { target: sv, source: la, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: se, source: dv, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: dv, source: ks, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: la, source: hs, weight: 50.0 }).unwrap();
            c.add(IgpLinkWeight { target: ks, source: hs, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: ks, source: ip, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: hs, source: at, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: ip, source: at, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: ip, source: ch, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: at, source: dc, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: ch, source: ny, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: dc, source: ny, weight: 10.0 }).unwrap();
            c.add(IgpLinkWeight { target: sv, source: e1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: la, source: e2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: ch, source: e3, weight: 1.0 }).unwrap();

            c.add(BgpSession { source: dv, target: hs, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: dv, target: ip, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: hs, target: ip, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: dv, target: sv, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: dv, target: se, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: dv, target: ks, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: hs, target: la, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: hs, target: at, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: ip, target: ch, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: ip, target: ny, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: ip, target: dc, session_type: IBgpClient }).unwrap();

            c.add(BgpSession { source: sv, target: e1, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: la, target: e2, session_type: EBgp }).unwrap();

            if variant == 0 {
                // nothing to do
            } else if variant == 1 {
                c.add(BgpSession { source: ch, target: e3, session_type: EBgp }).unwrap();
                c.add(BgpRouteMap {
                    router: ch,
                    direction: RouteMapDirection::Incoming,
                    map: RouteMapBuilder::new()
                        .allow()
                        .order(10)
                        .match_neighbor(e3)
                        .set_local_pref(50)
                        .build(),
                })
                .unwrap();
            }
        } else if variant == 2 || variant == 3 || variant == 4 || variant == 5 || variant == 6 {
            let la_a = net.get_router_id("Ext A Los Angeles").unwrap();
            let at_a = net.get_router_id("Ext A Atlanta").unwrap();
            let se_b = net.get_router_id("Ext B Seattle").unwrap();
            let ch_b = net.get_router_id("Ext B Chicago").unwrap();
            let hs_c = net.get_router_id("Ext C Huston").unwrap();
            let at_c = net.get_router_id("Ext C Atlanta").unwrap();
            let sv_d = net.get_router_id("Ext D Sunnyvale").unwrap();
            let dc_d = net.get_router_id("Ext D Washington DC").unwrap();
            let dv_e = net.get_router_id("Ext E Denver").unwrap();
            let ny_e = net.get_router_id("Ext E New York").unwrap();

            // link weight
            c.add(IgpLinkWeight { source: sv, target: se, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { source: sv, target: dv, weight: 12.0 }).unwrap();
            c.add(IgpLinkWeight { source: sv, target: la, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { source: se, target: dv, weight: 8.0 }).unwrap();
            c.add(IgpLinkWeight { source: dv, target: ks, weight: 8.0 }).unwrap();
            c.add(IgpLinkWeight { source: la, target: hs, weight: 7.0 }).unwrap();
            c.add(IgpLinkWeight { source: ks, target: hs, weight: 12.0 }).unwrap();
            c.add(IgpLinkWeight { source: ks, target: ip, weight: 14.0 }).unwrap();
            c.add(IgpLinkWeight { source: hs, target: at, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { source: ip, target: at, weight: 16.0 }).unwrap();
            c.add(IgpLinkWeight { source: ip, target: ch, weight: 8.0 }).unwrap();
            c.add(IgpLinkWeight { source: at, target: dc, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { source: ch, target: ny, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { source: dc, target: ny, weight: 2.0 }).unwrap();
            c.add(IgpLinkWeight { source: la, target: la_a, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: at, target: at_a, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: se, target: se_b, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: ch, target: ch_b, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: hs, target: hs_c, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: at, target: at_c, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: sv, target: sv_d, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: dc, target: dc_d, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: dv, target: dv_e, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: ny, target: ny_e, weight: 1.0 }).unwrap();

            c.add(IgpLinkWeight { target: sv, source: se, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { target: sv, source: dv, weight: 12.0 }).unwrap();
            c.add(IgpLinkWeight { target: sv, source: la, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { target: se, source: dv, weight: 8.0 }).unwrap();
            c.add(IgpLinkWeight { target: dv, source: ks, weight: 8.0 }).unwrap();
            c.add(IgpLinkWeight { target: la, source: hs, weight: 7.0 }).unwrap();
            c.add(IgpLinkWeight { target: ks, source: hs, weight: 12.0 }).unwrap();
            c.add(IgpLinkWeight { target: ks, source: ip, weight: 14.0 }).unwrap();
            c.add(IgpLinkWeight { target: hs, source: at, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { target: ip, source: at, weight: 16.0 }).unwrap();
            c.add(IgpLinkWeight { target: ip, source: ch, weight: 8.0 }).unwrap();
            c.add(IgpLinkWeight { target: at, source: dc, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { target: ch, source: ny, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { target: dc, source: ny, weight: 2.0 }).unwrap();
            c.add(IgpLinkWeight { target: la, source: la_a, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: at, source: at_a, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: se, source: se_b, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: ch, source: ch_b, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: hs, source: hs_c, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: at, source: at_c, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: sv, source: sv_d, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: dc, source: dc_d, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: dv, source: dv_e, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: ny, source: ny_e, weight: 1.0 }).unwrap();

            // ebgp sessions
            c.add(BgpSession { source: la, target: la_a, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: at, target: at_a, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: se, target: se_b, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: ch, target: ch_b, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: hs, target: hs_c, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: at, target: at_c, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: sv, target: sv_d, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: dc, target: dc_d, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: dv, target: dv_e, session_type: EBgp }).unwrap();
            c.add(BgpSession { source: ny, target: ny_e, session_type: EBgp }).unwrap();

            // LP settings
            c.add(BgpRouteMap {
                router: la,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(la_a)
                    .set_local_pref(50)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: at,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(at_a)
                    .set_local_pref(50)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: se,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(se_b)
                    .set_local_pref(50)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: ch,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(ch_b)
                    .set_local_pref(50)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: hs,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(hs_c)
                    .set_local_pref(100)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: at,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(20)
                    .match_neighbor(at_c)
                    .set_local_pref(100)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: sv,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(sv_d)
                    .set_local_pref(200)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: dc,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(dc_d)
                    .set_local_pref(200)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: dv,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(dv_e)
                    .set_local_pref(200)
                    .build(),
            })
            .unwrap();
            c.add(BgpRouteMap {
                router: ny,
                direction: RouteMapDirection::Incoming,
                map: RouteMapBuilder::new()
                    .allow()
                    .order(10)
                    .match_neighbor(ny_e)
                    .set_local_pref(200)
                    .build(),
            })
            .unwrap();

            // RR topo
            c.add(BgpSession { source: dv, target: hs, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: dv, target: ip, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: hs, target: ip, session_type: IBgpPeer }).unwrap();
            c.add(BgpSession { source: dv, target: sv, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: dv, target: se, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: dv, target: ks, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: hs, target: la, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: hs, target: at, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: ip, target: ch, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: ip, target: ny, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: ip, target: dc, session_type: IBgpClient }).unwrap();
        } else {
            panic!("Initial variant number!")
        }

        c
    }

    fn get_policy(net: &Network, variant: usize) -> HardPolicy {
        let sv = net.get_router_id("Sunnyvale").unwrap();
        let se = net.get_router_id("Seattle").unwrap();
        let dv = net.get_router_id("Denver").unwrap();
        let la = net.get_router_id("Los Angeles").unwrap();
        let hs = net.get_router_id("Huston").unwrap();
        let ks = net.get_router_id("Kansas City").unwrap();
        let ip = net.get_router_id("Indianapolis").unwrap();
        let at = net.get_router_id("Atlanta").unwrap();
        let dc = net.get_router_id("Washington DC").unwrap();
        let ny = net.get_router_id("New York").unwrap();
        let ch = net.get_router_id("Chicago").unwrap();

        if variant == 0 || variant == 1 {
            let routers = vec![se, dv, hs, ks, ip, at, dc, ny, ch /*sv, la*/];

            let p = Prefix(0);

            let initial_prop_vars = routers
                .iter()
                .map(|r| Reachable(*r, p, Some(PathCondition::Edge(dv, sv))))
                .collect::<Vec<_>>();

            let final_prop_vars = routers
                .iter()
                .map(|r| Reachable(*r, p, Some(PathCondition::Edge(hs, la))))
                .collect::<Vec<_>>();

            let transient_prop_vars = routers
                .iter()
                .map(|r| {
                    TransientPath(
                        *r,
                        p,
                        PathCondition::Or(vec![
                            PathCondition::Edge(dv, sv),
                            PathCondition::Edge(hs, la),
                        ]),
                    )
                })
                .collect::<Vec<_>>();

            let prop_vars = initial_prop_vars
                .into_iter()
                .chain(final_prop_vars.into_iter())
                .chain(transient_prop_vars.into_iter())
                .collect::<Vec<_>>();

            let initial = (0..routers.len())
                .map(|x| Box::new(x + 0 * routers.len()) as Box<dyn LTLOperator>)
                .collect::<Vec<_>>();
            let finally = (0..routers.len())
                .map(|x| Box::new(x + 1 * routers.len()) as Box<dyn LTLOperator>)
                .collect::<Vec<_>>();
            let transient = (0..routers.len())
                .map(|x| Box::new(x + 2 * routers.len()) as Box<dyn LTLOperator>)
                .collect::<Vec<_>>();

            HardPolicy::new(
                prop_vars,
                LTLModal::Now(Box::new(LTLBoolean::And(vec![
                    Box::new(LTLModal::Until(
                        Box::new(LTLBoolean::And(initial)),
                        Box::new(LTLModal::Globally(Box::new(LTLBoolean::And(finally)))),
                    )),
                    Box::new(LTLModal::Globally(Box::new(LTLBoolean::And(transient)))),
                ]))),
            )
        } else if variant == 2 {
            // connectivity
            HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter())
        } else if variant == 3 {
            // connectivity, for peer C, no traffic is allowed to shift
            let p_a = Prefix(1);

            let prop_vars = iproduct!(net.get_routers().iter(), net.get_known_prefixes().iter())
                .map(|(&r, &p)| {
                    Reachable(
                        r,
                        p,
                        if p == p_a {
                            Some(if r == se {
                                Positional(vec![Fix(se), Fix(sv), Fix(la), Any])
                            } else if r == sv {
                                Positional(vec![Fix(sv), Fix(la), Any])
                            } else if r == dv {
                                Positional(vec![Fix(dv), Fix(ks), Fix(hs), Fix(at), Any])
                            } else if r == la {
                                Positional(vec![Fix(la), Any])
                            } else if r == ks {
                                Positional(vec![Fix(ks), Fix(hs), Fix(at), Any])
                            } else if r == hs {
                                Positional(vec![Fix(hs), Fix(at), Any])
                            } else if r == ip {
                                Positional(vec![Fix(ip), Fix(at), Any])
                            } else if r == at {
                                Positional(vec![Fix(at), Any])
                            } else if r == ch {
                                Positional(vec![Fix(ch), Fix(ny), Fix(dc), Fix(at), Any])
                            } else if r == ny {
                                Positional(vec![Fix(ny), Fix(dc), Fix(at), Any])
                            } else if r == dc {
                                Positional(vec![Fix(dc), Fix(at), Any])
                            } else {
                                unreachable!("No additional internal routers")
                            })
                        } else {
                            None
                        },
                    )
                })
                .collect::<Vec<_>>();

            HardPolicy::globally(prop_vars)
        } else if variant == 4 {
            // connectivity, for peer C, no traffic is allowed to shift
            let p_a = Prefix(1);
            let p_b = Prefix(2);

            let prop_vars = iproduct!(net.get_routers().iter(), net.get_known_prefixes().iter())
                .map(|(&r, &p)| {
                    Reachable(
                        r,
                        p,
                        if p == p_a {
                            Some(if r == se {
                                Positional(vec![Fix(se), Fix(sv), Fix(la), Any])
                            } else if r == sv {
                                Positional(vec![Fix(sv), Fix(la), Any])
                            } else if r == dv {
                                Positional(vec![Fix(dv), Fix(ks), Fix(hs), Fix(at), Any])
                            } else if r == la {
                                Positional(vec![Fix(la), Any])
                            } else if r == ks {
                                Positional(vec![Fix(ks), Fix(hs), Fix(at), Any])
                            } else if r == hs {
                                Positional(vec![Fix(hs), Fix(at), Any])
                            } else if r == ip {
                                Positional(vec![Fix(ip), Fix(at), Any])
                            } else if r == at {
                                Positional(vec![Fix(at), Any])
                            } else if r == ch {
                                Positional(vec![Fix(ch), Fix(ny), Fix(dc), Fix(at), Any])
                            } else if r == ny {
                                Positional(vec![Fix(ny), Fix(dc), Fix(at), Any])
                            } else if r == dc {
                                Positional(vec![Fix(dc), Fix(at), Any])
                            } else {
                                unreachable!("No additional internal routers")
                            })
                        } else if p == p_b {
                            Some(if r == se {
                                Positional(vec![Fix(se), Any])
                            } else if r == sv {
                                Positional(vec![Fix(sv), Fix(se), Any])
                            } else if r == dv {
                                Positional(vec![Fix(dv), Fix(se), Any])
                            } else if r == la {
                                Positional(vec![Fix(la), Fix(sv), Fix(se), Any])
                            } else if r == ks {
                                Positional(vec![Fix(ks), Fix(dv), Fix(se), Any])
                            } else if r == hs {
                                Positional(vec![Fix(hs), Fix(la), Fix(sv), Fix(se), Any])
                            } else if r == ip {
                                Positional(vec![Fix(ip), Fix(ch), Any])
                            } else if r == at {
                                Positional(vec![Fix(at), Fix(hs), Fix(la), Fix(sv), Fix(se), Any])
                            } else if r == ch {
                                Positional(vec![Fix(ch), Any])
                            } else if r == ny {
                                Positional(vec![Fix(ny), Fix(ch), Any])
                            } else if r == dc {
                                Positional(vec![Fix(dc), Fix(ny), Fix(ch), Any])
                            } else {
                                unreachable!("No additional internal routers")
                            })
                        } else {
                            None
                        },
                    )
                })
                .collect::<Vec<_>>();

            HardPolicy::globally(prop_vars)
        } else if variant == 5 {
            // connectivity, for peer C, no traffic is allowed to shift
            let p_a = Prefix(1);
            let p_b = Prefix(2);
            let p_c = Prefix(3);

            let prop_vars = iproduct!(net.get_routers().iter(), net.get_known_prefixes().iter())
                .map(|(&r, &p)| {
                    Reachable(
                        r,
                        p,
                        if p == p_a {
                            Some(if r == se {
                                Positional(vec![Fix(se), Fix(sv), Fix(la), Any])
                            } else if r == sv {
                                Positional(vec![Fix(sv), Fix(la), Any])
                            } else if r == dv {
                                Positional(vec![Fix(dv), Fix(ks), Fix(hs), Fix(at), Any])
                            } else if r == la {
                                Positional(vec![Fix(la), Any])
                            } else if r == ks {
                                Positional(vec![Fix(ks), Fix(hs), Fix(at), Any])
                            } else if r == hs {
                                Positional(vec![Fix(hs), Fix(at), Any])
                            } else if r == ip {
                                Positional(vec![Fix(ip), Fix(at), Any])
                            } else if r == at {
                                Positional(vec![Fix(at), Any])
                            } else if r == ch {
                                Positional(vec![Fix(ch), Fix(ny), Fix(dc), Fix(at), Any])
                            } else if r == ny {
                                Positional(vec![Fix(ny), Fix(dc), Fix(at), Any])
                            } else if r == dc {
                                Positional(vec![Fix(dc), Fix(at), Any])
                            } else {
                                unreachable!("No additional internal routers")
                            })
                        } else if p == p_b {
                            Some(if r == se {
                                Positional(vec![Fix(se), Any])
                            } else if r == sv {
                                Positional(vec![Fix(sv), Fix(se), Any])
                            } else if r == dv {
                                Positional(vec![Fix(dv), Fix(se), Any])
                            } else if r == la {
                                Positional(vec![Fix(la), Fix(sv), Fix(se), Any])
                            } else if r == ks {
                                Positional(vec![Fix(ks), Fix(dv), Fix(se), Any])
                            } else if r == hs {
                                Positional(vec![Fix(hs), Fix(la), Fix(sv), Fix(se), Any])
                            } else if r == ip {
                                Positional(vec![Fix(ip), Fix(ch), Any])
                            } else if r == at {
                                Positional(vec![Fix(at), Fix(hs), Fix(la), Fix(sv), Fix(se), Any])
                            } else if r == ch {
                                Positional(vec![Fix(ch), Any])
                            } else if r == ny {
                                Positional(vec![Fix(ny), Fix(ch), Any])
                            } else if r == dc {
                                Positional(vec![Fix(dc), Fix(ny), Fix(ch), Any])
                            } else {
                                unreachable!("No additional internal routers")
                            })
                        } else if p == p_c {
                            Some(if r == se {
                                Positional(vec![Fix(se), Fix(sv), Fix(la), Fix(hs), Any])
                            } else if r == sv {
                                Positional(vec![Fix(sv), Fix(la), Fix(hs), Any])
                            } else if r == dv {
                                Positional(vec![Fix(dv), Fix(ks), Fix(hs), Any])
                            } else if r == la {
                                Positional(vec![Fix(la), Fix(hs), Any])
                            } else if r == ks {
                                Positional(vec![Fix(ks), Fix(hs), Any])
                            } else if r == hs {
                                Positional(vec![Fix(hs), Any])
                            } else if r == ip {
                                Positional(vec![Fix(ip), Fix(at), Any])
                            } else if r == at {
                                Positional(vec![Fix(at), Any])
                            } else if r == ch {
                                Positional(vec![Fix(ch), Fix(ny), Fix(dc), Fix(at), Any])
                            } else if r == ny {
                                Positional(vec![Fix(ny), Fix(dc), Fix(at), Any])
                            } else if r == dc {
                                Positional(vec![Fix(dc), Fix(at), Any])
                            } else {
                                unreachable!("No additional internal routers")
                            })
                        } else {
                            None
                        },
                    )
                })
                .collect::<Vec<_>>();

            HardPolicy::globally(prop_vars)
        } else if variant == 6 {
            let p_a = Prefix(1);
            let p_b = Prefix(2);
            let p_c = Prefix(3);
            let p_d = Prefix(4);
            let p_e = Prefix(5);
            let p_i = Prefix(0);

            let se_ = Fix(se);
            let sv_ = Fix(sv);
            let dv_ = Fix(dv);
            let la_ = Fix(la);
            let ks_ = Fix(ks);
            let hs_ = Fix(hs);
            let ip_ = Fix(ip);
            let at_ = Fix(at);
            let ch_ = Fix(ch);
            let dc_ = Fix(dc);
            let ny_ = Fix(ny);

            let prop_vars = vec![
                // before
                Reachable(se, p_a, Some(Positional(vec![se_, sv_, la_, Any]))),
                Reachable(se, p_b, Some(Positional(vec![se_, Any]))),
                Reachable(se, p_c, Some(Positional(vec![se_, sv_, la_, hs_, Any]))),
                Reachable(se, p_d, Some(Positional(vec![se_, sv_, Any]))),
                Reachable(se, p_e, Some(Positional(vec![se_, dv_, Any]))),
                Reachable(se, p_i, Some(Positional(vec![se_, dv_, Any]))),
                Reachable(sv, p_a, Some(Positional(vec![sv_, la_, Any]))),
                Reachable(sv, p_b, Some(Positional(vec![sv_, se_, Any]))),
                Reachable(sv, p_c, Some(Positional(vec![sv_, la_, hs_, Any]))),
                Reachable(sv, p_d, Some(Positional(vec![sv_, Any]))),
                Reachable(sv, p_e, Some(Positional(vec![sv_, dv_, Any]))),
                Reachable(sv, p_i, Some(Positional(vec![sv_, Any]))),
                Reachable(dv, p_a, Some(Positional(vec![dv_, ks_, hs_, at_, Any]))),
                Reachable(dv, p_b, Some(Positional(vec![dv_, se_, Any]))),
                Reachable(dv, p_c, Some(Positional(vec![dv_, ks_, hs_, Any]))),
                Reachable(dv, p_d, Some(Positional(vec![dv_, sv_, Any]))),
                Reachable(dv, p_e, Some(Positional(vec![dv_, Any]))),
                Reachable(dv, p_i, Some(Positional(vec![dv_, Any]))),
                Reachable(la, p_a, Some(Positional(vec![la_, Any]))),
                Reachable(la, p_b, Some(Positional(vec![la_, sv_, se_, Any]))),
                Reachable(la, p_c, Some(Positional(vec![la_, hs_, Any]))),
                Reachable(la, p_d, Some(Positional(vec![la_, sv_, Any]))),
                Reachable(la, p_e, Some(Positional(vec![la_, sv_, dv_, Any]))),
                Reachable(la, p_i, Some(Positional(vec![la_, sv_, Any]))),
                Reachable(ks, p_a, Some(Positional(vec![ks_, hs_, at_, Any]))),
                Reachable(ks, p_b, Some(Positional(vec![ks_, dv_, se_, Any]))),
                Reachable(ks, p_c, Some(Positional(vec![ks_, hs_, Any]))),
                Reachable(ks, p_d, Some(Positional(vec![ks_, dv_, sv_, Any]))),
                Reachable(ks, p_e, Some(Positional(vec![ks_, dv_, Any]))),
                Reachable(ks, p_i, Some(Positional(vec![ks_, dv_, Any]))),
                Reachable(hs, p_a, Some(Positional(vec![hs_, at_, Any]))),
                Reachable(hs, p_b, Some(Positional(vec![hs_, la_, sv_, se_, Any]))),
                Reachable(hs, p_c, Some(Positional(vec![hs_, Any]))),
                Reachable(hs, p_d, Some(Positional(vec![hs_, la_, sv_, Any]))),
                Reachable(hs, p_e, Some(Positional(vec![hs_, ks_, dv_, Any]))),
                Reachable(hs, p_i, Some(Positional(vec![hs_, ks_, dv_, Any]))),
                Reachable(ip, p_a, Some(Positional(vec![ip_, at_, Any]))),
                Reachable(ip, p_b, Some(Positional(vec![ip_, ch_, Any]))),
                Reachable(ip, p_c, Some(Positional(vec![ip_, at_, Any]))),
                Reachable(ip, p_d, Some(Positional(vec![ip_, ch_, ny_, dc_, Any]))),
                Reachable(ip, p_e, Some(Positional(vec![ip_, ch_, ny_, Any]))),
                Reachable(ip, p_i, Some(Positional(vec![ip_, ch_, ny_, Any]))),
                Reachable(at, p_a, Some(Positional(vec![at_, Any]))),
                Reachable(at, p_b, Some(Positional(vec![at_, hs_, la_, sv_, se_, Any]))),
                Reachable(at, p_c, Some(Positional(vec![at_, Any]))),
                Reachable(at, p_d, Some(Positional(vec![at_, hs_, la_, sv_, Any]))),
                Reachable(at, p_e, Some(Positional(vec![at_, hs_, ks_, dv_, Any]))),
                Reachable(at, p_i, Some(Positional(vec![at_, hs_, ks_, dv_, Any]))),
                Reachable(ch, p_a, Some(Positional(vec![ch_, ny_, dc_, at_, Any]))),
                Reachable(ch, p_b, Some(Positional(vec![ch_, Any]))),
                Reachable(ch, p_c, Some(Positional(vec![ch_, ny_, dc_, at_, Any]))),
                Reachable(ch, p_d, Some(Positional(vec![ch_, ny_, dc_, Any]))),
                Reachable(ch, p_e, Some(Positional(vec![ch_, ny_, Any]))),
                Reachable(ch, p_i, Some(Positional(vec![ch_, ny_, Any]))),
                Reachable(dc, p_a, Some(Positional(vec![dc_, at_, Any]))),
                Reachable(dc, p_b, Some(Positional(vec![dc_, ny_, ch_, Any]))),
                Reachable(dc, p_c, Some(Positional(vec![dc_, at_, Any]))),
                Reachable(dc, p_d, Some(Positional(vec![dc_, Any]))),
                Reachable(dc, p_e, Some(Positional(vec![dc_, ny_, Any]))),
                Reachable(dc, p_i, Some(Positional(vec![dc_, Any]))),
                Reachable(ny, p_a, Some(Positional(vec![ny_, dc_, at_, Any]))),
                Reachable(ny, p_b, Some(Positional(vec![ny_, ch_, Any]))),
                Reachable(ny, p_c, Some(Positional(vec![ny_, dc_, at_, Any]))),
                Reachable(ny, p_d, Some(Positional(vec![ny_, dc_, Any]))),
                Reachable(ny, p_e, Some(Positional(vec![ny_, Any]))),
                Reachable(ny, p_i, Some(Positional(vec![ny_, Any]))),
                // after
                Reachable(se, p_a, Some(Positional(vec![se_, sv_, la_, Any]))),
                Reachable(se, p_b, Some(Positional(vec![se_, Any]))),
                Reachable(se, p_c, Some(Positional(vec![se_, sv_, la_, hs_, Any]))),
                Reachable(se, p_d, Some(Positional(vec![se_, sv_, Any]))),
                Reachable(se, p_e, Some(Positional(vec![se_, dv_, Any]))),
                Reachable(se, p_i, Some(Positional(vec![se_, dv_, Any]))),
                Reachable(sv, p_a, Some(Positional(vec![sv_, la_, Any]))),
                Reachable(sv, p_b, Some(Positional(vec![sv_, se_, Any]))),
                Reachable(sv, p_c, Some(Positional(vec![sv_, la_, hs_, Any]))),
                Reachable(sv, p_d, Some(Positional(vec![sv_, Any]))),
                Reachable(sv, p_e, Some(Positional(vec![sv_, dv_, Any]))),
                Reachable(sv, p_i, Some(Positional(vec![sv_, Any]))),
                Reachable(dv, p_a, Some(Positional(vec![dv_, ks_, hs_, at_, Any]))),
                Reachable(dv, p_b, Some(Positional(vec![dv_, se_, Any]))),
                Reachable(dv, p_c, Some(Positional(vec![dv_, ks_, hs_, Any]))),
                Reachable(dv, p_d, Some(Positional(vec![dv_, sv_, Any]))),
                Reachable(dv, p_e, Some(Positional(vec![dv_, Any]))),
                Reachable(dv, p_i, Some(Positional(vec![dv_, Any]))),
                Reachable(la, p_a, Some(Positional(vec![la_, Any]))),
                Reachable(la, p_b, Some(Positional(vec![la_, sv_, se_, Any]))),
                Reachable(la, p_c, Some(Positional(vec![la_, hs_, Any]))),
                Reachable(la, p_d, Some(Positional(vec![la_, hs_, at_, dc_, Any]))),
                Reachable(la, p_e, Some(Positional(vec![la_, hs_, at_, dc_, ny_, Any]))),
                Reachable(la, p_i, Some(Positional(vec![la_, hs_, at_, dc_, Any]))),
                Reachable(ks, p_a, Some(Positional(vec![ks_, hs_, at_, Any]))),
                Reachable(ks, p_b, Some(Positional(vec![ks_, dv_, se_, Any]))),
                Reachable(ks, p_c, Some(Positional(vec![ks_, hs_, Any]))),
                Reachable(ks, p_d, Some(Positional(vec![ks_, dv_, sv_, Any]))),
                Reachable(ks, p_e, Some(Positional(vec![ks_, dv_, Any]))),
                Reachable(ks, p_i, Some(Positional(vec![ks_, dv_, Any]))),
                Reachable(hs, p_a, Some(Positional(vec![hs_, at_, Any]))),
                Reachable(hs, p_b, Some(Positional(vec![hs_, la_, sv_, se_, Any]))),
                Reachable(hs, p_c, Some(Positional(vec![hs_, Any]))),
                Reachable(hs, p_d, Some(Positional(vec![hs_, at_, dc_, Any]))),
                Reachable(hs, p_e, Some(Positional(vec![hs_, at_, dc_, ny_, Any]))),
                Reachable(hs, p_i, Some(Positional(vec![hs_, at_, dc_, Any]))),
                Reachable(ip, p_a, Some(Positional(vec![ip_, at_, Any]))),
                Reachable(ip, p_b, Some(Positional(vec![ip_, ch_, Any]))),
                Reachable(ip, p_c, Some(Positional(vec![ip_, at_, Any]))),
                Reachable(ip, p_d, Some(Positional(vec![ip_, ch_, ny_, dc_, Any]))),
                Reachable(ip, p_e, Some(Positional(vec![ip_, ch_, ny_, Any]))),
                Reachable(ip, p_i, Some(Positional(vec![ip_, ch_, ny_, Any]))),
                Reachable(at, p_a, Some(Positional(vec![at_, Any]))),
                Reachable(at, p_b, Some(Positional(vec![at_, hs_, la_, sv_, se_, Any]))),
                Reachable(at, p_c, Some(Positional(vec![at_, Any]))),
                Reachable(at, p_d, Some(Positional(vec![at_, dc_, Any]))),
                Reachable(at, p_e, Some(Positional(vec![at_, dc_, ny_, Any]))),
                Reachable(at, p_i, Some(Positional(vec![at_, dc_, Any]))),
                Reachable(ch, p_a, Some(Positional(vec![ch_, ny_, dc_, at_, Any]))),
                Reachable(ch, p_b, Some(Positional(vec![ch_, Any]))),
                Reachable(ch, p_c, Some(Positional(vec![ch_, ny_, dc_, at_, Any]))),
                Reachable(ch, p_d, Some(Positional(vec![ch_, ny_, dc_, Any]))),
                Reachable(ch, p_e, Some(Positional(vec![ch_, ny_, Any]))),
                Reachable(ch, p_i, Some(Positional(vec![ch_, ny_, Any]))),
                Reachable(dc, p_a, Some(Positional(vec![dc_, at_, Any]))),
                Reachable(dc, p_b, Some(Positional(vec![dc_, ny_, ch_, Any]))),
                Reachable(dc, p_c, Some(Positional(vec![dc_, at_, Any]))),
                Reachable(dc, p_d, Some(Positional(vec![dc_, Any]))),
                Reachable(dc, p_e, Some(Positional(vec![dc_, ny_, Any]))),
                Reachable(dc, p_i, Some(Positional(vec![dc_, Any]))),
                Reachable(ny, p_a, Some(Positional(vec![ny_, dc_, at_, Any]))),
                Reachable(ny, p_b, Some(Positional(vec![ny_, ch_, Any]))),
                Reachable(ny, p_c, Some(Positional(vec![ny_, dc_, at_, Any]))),
                Reachable(ny, p_d, Some(Positional(vec![ny_, dc_, Any]))),
                Reachable(ny, p_e, Some(Positional(vec![ny_, Any]))),
                Reachable(ny, p_i, Some(Positional(vec![ny_, Any]))),
            ];

            let num_flows = 11 * 6;

            HardPolicy::new(
                prop_vars,
                LTLModal::Now(Box::new(LTLBoolean::And(
                    (0..num_flows)
                        .map(|i| {
                            Box::new(LTLModal::Until(
                                Box::new(i),
                                Box::new(LTLModal::Globally(Box::new(i + num_flows))),
                            )) as Box<dyn LTLOperator>
                        })
                        .collect(),
                ))),
            )
        } else {
            panic!("Invalid variant number!")
        }
    }
}
