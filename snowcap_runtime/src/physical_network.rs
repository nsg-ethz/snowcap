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

//! # Physical representation of the abstract network from `snowcap`
//!
//! This module uses GNS3 to simulate the network, provided by `snowcap`.

use gns3::*;
use snowcap::netsim::config::*;
use snowcap::netsim::external_router::ExternalRouter;
use snowcap::netsim::route_map::RouteMap;
use snowcap::netsim::route_map::*;
use snowcap::netsim::*;

use crate::config::{apply_config, parse_modifier};
use crate::frr_conn::{FrrConnection, RoutingTable};
use crate::pcap_reader::{extract_pcap_flows, path_inference};
use crate::python_conn::PythonConnection;

use log::*;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const CONVERGE_CHECK_INTERVAL_MS: u64 = 1000;
const CONVERGE_CHECK_NUM_INVARIANT: usize = 5;
const WAIT_NETWORK_INITIALIZE_S: u64 = 200;

/// Start Router-ID, used to internally represent clients (python clients, or VPCS).
pub const CLIENT_ID_BASE: u32 = 1000000;

const ROUTER_TEMPLATE_NAME: &str = "FRR 7.3.1";
const CLIENT_TEMPLATE_NAME: &str = "Python, Go, Perl, PHP";

const PYTHON_SENDER_PROGRAM: &str = "
import socket, sys, time
seq = 0
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
while True:
    i = 1
    while i + 2 <= len(sys.argv):
        data = int(sys.argv[i + 1]).to_bytes(4, byteorder='big') + seq.to_bytes(4, byteorder='big')
        sock.sendto(data, (sys.argv[i], 5001))
        i += 2
    seq += 1
    time.sleep(0.01)
";

/// # Physical Network
///
/// This is the main datastructure, which contains all information about the physical network. it
/// holds the reference to the GNS3 server, and handles the project. When creating a new instance,
/// it will automatically create a project, and all necessary network devices and links. It will
/// also configure the network automatically.
///
/// For each origin prefix, a router and a network is generated. Note, that every origin is only
/// generated once. Also, notice that the AS path is not edited. This means, that any information
/// form the input network, in terms of AS path, is lost! Each origin router is given a new router
/// ID, starting from the highest currently used router id.
///
/// ## IP Convention
///
/// - **Internal Routers**, with router id `x`
///   - Loopback Address: `10.0.x.1/32`
///   - Address of the interface towards the client: `10.0.x.2/24`
///   - Address of the client: `10.0.x.3/24`
/// - **External routers**, with router id `x`
///   - Loopback Address: `(100 + x).0.0.1/32`
///   - Address of the interface towards the client: `(100 + x).0.0.2/24`
///   - Address of the client: `(100 + x).0.0.2/24`
/// - **Origin routers**, with router id `x`, advertising prefix `p`
///   - Loopback Address: `(200 + x).0.0.1/32`
///   - Address of the interface towards the client: `(200 + x).0.0.2/24`
///   - Address of the client: `(200 + x).0.0.2/24`
/// - **Links: Internal --- Internal/External**, with link i 'x', from router `a` to router `b`
///   - Address of router a: `10.1.x.1/32`
///   - Address of router b: `10.1.x.2/32`
/// - **Links: External --- Origin**, from external router `x` to origin router `o` with orefix `p`
///   - Address of origin router o: `(200 + o).1.x.1/32`
///   - Address of external router b: `(200 + o).1.x.2/32`
#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalNetwork {
    server: GNS3Server,
    project_id: String,
    num_explicit_routers: usize,
    num_origin_routers: usize,
    prefixes: Vec<Prefix>,
    /// Vector of all routers in the network
    pub routers: Vec<PhysicalRouter>,
    /// Vector of all links in the network
    pub links: Vec<PhysicalLink>,
    /// Vector of all clients in the network
    pub clients: Vec<PhysicalClient>,
    ip_lookup: HashMap<RouterId, [u8; 4]>,
    prefix_router_lookup: HashMap<Prefix, RouterId>,
    reverse_ip_lookup: HashMap<[u8; 4], RouterId>,
    flow_lookup: HashMap<(RouterId, Prefix), u32>,
    frr_template_id: String,
    client_tempate_id: String,
    persistent_gns_project: bool,
}

impl PhysicalNetwork {
    /// Generate the physical network
    pub fn new(
        net: &Network,
        name: impl AsRef<str>,
        persistent_gns_project: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let config = net.current_config();
        let mut server = GNS3Server::new("localhost", 3080)?;

        // delete net if it already exists
        if let Some(project) =
            server.get_projects().unwrap().into_iter().find(|p| p.name.as_str() == name.as_ref())
        {
            server.delete_project(project.id).unwrap();
        }

        let project_id = server.create_project(name)?.id;
        let num_explicit_routers = net.num_devices();
        let num_origin_routers = net.get_known_prefixes().len();
        let num_devices = num_explicit_routers + num_origin_routers;

        // create routers
        let mut router_template = None;
        let mut client_template = None;
        server.get_templates()?.into_iter().for_each(|t| {
            if t.name == ROUTER_TEMPLATE_NAME {
                router_template = Some(t.id);
            } else if t.name == CLIENT_TEMPLATE_NAME {
                client_template = Some(t.id);
            }
        });

        let mut phys_net = Self {
            server,
            project_id,
            num_explicit_routers,
            num_origin_routers,
            prefixes: net.get_known_prefixes().iter().cloned().collect(),
            routers: Vec::with_capacity(num_devices),
            links: Vec::with_capacity(net.links_symmetric().count()),
            clients: Vec::with_capacity(num_devices),
            prefix_router_lookup: HashMap::new(),
            ip_lookup: HashMap::new(),
            reverse_ip_lookup: HashMap::new(),
            flow_lookup: HashMap::new(),
            frr_template_id: router_template.unwrap(),
            client_tempate_id: client_template.unwrap(),
            persistent_gns_project,
        };

        phys_net.create_routers(net)?;
        phys_net.create_origin_routers(net)?;
        phys_net.create_all_links(net)?;
        phys_net.create_links_to_origin(net)?;
        phys_net.create_clients_on_all_routers();
        apply_config(&mut phys_net, config);
        phys_net.setup_ip_lookup();
        phys_net.prepare_flows();

        info!("Starting the netowrk...");
        phys_net.server.start_all_nodes()?;

        phys_net.setup_clients()?;
        phys_net.setup_frr_routers()?;

        info!("Network successfully configured! waiting for convergence...");
        thread::sleep(Duration::from_secs(WAIT_NETWORK_INITIALIZE_S));
        phys_net.wait_converge()?;

        Ok(phys_net)
    }

