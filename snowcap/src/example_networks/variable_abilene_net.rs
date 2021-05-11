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

use super::{repetitions::Repetitions, ExampleNetwork};
use crate::hard_policies::*;
use crate::netsim::config::{Config, ConfigExpr::*};
use crate::netsim::route_map::*;
use crate::netsim::{AsId, BgpSessionType::*, Network, Prefix};
use std::marker::PhantomData;

/// # Variable Abilene Network
///
/// This network is taken from [topology-zoo](http://topology-zoo.org/dataset.html), and consists of
/// 11 internal routers.
///
/// ![Variable Abilene Network](https://n.ethz.ch/~sctibor/images/usa_2.svg)
///
/// The reconfiguratoin scenario is variable. The type argument controls the number of link weights,
/// that will be doubled during the reconfiguration. The variant flag is used only for determining
/// the policy. The variant is treated the number of flows, for which the condition is:
///
/// $$\phi_{old}\ \mathbf{U}\ \phi_{new}$$
///
/// Here, $\phi_{old}$ is the path in the initial configuration, and $\phi_{new}$ is the path in the
/// new configuraiton. For all other flows, we require that they are globally reachable.
///
/// The variant must be larger than 0 and smaller than 66, and the type argument must be smaller
/// than 14.
pub struct VariableAbileneNetwork<R: Repetitions> {
    phantom: PhantomData<R>,
}

impl<R: Repetitions> ExampleNetwork for VariableAbileneNetwork<R> {
    /// Get raw network without configuration
    fn net(initial_variant: usize) -> Network {
        let mut net = Network::new();

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
        let as_p_i = AsId(1);
        let as_p_a = AsId(65201);
        let as_p_b = AsId(65202);
        let as_p_c = AsId(65203);
        let as_p_d = AsId(65204);
        let as_p_e = AsId(65205);

        // add routers
        let sv = net.add_router("Sunnyvale"); // 0
        let se = net.add_router("Seattle"); // 1
        let dv = net.add_router("Denver"); // 2
        let la = net.add_router("LosAngeles"); // 3
        let hs = net.add_router("Huston"); // 4
        let ks = net.add_router("KansasCity"); // 5
        let ip = net.add_router("Indianapolis"); // 6
        let at = net.add_router("Atlanta"); // 7
        let dc = net.add_router("WashingtonDC"); // 8
        let ny = net.add_router("NewYork"); // 9
        let ch = net.add_router("Chicago"); // 10

        let la_a = net.add_external_router("ext_A_LosAngeles", as_a); // 11
        let at_a = net.add_external_router("ext_A_Atlanta", as_a); // 12
        let se_b = net.add_external_router("ext_B_Seattle", as_b); // 13
        let ch_b = net.add_external_router("ext_B_Chicago", as_b); // 14
        let hs_c = net.add_external_router("ext_C_Huston", as_c); // 15
        let at_c = net.add_external_router("ext_C_Atlanta", as_c); // 16
        let sv_d = net.add_external_router("ext_D_Sunnyvale", as_d); // 17
        let dc_d = net.add_external_router("ext_D_WashingtonDC", as_d); // 18
        let dv_e = net.add_external_router("ext_E_Denver", as_e); // 19
        let ny_e = net.add_external_router("ext_E_NewYork", as_e); // 20

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

        net.advertise_external_route(la_a, p_a, vec![as_a, as_p_a], None, None).unwrap();
        net.advertise_external_route(at_a, p_a, vec![as_a, as_p_a], None, None).unwrap();
        net.advertise_external_route(se_b, p_b, vec![as_b, as_p_b], None, None).unwrap();
        net.advertise_external_route(ch_b, p_b, vec![as_b, as_p_b], None, None).unwrap();
        net.advertise_external_route(hs_c, p_c, vec![as_c, as_p_c], None, None).unwrap();
        net.advertise_external_route(at_c, p_c, vec![as_c, as_p_c], None, None).unwrap();
        net.advertise_external_route(sv_d, p_d, vec![as_d, as_p_d], None, None).unwrap();
        net.advertise_external_route(dc_d, p_d, vec![as_d, as_p_d], None, None).unwrap();
        net.advertise_external_route(dv_e, p_e, vec![as_e, as_p_e], None, None).unwrap();
        net.advertise_external_route(ny_e, p_e, vec![as_e, as_p_e], None, None).unwrap();
        net.advertise_external_route(sv_d, p_i, vec![as_d, as_p_i], None, None).unwrap();
        net.advertise_external_route(dc_d, p_i, vec![as_d, as_p_i], None, None).unwrap();
        net.advertise_external_route(dv_e, p_i, vec![as_e, as_p_i], None, None).unwrap();
        net.advertise_external_route(ny_e, p_i, vec![as_e, as_p_i], None, None).unwrap();

        net
    }

