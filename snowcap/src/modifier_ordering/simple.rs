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

//! # Simple Ordering

use super::{CompleteOrdering, ModifierOrdering};
use crate::netsim::config::ConfigExpr::{
    self, BgpRouteMap, BgpSession, IgpLinkWeight, StaticRoute,
};
use crate::netsim::config::ConfigModifier::{self, Insert, Remove, Update};
use crate::netsim::BgpSessionType::*;
use crate::netsim::RouterId;
use std::cmp::Ordering;

/// #Simple Ordering
/// The following ordering is used:
/// - Modification type: Insert < Update < Remove
/// - Expression type: IgpLinkWeight < BgpSession < BgpLocalPref
/// - Values of each expression.
pub struct SimpleOrdering {}

impl ModifierOrdering<ConfigModifier> for SimpleOrdering {
    fn order(a: &ConfigModifier, b: &ConfigModifier) -> Ordering {
        match (a, b) {
            (Insert(ea), Insert(eb)) => order_expr(ea, eb),
            (Insert(_), Update { .. }) => Ordering::Less,
            (Insert(_), Remove(_)) => Ordering::Less,
            (Update { .. }, Insert(_)) => Ordering::Greater,
            (Update { to: ea, .. }, Update { to: eb, .. }) => order_expr(ea, eb),
            (Update { .. }, Remove(_)) => Ordering::Less,
            (Remove(_), Insert(_)) => Ordering::Greater,
            (Remove(_), Update { .. }) => Ordering::Greater,
            (Remove(ea), Remove(eb)) => order_expr(ea, eb),
        }
    }
}

fn order_expr(a: &ConfigExpr, b: &ConfigExpr) -> Ordering {
    match (a, b) {
        (
            StaticRoute { router: ra, prefix: pa, target: ta },
            StaticRoute { router: rb, prefix: pb, target: tb },
        ) => match pa.0.cmp(&pb.0) {
            Ordering::Equal => order_two_routers(ra, rb, ta, tb),
            o => o,
        },
        (StaticRoute { .. }, IgpLinkWeight { .. }) => Ordering::Less,
        (StaticRoute { .. }, BgpSession { .. }) => Ordering::Less,
        (StaticRoute { .. }, BgpRouteMap { .. }) => Ordering::Less,
        (IgpLinkWeight { .. }, StaticRoute { .. }) => Ordering::Greater,
        (
            IgpLinkWeight { source: sa, target: ta, weight: wa },
            IgpLinkWeight { source: sb, target: tb, weight: wb },
        ) => match wa.partial_cmp(wb) {
            Some(Ordering::Equal) | None => order_two_routers(sa, sb, ta, tb),
            Some(o) => o,
        },
        (IgpLinkWeight { .. }, BgpSession { .. }) => Ordering::Less,
        (IgpLinkWeight { .. }, BgpRouteMap { .. }) => Ordering::Less,
        (BgpSession { .. }, StaticRoute { .. }) => Ordering::Greater,
        (BgpSession { .. }, IgpLinkWeight { .. }) => Ordering::Greater,
        (
            BgpSession { source: sa, target: ta, session_type: xa },
            BgpSession { source: sb, target: tb, session_type: xb },
        ) => match (xa, xb) {
            (EBgp, EBgp) => order_two_routers(sa, sb, ta, tb),
            (EBgp, IBgpClient) => Ordering::Less,
            (EBgp, IBgpPeer) => Ordering::Less,
            (IBgpClient, EBgp) => Ordering::Greater,
            (IBgpClient, IBgpClient) => order_two_routers(sa, sb, ta, tb),
            (IBgpClient, IBgpPeer) => Ordering::Less,
            (IBgpPeer, EBgp) => Ordering::Greater,
            (IBgpPeer, IBgpClient) => Ordering::Greater,
            (IBgpPeer, IBgpPeer) => order_two_routers(sa, sb, ta, tb),
        },
        (BgpSession { .. }, BgpRouteMap { .. }) => Ordering::Less,
        (BgpRouteMap { .. }, StaticRoute { .. }) => Ordering::Greater,
        (BgpRouteMap { .. }, IgpLinkWeight { .. }) => Ordering::Greater,
        (BgpRouteMap { .. }, BgpSession { .. }) => Ordering::Greater,
        (
            BgpRouteMap { router: ra, direction: _, map: ma },
            BgpRouteMap { router: rb, direction: _, map: mb },
        ) => match ra.cmp(rb) {
            Ordering::Equal => ma.order().cmp(&mb.order),
            o => o,
        },
    }
}

impl CompleteOrdering for SimpleOrdering {}

fn order_two_routers(a1: &RouterId, b1: &RouterId, a2: &RouterId, b2: &RouterId) -> Ordering {
    if a1 < b1 {
        Ordering::Less
    } else if a1 > b1 {
        Ordering::Greater
    } else if a2 < b2 {
        Ordering::Less
    } else if a2 > b2 {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_sort() {
        let p1 = Insert(BgpSession { source: 1.into(), target: 1.into(), session_type: EBgp });
        let p2 = Insert(BgpSession { source: 2.into(), target: 2.into(), session_type: EBgp });
        let p3 = Insert(BgpSession { source: 3.into(), target: 3.into(), session_type: EBgp });
        let mut data = vec![p3.clone(), p2.clone(), p1.clone()];
        SimpleOrdering::sort(&mut data);
        assert_eq![data, vec![p1, p2, p3]];
    }
}