    /// Return the router name of a router
    pub fn router_name(&self, r: RouterId) -> &str {
        if r.index() >= CLIENT_ID_BASE as usize {
            self.clients.get(r.index() - CLIENT_ID_BASE as usize).unwrap().name.as_str()
        } else {
            self.routers.get(r.index()).unwrap().name.as_str()
        }
    }

    /// Return the router id of the origin router, responsible for a given prefix
    pub fn prefix_router_id(&self, prefix: Prefix) -> RouterId {
        *self.prefix_router_lookup.get(&prefix).unwrap()
    }

    /// Create all internal routers
    fn create_routers(&mut self, net: &Network) -> Result<(), Box<dyn Error>> {
        for i in 0..net.num_devices() {
            let router_id = (i as u32).into();
            match net.get_device(router_id) {
                NetworkDevice::InternalRouter(r) => {
                    let gns_node = self.server.create_node(r.name(), &self.frr_template_id)?;
                    self.routers.push(PhysicalRouter {
                        router_id,
                        name: r.name().to_string(),
                        as_id: r.as_id(),
                        gns_node,
                        loopback_addr: IpAddr::new(format!("10.0.{}.1", router_id.index()), 24),
                        ifaces: Vec::new(),
                        bgp_sessions: Vec::new(),
                        route_maps: Vec::new(),
                        static_routes: Vec::new(),
                        advertise_route: Some(IpAddr::new("10.0.0.0", 8)),
                        is_internal: true,
                    });
                }
                NetworkDevice::ExternalRouter(r) => {
                    let gns_node = self.server.create_node(r.name(), &self.frr_template_id)?;
                    self.routers.push(PhysicalRouter {
                        router_id,
                        name: r.name().to_string(),
                        as_id: r.as_id(),
                        gns_node,
                        loopback_addr: IpAddr::new(
                            format!("{}.0.0.1", router_id.index() + 100),
                            24,
                        ),
                        ifaces: Vec::new(),
                        bgp_sessions: Vec::new(),
                        route_maps: Vec::new(),
                        static_routes: Vec::new(),
                        advertise_route: Some(IpAddr::new(
                            format!("{}.0.0.0", router_id.index() + 100),
                            8,
                        )),
                        is_internal: false,
                    });
                }
                _ => unreachable!("Could not find device!"),
            }
            info!(
                "Create router: {} with ip {}, telnet port: {}",
                self.routers.last().unwrap().name,
                self.routers.last().unwrap().loopback_addr,
                self.routers.last().unwrap().gns_node.port
            );
        }
        Ok(())
    }

    /// Create all routers that originate a specific prefix
    fn create_origin_routers(&mut self, net: &Network) -> Result<(), Box<dyn Error>> {
        for prefix in self.prefixes.iter() {
            // get advertising routers
            let advertising_routers = Self::get_external_routers_with_prefix(net, *prefix);

            if advertising_routers.is_empty() {
                warn!("No external router actually advertises a prefix!");
                continue;
            }

            // extract as id and check if it is the same on all routers
            let mut as_id_iter = advertising_routers.iter().map(|r| {
                r.get_advertised_routes()
                    .iter()
                    .filter(|r| r.prefix == *prefix)
                    .map(|r| r.as_path.last().unwrap())
                    .next()
                    .unwrap()
            });
            let as_id: AsId = *as_id_iter.next().unwrap();
            assert!(as_id_iter.all(|x| as_id == *x));

            let name = format!("origin{}", prefix.0);
            let gns_node = self.server.create_node(&name, &self.frr_template_id)?;
            let origin_router_id = self.routers.len();
            self.routers.push(PhysicalRouter {
                router_id: (origin_router_id as u32).into(),
                name,
                as_id,
                gns_node,
                loopback_addr: IpAddr::new(format!("{}.0.0.1", prefix.0 + 200), 24),
                ifaces: Vec::new(),
                bgp_sessions: Vec::new(),
                route_maps: Vec::new(),
                static_routes: Vec::new(),
                advertise_route: Some(IpAddr::new(format!("{}.0.0.0", prefix.0 + 200), 8)),
                is_internal: false,
            });

            self.prefix_router_lookup.insert(*prefix, (origin_router_id as u32).into());

            info!(
                "Create router: {} with ip {}, telnet port: {}",
                self.routers.last().unwrap().name,
                self.routers.last().unwrap().loopback_addr,
                self.routers.last().unwrap().gns_node.port
            );
        }
        Ok(())
    }