    fn initial_config(net: &Network, _variant: usize) -> Config {
        let mut c = Config::new();

        let sv = net.get_router_id("Sunnyvale").unwrap();
        let se = net.get_router_id("Seattle").unwrap();
        let dv = net.get_router_id("Denver").unwrap();
        let la = net.get_router_id("LosAngeles").unwrap();
        let hs = net.get_router_id("Huston").unwrap();
        let ks = net.get_router_id("KansasCity").unwrap();
        let ip = net.get_router_id("Indianapolis").unwrap();
        let at = net.get_router_id("Atlanta").unwrap();
        let dc = net.get_router_id("WashingtonDC").unwrap();
        let ny = net.get_router_id("NewYork").unwrap();
        let ch = net.get_router_id("Chicago").unwrap();

        let la_a = net.get_router_id("ext_A_LosAngeles").unwrap();
        let at_a = net.get_router_id("ext_A_Atlanta").unwrap();
        let se_b = net.get_router_id("ext_B_Seattle").unwrap();
        let ch_b = net.get_router_id("ext_B_Chicago").unwrap();
        let hs_c = net.get_router_id("ext_C_Huston").unwrap();
        let at_c = net.get_router_id("ext_C_Atlanta").unwrap();
        let sv_d = net.get_router_id("ext_D_Sunnyvale").unwrap();
        let dc_d = net.get_router_id("ext_D_WashingtonDC").unwrap();
        let dv_e = net.get_router_id("ext_E_Denver").unwrap();
        let ny_e = net.get_router_id("ext_E_NewYork").unwrap();

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

        // RR-Topo with 2 levels, and 1 top-level RR in the center

        // 1-level RRs
        // c.add(BgpSession { source: dv, target: hs, session_type: IBgpPeer }).unwrap();
        // c.add(BgpSession { source: dv, target: ip, session_type: IBgpPeer }).unwrap();
        // c.add(BgpSession { source: hs, target: ip, session_type: IBgpPeer }).unwrap();
        // c.add(BgpSession { source: dv, target: ks, session_type: IBgpClient }).unwrap();

        // 2-level RRs to dv
        c.add(BgpSession { source: dv, target: hs, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: dv, target: ip, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: dv, target: ks, session_type: IBgpClient }).unwrap();

        // 2-level RRs
        // c.add(BgpSession { source: ks, target: dv, session_type: IBgpClient }).unwrap();
        // c.add(BgpSession { source: ks, target: hs, session_type: IBgpClient }).unwrap();
        // c.add(BgpSession { source: ks, target: ip, session_type: IBgpClient }).unwrap();

        c.add(BgpSession { source: dv, target: sv, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: dv, target: se, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: hs, target: la, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: hs, target: at, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: ip, target: ch, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: ip, target: ny, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: ip, target: dc, session_type: IBgpClient }).unwrap();

        c
    }

