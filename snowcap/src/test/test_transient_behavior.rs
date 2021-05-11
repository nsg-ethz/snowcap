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

//! Test cases for transient behavior in the Network
//!
//! **TODO**: All these tests are deactivated, since this funcitonality is temporarily removed from
//! the Network.

use crate::hard_policies::Constraints;
use crate::netsim::{
    bgp::BgpSessionType::*,
    config::{Config, ConfigExpr::*, ConfigModifier::*},
    route_map::{RouteMapBuilder, RouteMapDirection::*},
    AsId, Network, Prefix,
};

#[test]
fn test_simple_case_correct() {
    // Network:
    //
    //              1        5
    // e1 ---- r1 ----- r2 ----- r3 ---- e3
    //

    let mut n = Network::new();
    let mut c = Config::new();

    let r1 = n.add_router("r1");
    let r2 = n.add_router("r2");
    let r3 = n.add_router("r3");
    let e1 = n.add_external_router("e1", AsId(65101));
    let e3 = n.add_external_router("e3", AsId(65103));

    n.add_link(r1, r2);
    n.add_link(r2, r3);
    n.add_link(r1, e1);
    n.add_link(r3, e3);

    c.add(IgpLinkWeight { source: r1, target: r2, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r2, target: r3, weight: 5.0 }).unwrap();
    c.add(IgpLinkWeight { source: r1, target: e1, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r3, target: e3, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r1, source: r2, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r2, source: r3, weight: 5.0 }).unwrap();
    c.add(IgpLinkWeight { target: r1, source: e1, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r3, source: e3, weight: 1.0 }).unwrap();

    c.add(BgpSession { source: r1, target: e1, session_type: EBgp }).unwrap();
    c.add(BgpSession { source: r3, target: e3, session_type: EBgp }).unwrap();
    c.add(BgpSession { source: r2, target: r1, session_type: IBgpClient }).unwrap();
    c.add(BgpSession { source: r2, target: r3, session_type: IBgpClient }).unwrap();

    n.set_config(&c).unwrap();

    // advertise the same prefix everywhere
    n.advertise_external_route(e1, Prefix(0), vec![AsId(65101), AsId(65200)], None, None).unwrap();
    n.advertise_external_route(e3, Prefix(0), vec![AsId(65103), AsId(65200)], None, None).unwrap();

    // check constraints
    let constraints = Constraints::reachability()
        .build(&n.get_routers(), &n.get_known_prefixes().iter().cloned().collect());
    assert!(constraints.check(&mut n.get_forwarding_state()).is_ok());

    n.apply_modifier_check_constraints(
        &Remove(BgpSession { source: r3, target: e3, session_type: EBgp }),
        &constraints,
    )
    .unwrap();
}

#[test]
fn simple_case_incorrect() {
    // Network:
    //
    //              1        5
    // e1 ---- r1 ----- r2 ----- r3 ---- e3
    //

    let mut n = Network::new();
    let mut c = Config::new();

    let r1 = n.add_router("r1");
    let r2 = n.add_router("r2");
    let r3 = n.add_router("r3");
    let e1 = n.add_external_router("e1", AsId(65101));
    let e3 = n.add_external_router("e3", AsId(65103));

    n.add_link(r1, r2);
    n.add_link(r2, r3);
    n.add_link(r1, e1);
    n.add_link(r3, e3);

    c.add(IgpLinkWeight { source: r1, target: r2, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r2, target: r3, weight: 5.0 }).unwrap();
    c.add(IgpLinkWeight { source: r1, target: e1, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r3, target: e3, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r1, source: r2, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r2, source: r3, weight: 5.0 }).unwrap();
    c.add(IgpLinkWeight { target: r1, source: e1, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r3, source: e3, weight: 1.0 }).unwrap();

    c.add(BgpSession { source: r1, target: e1, session_type: EBgp }).unwrap();
    c.add(BgpSession { source: r3, target: e3, session_type: EBgp }).unwrap();
    c.add(BgpSession { source: r2, target: r1, session_type: IBgpClient }).unwrap();
    c.add(BgpSession { source: r2, target: r3, session_type: IBgpClient }).unwrap();

    n.set_config(&c).unwrap();

    // advertise the same prefix everywhere
    n.advertise_external_route(e1, Prefix(0), vec![AsId(65101), AsId(65200)], None, None).unwrap();
    n.advertise_external_route(e3, Prefix(0), vec![AsId(65103), AsId(65200)], None, None).unwrap();

    // check constraints
    let constraints = Constraints::reachability()
        .build(&n.get_routers(), &n.get_known_prefixes().iter().cloned().collect());
    assert!(constraints.check(&mut n.get_forwarding_state()).is_ok());

    n.apply_modifier_check_constraints(
        &Remove(BgpSession { source: r1, target: e1, session_type: EBgp }),
        &constraints,
    )
    .unwrap_err();
}