    /// Create all links that are present in the network. The links from the external router to the
    /// prefix origins will not be created!
    fn create_all_links(&mut self, net: &Network) -> Result<(), Box<dyn Error>> {
        for (link_id, (a, b)) in net.links_symmetric().enumerate() {
            let iface_a = self.routers[a.index()].ifaces.len();
            let iface_b = self.routers[b.index()].ifaces.len();
            assert!(iface_a < self.routers[a.index()].gns_node.interfaces.len());
            assert!(iface_b < self.routers[b.index()].gns_node.interfaces.len());
            let gns_iface_a = self.routers[a.index()].gns_node.interfaces[iface_a].clone();
            let gns_iface_b = self.routers[b.index()].gns_node.interfaces[iface_b].clone();

            // create the link
            let gns_link = self.server.create_link(
                &self.routers[a.index()].gns_node,
                iface_a,
                &self.routers[b.index()].gns_node,
                iface_b,
            )?;

            let a_addr = IpAddr { addr: format!("10.1.{}.1", link_id), mask: 24 };
            let b_addr = IpAddr { addr: format!("10.1.{}.2", link_id), mask: 24 };

            self.links.push(PhysicalLink { gns_link, endpoint_a: *a, endpoint_b: *b });

            self.routers[a.index()].ifaces.push(IfaceInfo {
                neighbor: *b,
                neighbor_addr: b_addr.clone(),
                iface_addr: a_addr.clone(),
                gns_interface: gns_iface_a,
                enabled: false,
                cost: None,
                link_id: self.links.len(),
            });

            self.routers[b.index()].ifaces.push(IfaceInfo {
                neighbor: *a,
                neighbor_addr: a_addr.clone(),
                iface_addr: b_addr.clone(),
                gns_interface: gns_iface_b,
                enabled: false,
                cost: None,
                link_id: self.links.len(),
            });

            info!(
                "Created link: [{} <-> {}] with ip {} and {}",
                self.routers[a.index()].name,
                self.routers[b.index()].name,
                a_addr,
                b_addr,
            );
        }

        Ok(())
    }

    // Create the links to the origin routers
    fn create_links_to_origin(&mut self, net: &Network) -> Result<(), Box<dyn Error>> {
        for prefix in self.prefixes.iter() {
            let origin_router_index = self.get_origin_router_index(*prefix);
            for ext_router_id in
                Self::get_external_routers_with_prefix(net, *prefix).iter().map(|r| r.router_id())
            {
                let iface_origin = self.routers[origin_router_index].ifaces.len();
                let iface_ext = self.routers[ext_router_id.index()].ifaces.len();
                assert!(iface_origin < self.routers[origin_router_index].gns_node.interfaces.len());
                assert!(iface_ext < self.routers[ext_router_id.index()].gns_node.interfaces.len());
                let gns_iface_origin =
                    self.routers[origin_router_index].gns_node.interfaces[iface_origin].clone();
                let gns_iface_ext =
                    self.routers[ext_router_id.index()].gns_node.interfaces[iface_ext].clone();

                // create the link
                let gns_link = self.server.create_link(
                    &self.routers[origin_router_index].gns_node,
                    iface_origin,
                    &self.routers[ext_router_id.index()].gns_node,
                    iface_ext,
                )?;

                let origin_addr =
                    IpAddr::new(format!("{}.1.{}.1", 200 + prefix.0, ext_router_id.index()), 24);
                let ext_addr =
                    IpAddr::new(format!("{}.1.{}.2", 200 + prefix.0, ext_router_id.index()), 24);

                self.links.push(PhysicalLink {
                    gns_link,
                    endpoint_a: (origin_router_index as u32).into(),
                    endpoint_b: ext_router_id,
                });
                let link_id = self.links.len();

                self.routers[origin_router_index].ifaces.push(IfaceInfo {
                    neighbor: ext_router_id,
                    neighbor_addr: ext_addr.clone(),
                    iface_addr: origin_addr.clone(),
                    gns_interface: gns_iface_origin,
                    enabled: true,
                    cost: None,
                    link_id,
                });

                self.routers[ext_router_id.index()].ifaces.push(IfaceInfo {
                    neighbor: (origin_router_index as u32).into(),
                    neighbor_addr: origin_addr.clone(),
                    iface_addr: ext_addr.clone(),
                    gns_interface: gns_iface_ext,
                    enabled: true,
                    cost: None,
                    link_id,
                });

                // setup a session from the origin to the external router and viceversa
                let origin_as = self.routers[origin_router_index].as_id;
                let ext_as = self.routers[ext_router_id.index()].as_id;
                self.routers[origin_router_index].bgp_sessions.push(BgpSessionInfo {
                    neighbor: ext_router_id,
                    neighbor_addr: ext_addr.clone(),
                    neighbor_as_id: ext_as,
                    is_rr_client: false,
                    internal_session: false,
                });
                self.routers[ext_router_id.index()].bgp_sessions.push(BgpSessionInfo {
                    neighbor: (origin_router_index as u32).into(),
                    neighbor_addr: origin_addr.clone(),
                    neighbor_as_id: origin_as,
                    is_rr_client: false,
                    internal_session: false,
                });

                info!(
                    "Created link: [{} <-> {}] with ip {} and {}",
                    self.routers[origin_router_index].name,
                    self.routers[ext_router_id.index()].name,
                    origin_addr,
                    ext_addr,
                );
            }
        }
        Ok(())
    }