    #[rustfmt::skip]
    fn final_config(net: &Network, _variant: usize) -> Config {
        let mut c = Config::new();

        let sv = net.get_router_id("Sunnyvale").unwrap();
        let se = net.get_router_id("Seattle").unwrap();
        let dv = net.get_router_id("Denver").unwrap();
        let la = net.get_router_id("LosAngeles").unwrap();
        let hs = net.get_router_id("Huston").unwrap();
        let ks = net.get_router_id("KansasCity").unwrap();
        let ip = net.get_router_id("Indianapolis").unwrap();
        let at = net.get_router_id("Atlanta").unwrap();
        let dc = net.get_router_id("WashingtonDC").unwrap();
        let ny = net.get_router_id("NewYork").unwrap();
        let ch = net.get_router_id("Chicago").unwrap();

        let la_a = net.get_router_id("ext_A_LosAngeles").unwrap();
        let at_a = net.get_router_id("ext_A_Atlanta").unwrap();
        let se_b = net.get_router_id("ext_B_Seattle").unwrap();
        let ch_b = net.get_router_id("ext_B_Chicago").unwrap();
        let hs_c = net.get_router_id("ext_C_Huston").unwrap();
        let at_c = net.get_router_id("ext_C_Atlanta").unwrap();
        let sv_d = net.get_router_id("ext_D_Sunnyvale").unwrap();
        let dc_d = net.get_router_id("ext_D_WashingtonDC").unwrap();
        let dv_e = net.get_router_id("ext_E_Denver").unwrap();
        let ny_e = net.get_router_id("ext_E_NewYork").unwrap();

        let x = R::get_count();
        assert!(x <= 14);

        // link weight
        c.add(IgpLinkWeight { source: sv, target: se, weight: if x > 0 {10.0} else {5.0} }).unwrap();
        c.add(IgpLinkWeight { source: sv, target: dv, weight: if x > 1 {24.0} else {12.0} }).unwrap();
        c.add(IgpLinkWeight { source: sv, target: la, weight: if x > 2 {6.0} else {3.0} }).unwrap();
        c.add(IgpLinkWeight { source: se, target: dv, weight: if x > 3 {16.0} else {8.0} }).unwrap();
        c.add(IgpLinkWeight { source: dv, target: ks, weight: if x > 4 {16.0} else {8.0} }).unwrap();
        c.add(IgpLinkWeight { source: la, target: hs, weight: if x > 5 {14.0} else {7.0} }).unwrap();
        c.add(IgpLinkWeight { source: ks, target: hs, weight: if x > 6 {24.0} else {12.0} }).unwrap();
        c.add(IgpLinkWeight { source: ks, target: ip, weight: if x > 7 {28.0} else {14.0} }).unwrap();
        c.add(IgpLinkWeight { source: hs, target: at, weight: if x > 8 {10.0} else {5.0} }).unwrap();
        c.add(IgpLinkWeight { source: ip, target: at, weight: if x > 9 {32.0} else {16.0} }).unwrap();
        c.add(IgpLinkWeight { source: ip, target: ch, weight: if x > 10 {16.0} else {8.0} }).unwrap();
        c.add(IgpLinkWeight { source: at, target: dc, weight: if x > 11 {8.0} else {4.0} }).unwrap();
        c.add(IgpLinkWeight { source: ch, target: ny, weight: if x > 12 {10.0} else {5.0} }).unwrap();
        c.add(IgpLinkWeight { source: dc, target: ny, weight: if x > 13 {4.0} else {2.0} }).unwrap();
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

        c.add(IgpLinkWeight { target: sv, source: se, weight: if x > 0 {10.0} else {5.0} }).unwrap();
        c.add(IgpLinkWeight { target: sv, source: dv, weight: if x > 1 {24.0} else {12.0} }).unwrap();
        c.add(IgpLinkWeight { target: sv, source: la, weight: if x > 2 {6.0} else {3.0} }).unwrap();
        c.add(IgpLinkWeight { target: se, source: dv, weight: if x > 3 {16.0} else {8.0} }).unwrap();
        c.add(IgpLinkWeight { target: dv, source: ks, weight: if x > 4 {16.0} else {8.0} }).unwrap();
        c.add(IgpLinkWeight { target: la, source: hs, weight: if x > 5 {14.0} else {7.0} }).unwrap();
        c.add(IgpLinkWeight { target: ks, source: hs, weight: if x > 6 {24.0} else {12.0} }).unwrap();
        c.add(IgpLinkWeight { target: ks, source: ip, weight: if x > 7 {28.0} else {14.0} }).unwrap();
        c.add(IgpLinkWeight { target: hs, source: at, weight: if x > 8 {10.0} else {5.0} }).unwrap();
        c.add(IgpLinkWeight { target: ip, source: at, weight: if x > 9 {32.0} else {16.0} }).unwrap();
        c.add(IgpLinkWeight { target: ip, source: ch, weight: if x > 10 {16.0} else {8.0} }).unwrap();
        c.add(IgpLinkWeight { target: at, source: dc, weight: if x > 11 {8.0} else {4.0} }).unwrap();
        c.add(IgpLinkWeight { target: ch, source: ny, weight: if x > 12 {10.0} else {5.0} }).unwrap();
        c.add(IgpLinkWeight { target: dc, source: ny, weight: if x > 13 {4.0} else {2.0} }).unwrap();
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

        // RR-Topo with 3 top-level RRs in the center
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

        c
    }

