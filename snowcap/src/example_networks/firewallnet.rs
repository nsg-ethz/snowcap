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

//! # Firewall Network

use super::ExampleNetwork;
use crate::hard_policies::*;
use crate::netsim::config::{Config, ConfigExpr::*};
use crate::netsim::{AsId, BgpSessionType::*, Network, Prefix};

/// # FirewallNet
///
/// ## Variant 0:
///
/// Hard Policy: `rx` must switch from using the old firewall between `r2` and `r6` (condition
/// $x_1$), to using the new firewall between `r1` and `r4` (condition $x_2$). We have the following
/// condition:
///
/// $$x_1\ \mathbf{U}\ \mathbf{G}\ x_2$$
///
/// ![FirewallNet](https://n.ethz.ch/~sctibor/images/Firewall_0.svg)
///
/// ## Variant 1:
///
/// This variant is the same as variant 0, but with a static route from `rx` to `r1`.
///
/// Hard Policy: `rx` must switch from using the old firewall between `r2` and `r6` (condition
/// $x_1$), to using the new firewall between `r1` and `r4` (condition $x_2$). In addition, we want
/// to make sure that during convergence, either the old or the new firewall are used (condition
/// $x_3$).
///
/// $$(x_1\ \mathbf{U}\ \mathbf{G}\ x_2) \land (\mathbf{G}\ x_3)$$
///
/// ![FirewallNet](https://n.ethz.ch/~sctibor/images/Firewall_1.svg)
pub struct FirewallNet {}

impl ExampleNetwork for FirewallNet {
    fn net(initial_variant: usize) -> Network {
        let mut net = Network::new();

        // add routers
        let rx = net.add_router("rx");
        let r1 = net.add_router("r1");
        let r2 = net.add_router("r2");
        let r3 = net.add_router("r3");
        let r4 = net.add_router("r4");
        let r5 = net.add_router("r5");
        let r6 = net.add_router("r6");
        let b1 = net.add_router("b1");
        let b2 = net.add_router("b2");
        let e1 = net.add_external_router("e1", AsId(65101));
        let e2 = net.add_external_router("e2", AsId(65102));

        // add links
        net.add_link(rx, r1);
        net.add_link(rx, r5);
        net.add_link(r1, r2);
        net.add_link(r1, r3);
        net.add_link(r1, r4);
        net.add_link(r2, r6);
        net.add_link(r3, r6);
        net.add_link(r4, r6);
        net.add_link(r4, b2);
        net.add_link(r4, r5);
        net.add_link(r5, b2);
        net.add_link(r6, b1);
        net.add_link(b1, e1);
        net.add_link(b2, e2);

        let cf = Self::initial_config(&net, initial_variant);
        net.set_config(&cf).unwrap();

        // advertise prefixes
        net.advertise_external_route(e1, Prefix(0), vec![AsId(65101), AsId(65200)], None, None)
            .unwrap();
        net.advertise_external_route(e2, Prefix(0), vec![AsId(65102), AsId(65200)], None, None)
            .unwrap();

        net
    }