    /// Add a vpcs node to every router in the network
    fn create_clients_on_all_routers(&mut self) {
        for r in self.routers.iter_mut() {
            // get the ip of the router
            let lo_ip = r.loopback_addr.clone();
            let router_ip = lo_ip.add_one();
            let client_ip = router_ip.add_one();
            let client_name = format!("{}-client", r.name);
            let client_id: RouterId = (CLIENT_ID_BASE + r.router_id.index() as u32).into();

            // create the vpcs
            let gns_client =
                self.server.create_node(&client_name, &self.client_tempate_id).unwrap();

            self.clients.push(PhysicalClient {
                name: client_name,
                client_id,
                gns_client,
                addr: client_ip.clone(),
                gateway_addr: router_ip.clone(),
                flows: Vec::new(),
            });

            // add the link to the router
            let iface = r.ifaces.len();
            assert!(iface < r.gns_node.interfaces.len());
            let gns_iface = r.gns_node.interfaces[iface].clone();

            // create the link on gns3
            let gns_link = self
                .server
                .create_link(&r.gns_node, iface, &self.clients.last().unwrap().gns_client, 0)
                .unwrap();

            self.links.push(PhysicalLink {
                gns_link,
                endpoint_a: r.router_id,
                endpoint_b: client_id,
            });
            let link_id = self.links.len();

            r.ifaces.push(IfaceInfo {
                neighbor: client_id,
                neighbor_addr: client_ip.clone(),
                iface_addr: router_ip.clone(),
                gns_interface: gns_iface,
                enabled: true,
                cost: None,
                link_id,
            });
            info!("Created client: {}, ip: {}", self.clients.last().unwrap().name, client_ip);
        }
    }

    /// Setup the reverse IP lookup
    fn setup_ip_lookup(&mut self) {
        for router in self.routers.iter() {
            let addr_parts = router.loopback_addr.addr_parts();
            self.reverse_ip_lookup.insert(addr_parts, router.router_id);
            self.ip_lookup.insert(router.router_id, addr_parts);
            for iface in router.ifaces.iter() {
                let addr_parts = iface.iface_addr.addr_parts();
                self.reverse_ip_lookup.insert(addr_parts, router.router_id);
            }
        }
        for client in self.clients.iter() {
            let addr_parts = client.addr.addr_parts();
            self.ip_lookup.insert(client.client_id, addr_parts);
        }
    }

    /// Prepare all flows for all clients
    fn prepare_flows(&mut self) {
        let mut flow_id: u32 = 0;
        for i in 0..self.clients.len() {
            if self.routers[i].is_internal {
                for p in self.prefixes.iter() {
                    self.flow_lookup.insert((self.clients[i].client_id, *p), flow_id);
                    // get the target ip
                    let origin_router = self.prefix_router_lookup.get(p).unwrap();
                    let origin_client_addr = *self
                        .ip_lookup
                        .get(&self.clients[origin_router.index()].client_id)
                        .unwrap();
                    self.clients[i].flows.push((origin_client_addr, flow_id));
                    flow_id += 1;
                }
            }
        }
    }

    /// configure all clients
    fn setup_clients(&mut self) -> Result<(), Box<dyn Error>> {
        info!("configuring clients...");
        let mut jobs = Vec::with_capacity(self.clients.len());
        for client in self.clients.iter().cloned() {
            jobs.push(thread::spawn(move || {
                let mut c = match PythonConnection::new(client.gns_client.port) {
                    Ok(c) => c,
                    Err(e) => {
                        error!("Cannot setup a session to the client {}: {}", client.name, e);
                        return Err(format!("{}", e));
                    }
                };
                if let Err(e) = c.configure(&client.addr, &client.gateway_addr) {
                    error!("Client {} configuration eror: {}", client.name, e);
                    return Err(format!("{}", e));
                }
                Ok(())
            }));
        }

        for handle in jobs {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => panic!("Something wierd happened with the threads!"),
            }
        }

        info!("restarting all clients");
        // restart all clients
        for client in self.clients.iter() {
            self.server.stop_node(&client.gns_client.id)?;
        }
        for client in self.clients.iter() {
            self.server.start_node(&client.gns_client.id)?;
        }

        let mut jobs = Vec::with_capacity(self.clients.len());
        for client in self.clients.iter().cloned() {
            jobs.push(thread::spawn(move || {
                let mut c = match PythonConnection::new(client.gns_client.port) {
                    Ok(c) => c,
                    Err(e) => {
                        error!("Cannot setup a session to the client {}: {}", client.name, e);
                        return Err(format!("{}", e));
                    }
                };
                if let Err(e) = c.create_file("/root/sender.py", PYTHON_SENDER_PROGRAM) {
                    error!("Client {} error while writing program: {}", client.name, e);
                    return Err(format!("{}", e));
                }
                info!("Client {} configured successfully", client.name);
                Ok(())
            }));
        }