#[test]
fn multiple_options_incorrect() {
    // Network:
    //
    // e1 ---- r1 ------ r3 ---- e3
    //      .-'  \      /
    // e2 -'      \    /
    //              r2

    let mut n = Network::new();
    let mut c = Config::new();

    let r1 = n.add_router("r1");
    let r2 = n.add_router("r2");
    let r3 = n.add_router("r3");
    let e1 = n.add_external_router("e1", AsId(65101));
    let e2 = n.add_external_router("e2", AsId(65102));
    let e3 = n.add_external_router("e3", AsId(65103));

    n.add_link(r1, r2);
    n.add_link(r1, r3);
    n.add_link(r2, r3);
    n.add_link(r1, e1);
    n.add_link(r1, e2);
    n.add_link(r3, e3);

    c.add(IgpLinkWeight { source: r1, target: r2, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r1, target: r3, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r2, target: r3, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r1, target: e1, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r1, target: e2, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r3, target: e3, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r1, source: r2, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r1, source: r3, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r2, source: r3, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r1, source: e1, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r1, source: e2, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r3, source: e3, weight: 1.0 }).unwrap();

    c.add(BgpSession { source: r1, target: e1, session_type: EBgp }).unwrap();
    c.add(BgpSession { source: r1, target: e2, session_type: EBgp }).unwrap();
    c.add(BgpSession { source: r3, target: e3, session_type: EBgp }).unwrap();
    c.add(BgpSession { source: r1, target: r2, session_type: IBgpPeer }).unwrap();
    c.add(BgpSession { source: r1, target: r3, session_type: IBgpPeer }).unwrap();
    c.add(BgpSession { source: r2, target: r3, session_type: IBgpPeer }).unwrap();

    c.add(BgpRouteMap {
        router: r1,
        direction: Incoming,
        map: RouteMapBuilder::new()
            .order(100)
            .allow()
            .match_neighbor(e1)
            .set_local_pref(200)
            .build(),
    })
    .unwrap();
    c.add(BgpRouteMap {
        router: r1,
        direction: Incoming,
        map: RouteMapBuilder::new()
            .order(101)
            .allow()
            .match_neighbor(e2)
            .set_local_pref(50)
            .build(),
    })
    .unwrap();

    n.set_config(&c).unwrap();

    // advertise the same prefix everywhere
    n.advertise_external_route(e1, Prefix(0), vec![AsId(65101), AsId(65200)], None, None).unwrap();
    n.advertise_external_route(e2, Prefix(0), vec![AsId(65102), AsId(65200)], None, None).unwrap();
    n.advertise_external_route(e3, Prefix(0), vec![AsId(65103), AsId(65200)], None, None).unwrap();

    // check constraints
    let constraints = Constraints::reachability()
        .build(&n.get_routers(), &n.get_known_prefixes().iter().cloned().collect());
    assert!(constraints.check(&mut n.get_forwarding_state()).is_ok());

    n.apply_modifier_check_constraints(
        &Remove(BgpSession { source: r1, target: e1, session_type: EBgp }),
        &constraints,
    )
    .unwrap_err();
}

