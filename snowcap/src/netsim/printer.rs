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

//! # Helper (printer) functions for the Network
//! Module containing helper functions to get formatted strings and print information about the
//! network.

use crate::netsim::bgp::{BgpEvent, BgpRibEntry, BgpRoute};
use crate::netsim::config::{Config, ConfigExpr, ConfigModifier, ConfigPatch};
use crate::netsim::event::Event;
use crate::netsim::network::Network;
use crate::netsim::route_map::*;
use crate::netsim::router::Router;
use crate::netsim::{BgpSessionType, NetworkError, Prefix};

/// Get a vector of strings, which represent the bgp table. Each `String` in the vector represents
/// one line (one known route). The strings are formatted, and the names of the routers are
/// inserted. The chosen route is prefixed with a `*`, while the known but not chosen routes are
/// prefixed with nothing (` `).
pub fn bgp_table(
    net: &Network,
    router: &Router,
    prefix: Prefix,
) -> Result<Vec<String>, NetworkError> {
    let mut result: Vec<String> = Vec::new();
    let selected_entry = router.get_selected_bgp_route(prefix);
    let mut found = false;
    for entry in router.get_known_bgp_routes(prefix)? {
        let mut entry_str = if selected_entry.as_ref() == Some(&entry) {
            found = true;
            String::from("* ")
        } else {
            String::from("  ")
        };
        entry_str.push_str(&bgp_entry(net, &entry)?);
        result.push(entry_str);
    }
    if selected_entry.is_some() && !found {
        Err(NetworkError::InvalidBgpTable(router.router_id()))
    } else {
        Ok(result)
    }
}

/// Returns the formatted string for a given RIBEntry of a BGP route. The entry is formatted such
/// that all router names are inserted.
pub fn bgp_entry(net: &Network, entry: &BgpRibEntry) -> Result<String, NetworkError> {
    Ok(format!(
        "prefix: {p}, as_path: {path:?}, local_pref: {lp}, MED: {med}, IGP Cost: {cost}, next_hop: {nh}, from: {next}",
        p = entry.route.prefix.0,
        path = entry.route.as_path.iter().map(|x| x.0).collect::<Vec<u32>>(),
        lp = entry.route.local_pref.unwrap_or(100),
        med = entry.route.med.unwrap_or(0),
        cost = entry.igp_cost.unwrap_or(0.0),
        nh = net.get_router_name(entry.route.next_hop)?,
        next = net.get_router_name(entry.from_id)?,
    ))
}

/// Returns a formatted string for a given BGP route.
pub fn bgp_route(net: &Network, route: &BgpRoute) -> Result<String, NetworkError> {
    let mut result = format!(
        "prefix: {}, AsPath: {:?}, next hop: {}",
        route.prefix.0,
        route.as_path.iter().map(|x| x.0).collect::<Vec<_>>(),
        net.get_router_name(route.next_hop)?
    );
    if let Some(local_pref) = route.local_pref {
        result.push_str(&format!(", local pref: {}", local_pref))
    }
    if let Some(med) = route.med {
        result.push_str(&format!(", MED: {}", med))
    }
    if let Some(community) = route.community {
        result.push_str(&format!(", community: {}", community))
    }
    Ok(result)
}

/// Return a formatted string for a given event
pub fn event(net: &Network, event: &Event) -> Result<String, NetworkError> {
    Ok(match event {
        Event::Bgp(from, to, BgpEvent::Update(route)) => format!(
            "BGP Event: {} -> {}: Update [{}]",
            net.get_router_name(*from)?,
            net.get_router_name(*to)?,
            bgp_route(net, route)?
        ),
        Event::Bgp(from, to, BgpEvent::Withdraw(prefix)) => format!(
            "BGP Event: {} -> {}: Withdraw prefix {}",
            net.get_router_name(*from)?,
            net.get_router_name(*to)?,
            prefix.0
        ),
        Event::Config(modifier) => format!("Apply Config: {}", config_modifier(net, modifier)?,),
        Event::AdvertiseExternalRoute(r, route) => {
            format!("{} advertisees route [{}]", net.get_router_name(*r)?, bgp_route(net, route)?)
        }
        Event::WithdrawExternalRoute(r, prefix) => {
            format!("{} withdraws route for prefix {}", net.get_router_name(*r)?, prefix.0)
        }
    })
}

/// Print the bgp table of a given router.
pub fn print_bgp_table(net: &Network, router: &Router, prefix: Prefix) -> Result<(), NetworkError> {
    println!("BGP table of {} for {:?}", router.name(), prefix);
    let table = bgp_table(net, router, prefix)?;
    for entry in table {
        println!("{}", entry);
    }
    Ok(())
}