        for handle in jobs {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => panic!("Something wierd happened with the threads!"),
            }
        }

        Ok(())
    }

    fn setup_frr_routers(&mut self) -> Result<(), Box<dyn Error>> {
        info!("connecting terminals...");
        let mut jobs = Vec::with_capacity(self.routers.len());
        for i in 0..self.routers.len() {
            let r = self.routers.get(i).unwrap().clone();
            jobs.push(thread::spawn(move || {
                let mut c = match FrrConnection::new(r.gns_node.port) {
                    Ok(c) => c,
                    Err(e) => {
                        error!("Cannot setup a session to the router {}: {}", r.name, e);
                        return Err(format!("{}", e));
                    }
                };
                info!("connected to terminal {}", r.name);
                if let Err(e) = c.initialize_config(&r) {
                    error!("{} configuration eror: {}", r.name, e);
                    return Err(format!("{}", e));
                }
                info!("{} configured successfully", r.name);
                Ok(())
            }));
        }

        for handle in jobs {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => panic!("Something wierd happened with the threads!"),
            }
        }

        Ok(())
    }

    /// Extract the routers that advertise a specific prefix
    fn get_external_routers_with_prefix(net: &Network, prefix: Prefix) -> Vec<&ExternalRouter> {
        // extract the as id of this prefix
        net.get_external_routers()
            .into_iter()
            .map(|r| net.get_device(r).unwrap_external())
            .filter(|r| r.advertised_prefixes().contains(&prefix))
            .collect()
    }

    /// Returns the index of an origin router in the structure
    fn get_origin_router_index(&self, prefix: Prefix) -> usize {
        self.num_explicit_routers + prefix.0 as usize
    }

    /// Wait until the network has converged. We call a network to be converged, if after 10
    /// consecutive trials (with 3 second delay) are identical.
    pub fn wait_converge(&self) -> Result<(), Box<dyn Error>> {
        let now = std::time::SystemTime::now();
        // get the initial routing tables
        let mut current_rt = self.get_routing_tables()?;
        let mut unchanged = 0;
        while unchanged < (CONVERGE_CHECK_NUM_INVARIANT - 1) {
            std::thread::sleep(std::time::Duration::from_millis(CONVERGE_CHECK_INTERVAL_MS));
            let new_rt = self.get_routing_tables()?;
            if new_rt == current_rt {
                unchanged += 1;
            } else {
                unchanged = 0;
                current_rt = new_rt;
            }
        }
        info!("Network converged after {} seconds", now.elapsed().unwrap().as_secs());
        Ok(())
    }

    /// apply a modifier, wait until everything has converged, and check all flows
    #[allow(clippy::type_complexity, clippy::needless_collect, clippy::map_collect_result_unit)]
    pub fn apply_modifier_wait_convergence_check_flows(
        &mut self,
        modifier: &ConfigModifier,
    ) -> Result<HashMap<(RouterId, Prefix), HashMap<Option<Vec<RouterId>>, usize>>, Box<dyn Error>>
    {
        // start to capture
        self.start_capture()?;

        // start the flows
        let stop = Arc::new(AtomicBool::new(false));
        let handles: Vec<thread::JoinHandle<Result<(), String>>> = self
            .clients
            .iter()
            .filter(|c| !c.flows.is_empty())
            .map(|c| {
                let port = c.gns_client.port;
                let flows = c.flows.clone();
                let s = stop.clone();
                thread::spawn(move || {
                    let mut c = PythonConnection::new(port).map_err(|e| format!("{}", e))?;
                    c.run_program(format!(
                        "python3 /root/sender.py {}",
                        flows
                            .into_iter()
                            .map(|(ip, flow_id)| format!(
                                "{}.{}.{}.{} {}",
                                ip[0], ip[1], ip[2], ip[3], flow_id
                            ))
                            .collect::<Vec<_>>()
                            .join(" "),
                    ))
                    .map_err(|e| format!("{}", e))?;
                    // wait until the stop command is received
                    while !s.load(Relaxed) {
                        thread::sleep(Duration::from_secs(1));
                    }
                    // send control-c
                    c.ctrl_c().map_err(|e| format!("{}", e))?;
                    Ok(())
                })
            })
            .collect::<Vec<_>>();

        // wait 5 seconds until all flows have started sending their packets
        thread::sleep(Duration::from_secs(5));

        // apply the modifier
        self.apply_modifier(modifier)?;

        info!("waiting for convergence");
        // wait until convergence
        self.wait_converge()?;

        // stop the flows
        stop.store(true, Relaxed);
        handles.into_iter().map(|h| h.join().unwrap()).collect::<Result<(), String>>()?;

        // Stop the capturing
        self.stop_capture()?;

        // extract all path information
        self.read_infer_flows()
    }

    #[allow(clippy::type_complexity, clippy::needless_collect, clippy::map_collect_result_unit)]
    /// apply a modifier, wait until everything has converged, and check all flows
    pub fn apply_all_modifiers_wait_convergence_check_flows(
        &mut self,
        modifiers: &[ConfigModifier],
        pause_duration_s: u64,
    ) -> Result<HashMap<(RouterId, Prefix), HashMap<Option<Vec<RouterId>>, usize>>, Box<dyn Error>>
    {
        // start to capture
        self.start_capture()?;

        // start the flows
        let stop = Arc::new(AtomicBool::new(false));
        let handles: Vec<thread::JoinHandle<Result<(), String>>> = self
            .clients
            .iter()
            .filter(|c| !c.flows.is_empty())
            .map(|c| {
                let port = c.gns_client.port;
                let flows = c.flows.clone();
                let s = stop.clone();
                thread::spawn(move || {
                    let mut c = PythonConnection::new(port).map_err(|e| format!("{}", e))?;
                    c.run_program(format!(
                        "python3 /root/sender.py {}",
                        flows
                            .into_iter()
                            .map(|(ip, flow_id)| format!(
                                "{}.{}.{}.{} {}",
                                ip[0], ip[1], ip[2], ip[3], flow_id
                            ))
                            .collect::<Vec<_>>()
                            .join(" "),
                    ))
                    .map_err(|e| format!("{}", e))?;
                    // wait until the stop command is received
                    while !s.load(Relaxed) {
                        thread::sleep(Duration::from_secs(1));
                    }
                    // send control-c
                    c.ctrl_c().map_err(|e| format!("{}", e))?;
                    Ok(())
                })
            })
            .collect::<Vec<_>>();

        for m in modifiers {
            // wait 5 seconds until all flows have started sending their packets
            thread::sleep(Duration::from_secs(pause_duration_s));

            // apply the modifier
            self.apply_modifier(m)?;
        }

        info!("waiting for convergence");
        // wait until convergence
        self.wait_converge()?;

        // stop the flows
        stop.store(true, Relaxed);
        handles.into_iter().map(|h| h.join().unwrap()).collect::<Result<(), String>>()?;

        // Stop the capturing
        self.stop_capture()?;

        // extract all path information
        self.read_infer_flows()
    }

    /// Apply a modifier without monitoring the network
    fn apply_modifier(&mut self, modifier: &ConfigModifier) -> Result<(), Box<dyn Error>> {
        let commands = parse_modifier(self, modifier);
        for (target, commands) in commands {
            let mut term = FrrConnection::new(self.routers[target.index()].gns_node.port)?;
            term.reconfigure(commands)?;
        }
        Ok(())
    }

    /// Start capture on all links
    pub fn start_capture(&mut self) -> Result<(), Box<dyn Error>> {
        for l in self.links.iter_mut() {
            let link_id = &l.gns_link.id;
            l.gns_link = self.server.start_capture(link_id)?;
        }

        Ok(())
    }

    /// Start capture on all links
    pub fn stop_capture(&mut self) -> Result<(), Box<dyn Error>> {
        for l in self.links.iter_mut() {
            let link_id = &l.gns_link.id;
            l.gns_link = self.server.stop_capture(link_id)?;
        }

        Ok(())
    }

    /// Read all pcap files and infer all flows
    #[allow(clippy::type_complexity)]
    fn read_infer_flows(
        &self,
    ) -> Result<HashMap<(RouterId, Prefix), HashMap<Option<Vec<RouterId>>, usize>>, Box<dyn Error>>
    {
        // extract all capture flow information
        let captures = self
            .links
            .iter()
            .map(|l| match extract_pcap_flows(l.gns_link.capture_file_path.as_ref().unwrap()) {
                Ok(cap) => Ok((l.endpoint_a, l.endpoint_b, cap)),
                Err(e) => Err(e),
            })
            .collect::<Result<Vec<(RouterId, RouterId, HashMap<u32, Vec<u32>>)>, Box<dyn Error>>>(
            )?;

        // infer all flows from this
        Ok(path_inference(captures, &self.flow_lookup))
    }

    /// Get all routing tables in parallel
    #[allow(clippy::needless_collect)]
    fn get_routing_tables(&self) -> Result<Vec<RoutingTable>, Box<dyn Error>> {
        let jobs = self
            .routers
            .iter()
            .map(|r| {
                let port = r.gns_node.port;
                let name = r.name.clone();
                thread::spawn(move || {
                    let mut c = FrrConnection::new(port).map_err(|e| format!("{}", e))?;
                    c.get_routing_table()
                        .map_err(|e| format!("Cannot parse routing table from {}: {}", name, e))
                })
            })
            .collect::<Vec<_>>();
        jobs.into_iter()
            .map(|handle| match handle.join().unwrap() {
                Ok(table) => Ok(table),
                Err(e) => Err(e.into()),
            })
            .collect::<Vec<Result<RoutingTable, Box<dyn Error>>>>()
            .into_iter()
            .collect()
    }

    /// Get all paths in the network from every explicit router to every prefix. Here, we use the
    /// traceroute functionality!
    #[allow(clippy::type_complexity, clippy::needless_collect)]
    pub fn get_all_paths(
        &self,
    ) -> Result<HashMap<RouterId, HashMap<Prefix, Option<Vec<RouterId>>>>, Box<dyn Error>> {
        let mut result: HashMap<RouterId, HashMap<Prefix, Option<Vec<RouterId>>>> =
            (0..self.num_explicit_routers).map(|i| ((i as u32).into(), HashMap::new())).collect();

        for prefix in self.prefixes.iter() {
            let target_ip =
                self.routers[self.get_origin_router_index(*prefix)].loopback_addr.clone();
            let jobs = (0..self.num_explicit_routers)
                .map(|i| {
                    let port = self.routers[i].gns_node.port;
                    let ip = target_ip.clone();
                    thread::spawn(move || {
                        let mut c = FrrConnection::new(port).map_err(|e| format!("{}", e))?;
                        c.traceroute(&ip).map_err(|e| format!("{}", e))
                    })
                })
                .collect::<Vec<_>>();

            // wait until all traceroutes are finished
            let mut err: Option<String> = None;
            for (i, handle) in jobs.into_iter().enumerate() {
                match handle.join() {
                    Ok(Ok(Some(path))) => {
                        let path = path
                            .into_iter()
                            .map(|ip| *self.reverse_ip_lookup.get(&ip).unwrap())
                            .collect();
                        result.get_mut(&(i as u32).into()).unwrap().insert(*prefix, Some(path));
                    }
                    Ok(Ok(None)) => {
                        result.get_mut(&(i as u32).into()).unwrap().insert(*prefix, None);
                    }
                    Ok(Err(e)) => err = Some(e),
                    Err(e) => err = Some(format!("Thread error:  {:?}", e)),
                }
            }

            if let Some(err) = err {
                return Err(err.into());
            }
        }

        Ok(result)
    }

    /// Returns the router name
    pub fn get_router_name(&self, id: RouterId) -> &str {
        self.routers.get(id.index()).map(|r| r.name.as_str()).unwrap_or("?")
    }
}

