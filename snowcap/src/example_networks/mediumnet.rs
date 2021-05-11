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

//! # Mediumnet Network

use super::ExampleNetwork;
use crate::hard_policies::HardPolicy;
use crate::netsim::config::{Config, ConfigExpr::*};
use crate::netsim::{AsId, BgpSessionType::*, Network, Prefix};

/// # Simplenet
///
/// Simplenet has 8 external routers, advertizing four different prefixes, and 14 internal routers.
///
/// ## Variants
///
/// ### Initial Variants
/// 0. Single level, reasonable
/// 1. Single level, unreasonable
///
/// ### Final Variants
/// 0. Two levels, reasonable, no IGP change
/// 1. Two levels, reasonable, with IGP weight change
/// 2. Two levels, unreasonable, no IGP change
/// 3. Two levels, unreasonable, with IGP weight change
///
///
/// ```text
///                  t1 -----80----- t2 ------.
///                .'| '.          .'| '.      '.
///             12'  |   15      18  |   '3      20
///            .'    |     '.  .'    |     '.      '.
///          r1      1       r2 --1--|------ r3 --1- r4
///        .'| '.    |       | '.    |       | '.      '.
///      1'  8   2.  |       4   3.  10      8   16      3.
///    .'    |     '.|       |     '.|       |     '.      '.
///  b1 -1-- b2      b3 -1-- b4      b5 -1-- b6      b7 -1-- b8
///  |       |       |       |       |       |       |       |
///  |       |       |       |       |       |       |       |
///  |       |       |       |       |       |       |       |
///  e1      e2      e3      e4      e5      e6      e7      e8
/// (1)     (1,2)   (2)     (2,3)   (3)     (3,4)   (4)     (1,4)
/// ```
pub struct MediumNet {}

impl ExampleNetwork for MediumNet {
    fn net(initial_variant: usize) -> Network {
        let mut net = Network::new();

        let b1 = net.add_router("b1");
        let b2 = net.add_router("b2");
        let b3 = net.add_router("b3");
        let b4 = net.add_router("b4");
        let b5 = net.add_router("b5");
        let b6 = net.add_router("b6");
        let b7 = net.add_router("b7");
        let b8 = net.add_router("b8");
        let r1 = net.add_router("r1");
        let r2 = net.add_router("r2");
        let r3 = net.add_router("r3");
        let r4 = net.add_router("r4");
        let t1 = net.add_router("t1");
        let t2 = net.add_router("t2");
        let e1 = net.add_external_router("e1", AsId(65101));
        let e2 = net.add_external_router("e2", AsId(65102));
        let e3 = net.add_external_router("e3", AsId(65103));
        let e4 = net.add_external_router("e4", AsId(65104));
        let e5 = net.add_external_router("e5", AsId(65105));
        let e6 = net.add_external_router("e6", AsId(65106));
        let e7 = net.add_external_router("e7", AsId(65107));
        let e8 = net.add_external_router("e8", AsId(65108));

        net.add_link(b1, b2);
        net.add_link(b1, r1);
        net.add_link(b2, r1);
        net.add_link(b3, r1);
        net.add_link(b3, t1);
        net.add_link(b3, b4);
        net.add_link(b4, r2);
        net.add_link(b5, r2);
        net.add_link(b5, t2);
        net.add_link(b5, b6);
        net.add_link(b6, r3);
        net.add_link(b7, r3);
        net.add_link(b7, b8);
        net.add_link(b8, r4);
        net.add_link(r1, t1);
        net.add_link(r2, t1);
        net.add_link(r2, t2);
        net.add_link(r2, r3);
        net.add_link(r3, t2);
        net.add_link(r3, r4);
        net.add_link(r4, t2);
        net.add_link(t1, t2);
        net.add_link(e1, b1);
        net.add_link(e2, b2);
        net.add_link(e3, b3);
        net.add_link(e4, b4);
        net.add_link(e5, b5);
        net.add_link(e6, b6);
        net.add_link(e7, b7);
        net.add_link(e8, b8);

        let cs = Self::initial_config(&net, initial_variant);
        net.set_config(&cs).unwrap();

        net.advertise_external_route(e1, Prefix(1), vec![AsId(65101), AsId(65201)], None, None)
            .unwrap();
        net.advertise_external_route(e2, Prefix(1), vec![AsId(65102), AsId(65201)], None, None)
            .unwrap();
        net.advertise_external_route(e2, Prefix(2), vec![AsId(65102), AsId(65202)], None, None)
            .unwrap();
        net.advertise_external_route(e3, Prefix(2), vec![AsId(65103), AsId(65202)], None, None)
            .unwrap();
        net.advertise_external_route(e4, Prefix(2), vec![AsId(65104), AsId(65202)], None, None)
            .unwrap();
        net.advertise_external_route(e4, Prefix(3), vec![AsId(65104), AsId(65203)], None, None)
            .unwrap();
        net.advertise_external_route(e5, Prefix(3), vec![AsId(65105), AsId(65203)], None, None)
            .unwrap();
        net.advertise_external_route(e6, Prefix(3), vec![AsId(65106), AsId(65203)], None, None)
            .unwrap();
        net.advertise_external_route(e6, Prefix(4), vec![AsId(65106), AsId(65204)], None, None)
            .unwrap();
        net.advertise_external_route(e7, Prefix(4), vec![AsId(65107), AsId(65204)], None, None)
            .unwrap();
        net.advertise_external_route(e8, Prefix(4), vec![AsId(65108), AsId(65204)], None, None)
            .unwrap();
        net.advertise_external_route(e8, Prefix(1), vec![AsId(65108), AsId(65201)], None, None)
            .unwrap();

        net
    }

