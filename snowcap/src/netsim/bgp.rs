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

//! Module containing definitions for BGP

use crate::netsim::{AsId, LinkWeight, Prefix, RouterId};
use std::cmp::Ordering;

/// Bgp Route
/// The following attributes are omitted
/// - ORIGIN: assumed to be always set to IGP
/// - ATOMIC_AGGREGATE: not used
/// - AGGREGATOR: not used
#[derive(Debug, Clone, Eq)]
pub struct BgpRoute {
    /// IP PREFIX (represented as a simple number)
    pub prefix: Prefix,
    /// AS-PATH, where the origin of the route is last, and the ID of a new AS is prepended.
    pub as_path: Vec<AsId>,
    /// NEXT-HOP for reaching the source of the route.
    pub next_hop: RouterId,
    /// LOCAL-PREF
    pub local_pref: Option<u32>,
    /// MED (Multi-Exit Discriminator)
    pub med: Option<u32>,
    /// Community
    pub community: Option<u32>,
}

impl BgpRoute {
    /// Applies the default values for any non-mandatory field
    #[allow(dead_code)]
    pub(crate) fn apply_default(&mut self) {
        self.local_pref = Some(self.local_pref.unwrap_or(100));
        self.med = Some(self.med.unwrap_or(0));
    }

    /// returns a clone of self, with the default values applied for any non-mandatory field.
    pub(crate) fn clone_default(&self) -> Self {
        Self {
            prefix: self.prefix,
            as_path: self.as_path.clone(),
            next_hop: self.next_hop,
            local_pref: Some(self.local_pref.unwrap_or(100)),
            med: Some(self.med.unwrap_or(0)),
            community: Some(self.community.unwrap_or(0)),
        }
    }
}

impl PartialEq for BgpRoute {
    fn eq(&self, other: &Self) -> bool {
        let s = self.clone_default();
        let o = other.clone_default();
        s.prefix == o.prefix
            && s.as_path == other.as_path
            && s.next_hop == o.next_hop
            && s.local_pref == o.local_pref
            && s.med == o.med
            && s.community == o.community
    }
}

impl std::hash::Hash for BgpRoute {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let s = self.clone_default();
        s.prefix.hash(state);
        s.as_path.hash(state);
        s.next_hop.hash(state);
        s.local_pref.hash(state);
        s.med.hash(state);
        s.community.hash(state);
    }
}

/// Type of a BGP session
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BgpSessionType {
    /// iBGP session with a peer (or from a coient with a Route Reflector)
    IBgpPeer,
    /// iBGP session from a Route Reflector with a client
    IBgpClient,
    /// eBGP session
    EBgp,
}

impl BgpSessionType {
    /// returns true if the session type is EBgp
    pub fn is_ebgp(&self) -> bool {
        matches!(self, Self::EBgp)
    }

    /// returns true if the session type is IBgp
    pub fn is_ibgp(&self) -> bool {
        !self.is_ebgp()
    }
}

/// BGP Events
#[derive(Debug, Clone, PartialEq)]
pub enum BgpEvent {
    /// Withdraw a previously advertised route
    Withdraw(Prefix),
    /// Update a route, or add a new one.
    Update(BgpRoute),
}

impl BgpEvent {
    /// Returns the prefix for which this event is responsible
    pub fn prefix(&self) -> Prefix {
        match self {
            Self::Withdraw(p) => *p,
            Self::Update(r) => r.prefix,
        }
    }
}

/// BGP RIB Table entry
#[derive(Debug, Clone)]
pub struct BgpRibEntry {
    /// the actual bgp route
    pub(crate) route: BgpRoute,
    /// the type of session, from which the route was learned
    pub(crate) from_type: BgpSessionType,
    /// the client from which the route was learned
    pub(crate) from_id: RouterId,
    /// the client to which the route is distributed (only in RibOut)
    pub(crate) to_id: Option<RouterId>,
    /// the igp cost to the next_hop
    pub(crate) igp_cost: Option<LinkWeight>,
}

impl PartialEq for BgpRibEntry {
    fn eq(&self, other: &Self) -> bool {
        self.route == other.route && self.from_id == other.from_id
    }
}

impl PartialOrd for BgpRibEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let s = self.route.clone_default();
        let o = other.route.clone_default();

        match s.local_pref.unwrap().cmp(&o.local_pref.unwrap()) {
            Ordering::Equal => {}
            o => return Some(o),
        }

        match s.as_path.len().cmp(&o.as_path.len()) {
            Ordering::Equal => {}
            Ordering::Greater => return Some(Ordering::Less),
            Ordering::Less => return Some(Ordering::Greater),
        }

        match s.med.unwrap().cmp(&o.med.unwrap()) {
            Ordering::Equal => {}
            Ordering::Greater => return Some(Ordering::Less),
            Ordering::Less => return Some(Ordering::Greater),
        }

        if self.from_type.is_ebgp() && other.from_type.is_ibgp() {
            return Some(Ordering::Greater);
        } else if self.from_type.is_ibgp() && other.from_type.is_ebgp() {
            return Some(Ordering::Less);
        }

        match self.igp_cost.unwrap().partial_cmp(&other.igp_cost.unwrap()) {
            Some(Ordering::Equal) | None => {}
            Some(Ordering::Greater) => return Some(Ordering::Less),
            Some(Ordering::Less) => return Some(Ordering::Greater),
        }

        match s.next_hop.cmp(&o.next_hop) {
            Ordering::Equal => {}
            Ordering::Greater => return Some(Ordering::Less),
            Ordering::Less => return Some(Ordering::Greater),
        }

        match self.from_id.cmp(&other.from_id) {
            Ordering::Equal => Some(Ordering::Equal),
            Ordering::Greater => Some(Ordering::Less),
            Ordering::Less => Some(Ordering::Greater),
        }
    }
}