impl Drop for PhysicalNetwork {
    fn drop(&mut self) {
        if !self.persistent_gns_project {
            self.server.delete_project(&self.project_id).unwrap();
        }
    }
}

/// All information about the physical client
#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalClient {
    /// Name of the client
    pub name: String,
    /// Router ID of the client, starting with [`CLIENT_ID_BASE`]
    pub client_id: RouterId,
    /// ALL information from GNS3
    pub gns_client: GNS3Node,
    /// IP address
    pub addr: IpAddr,
    /// Gateway Address (Address of the connected router)
    pub gateway_addr: IpAddr,
    /// Flows to probe
    pub flows: Vec<([u8; 4], u32)>,
}

/// All information about the physical link
#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalLink {
    /// GNS3 link information (including capture file, if enabled)
    pub gns_link: GNS3Link,
    /// Router ID of endpoint a
    pub endpoint_a: RouterId,
    /// Router ID of endpoint b
    pub endpoint_b: RouterId,
}

/// All information about a physical router, needed to configure the router
#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalRouter {
    /// Router ID
    pub router_id: RouterId,
    /// Name of the router
    pub name: String,
    /// GNS3 Node Information
    pub gns_node: GNS3Node,
    /// Address of the loopback interface, should have a net mask of `32`!
    pub loopback_addr: IpAddr,
    /// AS Id (must be greater tham 0)
    pub as_id: AsId,
    /// List of Interfaces
    pub ifaces: Vec<IfaceInfo>,
    /// List of active BGP sessions
    pub bgp_sessions: Vec<BgpSessionInfo>,
    /// List of configured Route-Maps
    pub route_maps: Vec<RouteMapInfo>,
    /// List of all static routes
    pub static_routes: Vec<StaticRouteInfo>,
    /// List of all routes that are advertised by this router via BGP
    pub advertise_route: Option<IpAddr>,
    /// Flag, if the router is internal or not
    pub is_internal: bool,
}