    fn get_policy(net: &Network, mut variant: usize) -> HardPolicy {
        assert!(variant <= net.get_routers().len() * net.get_known_prefixes().len());

        // prepare the initial state
        let initial_config = Self::initial_config(net, variant);
        let mut initial_net = net.clone();
        initial_net.set_config(&initial_config).unwrap();
        let mut initial_state = initial_net.get_forwarding_state();

        // prepare the final state
        let final_config = Self::final_config(net, variant);
        let mut final_net = net.clone();
        final_net.set_config(&final_config).unwrap();
        let mut final_state = final_net.get_forwarding_state();

        let mut prop_vars: Vec<Condition> = Vec::new();
        let mut formula_parts: Vec<Box<dyn LTLOperator>> = Vec::new();

        let sv = net.get_router_id("Sunnyvale").unwrap();
        let se = net.get_router_id("Seattle").unwrap();
        let dv = net.get_router_id("Denver").unwrap();
        let la = net.get_router_id("LosAngeles").unwrap();
        let hs = net.get_router_id("Huston").unwrap();
        let ks = net.get_router_id("KansasCity").unwrap();
        let ip = net.get_router_id("Indianapolis").unwrap();
        let at = net.get_router_id("Atlanta").unwrap();
        let dc = net.get_router_id("WashingtonDC").unwrap();
        let ny = net.get_router_id("NewYork").unwrap();
        let ch = net.get_router_id("Chicago").unwrap();
        let routers = vec![sv, se, dv, la, hs, ks, ip, at, dc, ny, ch];
        let prefixes = vec![Prefix(1), Prefix(2), Prefix(3), Prefix(4), Prefix(5), Prefix(0)];

        for &p in prefixes.iter() {
            for &r in routers.iter() {
                if variant > 0 {
                    let var_idx_before = prop_vars.len();
                    let var_idx_after = var_idx_before + 1;
                    prop_vars.push(Condition::Reachable(
                        r,
                        p,
                        Some(PathCondition::Positional(
                            initial_state
                                .get_route(r, p)
                                .unwrap()
                                .into_iter()
                                .map(|x| Waypoint::Fix(x))
                                .collect(),
                        )),
                    ));
                    prop_vars.push(Condition::Reachable(
                        r,
                        p,
                        Some(PathCondition::Positional(
                            final_state
                                .get_route(r, p)
                                .unwrap()
                                .into_iter()
                                .map(|x| Waypoint::Fix(x))
                                .collect(),
                        )),
                    ));
                    formula_parts.push(Box::new(LTLModal::Until(
                        Box::new(var_idx_before),
                        Box::new(var_idx_after),
                    )));
                    variant -= 1;
                } else {
                    let var_idx = prop_vars.len();
                    prop_vars.push(Condition::Reachable(r, p, None));
                    formula_parts.push(Box::new(LTLModal::Globally(Box::new(var_idx))));
                }
            }
        }

        let ltl_formula = LTLModal::Now(Box::new(LTLBoolean::And(formula_parts)));

        HardPolicy::new(prop_vars, ltl_formula)
    }
}