    /// #Variant 0: Single level, reasonable
    /// - Weights according to the main documentation
    /// - The following BGP sessions are set
    ///   - e\[i\] --> b\[i\] (eBGP)
    ///   - iBGP Peer full mesh between r1, r2, r3, r4, t1 and t2
    ///   - b1 --> r1 (iBGP Client)
    ///   - b2 --> r1 (iBGP Client)
    ///   - b3 --> r1 (iBGP Client)
    ///   - b3 --> t1 (iBGP Client)
    ///   - b4 --> r2 (iBGP Client)
    ///   - b5 --> r2 (iBGP Client)
    ///   - b5 --> t2 (iBGP Client)
    ///   - b6 --> r3 (iBGP Client)
    ///   - b7 --> r3 (iBGP Client)
    ///   - b8 --> r4 (iBGP Client)
    /// #Variant 0: Single level, chaotic
    /// - Weights according to the main documentation
    /// - The following BGP sessions are set
    ///   - e\[i\] --> b\[i\] (eBGP)
    ///   - iBGP Peer full mesh between r1, r2, r3, r4, t1 and t2
    ///   - b1 --> t2 (iBGP Client)
    ///   - b2 --> r4 (iBGP Client)
    ///   - b3 --> r3 (iBGP Client)
    ///   - b4 --> r1 (iBGP Client)
    ///   - b5 --> t1 (iBGP Client)
    ///   - b6 --> r4 (iBGP Client)
    ///   - b7 --> t1 (iBGP Client)
    ///   - b8 --> r1 (iBGP Client)
    fn initial_config(net: &Network, variant: usize) -> Config {
        let mut c = Config::new();

        let b1 = net.get_router_id("b1").unwrap();
        let b2 = net.get_router_id("b2").unwrap();
        let b3 = net.get_router_id("b3").unwrap();
        let b4 = net.get_router_id("b4").unwrap();
        let b5 = net.get_router_id("b5").unwrap();
        let b6 = net.get_router_id("b6").unwrap();
        let b7 = net.get_router_id("b7").unwrap();
        let b8 = net.get_router_id("b8").unwrap();
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let t1 = net.get_router_id("t1").unwrap();
        let t2 = net.get_router_id("t2").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e2 = net.get_router_id("e2").unwrap();
        let e3 = net.get_router_id("e3").unwrap();
        let e4 = net.get_router_id("e4").unwrap();
        let e5 = net.get_router_id("e5").unwrap();
        let e6 = net.get_router_id("e6").unwrap();
        let e7 = net.get_router_id("e7").unwrap();
        let e8 = net.get_router_id("e8").unwrap();

        c.add(IgpLinkWeight { source: b1, target: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b1, target: r1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b2, target: r1, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { source: b3, target: r1, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { source: b3, target: t1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b3, target: b4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b4, target: r2, weight: 4.0 }).unwrap();
        c.add(IgpLinkWeight { source: b5, target: r2, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { source: b5, target: t2, weight: 10.0 }).unwrap();
        c.add(IgpLinkWeight { source: b5, target: b6, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b6, target: r3, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { source: b7, target: r3, weight: 16.0 }).unwrap();
        c.add(IgpLinkWeight { source: b7, target: b8, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b8, target: r4, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { source: r1, target: t1, weight: 12.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: t1, weight: 15.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: t2, weight: 18.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: r3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r3, target: t2, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { source: r3, target: r4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r4, target: t2, weight: 20.0 }).unwrap();
        c.add(IgpLinkWeight { source: t1, target: t2, weight: 80.0 }).unwrap();
        c.add(IgpLinkWeight { source: e1, target: b1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e2, target: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e3, target: b3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e4, target: b4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e5, target: b5, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e6, target: b6, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e7, target: b7, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e8, target: b8, weight: 1.0 }).unwrap();
        // symmetric weights
        c.add(IgpLinkWeight { target: b1, source: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b1, source: r1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b2, source: r1, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { target: b3, source: r1, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { target: b3, source: t1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b3, source: b4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b4, source: r2, weight: 4.0 }).unwrap();
        c.add(IgpLinkWeight { target: b5, source: r2, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { target: b5, source: t2, weight: 10.0 }).unwrap();
        c.add(IgpLinkWeight { target: b5, source: b6, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b6, source: r3, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { target: b7, source: r3, weight: 16.0 }).unwrap();
        c.add(IgpLinkWeight { target: b7, source: b8, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b8, source: r4, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { target: r1, source: t1, weight: 12.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: t1, weight: 15.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: t2, weight: 18.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: r3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r3, source: t2, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { target: r3, source: r4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r4, source: t2, weight: 20.0 }).unwrap();
        c.add(IgpLinkWeight { target: t1, source: t2, weight: 80.0 }).unwrap();
        c.add(IgpLinkWeight { target: e1, source: b1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: e2, source: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: e3, source: b3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: e4, source: b4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: e5, source: b5, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: e6, source: b6, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: e7, source: b7, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: e8, source: b8, weight: 1.0 }).unwrap();

        // add ebgp sessions
        c.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b3, target: e3, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b4, target: e4, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b5, target: e5, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b6, target: e6, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b7, target: e7, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b8, target: e8, session_type: EBgp }).unwrap();

        // add iBGP full mesh between r1, r2, r3, r4, t1 and t2
        c.add(BgpSession { source: r1, target: r2, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r1, target: r3, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r1, target: r4, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r1, target: t1, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r1, target: t2, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r2, target: r3, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r2, target: r4, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r2, target: t1, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r2, target: t2, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r3, target: r4, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r3, target: t1, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r3, target: t2, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r4, target: t1, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: r4, target: t2, session_type: IBgpPeer }).unwrap();
        c.add(BgpSession { source: t1, target: t2, session_type: IBgpPeer }).unwrap();

        // add variant depending layering
        if variant == 0 {
            c.add(BgpSession { source: r1, target: b1, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r1, target: b2, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r1, target: b3, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: t1, target: b3, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r2, target: b4, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r2, target: b5, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: t2, target: b5, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r3, target: b6, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r3, target: b7, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r4, target: b8, session_type: IBgpClient }).unwrap();
        } else if variant == 1 {
            c.add(BgpSession { source: t2, target: b1, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r4, target: b2, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r3, target: b3, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r1, target: b4, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: t1, target: b5, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r4, target: b6, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: t1, target: b7, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r1, target: b8, session_type: IBgpClient }).unwrap();
        } else {
            panic!("Invalid variant number!")
        }

        c
    }

    /// # Variant 0: 2 Levels, reasonable, no IGP change
    /// - Weights according to the main documentation
    /// - The following BGP sessions are set
    ///   - e\[i\] --> b\[i\] (eBGP)
    ///   - b1 --> r1 (iBGP Client)
    ///   - b2 --> r1 (iBGP Client)
    ///   - b3 --> r1 (iBGP Client)
    ///   - b3 --> t1 (iBGP Client)
    ///   - b4 --> r2 (iBGP Client)
    ///   - b5 --> r2 (iBGP Client)
    ///   - b5 --> t2 (iBGP Client)
    ///   - b6 --> r3 (iBGP Client)
    ///   - b7 --> r3 (iBGP Client)
    ///   - b8 --> r4 (iBGP Client)
    ///   - r1 --> t1 (iBGP Client)
    ///   - r2 --> t1 (iBGP Client)
    ///   - r2 --> t2 (iBGP Client)
    ///   - r3 --> t2 (iBGP Client)
    ///   - r4 --> t2 (iBGP Client)
    ///   - t1 --- t2 (iBGP Peer)
    /// # Variant 1: 2 Levels, reasonable, IGP weight change
    /// - Weights according to the main documentation, except:
    ///   - t1 --- t2: 1
    ///   - t2 --- r3: 20
    ///   - t1 --- b3: 15
    /// - Same BGP sessions as variant 0
    /// # Variant 2: 2 Levels, unreasonable, no IGP change
    /// - Weights according to the main documentation
    /// - The following BGP sessions are set
    ///   - b1 --> t2 (iBGP Client)
    ///   - b2 --> r4 (iBGP Client)
    ///   - b3 --> r3 (iBGP Client)
    ///   - b4 --> r1 (iBGP Client)
    ///   - b5 --> t1 (iBGP Client)
    ///   - b6 --> r4 (iBGP Client)
    ///   - b7 --> t1 (iBGP Client)
    ///   - b8 --> r1 (iBGP Client)
    ///   - r1 --> t1 (iBGP Client)
    ///   - r2 --> t1 (iBGP Client)
    ///   - r2 --> t2 (iBGP Client)
    ///   - r3 --> t2 (iBGP Client)
    ///   - r4 --> t2 (iBGP Client)
    ///   - t1 --- t2 (iBGP Peer)
    /// # Variant 3: 2 Levels, unreasonable, IGP weight change
    /// - Weights according to the main documentation, except:
    ///   - t1 --- t2: 1
    ///   - t2 --- r3: 20
    ///   - t1 --- b3: 15
    /// - The following BGP sessions are set
    ///   - b1 --> t2 (iBGP Client)
    ///   - b2 --> r4 (iBGP Client)
    ///   - b3 --> r3 (iBGP Client)
    ///   - b4 --> r1 (iBGP Client)
    ///   - b5 --> t1 (iBGP Client)
    ///   - b6 --> r4 (iBGP Client)
    ///   - b7 --> t1 (iBGP Client)
    ///   - b8 --> r1 (iBGP Client)
    ///   - r1 --> t1 (iBGP Client)
    ///   - r2 --> t1 (iBGP Client)
    ///   - r2 --> t2 (iBGP Client)
    ///   - r3 --> t2 (iBGP Client)
    ///   - r4 --> t2 (iBGP Client)
    ///   - t1 --- t2 (iBGP Peer)
    fn final_config(net: &Network, variant: usize) -> Config {
        let mut c = Config::new();

        let b1 = net.get_router_id("b1").unwrap();
        let b2 = net.get_router_id("b2").unwrap();
        let b3 = net.get_router_id("b3").unwrap();
        let b4 = net.get_router_id("b4").unwrap();
        let b5 = net.get_router_id("b5").unwrap();
        let b6 = net.get_router_id("b6").unwrap();
        let b7 = net.get_router_id("b7").unwrap();
        let b8 = net.get_router_id("b8").unwrap();
        let r1 = net.get_router_id("r1").unwrap();
        let r2 = net.get_router_id("r2").unwrap();
        let r3 = net.get_router_id("r3").unwrap();
        let r4 = net.get_router_id("r4").unwrap();
        let t1 = net.get_router_id("t1").unwrap();
        let t2 = net.get_router_id("t2").unwrap();
        let e1 = net.get_router_id("e1").unwrap();
        let e2 = net.get_router_id("e2").unwrap();
        let e3 = net.get_router_id("e3").unwrap();
        let e4 = net.get_router_id("e4").unwrap();
        let e5 = net.get_router_id("e5").unwrap();
        let e6 = net.get_router_id("e6").unwrap();
        let e7 = net.get_router_id("e7").unwrap();
        let e8 = net.get_router_id("e8").unwrap();

        c.add(IgpLinkWeight { source: b1, target: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b1, target: r1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b2, target: r1, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { source: b3, target: r1, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { source: b3, target: b4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b4, target: r2, weight: 4.0 }).unwrap();
        c.add(IgpLinkWeight { source: b5, target: r2, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { source: b5, target: t2, weight: 10.0 }).unwrap();
        c.add(IgpLinkWeight { source: b5, target: b6, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b6, target: r3, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { source: b7, target: r3, weight: 16.0 }).unwrap();
        c.add(IgpLinkWeight { source: b7, target: b8, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: b8, target: r4, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { source: r1, target: t1, weight: 12.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: t1, weight: 15.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: t2, weight: 18.0 }).unwrap();
        c.add(IgpLinkWeight { source: r2, target: r3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r3, target: r4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: r4, target: t2, weight: 20.0 }).unwrap();
        c.add(IgpLinkWeight { source: e1, target: b1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e2, target: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e3, target: b3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e4, target: b4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e5, target: b5, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e6, target: b6, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e7, target: b7, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { source: e8, target: b8, weight: 1.0 }).unwrap();
        // symmetric weights
        c.add(IgpLinkWeight { target: b1, source: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b1, source: r1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b2, source: r1, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { target: b3, source: r1, weight: 2.0 }).unwrap();
        c.add(IgpLinkWeight { target: b3, source: b4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b4, source: r2, weight: 4.0 }).unwrap();
        c.add(IgpLinkWeight { target: b5, source: r2, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { target: b5, source: t2, weight: 10.0 }).unwrap();
        c.add(IgpLinkWeight { target: b5, source: b6, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b6, source: r3, weight: 8.0 }).unwrap();
        c.add(IgpLinkWeight { target: b7, source: r3, weight: 16.0 }).unwrap();
        c.add(IgpLinkWeight { target: b7, source: b8, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: b8, source: r4, weight: 3.0 }).unwrap();
        c.add(IgpLinkWeight { target: r1, source: t1, weight: 12.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: t1, weight: 15.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: t2, weight: 18.0 }).unwrap();
        c.add(IgpLinkWeight { target: r2, source: r3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r3, source: r4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: r4, source: t2, weight: 20.0 }).unwrap();
        c.add(IgpLinkWeight { target: e1, source: b1, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: e2, source: b2, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: e3, source: b3, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: e4, source: b4, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: e5, source: b5, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: e6, source: b6, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: e7, source: b7, weight: 1.0 }).unwrap();
        c.add(IgpLinkWeight { target: e8, source: b8, weight: 1.0 }).unwrap();

        if variant == 0 || variant == 2 {
            // unaltered weights
            c.add(IgpLinkWeight { source: t1, target: t2, weight: 80.0 }).unwrap();
            c.add(IgpLinkWeight { target: t1, source: t2, weight: 80.0 }).unwrap();
            c.add(IgpLinkWeight { source: r3, target: t2, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { target: r3, source: t2, weight: 3.0 }).unwrap();
            c.add(IgpLinkWeight { source: b3, target: t1, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: b3, source: t1, weight: 1.0 }).unwrap();
        } else if variant == 1 || variant == 3 {
            // also change weights
            c.add(IgpLinkWeight { source: t1, target: t2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { target: t1, source: t2, weight: 1.0 }).unwrap();
            c.add(IgpLinkWeight { source: r3, target: t2, weight: 20.0 }).unwrap();
            c.add(IgpLinkWeight { target: r3, source: t2, weight: 20.0 }).unwrap();
            c.add(IgpLinkWeight { source: b3, target: t1, weight: 15.0 }).unwrap();
            c.add(IgpLinkWeight { target: b3, source: t1, weight: 15.0 }).unwrap();
        }

        // add ebgp sessions
        c.add(BgpSession { source: b1, target: e1, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b2, target: e2, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b3, target: e3, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b4, target: e4, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b5, target: e5, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b6, target: e6, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b7, target: e7, session_type: EBgp }).unwrap();
        c.add(BgpSession { source: b8, target: e8, session_type: EBgp }).unwrap();

        // add higher level hierarchy
        c.add(BgpSession { source: t1, target: r1, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: t1, target: r2, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: t2, target: r2, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: t2, target: r3, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: t2, target: r4, session_type: IBgpClient }).unwrap();
        c.add(BgpSession { source: t1, target: t2, session_type: IBgpPeer }).unwrap();

        // add lower level hierarchy
        if variant == 0 || variant == 1 {
            // reasonable lower level hierarchy
            c.add(BgpSession { source: r1, target: b1, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r1, target: b2, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r1, target: b3, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: t1, target: b3, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r2, target: b4, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r2, target: b5, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: t2, target: b5, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r3, target: b6, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r3, target: b7, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r4, target: b8, session_type: IBgpClient }).unwrap();
        } else if variant == 2 || variant == 3 {
            // unreasonable lower level hierearchy
            c.add(BgpSession { source: t2, target: b1, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r4, target: b2, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r3, target: b3, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r1, target: b4, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: t1, target: b5, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r4, target: b6, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: t1, target: b7, session_type: IBgpClient }).unwrap();
            c.add(BgpSession { source: r1, target: b8, session_type: IBgpClient }).unwrap();
        }

        c
    }

    fn get_policy(net: &Network, _variant: usize) -> HardPolicy {
        HardPolicy::reachability(net.get_routers().iter(), net.get_known_prefixes().iter())
    }
}
