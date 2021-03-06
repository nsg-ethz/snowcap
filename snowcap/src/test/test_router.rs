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

use crate::netsim::bgp::BgpSessionType::{EBgp, IBgpClient, IBgpPeer};
use crate::netsim::bgp::{BgpEvent, BgpRoute};
use crate::netsim::event::{Event, EventQueue};
use crate::netsim::external_router::*;
use crate::netsim::router::*;
use crate::netsim::types::IgpNetwork;
use crate::netsim::{AsId, DeviceError, Prefix};
use maplit::{hashmap, hashset};

#[test]
fn test_bgp_single() {
    let mut r = Router::new("test".to_string(), 0.into(), AsId(65001));
    let mut queue: EventQueue = EventQueue::new();
    r.establish_bgp_session(100.into(), EBgp, &mut queue, 0, false).unwrap();
    r.establish_bgp_session(1.into(), IBgpPeer, &mut queue, 0, false).unwrap();
    r.establish_bgp_session(2.into(), IBgpPeer, &mut queue, 0, false).unwrap();
    r.establish_bgp_session(3.into(), IBgpPeer, &mut queue, 0, false).unwrap();
    r.establish_bgp_session(4.into(), IBgpClient, &mut queue, 0, false).unwrap();
    r.establish_bgp_session(5.into(), IBgpClient, &mut queue, 0, false).unwrap();
    r.establish_bgp_session(6.into(), IBgpClient, &mut queue, 0, false).unwrap();
    r.igp_forwarding_table = hashmap! {
        100.into() => Some((100.into(), 0.0)),
        1.into()   => Some((1.into(), 1.0)),
        2.into()   => Some((2.into(), 1.0)),
        3.into()   => Some((2.into(), 4.0)),
        4.into()   => Some((4.into(), 2.0)),
        5.into()   => Some((4.into(), 6.0)),
        6.into()   => Some((1.into(), 13.0)),
        10.into()  => Some((1.into(), 6.0)),
        11.into()  => Some((1.into(), 15.0)),
    };

    let mut queue: EventQueue = EventQueue::new();

    /////////////////////
    // external update //
    /////////////////////

    r.handle_event(
        Event::Bgp(
            100.into(),
            0.into(),
            BgpEvent::Update(BgpRoute {
                prefix: Prefix(200),
                as_path: vec![AsId(1), AsId(2), AsId(3), AsId(4), AsId(5)],
                next_hop: 100.into(),
                local_pref: None,
                med: None,
                community: None,
            }),
        ),
        &mut queue,
        0,
    )
    .unwrap();

    // check that the router now has a route selected for 100 with the correct data
    let entry = r.get_selected_bgp_route(Prefix(200)).unwrap();
    assert_eq!(entry.from_type, EBgp);
    assert_eq!(entry.route.next_hop, 100.into());
    assert_eq!(entry.route.local_pref, Some(100));
    assert_eq!(queue.len(), 6);
    while let Some((job, _)) = queue.pop_front() {
        match job {
            Event::Bgp(from, _, BgpEvent::Update(r)) => {
                assert_eq!(from, 0.into());
                assert_eq!(r.next_hop, 100.into());
            }
            _ => assert!(false),
        }
    }
    // used for later
    let original_entry = entry.clone();

    /////////////////////
    // internal update //
    /////////////////////

    // update from route reflector

    r.handle_event(
        Event::Bgp(
            1.into(),
            0.into(),
            BgpEvent::Update(BgpRoute {
                prefix: Prefix(201),
                as_path: vec![AsId(1), AsId(2), AsId(3)],
                next_hop: 11.into(),
                local_pref: Some(50),
                med: None,
                community: None,
            }),
        ),
        &mut queue,
        0,
    )
    .unwrap();

    // check that the router now has a route selected for 100 with the correct data
    let entry = r.get_selected_bgp_route(Prefix(201)).unwrap();
    assert_eq!(entry.from_type, IBgpPeer);
    assert_eq!(entry.route.next_hop, 11.into());
    assert_eq!(entry.route.local_pref, Some(50));
    assert_eq!(queue.len(), 4);
    while let Some((job, _)) = queue.pop_front() {
        match job {
            Event::Bgp(from, to, BgpEvent::Update(r)) => {
                assert_eq!(from, 0.into());
                assert!(hashset![4, 5, 6, 100].contains(&(to.index() as usize)));
                if to == 100.into() {
                    assert_eq!(r.next_hop, 0.into());
                } else {
                    assert_eq!(r.next_hop, 11.into());
                }
            }
            _ => assert!(false),
        }
    }

    //////////////////
    // worse update //
    //////////////////

    // update from route reflector

    r.handle_event(
        Event::Bgp(
            2.into(),
            0.into(),
            BgpEvent::Update(BgpRoute {
                prefix: Prefix(200),
                as_path: vec![AsId(1), AsId(2), AsId(3), AsId(4), AsId(5)],
                next_hop: 10.into(),
                local_pref: None,
                med: None,
                community: None,
            }),
        ),
        &mut queue,
        0,
    )
    .unwrap();

    // check that
    let entry = r.get_selected_bgp_route(Prefix(200)).unwrap();
    assert_eq!(entry.from_type, EBgp);
    assert_eq!(entry.route.next_hop, 100.into());
    assert_eq!(queue.len(), 0);

    ///////////////////
    // better update //
    ///////////////////

    // update from route reflector

    r.handle_event(
        Event::Bgp(
            5.into(),
            0.into(),
            BgpEvent::Update(BgpRoute {
                prefix: Prefix(200),
                as_path: vec![
                    AsId(1),
                    AsId(2),
                    AsId(3),
                    AsId(4),
                    AsId(5),
                    AsId(6),
                    AsId(7),
                    AsId(8),
                    AsId(9),
                    AsId(10),
                ],
                next_hop: 5.into(),
                local_pref: Some(150),
                med: None,
                community: None,
            }),
        ),
        &mut queue,
        0,
    )
    .unwrap();

    // check that the router now has a route selected for 100 with the correct data
    let entry = r.get_selected_bgp_route(Prefix(200)).unwrap().clone();
    assert_eq!(entry.from_type, IBgpClient);
    assert_eq!(entry.route.next_hop, 5.into());
    assert_eq!(entry.route.local_pref, Some(150));
    assert_eq!(queue.len(), 7);
    while let Some((job, _)) = queue.pop_front() {
        match job {
            Event::Bgp(from, to, BgpEvent::Update(r)) => {
                assert_eq!(from, 0.into());
                assert!(hashset![1, 2, 3, 4, 6, 100].contains(&(to.index() as usize)));
                if to == 100.into() {
                    assert_eq!(r.next_hop, 0.into());
                    assert_eq!(r.local_pref, None);
                } else {
                    assert_eq!(r.next_hop, 5.into());
                    assert_eq!(r.local_pref, Some(150));
                }
            }
            Event::Bgp(from, to, BgpEvent::Withdraw(prefix)) => {
                assert_eq!(from, 0.into());
                assert_eq!(to, 5.into());
                assert_eq!(prefix, Prefix(200));
            }
            e => panic!("Invalid event: {:?}", e),
        }
    }

    ///////////////////////
    // retract bad route //
    ///////////////////////

    r.handle_event(Event::Bgp(2.into(), 0.into(), BgpEvent::Withdraw(Prefix(200))), &mut queue, 0)
        .unwrap();

    // check that the router now has a route selected for 100 with the correct data
    let new_entry = r.get_selected_bgp_route(Prefix(200)).unwrap();
    assert_eq!(new_entry, entry);
    assert_eq!(queue.len(), 0);

    ////////////////////////
    // retract good route //
    ////////////////////////

    r.handle_event(Event::Bgp(5.into(), 0.into(), BgpEvent::Withdraw(Prefix(200))), &mut queue, 0)
        .unwrap();

    // check that the router now has a route selected for 100 with the correct data
    //eprintln!("{:#?}", r);
    let new_entry = r.get_selected_bgp_route(Prefix(200)).unwrap();
    assert_eq!(new_entry, original_entry);
    assert_eq!(queue.len(), 7);
    while let Some((job, _)) = queue.pop_front() {
        match job {
            Event::Bgp(from, to, BgpEvent::Update(r)) => {
                assert_eq!(from, 0.into());
                assert!(hashset![1, 2, 3, 4, 5, 6].contains(&(to.index() as usize)));
                assert_eq!(r.next_hop, 100.into());
                assert_eq!(r.local_pref, Some(100));
            }
            Event::Bgp(from, to, BgpEvent::Withdraw(prefix)) => {
                assert_eq!(from, 0.into());
                assert_eq!(to, 100.into());
                assert_eq!(prefix, Prefix(200));
            }
            e => panic!("Invalid event: {:?}", e),
        }
    }

    ////////////////////////
    // retract last route //
    ////////////////////////

    r.handle_event(
        Event::Bgp(100.into(), 0.into(), BgpEvent::Withdraw(Prefix(200))),
        &mut queue,
        0,
    )
    .unwrap();

    // check that the router now has a route selected for 100 with the correct data
    assert!(r.get_selected_bgp_route(Prefix(200)).is_none());
    assert_eq!(queue.len(), 6);
    while let Some((job, _)) = queue.pop_front() {
        match job {
            Event::Bgp(from, to, BgpEvent::Withdraw(Prefix(200))) => {
                assert_eq!(from, 0.into());
                assert!(hashset![1, 2, 3, 4, 5, 6].contains(&(to.index() as usize)));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn test_fw_table_simple() {
    let mut net: IgpNetwork = IgpNetwork::new();
    let mut a = Router::new("A".to_string(), net.add_node(()), AsId(65001));
    let mut b = Router::new("B".to_string(), net.add_node(()), AsId(65001));
    let mut c = Router::new("C".to_string(), net.add_node(()), AsId(65001));
    let d = Router::new("D".to_string(), net.add_node(()), AsId(65001));
    let e = Router::new("E".to_string(), net.add_node(()), AsId(65001));

    net.add_edge(a.router_id(), b.router_id(), 1.0);
    net.add_edge(b.router_id(), c.router_id(), 1.0);
    net.add_edge(c.router_id(), d.router_id(), 1.0);
    net.add_edge(d.router_id(), e.router_id(), 1.0);
    net.add_edge(e.router_id(), d.router_id(), 1.0);
    net.add_edge(d.router_id(), c.router_id(), 1.0);
    net.add_edge(c.router_id(), b.router_id(), 1.0);
    net.add_edge(b.router_id(), a.router_id(), 1.0);

    /*
     * all weights = 1
     * c ----- c
     * |       |
     * |       |
     * b       d
     * |       |
     * |       |
     * a       e
     */

    a.write_igp_forwarding_table(&net, &mut EventQueue::new(), 0, false).unwrap();

    let expected_forwarding_table = hashmap! {
        a.router_id() => Some((a.router_id(), 0.0)),
        b.router_id() => Some((b.router_id(), 1.0)),
        c.router_id() => Some((b.router_id(), 2.0)),
        d.router_id() => Some((b.router_id(), 3.0)),
        e.router_id() => Some((b.router_id(), 4.0)),
    };

    let exp = &expected_forwarding_table;
    let acq = &a.igp_forwarding_table;

    for target in vec![&a, &b, &c, &d, &e] {
        assert_eq!(exp.get(&target.router_id()), acq.get(&target.router_id()));
    }

    b.write_igp_forwarding_table(&net, &mut EventQueue::new(), 0, false).unwrap();

    let expected_forwarding_table = hashmap! {
        a.router_id() => Some((a.router_id(), 1.0)),
        b.router_id() => Some((b.router_id(), 0.0)),
        c.router_id() => Some((c.router_id(), 1.0)),
        d.router_id() => Some((c.router_id(), 2.0)),
        e.router_id() => Some((c.router_id(), 3.0)),
    };

    let exp = &expected_forwarding_table;
    let acq = &b.igp_forwarding_table;

    for target in vec![&a, &b, &c, &d, &e] {
        assert_eq!(exp.get(&target.router_id()), acq.get(&target.router_id()));
    }

    c.write_igp_forwarding_table(&net, &mut EventQueue::new(), 0, false).unwrap();

    let expected_forwarding_table = hashmap! {
        a.router_id() => Some((b.router_id(), 2.0)),
        b.router_id() => Some((b.router_id(), 1.0)),
        c.router_id() => Some((c.router_id(), 0.0)),
        d.router_id() => Some((d.router_id(), 1.0)),
        e.router_id() => Some((d.router_id(), 2.0)),
    };

    let exp = &expected_forwarding_table;
    let acq = &c.igp_forwarding_table;

    for target in vec![&a, &b, &c, &d, &e] {
        assert_eq!(exp.get(&target.router_id()), acq.get(&target.router_id()));
    }
}

#[test]
fn test_igp_fw_table_complex() {
    let mut net: IgpNetwork = IgpNetwork::new();
    let mut a = Router::new("A".to_string(), net.add_node(()), AsId(65001));
    let b = Router::new("B".to_string(), net.add_node(()), AsId(65001));
    let mut c = Router::new("C".to_string(), net.add_node(()), AsId(65001));
    let d = Router::new("D".to_string(), net.add_node(()), AsId(65001));
    let e = Router::new("E".to_string(), net.add_node(()), AsId(65001));
    let f = Router::new("F".to_string(), net.add_node(()), AsId(65001));
    let g = Router::new("G".to_string(), net.add_node(()), AsId(65001));
    let h = Router::new("H".to_string(), net.add_node(()), AsId(65001));

    net.add_edge(a.router_id(), b.router_id(), 3.0);
    net.add_edge(b.router_id(), a.router_id(), 3.0);
    net.add_edge(a.router_id(), e.router_id(), 1.0);
    net.add_edge(e.router_id(), a.router_id(), 1.0);
    net.add_edge(b.router_id(), c.router_id(), 8.0);
    net.add_edge(c.router_id(), b.router_id(), 8.0);
    net.add_edge(b.router_id(), f.router_id(), 2.0);
    net.add_edge(f.router_id(), b.router_id(), 2.0);
    net.add_edge(c.router_id(), d.router_id(), 8.0);
    net.add_edge(d.router_id(), c.router_id(), 8.0);
    net.add_edge(c.router_id(), f.router_id(), 1.0);
    net.add_edge(f.router_id(), c.router_id(), 1.0);
    net.add_edge(c.router_id(), g.router_id(), 1.0);
    net.add_edge(g.router_id(), c.router_id(), 1.0);
    net.add_edge(d.router_id(), h.router_id(), 1.0);
    net.add_edge(h.router_id(), d.router_id(), 1.0);
    net.add_edge(e.router_id(), f.router_id(), 1.0);
    net.add_edge(f.router_id(), e.router_id(), 1.0);
    net.add_edge(f.router_id(), g.router_id(), 8.0);
    net.add_edge(g.router_id(), f.router_id(), 8.0);
    net.add_edge(g.router_id(), h.router_id(), 1.0);
    net.add_edge(h.router_id(), g.router_id(), 1.0);

    /*
     *    3      8      8
     * a ---- b ---- c ---- d
     * |      |    / |      |
     * |1    2|  --  |1     |1
     * |      | / 1  |      |
     * e ---- f ---- g ---- h
     *    1      8      1
     */

    a.write_igp_forwarding_table(&net, &mut EventQueue::new(), 0, false).unwrap();

    let expected_forwarding_table = hashmap! {
        a.router_id() => Some((a.router_id(), 0.0)),
        b.router_id() => Some((b.router_id(), 3.0)),
        c.router_id() => Some((e.router_id(), 3.0)),
        d.router_id() => Some((e.router_id(), 6.0)),
        e.router_id() => Some((e.router_id(), 1.0)),
        f.router_id() => Some((e.router_id(), 2.0)),
        g.router_id() => Some((e.router_id(), 4.0)),
        h.router_id() => Some((e.router_id(), 5.0)),
    };

    let exp = &expected_forwarding_table;
    let acq = &a.igp_forwarding_table;

    for target in vec![&a, &b, &c, &d, &e, &f, &g, &h] {
        assert_eq!(exp.get(&target.router_id()), acq.get(&target.router_id()));
    }

    c.write_igp_forwarding_table(&net, &mut EventQueue::new(), 0, false).unwrap();

    let expected_forwarding_table = hashmap! {
        a.router_id() => Some((f.router_id(), 3.0)),
        b.router_id() => Some((f.router_id(), 3.0)),
        c.router_id() => Some((c.router_id(), 0.0)),
        d.router_id() => Some((g.router_id(), 3.0)),
        e.router_id() => Some((f.router_id(), 2.0)),
        f.router_id() => Some((f.router_id(), 1.0)),
        g.router_id() => Some((g.router_id(), 1.0)),
        h.router_id() => Some((g.router_id(), 2.0)),
    };

    let exp = &expected_forwarding_table;
    let acq = &c.igp_forwarding_table;

    for target in vec![&a, &b, &c, &d, &e, &f, &g, &h] {
        assert_eq!(exp.get(&target.router_id()), acq.get(&target.router_id()));
    }
}

#[test]
fn external_router_advertise_to_neighbors() {
    // test that an external router will advertise a route to an already existing neighbor
    let mut r = ExternalRouter::new("router".to_string(), 0.into(), AsId(65001));
    let mut queue = EventQueue::new();

    // add the session
    r.establish_ebgp_session(1.into(), &mut queue, 0, false).unwrap();
    assert_eq!(queue.len(), 0);

    // add the session again and check that an error is returned
    assert_eq!(
        r.establish_ebgp_session(1.into(), &mut queue, 0, false),
        Err(DeviceError::SessionAlreadyExists(1.into()))
    );

    // advertise route
    r.advertise_prefix(Prefix(0), vec![AsId(0)], None, None, &mut queue, 0);

    // check that one event was created
    assert_eq!(queue.len(), 1);
    assert_eq!(
        queue.pop_front().unwrap().0,
        Event::Bgp(
            0.into(),
            1.into(),
            BgpEvent::Update(BgpRoute {
                prefix: Prefix(0),
                as_path: vec![AsId(0)],
                next_hop: 0.into(),
                local_pref: None,
                med: None,
                community: None,
            }),
        )
    );

    // emove the route
    r.widthdraw_prefix(Prefix(0), &mut queue, 0);

    // check that one event was created
    assert_eq!(queue.len(), 1);
    assert_eq!(
        queue.pop_front().unwrap().0,
        Event::Bgp(0.into(), 1.into(), BgpEvent::Withdraw(Prefix(0)))
    )
}

#[test]
fn external_router_new_neighbor() {
    // test that an external router will advertise a route to an already existing neighbor
    let mut r = ExternalRouter::new("router".to_string(), 0.into(), AsId(65001));
    let mut queue = EventQueue::new();

    // advertise route
    r.advertise_prefix(Prefix(0), vec![AsId(0)], None, None, &mut queue, 0);

    // check that no event was created
    assert_eq!(queue.len(), 0);

    // add a neighbor and check that the route is advertised
    r.establish_ebgp_session(1.into(), &mut queue, 0, false).unwrap();

    // check that one event was created
    assert_eq!(queue.len(), 1);
    assert_eq!(
        queue.pop_front().unwrap().0,
        Event::Bgp(
            0.into(),
            1.into(),
            BgpEvent::Update(BgpRoute {
                prefix: Prefix(0),
                as_path: vec![AsId(0)],
                next_hop: 0.into(),
                local_pref: None,
                med: None,
                community: None,
            }),
        )
    );

    // first, remove the neighbor, then stop advertising
    r.close_ebgp_session(1.into()).unwrap();
    assert_eq!(r.close_ebgp_session(1.into()), Err(DeviceError::NoBgpSession(1.into())));

    // then, withdraw the session
    r.widthdraw_prefix(Prefix(0), &mut queue, 0);
    assert_eq!(queue.len(), 0);
}