    fn initial_config(net: &Network, variant: usize) -> Config {
        let mut c = Config::new();

        let rx = net.get_router_id("rx").unwrap();
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let r5 = net.get_router_id("r5").unwrap();
        let r6 = net.get_router_id("r6").unwrap();
        let b1 = net.get_router_id("b1").unwrap();
        let b2 = net.get_router_id("b2").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e2 = net.get_router_id("e2").unwrap();

        // add the sessions
        c.add(BgpSession { source: rx, target: r1, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: rx, target: r5, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r1, target: r3, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r1, target: r4, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r2, target: r3, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r3, target: r6, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r4, target: r6, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r4, target: b2, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r4, target: r5, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r5, target: b2, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r6, target: b1, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();

        // add the igp link weights
        if variant == 0 || variant == 1 {
            c.add(IgpLinkWeight { source: rx, target: r1, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { source: rx, target: r5, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: r1, target: r2, weight: 2.0 }).unwrap();
            c.add(IgpLinkWeight { source: r1, target: r3, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { source: r1, target: r4, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { source: r2, target: r6, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { source: r3, target: r6, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: r4, target: r6, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { source: r4, target: r5, weight: 6.0 }).unwrap();
            c.add(IgpLinkWeight { source: r4, target: b2, weight: 7.0 }).unwrap();
            c.add(IgpLinkWeight { source: r6, target: b1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: b2, target: e2, weight: 1.0 }).unwrap();

            c.add(IgpLinkWeight { target: rx, source: r1, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { target: rx, source: r5, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: r1, source: r2, weight: 2.0 }).unwrap();
            c.add(IgpLinkWeight { target: r1, source: r3, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { target: r1, source: r4, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { target: r2, source: r6, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { target: r3, source: r6, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: r4, source: r6, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { target: r4, source: r5, weight: 6.0 }).unwrap();
            c.add(IgpLinkWeight { target: r4, source: b2, weight: 7.0 }).unwrap();
            c.add(IgpLinkWeight { target: r6, source: b1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: b1, source: e1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: b2, source: e2, weight: 1.0 }).unwrap();
        } else {
            panic!("Invalid variant!");
        }

        if variant == 1 {
            c.add(StaticRoute { router: rx, prefix: Prefix(0), target: r1 }).unwrap();
        }

        c
    }

    fn final_config(net: &Network, variant: usize) -> Config {
        let mut c = Config::new();

        let rx = net.get_router_id("rx").unwrap();
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let r5 = net.get_router_id("r5").unwrap();
        let r6 = net.get_router_id("r6").unwrap();
        let b1 = net.get_router_id("b1").unwrap();
        let b2 = net.get_router_id("b2").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e2 = net.get_router_id("e2").unwrap();

        // add the sessions
        c.add(BgpSession { source: rx, target: r1, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: rx, target: r5, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r1, target: r3, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r1, target: r4, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r2, target: r3, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r3, target: r6, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r4, target: r6, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r4, target: b2, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r4, target: r5, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r5, target: b2, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: r6, target: b1, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();

        // add the igp link weights
        if variant == 0 || variant == 1 {
            c.add(IgpLinkWeight { source: rx, target: r1, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { source: rx, target: r5, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: r1, target: r2, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { source: r1, target: r3, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { source: r1, target: r4, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { source: r2, target: r6, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { source: r3, target: r6, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { source: r4, target: r6, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { source: r4, target: r5, weight: 6.0 }).unwrap();
            c.add(IgpLinkWeight { source: r4, target: b2, weight: 9.0 }).unwrap();
            c.add(IgpLinkWeight { source: r6, target: b1, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { source: b1, target: e1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: b2, target: e2, weight: 1.0 }).unwrap();

            c.add(IgpLinkWeight { target: rx, source: r1, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { target: rx, source: r5, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: r1, source: r2, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { target: r1, source: r3, weight: 5.0 }).unwrap();
            c.add(IgpLinkWeight { target: r1, source: r4, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { target: r2, source: r6, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { target: r3, source: r6, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { target: r4, source: r6, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { target: r4, source: r5, weight: 6.0 }).unwrap();
            c.add(IgpLinkWeight { target: r4, source: b2, weight: 9.0 }).unwrap();
            c.add(IgpLinkWeight { target: r6, source: b1, weight: 4.0 }).unwrap();
            c.add(IgpLinkWeight { target: b1, source: e1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: b2, source: e2, weight: 1.0 }).unwrap();
        } else {
            panic!("Invalid variant!");
        }

        if variant == 1 {
            c.add(StaticRoute { router: rx, prefix: Prefix(0), target: r1 }).unwrap();
        }

        c
    }

    /// Return the hard policy for the firewall network and the given variant.
    fn get_policy(net: &Network, variant: usize) -> HardPolicy {
        if variant == 0 {
            let rx = net.get_router_id("rx").unwrap();
            let r1 = net.get_router_id("r1").unwrap();
            let r2 = net.get_router_id("r2").unwrap();
            let r4 = net.get_router_id("r4").unwrap();
            let r6 = net.get_router_id("r6").unwrap();
            HardPolicy::new(
                vec![
                    Condition::Reachable(rx, Prefix(0), Some(PathCondition::Edge(r2, r6))),
                    Condition::Reachable(rx, Prefix(0), Some(PathCondition::Edge(r1, r4))),
                ],
                LTLModal::Until(Box::new(0), Box::new(LTLModal::Globally(Box::new(1)))),
            )
        } else if variant == 1 {
            let rx = net.get_router_id("rx").unwrap();
            let r1 = net.get_router_id("r1").unwrap();
            let r2 = net.get_router_id("r2").unwrap();
            let r4 = net.get_router_id("r4").unwrap();
            let r6 = net.get_router_id("r6").unwrap();
            HardPolicy::new(
                vec![
                    Condition::Reachable(rx, Prefix(0), Some(PathCondition::Edge(r2, r6))),
                    Condition::Reachable(rx, Prefix(0), Some(PathCondition::Edge(r1, r4))),
                    Condition::TransientPath(
                        rx,
                        Prefix(0),
                        PathCondition::Or(vec![
                            PathCondition::Edge(r2, r6),
                            PathCondition::Edge(r1, r4),
                        ]),
                    ),
                ],
                LTLModal::Now(Box::new(LTLBoolean::And(vec![
                    Box::new(LTLModal::Until(
                        Box::new(0),
                        Box::new(LTLModal::Globally(Box::new(1))),
                    )),
                    Box::new(LTLModal::Globally(Box::new(2))),
                ]))),
            )
        } else {
            panic!("Invalid variant!");
        }
    }
}