/// Returns the config expr as a string, where all router names are inserted.
pub fn config_expr(net: &Network, expr: &ConfigExpr) -> Result<String, NetworkError> {
    Ok(match expr {
        ConfigExpr::IgpLinkWeight { source, target, weight } => format!(
            "IGP Link Weight: {} -> {}: {}",
            net.get_router_name(*source)?,
            net.get_router_name(*target)?,
            weight
        ),
        ConfigExpr::BgpSession { source, target, session_type } => format!(
            "BGP Session: {} -> {}: type: {}",
            net.get_router_name(*source)?,
            net.get_router_name(*target)?,
            match session_type {
                BgpSessionType::EBgp => "eBGP",
                BgpSessionType::IBgpClient => "iBGP Client",
                BgpSessionType::IBgpPeer => "iBGP Peer",
            }
        ),
        ConfigExpr::BgpRouteMap { router, direction, map } => format!(
            "BGP Route Map on {} [{}]: {}",
            net.get_router_name(*router)?,
            match direction {
                RouteMapDirection::Incoming => "in",
                RouteMapDirection::Outgoing => "out",
            },
            route_map(net, map)?
        ),
        ConfigExpr::StaticRoute { router, prefix, target } => format!(
            "Static Route: {}: Prefix {} via {}",
            net.get_router_name(*router)?,
            prefix.0,
            net.get_router_name(*target)?,
        ),
    })
}

/// Returns a formatted string for the given modifier, where all router names are inserted.
pub fn config_modifier(net: &Network, modifier: &ConfigModifier) -> Result<String, NetworkError> {
    Ok(match modifier {
        ConfigModifier::Insert(e) => format!("INSERT {}", config_expr(net, e)?),
        ConfigModifier::Remove(e) => format!("REMOVE {}", config_expr(net, e)?),
        ConfigModifier::Update { from: _, to } => format!("MODIFY {}", config_expr(net, to)?),
    })
}

/// Returns a formatted string of the route map, where all router names are inserted
pub fn route_map(net: &Network, map: &RouteMap) -> Result<String, NetworkError> {
    Ok(format!(
        "{} {} {} set [{}]",
        match map.state {
            RouteMapState::Allow => "allow",
            RouteMapState::Deny => "deny ",
        },
        map.order,
        if map.conds.is_empty() {
            String::from("*")
        } else {
            map.conds
                .iter()
                .map(|c| route_map_match(net, c).unwrap())
                .collect::<Vec<_>>()
                .join(" AND ")
        },
        map.set.iter().map(|s| route_map_set(net, s).unwrap()).collect::<Vec<_>>().join(", ")
    ))
}

/// Print the complete configuration to stdout
pub fn print_config(net: &Network, config: &Config) -> Result<(), NetworkError> {
    println!("Config {{");
    for expr in config.expr.values() {
        println!("    {}", config_expr(net, expr)?);
    }
    println!("}}");
    Ok(())
}

/// Print the configuration patch to stdout
pub fn print_config_patch(net: &Network, patch: &ConfigPatch) -> Result<(), NetworkError> {
    println!("ConfigPatch {{");
    for modifier in patch.modifiers.iter() {
        println!("    {}", config_modifier(net, modifier)?);
    }
    println!("}}");
    Ok(())
}

fn route_map_match(net: &Network, map_match: &RouteMapMatch) -> Result<String, NetworkError> {
    Ok(match map_match {
        RouteMapMatch::Neighbor(n) => format!("Neighbor {}", net.get_router_name(*n)?),
        RouteMapMatch::Prefix(c) => format!("Prefix == {}", c),
        RouteMapMatch::AsPath(c) => format!("{}", c),
        RouteMapMatch::NextHop(nh) => format!("NextHop == {}", net.get_router_name(*nh)?),
        RouteMapMatch::Community(Some(c)) => format!("Community {}", c),
        RouteMapMatch::Community(None) => "Community empty".to_string(),
    })
}

fn route_map_set(net: &Network, map_set: &RouteMapSet) -> Result<String, NetworkError> {
    Ok(match map_set {
        RouteMapSet::NextHop(nh) => format!("NextHop = {}", net.get_router_name(*nh)?),
        RouteMapSet::LocalPref(Some(lp)) => format!("LocalPref = {}", lp),
        RouteMapSet::LocalPref(None) => "clear LocalPref".to_string(),
        RouteMapSet::Med(Some(med)) => format!("MED = {}", med),
        RouteMapSet::Med(None) => "clear MED".to_string(),
        RouteMapSet::IgpCost(w) => format!("IgpCost = {:.2}", w),
        RouteMapSet::Community(Some(c)) => format!("Community = {}", c),
        RouteMapSet::Community(None) => "clear Community".to_string(),
    })
}