/// BGP Session Information
#[derive(Debug, Clone, PartialEq)]
pub struct BgpSessionInfo {
    /// Neighbor, with which the current router is connected
    pub neighbor: RouterId,
    /// BGP address of the neighbor
    pub neighbor_addr: IpAddr,
    /// BGP AS ID of the neighbor
    pub neighbor_as_id: AsId,
    /// Flag, which should be set to true if this router is a route-reflector and the other the
    /// client
    pub is_rr_client: bool,
    /// Flag, if the session is internal or not.
    pub internal_session: bool,
}

/// Route Map Information
#[derive(Debug, Clone, PartialEq)]
pub struct RouteMapInfo {
    /// Name of the route map
    pub name: String,
    /// State (either `permit` or `deny`)
    pub state: &'static str,
    /// Order, when to apply the route map
    pub order: u32,
    /// Direction (either `out` or `in`)
    pub direction: &'static str,
    /// Match statements, allowed by [FRR](https://docs.frrouting.org/en/latest/routemap.html)
    pub match_statements: HashMap<&'static str, String>,
    /// Set statements, allowed by [FRR](https://docs.frrouting.org/en/latest/routemap.html)
    pub set_statements: HashMap<&'static str, String>,
}

impl RouteMapInfo {
    /// Build a RouteMapInfo from a [`RouteMap`](snowcap::netsim::route_map::RouteMap)
    pub fn from_route_map(
        name: String,
        router_id: RouterId,
        direction: RouteMapDirection,
        map: &RouteMap,
        routers: &[PhysicalRouter],
    ) -> Self {
        Self {
            name,
            state: if map.state().is_allow() { "permit" } else { "deny" },
            order: map.order() as u32,
            direction: if direction == RouteMapDirection::Incoming { "in" } else { "out" },
            match_statements: map
                .conds()
                .iter()
                .map(|cond| match cond {
                    RouteMapMatch::Neighbor(neighbor_id) => (
                        "peer",
                        routers[neighbor_id.index()]
                            .ifaces
                            .iter()
                            .find(|i| i.neighbor == router_id)
                            .unwrap()
                            .iface_addr
                            .to_string(),
                    ),
                    RouteMapMatch::Prefix(_) => todo!(),
                    RouteMapMatch::AsPath(_) => todo!(),
                    RouteMapMatch::NextHop(_) => todo!(),
                    RouteMapMatch::Community(_) => todo!(),
                })
                .collect(),
            set_statements: map
                .actions()
                .iter()
                .filter_map(|action| match action {
                    RouteMapSet::NextHop(r) => {
                        Some(("ip next-hop", routers[r.index()].loopback_addr.addr.clone()))
                    }
                    RouteMapSet::LocalPref(Some(lp)) => {
                        Some(("local-preference", format!("{}", lp)))
                    }
                    RouteMapSet::LocalPref(None) => Some(("local-preference", String::from("+0"))),
                    RouteMapSet::Med(Some(med)) => Some(("metric", format!("{}", med))),
                    RouteMapSet::Med(None) => Some(("metric", String::from("+0"))),
                    RouteMapSet::IgpCost(_) => panic!("IGP const cannot be changed on FRR"),
                    RouteMapSet::Community(Some(c)) => Some(("community", format!("{}", c))),
                    RouteMapSet::Community(None) => panic!("Communities are not yet supported"),
                })
                .collect(),
        }
    }
}

/// Information about the static route
#[derive(Debug, Clone, PartialEq)]
pub struct StaticRouteInfo {
    /// Prefix, to match on
    pub addr: IpAddr,
    /// Next hop, should be given as a IP address.
    pub next_hop: String,
}

