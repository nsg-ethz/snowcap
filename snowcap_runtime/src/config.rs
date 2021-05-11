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

//! This module is responsible for parsing the config correctly

use crate::physical_network::*;
use snowcap::netsim::config::{
    Config,
    ConfigExpr::{BgpRouteMap, BgpSession, IgpLinkWeight, StaticRoute},
    ConfigModifier::{self, Insert, Remove, Update},
};
use snowcap::netsim::route_map::RouteMapDirection;
use snowcap::netsim::{BgpSessionType, RouterId};

/// Apply an entire configuration on the physical network. This funciton generates no commands to be
/// executed. If this is needed, use [`parse_modifier`].
pub fn apply_config(phys_net: &mut PhysicalNetwork, config: &Config) {
    // parse configuration, and update the physical network
    for expr in config.iter() {
        parse_modifier(phys_net, &Insert(expr.clone()));
    }
}

/// Apply the modifier to the settings in physnet, and generate a set of commands that can be
/// executed
pub fn parse_modifier(
    phys_net: &mut PhysicalNetwork,
    modifier: &ConfigModifier,
) -> Vec<(RouterId, Vec<String>)> {
    match modifier {
        // create a new bgp session!
        Insert(BgpSession { source, target, session_type }) => {
            // get the source and target address and as
            let (source_addr, target_addr) =
                get_bgp_peering_addr(phys_net, *source, *target, *session_type);
            let (source_as, target_as) = (
                phys_net.routers[source.index()].as_id.clone(),
                phys_net.routers[target.index()].as_id.clone(),
            );

            // write into the datastructure
            phys_net.routers[source.index()].bgp_sessions.push(BgpSessionInfo {
                neighbor: *target,
                neighbor_addr: target_addr.clone(),
                neighbor_as_id: target_as,
                is_rr_client: *session_type == BgpSessionType::IBgpClient,
                internal_session: session_type.is_ibgp(),
            });
            phys_net.routers[target.index()].bgp_sessions.push(BgpSessionInfo {
                neighbor: *source,
                neighbor_addr: source_addr.clone(),
                neighbor_as_id: source_as,
                is_rr_client: false,
                internal_session: session_type.is_ibgp(),
            });

            let (iface_source, iface_target, peer_group) = if session_type.is_ibgp() {
                ("lo".to_string(), "lo".to_string(), "internal")
            } else {
                (
                    get_interface_from_to(phys_net, *source, *target),
                    get_interface_from_to(phys_net, *target, *source),
                    "external",
                )
            };

            vec![
                (
                    *source,
                    if *session_type == BgpSessionType::IBgpClient {
                        vec![
                            format!("router bgp {}", source_as.0),
                            format!("neighbor {} remote-as {}", target_addr.addr, target_as.0),
                            format!("neighbor {} update-source {}", target_addr.addr, iface_source),
                            format!("neighbor {} peer-group {}", target_addr.addr, peer_group),
                            format!("neighbor {} route-reflector-client", target_addr.addr),
                        ]
                    } else {
                        vec![
                            format!("router bgp {}", source_as.0),
                            format!("neighbor {} remote-as {}", target_addr.addr, target_as.0),
                            format!("neighbor {} update-source {}", target_addr.addr, iface_source),
                            format!("neighbor {} peer-group {}", target_addr.addr, peer_group),
                        ]
                    },
                ),
                (
                    *target,
                    vec![
                        format!("router bgp {}", target_as.0),
                        format!("neighbor {} remote-as {}", source_addr.addr, source_as.0),
                        format!("neighbor {} update-source {}", source_addr.addr, iface_target),
                        format!("neighbor {} peer-group {}", source_addr.addr, peer_group),
                    ],
                ),
            ]
        }

        // enable the interface and set the cost if necessary, but only if the cost is not infinity.
        Insert(IgpLinkWeight { source, target, weight }) => {
            let link_idx = get_iface_idx(phys_net, *source, *target);
            let iface_name = get_interface_from_to(phys_net, *source, *target);

            // check if the weight is set. If not, then consider the interface as disabled
            if weight.is_infinite() {
                // assert that the interface was already disabled!
                assert!(!phys_net.routers[source.index()].ifaces[link_idx].enabled);
                vec![]
            } else {
                // enable the interface
                phys_net.routers[source.index()].ifaces[link_idx].enabled = true;

                // get the address
                let iface_addr =
                    phys_net.routers[source.index()].ifaces[link_idx].iface_addr.clone();

                // check if both routers are internal, in which case we set the cost
                if phys_net.routers[source.index()].is_internal
                    && phys_net.routers[target.index()].is_internal
                {
                    // set the cost
                    phys_net.routers[source.index()].ifaces[link_idx].cost =
                        Some(weight.round() as u32);
                    vec![(
                        *source,
                        vec![
                            format!("interface {}", iface_name),
                            format!("ip address {}", iface_addr),
                            format!("ip ospf 1 area 0"),
                            format!("ip ospf cost {}", weight.round() as u32),
                        ],
                    )]
                } else {
                    // enable the interface wihtout setting the cost
                    phys_net.routers[source.index()].ifaces[link_idx].cost = None;
                    vec![(
                        *source,
                        vec![
                            format!("interface {}", iface_name),
                            format!("ip address {}", iface_addr),
                        ],
                    )]
                }
            }
        }

        // Create a new route map
        Insert(BgpRouteMap { router, direction, map }) => {
            // create the new route map info and add it to the datastructure
            let rm_name = get_route_map_name(phys_net, *router);
            let rm =
                RouteMapInfo::from_route_map(rm_name, *router, *direction, map, &phys_net.routers);
            phys_net.routers[router.index()].route_maps.push(rm.clone());

            // create the updates
            let mut cmds = vec![format!("route-map {} {} {}", rm.name, rm.state, rm.order)];
            for (key, value) in rm.match_statements {
                cmds.push(format!("match {} {}", key, value));
            }
            for (key, value) in rm.set_statements {
                cmds.push(format!("set {} {}", key, value));
            }
            cmds.push(format!("exit"));

            // also create the update for enabling them in the bgp configuration
            cmds.push(format!("router bgp {}", phys_net.routers[router.index()].as_id.0));
            cmds.push(format!("address-family ipv4"));
            cmds.push(format!("neighbor internal route-map {} {}", rm.name, rm.direction));
            cmds.push(format!("neighbor external route-map {} {}", rm.name, rm.direction));
            cmds.push(format!("exit"));
            cmds.push(format!("exit"));

            vec![(*router, cmds)]
        }

        // insert the static route
        Insert(StaticRoute { router, prefix, target }) => {
            let link_idx = get_iface_idx(phys_net, *router, *target);
            let next_hop_addr =
                phys_net.routers[router.index()].ifaces[link_idx].neighbor_addr.addr.clone();
            let addr = phys_net.routers[phys_net.prefix_router_id(*prefix).index()]
                .advertise_route
                .as_ref()
                .unwrap()
                .clone();
            phys_net.routers[router.index()]
                .static_routes
                .push(StaticRouteInfo { addr: addr.clone(), next_hop: next_hop_addr.clone() });
            vec![(*router, vec![format!("ip route {} {}", addr, next_hop_addr)])]
        }

        // remove the existing bgp session!
        Remove(BgpSession { source, target, .. }) => {
            let source_idx = phys_net.routers[source.index()]
                .bgp_sessions
                .iter()
                .position(|s| s.neighbor == *target)
                .expect("Session does not already exist!");
            let target_idx = phys_net.routers[target.index()]
                .bgp_sessions
                .iter()
                .position(|s| s.neighbor == *source)
                .expect("Session does not already exist!");

            let ss = phys_net.routers[source.index()].bgp_sessions.remove(source_idx);
            let ts = phys_net.routers[target.index()].bgp_sessions.remove(target_idx);

            vec![
                (
                    *source,
                    vec![
                        format!("router bgp {}", ts.neighbor_as_id.0),
                        format!(
                            "no neighbor {} remote-as {}",
                            ss.neighbor_addr.addr, ss.neighbor_as_id.0
                        ),
                    ],
                ),
                (
                    *source,
                    vec![
                        format!("router bgp {}", ss.neighbor_as_id.0),
                        format!(
                            "no neighbor {} remote-as {}",
                            ts.neighbor_addr.addr, ts.neighbor_as_id.0
                        ),
                    ],
                ),
            ]
        }

        // remove the interface and the ospf cost, but only if the interface is enabled!
        Remove(IgpLinkWeight { source, target, .. }) => {
            let link_idx = get_iface_idx(phys_net, *source, *target);
            let iface_name = get_interface_from_to(phys_net, *source, *target);

            // check if the weight is set. If not, then consider the interface as disabled
            if phys_net.routers[source.index()].ifaces[link_idx].enabled {
                let old_cost = phys_net.routers[source.index()].ifaces[link_idx].cost;
                let old_addr = phys_net.routers[source.index()].ifaces[link_idx].iface_addr.clone();
                phys_net.routers[source.index()].ifaces[link_idx].enabled = false;
                phys_net.routers[source.index()].ifaces[link_idx].cost = None;

                if let Some(old_cost) = old_cost {
                    vec![(
                        *source,
                        vec![
                            format!("interface {}", iface_name),
                            format!("no ip ospf cost {}", old_cost),
                            format!("no ip ospf 1 area 0"),
                            format!("no ip address {}", old_addr),
                        ],
                    )]
                } else {
                    vec![(
                        *source,
                        vec![
                            format!("interface {}", iface_name),
                            format!("no ip address {}", old_addr),
                        ],
                    )]
                }
            } else {
                // nothing to do, interface is already disabled
                vec![]
            }
        }

        // remove a route map
        Remove(BgpRouteMap { router, direction, map }) => {
            // get the route map with the same order and direction
            let direction = match direction {
                RouteMapDirection::Incoming => "in",
                RouteMapDirection::Outgoing => "out",
            };
            let rm_pos = phys_net.routers[router.index()]
                .route_maps
                .iter()
                .position(|rm| rm.direction == direction && rm.order == map.order() as u32)
                .expect("RouteMap does not exist!");
            // remove the route map
            let rm = phys_net.routers[router.index()].route_maps.remove(rm_pos);

            // create the updates
            let mut cmds = Vec::with_capacity(7);
            // first disable the route map in bgp
            cmds.push(format!("router bgp {}", phys_net.routers[router.index()].as_id.0));
            cmds.push(format!("address-family ipv4"));
            cmds.push(format!("no neighbor internal route-map {} {}", rm.name, rm.direction));
            cmds.push(format!("no neighbor external route-map {} {}", rm.name, rm.direction));
            cmds.push(format!("exit"));
            cmds.push(format!("exit"));

            // then, delete the route map
            cmds.push(format!("no route-map {} {} {}", rm.name, rm.state, rm.order));

            vec![(*router, cmds)]
        }

        // remove the static route of the prefix, no matter where it points.
        Remove(StaticRoute { router, prefix, .. }) => {
            let addr = phys_net.routers[phys_net.prefix_router_id(*prefix).index()]
                .advertise_route
                .as_ref()
                .unwrap();
            // search this entry and remove it from the static routes
            let pos = phys_net.routers[router.index()]
                .static_routes
                .iter()
                .position(|sr| sr.addr == *addr)
                .expect("Static route to remove does not exist!");
            let old_sr = phys_net.routers[router.index()].static_routes.remove(pos);
            vec![(*router, vec![format!("no ip route {} {}", old_sr.addr, old_sr.next_hop)])]
        }

        // Here, the session can either change from RR->Source to Peer<->Peer, or viceversa. We just
        // check this here!
        Update { from: BgpSession { .. }, to: BgpSession { source, target, session_type } } => {
            // only internal sessions can be updated
            assert!(phys_net.routers[source.index()].is_internal);
            assert!(phys_net.routers[target.index()].is_internal);

            let source_idx = phys_net.routers[source.index()]
                .bgp_sessions
                .iter()
                .position(|s| s.neighbor == *target)
                .expect("Session does not already exist!");
            let target_idx = phys_net.routers[target.index()]
                .bgp_sessions
                .iter()
                .position(|s| s.neighbor == *source)
                .expect("Session does not already exist!");

            let ss = phys_net.routers[source.index()].bgp_sessions[source_idx].clone();
            let ts = phys_net.routers[target.index()].bgp_sessions[target_idx].clone();

            if *session_type == BgpSessionType::IBgpClient {
                // update from peer to client
                phys_net.routers[source.index()].bgp_sessions[source_idx].is_rr_client = true;
                vec![(
                    *source,
                    vec![
                        format!("router bgp {}", ts.neighbor_as_id.0),
                        format!("neighbor {} route-reflector-client", ss.neighbor_addr.addr),
                    ],
                )]
            } else if ss.is_rr_client == true {
                phys_net.routers[source.index()].bgp_sessions[source_idx].is_rr_client = false;
                vec![(
                    *source,
                    vec![
                        format!("router bgp {}", ts.neighbor_as_id.0),
                        format!("no neighbor {} route-reflector-client", ss.neighbor_addr.addr),
                    ],
                )]
            } else {
                phys_net.routers[target.index()].bgp_sessions[target_idx].is_rr_client = false;
                vec![(
                    *source,
                    vec![
                        format!("router bgp {}", ss.neighbor_as_id.0),
                        format!("no neighbor {} route-reflector-client", ts.neighbor_addr.addr),
                    ],
                )]
            }
        }

        // Change the link weight. For this, source and target must be the same. If the link was
        // disabled previously, then just generate it!
        Update { from: IgpLinkWeight { source, target, .. }, to: IgpLinkWeight { weight, .. } } => {
            let link_idx = get_iface_idx(phys_net, *source, *target);

            // check if the link is already enabled
            if phys_net.routers[source.index()].ifaces[link_idx].enabled {
                // if the new weight is infinite, then just remove the old weight
                if weight.is_infinite() {
                    // just remove the old weight
                    parse_modifier(
                        phys_net,
                        &Remove(IgpLinkWeight {
                            source: *source,
                            target: *target,
                            weight: *weight,
                        }),
                    )
                } else {
                    // ospf weight needs to change. But first, check if both routers are internal.
                    // If not, then this operation is not permitted
                    if !(phys_net.routers[source.index()].is_internal
                        && phys_net.routers[target.index()].is_internal)
                    {
                        panic!("Link weight can only be changed in between internal routers");
                    }

                    // update the interface
                    let iface_name = get_interface_from_to(phys_net, *source, *target);
                    phys_net.routers[source.index()].ifaces[link_idx].cost =
                        Some(weight.round() as u32);
                    vec![(
                        *source,
                        vec![
                            format!("interface {}", iface_name),
                            format!("ip ospf cost {}", weight.round() as u32),
                        ],
                    )]
                }
            } else {
                // link is not enabled! This is the same as inserting a new modifier with the new
                // weight
                parse_modifier(
                    phys_net,
                    &Insert(IgpLinkWeight { source: *source, target: *target, weight: *weight }),
                )
            }
        }
        // router, direction and map.order() must be the same
        Update { from: BgpRouteMap { router, direction, .. }, to: BgpRouteMap { map, .. } } => {
            // get the route map with the same order and direction
            let dir = match direction {
                RouteMapDirection::Incoming => "in",
                RouteMapDirection::Outgoing => "out",
            };
            let rm_pos = phys_net.routers[router.index()]
                .route_maps
                .iter()
                .position(|rm| rm.direction == dir && rm.order == map.order() as u32)
                .expect("RouteMap does not exist!");
            // remove the route map
            let old_rm = phys_net.routers[router.index()].route_maps.remove(rm_pos);

            // create the new route map with the same name as the old one.
            let new_rm = RouteMapInfo::from_route_map(
                old_rm.name.clone(),
                *router,
                *direction,
                map,
                &phys_net.routers,
            );
            // add the new rm to the datastructure
            phys_net.routers[router.index()].route_maps.push(new_rm.clone());

            // create the updates
            let mut cmds =
                vec![format!("route-map {} {} {}", new_rm.name, new_rm.state, new_rm.order)];

            // modify the match statements

            // update existing entries and add new entries
            for (key, new_val) in new_rm.match_statements.iter() {
                if let Some(old_val) = old_rm.match_statements.get(key) {
                    // either unmodified, or it changes
                    if new_val == old_val {
                        // nothing to do
                    } else {
                        cmds.push(format!("no match {} {}", key, old_val));
                        cmds.push(format!("match {} {}", key, new_val));
                    }
                } else {
                    cmds.push(format!("match {} {}", key, new_val));
                }
            }

            // delete old entries
            for (key, old_val) in old_rm.match_statements.iter() {
                if !new_rm.match_statements.contains_key(key) {
                    cmds.push(format!("no match {} {}", key, old_val));
                }
            }

            // do the same for set statements

            // update existing entries and add new entries
            for (key, new_val) in new_rm.set_statements.iter() {
                if let Some(old_val) = old_rm.set_statements.get(key) {
                    // either unmodified, or it changes
                    if new_val == old_val {
                        // nothing to do
                    } else {
                        cmds.push(format!("no set {} {}", key, old_val));
                        cmds.push(format!("set {} {}", key, new_val));
                    }
                } else {
                    cmds.push(format!("set {} {}", key, new_val));
                }
            }

            // delete old entries
            for (key, old_val) in old_rm.set_statements.iter() {
                if !new_rm.set_statements.contains_key(key) {
                    cmds.push(format!("no set {} {}", key, old_val));
                }
            }

            vec![(*router, cmds)]
        }

        // Change the static route to a different location. For this, the router and the prefix must
        // be the same
        Update { from: StaticRoute { router, prefix, .. }, to: StaticRoute { target, .. } } => {
            let link_idx = get_iface_idx(phys_net, *router, *target);
            let new_next_hop_addr =
                phys_net.routers[router.index()].ifaces[link_idx].neighbor_addr.addr.clone();
            let addr = phys_net.routers[phys_net.prefix_router_id(*prefix).index()]
                .advertise_route
                .as_ref()
                .unwrap()
                .clone();
            // search this entry and remove it from the static routes
            let pos = phys_net.routers[router.index()]
                .static_routes
                .iter()
                .position(|sr| sr.addr == addr)
                .expect("Static route to remove does not exist!");
            let old_sr = phys_net.routers[router.index()].static_routes.remove(pos);
            phys_net.routers[router.index()]
                .static_routes
                .push(StaticRouteInfo { addr: addr.clone(), next_hop: new_next_hop_addr.clone() });
            vec![(
                *router,
                vec![
                    format!("ip route {} {}", addr, new_next_hop_addr),
                    format!("no ip route {} {}", old_sr.addr, old_sr.next_hop),
                ],
            )]
        }
        modifier => panic!("Invalid Modifier: {:?}", modifier),
    }
}