#[test]
fn multiple_options_correct() {
    // Network:
    //
    // e1 ---- r1 ------ r3 ---- e3
    //      .-'  \      /
    // e2 -'      \    /
    //              r2 ----- e4

    let mut n = Network::new();
    let mut c = Config::new();

    let r1 = n.add_router("r1");
    let r2 = n.add_router("r2");
    let r3 = n.add_router("r3");
    let e1 = n.add_external_router("e1", AsId(65101));
    let e2 = n.add_external_router("e2", AsId(65102));
    let e3 = n.add_external_router("e3", AsId(65103));
    let e4 = n.add_external_router("e4", AsId(65104));

    n.add_link(r1, r2);
    n.add_link(r1, r3);
    n.add_link(r2, r3);
    n.add_link(r1, e1);
    n.add_link(r1, e2);
    n.add_link(r3, e3);
    n.add_link(r2, e4);

    c.add(IgpLinkWeight { source: r1, target: r2, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r1, target: r3, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r2, target: r3, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r1, target: e1, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r1, target: e2, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r3, target: e3, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { source: r2, target: e4, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r1, source: r2, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r1, source: r3, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r2, source: r3, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r1, source: e1, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r1, source: e2, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r3, source: e3, weight: 1.0 }).unwrap();
    c.add(IgpLinkWeight { target: r2, source: e4, weight: 1.0 }).unwrap();

    c.add(BgpSession { source: r1, target: e1, session_type: EBgp }).unwrap();
    c.add(BgpSession { source: r1, target: e2, session_type: EBgp }).unwrap();
    c.add(BgpSession { source: r3, target: e3, session_type: EBgp }).unwrap();
    c.add(BgpSession { source: r2, target: e4, session_type: EBgp }).unwrap();
    c.add(BgpSession { source: r1, target: r2, session_type: IBgpPeer }).unwrap();
    c.add(BgpSession { source: r1, target: r3, session_type: IBgpPeer }).unwrap();
    c.add(BgpSession { source: r2, target: r3, session_type: IBgpPeer }).unwrap();

    c.add(BgpRouteMap {
        router: r1,
        direction: Incoming,
        map: RouteMapBuilder::new()
            .order(100)
            .allow()
            .match_neighbor(e1)
            .set_local_pref(200)
            .build(),
    })
    .unwrap();
    c.add(BgpRouteMap {
        router: r1,
        direction: Incoming,
        map: RouteMapBuilder::new()
            .order(101)
            .allow()
            .match_neighbor(e2)
            .set_local_pref(50)
            .build(),
    })
    .unwrap();
    c.add(BgpRouteMap {
        router: r1,
        direction: Incoming,
        map: RouteMapBuilder::new().order(10).allow().match_neighbor(e4).set_local_pref(50).build(),
    })
    .unwrap();

    n.set_config(&c).unwrap();

    // advertise the same prefix everywhere
    n.advertise_external_route(e1, Prefix(0), vec![AsId(65101), AsId(65200)], None, None).unwrap();
    n.advertise_external_route(e2, Prefix(0), vec![AsId(65102), AsId(65200)], None, None).unwrap();
    n.advertise_external_route(e3, Prefix(0), vec![AsId(65103), AsId(65200)], None, None).unwrap();
    n.advertise_external_route(e4, Prefix(0), vec![AsId(65104), AsId(65200)], None, None).unwrap();

    // check constraints
    let constraints = Constraints::reachability()
        .build(&n.get_routers(), &n.get_known_prefixes().iter().cloned().collect());
    assert!(constraints.check(&mut n.get_forwarding_state()).is_ok());

    n.apply_modifier_check_constraints(
        &Remove(BgpSession { source: r1, target: e1, session_type: EBgp }),
        &constraints,
    )
    .unwrap();
}