/// Interface Information
#[derive(Debug, Clone, PartialEq)]
pub struct IfaceInfo {
    /// Neighbor Router ID
    pub neighbor: RouterId,
    /// Neighbor address (connected to this interface)
    pub neighbor_addr: IpAddr,
    /// Interface address
    pub iface_addr: IpAddr,
    /// GNS3 Interface information
    pub gns_interface: GNS3Interface,
    /// Flag, if this interface is enabled or not
    pub enabled: bool,
    /// OSPF cost of this link
    pub cost: Option<u32>,
    pub(crate) link_id: usize,
}

/// IP Address
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IpAddr {
    /// Address
    pub addr: String,
    /// Network Mask
    pub mask: u32,
}

impl IpAddr {
    /// Create a new IP addr
    pub fn new(addr: impl Into<String>, mask: u32) -> Self {
        Self { addr: addr.into(), mask }
    }

    /// represent mask as xxx.xxx.xxx.xxx
    ///
    /// ```
    /// # use snowcap_runtime::physical_network::IpAddr;
    /// let addr = IpAddr::new("10.100.22.5", 17);
    /// assert_eq!(addr.repr_mask(), "255.255.128.0");
    /// ```
    pub fn repr_mask(&self) -> String {
        let x: i32 = self.mask as i32;
        format!(
            "{}.{}.{}.{}",
            Self::partial_mask(x),
            Self::partial_mask(x - 8),
            Self::partial_mask(x - 16),
            Self::partial_mask(x - 24),
        )
    }

    #[allow(dead_code)]
    fn partial_mask(x: i32) -> u8 {
        if x <= 0 {
            0
        } else if x == 1 {
            128
        } else if x == 2 {
            192
        } else if x == 3 {
            224
        } else if x == 4 {
            240
        } else if x == 5 {
            248
        } else if x == 6 {
            252
        } else if x == 7 {
            254
        } else {
            255
        }
    }

    /// Get the address, masked with the mask.
    ///
    /// ```
    /// # use snowcap_runtime::physical_network::IpAddr;
    /// let addr = IpAddr::new("10.100.22.5", 16);
    /// assert_eq!(addr.get_network(), IpAddr::new("10.100.0.0", 16));
    /// ```
    #[allow(dead_code)]
    pub fn get_network(&self) -> IpAddr {
        let mask = self.mask as i32;
        let mask = [
            Self::partial_mask(mask),
            Self::partial_mask(mask - 8),
            Self::partial_mask(mask - 16),
            Self::partial_mask(mask - 24),
        ];
        let parts = self.addr_parts();
        let masked: [u8; 4] =
            [mask[0] & parts[0], mask[1] & parts[1], mask[2] & parts[2], mask[3] & parts[3]];
        Self::new(format!("{}.{}.{}.{}", masked[0], masked[1], masked[2], masked[3]), self.mask)
    }

    /// Add one to the ip address (only to the last 8 bits)
    ///
    /// ```
    /// # use snowcap_runtime::physical_network::IpAddr;
    /// let addr = IpAddr::new("10.100.22.5", 17);
    /// assert_eq!(addr.add_one(), IpAddr::new("10.100.22.6", 17));
    /// ```
    pub fn add_one(&self) -> Self {
        let parts = self.addr_parts();
        Self::new(format!("{}.{}.{}.{}", parts[0], parts[1], parts[2], parts[3] + 1), self.mask)
    }

    /// Get the address parts of the IP address
    ///
    /// ```
    /// # use snowcap_runtime::physical_network::IpAddr;
    /// let addr = IpAddr::new("10.100.22.5", 17);
    /// assert_eq!(addr.addr_parts(), [10, 100, 22, 5]);
    /// ```
    pub fn addr_parts(&self) -> [u8; 4] {
        let parts = self.addr.split('.').collect::<Vec<_>>();
        assert!(parts.len() == 4);
        [
            parts[0].parse().unwrap(),
            parts[1].parse().unwrap(),
            parts[2].parse().unwrap(),
            parts[3].parse().unwrap(),
        ]
    }

    /// create an IP address from a string of the shape X.X.X.X/X
    ///
    /// ```
    /// # use snowcap_runtime::physical_network::IpAddr;
    /// let addr = IpAddr::try_from_str("10.100.22.5/17").unwrap();
    /// assert_eq!(addr, IpAddr::new("10.100.22.5", 17));
    /// ```
    pub fn try_from_str(s: impl AsRef<str>) -> Result<Self, Box<dyn Error>> {
        let parts = s.as_ref().split('/').collect::<Vec<_>>();
        let error: String = format!("Invalid IP string: {}", s.as_ref());
        if parts.len() != 2 {
            return Err(error.into());
        }
        // get all the four ip parts
        let ip_parts = parts[0].split('.').collect::<Vec<_>>();
        if ip_parts.len() != 4 {
            return Err(error.into());
        }
        let ip: [u8; 4] = [
            ip_parts[0].parse().map_err(|_| error.clone())?,
            ip_parts[1].parse().map_err(|_| error.clone())?,
            ip_parts[2].parse().map_err(|_| error.clone())?,
            ip_parts[3].parse().map_err(|_| error.clone())?,
        ];
        let mask: u32 = parts[1].parse().map_err(|_| error.clone())?;
        Ok(Self::new(format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]), mask))
    }
}

impl fmt::Display for IpAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.addr, self.mask)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn ip_addr() {
        assert_eq!(&IpAddr::new("", 4).repr_mask(), "240.0.0.0");
        assert_eq!(&IpAddr::new("", 11).repr_mask(), "255.224.0.0");
        assert_eq!(&IpAddr::new("", 15).repr_mask(), "255.254.0.0");
        assert_eq!(&IpAddr::new("", 24).repr_mask(), "255.255.255.0");
        assert_eq!(&IpAddr::new("", 25).repr_mask(), "255.255.255.128");
    }
}