fn get_interface_from_to(
    phys_net: &mut PhysicalNetwork,
    source: RouterId,
    target: RouterId,
) -> String {
    let iface_idx = get_iface_idx(phys_net, source, target);
    phys_net.routers[source.index()].ifaces[iface_idx].gns_interface.name.clone()
}

fn get_iface_idx(phys_net: &mut PhysicalNetwork, source: RouterId, target: RouterId) -> usize {
    phys_net.routers[source.index()]
        .ifaces
        .iter()
        .position(|x| x.neighbor == target)
        .expect("Link does not exist")
}

fn get_bgp_peering_addr(
    phys_net: &mut PhysicalNetwork,
    source: RouterId,
    target: RouterId,
    session_type: BgpSessionType,
) -> (IpAddr, IpAddr) {
    if session_type.is_ebgp() {
        phys_net.routers[source.index()]
            .ifaces
            .iter()
            .filter(|i| i.neighbor == target)
            .map(|i| (i.iface_addr.clone(), i.neighbor_addr.clone()))
            .next()
            .expect("eBGP peers must be directly connected")
    } else {
        (
            phys_net.routers[source.index()].loopback_addr.clone(),
            phys_net.routers[target.index()].loopback_addr.clone(),
        )
    }
}

fn get_route_map_name(phys_net: &mut PhysicalNetwork, router: RouterId) -> String {
    format!(
        "{}_RM_{}",
        &phys_net.routers[router.index()].name,
        phys_net.routers[router.index()].route_maps.len()
    )
}
