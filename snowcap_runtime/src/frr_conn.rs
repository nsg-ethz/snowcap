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

//! Utilities for telnet interactions

use crate::physical_network::{IpAddr, PhysicalRouter};

use log::*;
use regex::Regex;
use telnet::{Telnet, TelnetEvent};

use std::thread::sleep;

use std::error::Error;
use std::str;
use std::time::{Duration, SystemTime};

const CMD_WAIT: u64 = 30;

/// # Connection to FRRouting
///
/// This struct can be used to communicate with an FRR instance, running inside GNS3, using telnet.
/// It can be used to configure and reconfigure the router, as well as to check its forwarding table
/// and the result of simple traceroute commands.
///
/// All commands are synchronous and blocking. Non-blocking implementation could be used
/// theoretically, but it was not worth the hassle. However, the blocking wait functions are all
/// implemented using a busy loop with a very large sleep timer in between. This still allows many
/// parallel connections to be established at the same time.
///
/// This struct does not implement `Copy`, `Sync` or `Send`, since it involves communicating with
/// a stream from the OS.
pub struct FrrConnection {
    c: Telnet,
    prompt_re: Regex,
    root_prompt_re: Regex,
    traceroute_re: Regex,
    logging: bool,
}

impl std::fmt::Debug for FrrConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FrrConnection")
    }
}

const SHELL_ENDING: [u8; 4] = [27, 91, 54, 110];

impl FrrConnection {
    /// create a new connection to a device
    pub fn new(port: u16) -> Result<Self, Box<dyn Error>> {
        let prompt_re = Regex::new(r"(?m)[a-zA-Z0-9_\-.():~/]+# \z").unwrap();
        let root_prompt_re = Regex::new(r"(?m)[a-zA-Z0-9_\-.]+# \z").unwrap();
        let traceroute_re =
            Regex::new(r"^ ?\d{1,2} +(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}) +\d+\.\d+ ms$").unwrap();

        let mut c = Telnet::connect(("localhost", port), 2048)?;
        // receive all initial events
        while let Ok(event) = c.read_timeout(Duration::from_millis(1)) {
            if matches!(event, TelnetEvent::TimedOut) {
                break;
            }
        }

        c.write("\n".as_bytes())?;

        let now = SystemTime::now();

        let mut result = String::new();
        loop {
            let event = c.read_nonblocking()?;
            match event {
                telnet::TelnetEvent::NoData => {
                    if now.elapsed()? > Duration::from_secs(100) {
                        error!("FRR is in an invalid state, or did not boot up! Port: {}", port);
                        return Err("FRR is in an invalid state, or did not boot up!".into());
                    }
                    sleep(Duration::from_millis(10));
                }
                telnet::TelnetEvent::Data(d) => result.push_str(str::from_utf8(&d)?),
                _ => {}
            }
            if root_prompt_re.is_match(&result) {
                break;
            }
        }

        let mut s = Self { c, prompt_re, root_prompt_re, traceroute_re, logging: false };

        s.send_wait("terminal length 0\n")?;

        Ok(s)
    }

    /// Reconfigure a specific option in the router. if the configuration needs to happen inside a
    /// nested group, use the first element in the expr to navigate into this position, and the
    /// last to set the actual configuration.
    pub fn reconfigure(&mut self, expr: Vec<impl AsRef<str>>) -> Result<(), Box<dyn Error>> {
        // check that we are in normal mode
        self.check_normal_mode()?;

        // enter config mode
        self.send_wait("config\n")?;

        for e in expr {
            self.config_expr(format!("{}\n", e.as_ref().trim()))?;
        }

        // go back until we are outside
        loop {
            let prompt = self.send_wait("exit\n")?;
            if self.root_prompt_re.is_match(&prompt) {
                break;
            }
        }

        Ok(())
    }

    /// Perform a traceroute, and get the result back, as a vector of IP addresses.
    pub fn traceroute(&mut self, target: &IpAddr) -> Result<Option<Vec<[u8; 4]>>, Box<dyn Error>> {
        // check that we are in normal mode
        self.check_normal_mode()?;

        // exit to shell
        self.send_wait("exit\n")?;
        // perform traceroute
        self.send(format!("traceroute -n -w 1 -q 1 {}\n", target.addr))?;

        let traceroute_result = self.receive_until_prompt(32)?;

        // go back to vtysh
        self.send_wait("vtysh\n")?;

        // parse result
        let mut lines = traceroute_result.lines();
        // skip the first line, because it contains the traceroute command
        lines.next();
        // skip the second line, because it shows information about the traceroute command
        if let Some(traceroute_info_line) = lines.next() {
            if !traceroute_info_line.starts_with("traceroute to") {
                return Err(format!(
                    "traceroute error: invalid info line: {}",
                    traceroute_info_line
                )
                .into());
            }
        } else {
            return Err("traceroute error: too few output lines".into());
        }

        let mut path = Vec::new();

        // parse all the remaining lines
        for line in lines {
            // if the line matches the prompt, break out of the loop
            if self.prompt_re.is_match(line) {
                break;
            }

            // check if the regex matched
            match self.traceroute_re.captures(line) {
                Some(caps) => {
                    let addr = caps.get(1).unwrap().as_str();
                    let mut parts = addr.split('.');
                    path.push([
                        parts.next().unwrap().parse::<u8>()?,
                        parts.next().unwrap().parse::<u8>()?,
                        parts.next().unwrap().parse::<u8>()?,
                        parts.next().unwrap().parse::<u8>()?,
                    ]);
                }
                None => {
                    // Seems like the traceroute failed
                    return Ok(None);
                }
            }
        }

        Ok(Some(path))
    }

    /// Returns the current routing table
    #[allow(clippy::needless_collect)] // needless collect necessary to reverse the list
    pub fn get_routing_table(&mut self) -> Result<RoutingTable, Box<dyn Error>> {
        let result = self.send_wait("show ip route\n")?;
        let lines = result.lines().collect::<Vec<_>>();
        let table_str =
            lines.into_iter().skip(7).rev().skip(1).rev().collect::<Vec<_>>().join("\n");
        RoutingTable::from(table_str)
    }

    /// Apply an entire config to the router
    pub fn initialize_config(&mut self, router: &PhysicalRouter) -> Result<(), Box<dyn Error>> {
        // check that we are in normal mode
        self.check_normal_mode()?;

        // set the hostname
        self.send_wait("exit\n")?;
        self.send_wait(format!("./set_hostname {}\n", router.name))?;

        // disable reverse path filtering
        self.send_wait("echo 0 > /proc/sys/net/ipv4/conf/all/rp_filter\n")?;
        for iface in router.gns_node.interfaces.iter() {
            self.send_wait(format!("echo 0 > /proc/sys/net/ipv4/conf/{}/rp_filter\n", iface.name))?;
        }

        self.send_wait("vtysh\n")?;

        self.send_wait("terminal length 0\n")?;

        // switch into config mode
        self.config_expr("config\n")?;

        // set hostname
        self.config_expr(format!("hostname {}\n", router.name))?;

        // configure loopback interface
        self.config_expr("interface lo\n")?;
        self.config_expr(format!("ip address {}/32\n", router.loopback_addr.addr))?;
        self.config_expr("exit\n")?;

        // confgure ospf
        if router.ifaces.iter().any(|i| i.cost.is_some()) {
            self.config_expr("router ospf 1\n")?;
            self.config_expr(format!("router-id {}\n", router.loopback_addr.addr))?;
            self.config_expr("redistribute connected\n")?;
            self.config_expr("exit\n")?;
        }

        // configure every interface
        for iface in &router.ifaces {
            // extract the cost from the configuration
            if iface.enabled {
                self.config_expr(format!("interface {}\n", iface.gns_interface.short_name))?;
                self.config_expr(format!("ip address {}\n", iface.iface_addr))?;
                if let Some(cost) = iface.cost.as_ref() {
                    self.config_expr("ip ospf 1 area 0\n")?;
                    self.config_expr(format!("ip ospf cost {}\n", cost))?;
                }
                self.config_expr("exit\n")?;
            }
        }

        // configure route maps
        for rm in &router.route_maps {
            self.config_expr(format!("route-map {} {} {}\n", rm.name, rm.state, rm.order))?;
            for (key, value) in rm.match_statements.iter() {
                self.config_expr(format!("match {} {}\n", key, value))?; // exit router bgp
            }
            for (key, value) in rm.set_statements.iter() {
                self.config_expr(format!("set {} {}\n", key, value))?; // exit router bgp
            }
            self.config_expr("exit\n")?; // exit router bgp

            // configure to allow per default
            self.config_expr(format!("route-map {} permit 65535\n", rm.name))?;
            self.config_expr("exit\n")?;
        }

        // configure BGP
        self.config_expr(format!("router bgp {}\n", router.as_id.0))?;
        self.config_expr(format!("bgp router-id {}\n", router.loopback_addr.addr))?;
        self.config_expr("bgp log-neighbor-changes\n")?;
        self.config_expr("bgp bestpath compare-routerid\n")?;
        self.config_expr("bgp route-reflector allow-outbound-policy\n")?;
        self.config_expr("neighbor internal peer-group\n")?;
        self.config_expr("neighbor external peer-group\n")?;
        for session in &router.bgp_sessions {
            let n_addr = session.neighbor_addr.addr.as_str();
            self.config_expr(format!(
                "neighbor {} remote-as {}\n",
                n_addr, session.neighbor_as_id.0
            ))?;
            self.config_expr(format!(
                "neighbor {} update-source {}\n",
                n_addr,
                if session.internal_session {
                    "lo"
                } else {
                    router
                        .ifaces
                        .iter()
                        .filter(|i| i.neighbor == session.neighbor)
                        .map(|i| i.gns_interface.short_name.as_str())
                        .next()
                        .expect("No direct connection with the neighbor")
                }
            ))?;
            self.config_expr(format!(
                "neighbor {} peer-group {}\n",
                n_addr,
                if session.internal_session { "internal" } else { "external" }
            ))?;
            if session.is_rr_client {
                self.config_expr(format!("neighbor {} route-reflector-client\n", n_addr))?;
            }
        }
        // enable ipv4 communication
        self.config_expr("address-family ipv4\n")?;
        if let Some(prefix) = router.advertise_route.as_ref() {
            self.config_expr(format!("network {}\n", prefix))?;
        }
        for rm in &router.route_maps {
            self.config_expr(format!(
                "neighbor internal route-map {} {}\n",
                rm.name, rm.direction
            ))?;
            self.config_expr(format!(
                "neighbor external route-map {} {}\n",
                rm.name, rm.direction
            ))?;
        }
        self.config_expr("exit\n")?; // exit address-family
        self.config_expr("exit\n")?; // exit router bgp

        // configure static routes
        for sr in &router.static_routes {
            self.config_expr(format!("ip route {} {}\n", sr.addr, sr.next_hop))?;
        }

        self.config_expr("exit\n")?; // exit config mode
        Ok(())
    }

    fn check_normal_mode(&mut self) -> Result<(), Box<dyn Error>> {
        self.send("\n")?;
        let prompt = self.receive_until_prompt(CMD_WAIT)?;
        if !self.root_prompt_re.is_match(&prompt) {
            Err("Router is in an invalid state!".into())
        } else {
            Ok(())
        }
    }

    fn config_expr(&mut self, data: impl AsRef<str>) -> Result<(), Box<dyn Error>> {
        let result = self.send_wait(data)?;
        if result.lines().count() != 2 {
            Err(format!("Error while applying configuration: \n{}", result).into())
        } else {
            Ok(())
        }
    }

    fn send_wait(&mut self, data: impl AsRef<str>) -> Result<String, Box<dyn Error>> {
        self.c.write(data.as_ref().as_bytes())?;
        self.receive_until_prompt(CMD_WAIT)
    }

    fn send(&mut self, data: impl AsRef<str>) -> Result<(), Box<dyn Error>> {
        self.c.write(data.as_ref().as_bytes())?;
        Ok(())
    }

    fn receive_until_prompt(&mut self, wait_secs: u64) -> Result<String, Box<dyn Error>> {
        let mut result = String::new();
        let now = SystemTime::now();
        loop {
            let event = self.c.read_nonblocking()?;
            match event {
                telnet::TelnetEvent::NoData => {
                    if now.elapsed()? > Duration::from_secs(wait_secs) {
                        eprintln!("{}", result);
                        return Err(format!(
                            "Took longer than {} second to receive an answer!",
                            wait_secs
                        )
                        .into());
                    }
                    sleep(Duration::from_millis(10));
                }
                telnet::TelnetEvent::Data(d) => {
                    result.push_str(str::from_utf8(&d)?);
                    let bytes = &result.as_str().as_bytes();
                    let num_bytes = bytes.len();
                    if num_bytes >= 4 && bytes[num_bytes - 4..] == SHELL_ENDING {
                        result.pop();
                        result.pop();
                        result.pop();
                        result.pop();
                    }
                    // first, check if the bytes end with some wierd ending
                    if self.prompt_re.is_match(&result) {
                        if self.logging {
                            eprintln!("{}", result);
                        }
                        return Ok(result.replace("\r\n", "\n"));
                    }
                }
                _ => {}
            }
        }
    }
}

/// Routing table, as a vector of routing table entries. The struct includes a parser to build such
/// a routing table from the output of the FRR command `show ip route`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoutingTable {
    /// Every entry from the routing table
    pub entries: Vec<RoutingTableEntry>,
}

impl RoutingTable {
    /// Parses the output of `show ip route`. This function requires you to pass in only the lines
    /// that actually contain the routing entries. If the input contains empty lines, or lines
    pub fn from(s: impl AsRef<str>) -> Result<Self, Box<dyn Error>> {
        let mut lines = s.as_ref().lines().peekable();
        let mut entries = Vec::new();
        while let Some(l) = lines.next() {
            let l = l.trim();
            if l.starts_with('*') || l.starts_with("via") {
                continue;
            }
            if l.starts_with('B') {
                entries.push(if let Some(next_line) = lines.peek() {
                    let next_line = next_line.trim();
                    if next_line.starts_with('*') || next_line.starts_with("via") {
                        let next_line = lines.next().unwrap().trim();
                        let entry_str = format!("{} {}", l, next_line);
                        RoutingTableEntry::from(&entry_str).map_err(|p| {
                            format!("Cannot parse entry at pos {}: {}", p, entry_str)
                        })?
                    } else {
                        RoutingTableEntry::from(l)
                            .map_err(|p| format!("Cannot parse entry at pos {}: {}", p, l))?
                    }
                } else {
                    RoutingTableEntry::from(l)
                        .map_err(|p| format!("Cannot parse entry at pos {}: {}", p, l))?
                })
            } else {
                entries.push(
                    RoutingTableEntry::from(l)
                        .map_err(|p| format!("Cannot parse entry at pos {}; {}", p, l))?,
                );
            }
        }
        Ok(Self { entries })
    }
}

/// RoutingTableEntry, including a parser that parses routing tables, received from the FRR command
/// `show ip route`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoutingTableEntry {
    /// Service from which the route was lerned
    pub origin: RteOrigin,
    /// Instance ID of the service (if multiple instances of the same service are running)
    pub instance_id: u32,
    /// If this route is selected or not
    pub selected: bool,
    /// If this route is written into the FIB table
    pub fib_route: bool,
    /// If this route is in the queue to be processed
    pub queued: bool,
    /// If this route is rejected
    pub rejected: bool,
    /// Network to which the route belongs
    pub network: IpAddr,
    /// Administrative Distance (used to choose which route to select, if multiple are known from
    /// different services).
    pub administrative_distance: u32,
    /// Metric of the route (general attribute for all services)
    pub metric: u32,
    /// Address of the next hop, as a string. This is `None` for routes directly connected, or for
    /// static routes.
    pub next_hop_addr: Option<String>,
    /// Interface of the next hop, as a string
    pub next_hop_iface: String,
    /// Address of the neighbor, from who this address was learned. This is only `Some` for BGP
    /// routes.
    pub learned_from: Option<String>,
}

impl RoutingTableEntry {
    /// Parse a single routing table entry to extract all information. The entry must be on one
    /// single line. Only FRR routing table entries will work! For BGP routes, also use the second
    /// line, if it exists.
    #[allow(clippy::unnecessary_cast)] // cast is actually necessary
    pub fn from(s: impl AsRef<str>) -> Result<Self, usize> {
        let mut s = s.as_ref();
        let tot_len = s.len();

        let mut entry = RoutingTableEntry {
            origin: RteOrigin::Connected,
            instance_id: 0,
            selected: false,
            fib_route: false,
            queued: false,
            rejected: false,
            network: IpAddr::new("0.0.0.0", 0),
            administrative_distance: 0,
            metric: 0,
            next_hop_addr: None,
            next_hop_iface: String::new(),
            learned_from: None,
        };

        // parse the origin
        entry.origin = RteOrigin::from(s).map_err(|_| 0 as usize)?;
        s = &s[1..];

        // parse instance_id
        let mut move_by = 0;
        let mut chars = s.chars();
        if chars.next() == Some('[') {
            move_by += 1;
            for c in chars {
                move_by += 1;
                match c {
                    ']' => break,
                    c if c.is_numeric() => {
                        let x: u32 = c.to_digit(10).unwrap();
                        entry.instance_id = (entry.instance_id * 10) + x;
                    }
                    _ => return Err(tot_len - s.len()),
                }
            }
        };
        s = &s[move_by..];

        // parse the selected route
        if s.starts_with('>') {
            entry.selected = true;
            s = &s[1..];
        }

        // parse the fib route
        if s.starts_with('*') {
            entry.fib_route = true;
            s = &s[1..];
        }

        // parse the queued
        if s.starts_with('q') {
            entry.queued = true;
            s = &s[1..];
        }

        // parse the rejected
        if s.starts_with('r') {
            entry.rejected = true;
            s = &s[1..];
        }

        // move by empty characters over
        s = s.trim_start();

        // get the part with the ip address that comes next (until the next space)
        let ip_len = s.chars().position(|c| c.is_whitespace()).ok_or(tot_len - s.len())?;
        entry.network = IpAddr::try_from_str(&s[..ip_len]).map_err(|_| tot_len - s.len())?;
        s = &s[ip_len..];
        s = s.trim_start();

        // parse until interface, time
        if entry.origin == RteOrigin::Connected {
            // we expect that we now have the string is directly connected,
            if !s.starts_with("is directly connected, ") {
                return Err(tot_len - s.len());
            }
            s = &s["is directly connected, ".len()..];
        } else {
            // parse the administrative distance and the weight
            if !s.starts_with('[') {
                return Err(tot_len - s.len());
            }
            s = &s[1..];
            let ad_len = s.chars().position(|c| c == '/').ok_or(tot_len - s.len())?;
            entry.administrative_distance =
                (&s[0..ad_len]).parse().map_err(|_| tot_len - s.len())?;
            s = &s[(ad_len + 1)..];
            let metric_len = s.chars().position(|c| c == ']').ok_or(tot_len - s.len())?;
            entry.metric = (&s[0..metric_len]).parse().map_err(|_| tot_len - s.len())?;
            s = &s[(metric_len + 1)..];
            s = s.trim_start();

            if s.starts_with("is directly connected, ") {
                s = &s["is directly connected, ".len()..];
            } else {
                // parse until the via argument
                // we expect that we now have the string is directly connected,
                if !s.starts_with("via ") {
                    return Err(tot_len - s.len());
                }
                s = &s["via ".len()..];
                // get the part with the ip address that comes next (until the next space)
                let ip_len =
                    s.chars().position(|c| c == ',' || c == ' ').ok_or(tot_len - s.len())?;
                entry.next_hop_addr = Some(String::from(&s[..ip_len]));
                s = &s[(ip_len + 1)..];
                s = s.trim_start();

                // check if the route is learned recursively
                if s.starts_with("(recursive), ") {
                    s = &s["(recursive), ".len()..];
                    s = s.trim_start_matches(|c: char| {
                        c.is_numeric() || c.is_whitespace() || c == ':'
                    });
                    // again, check if the star is present now
                    if s.starts_with('*') {
                        entry.fib_route = true;
                        s = &s[1..];
                    }
                    // trim all whitespace characters, again
                    s = s.trim_start();

                    // now, we expect a via
                    if !s.starts_with("via ") {
                        return Err(tot_len - s.len());
                    }
                    s = &s["via ".len()..];

                    // get the part with the ip address that comes next (until the next space)
                    let ip_len = s.chars().position(|c| c == ',').ok_or(tot_len - s.len())?;
                    entry.learned_from = Some(String::from(&s[..ip_len]));
                    s = &s[(ip_len + 1)..];
                    s = s.trim_start();
                }
            }
        }

        // parse the via
        let iface_len = s.chars().position(|c| c == ',').ok_or(tot_len - s.len())?;
        entry.next_hop_iface = String::from(&s[..iface_len]);

        Ok(entry)
    }
}

/// FRR Route Origin, which FRR service has written the route into the table
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RteOrigin {
    /// Route, learned form the kernel
    Kernel,
    /// Route learned, because this prefix is directly connected to the router.
    Connected,
    /// Static route
    Static,
    /// Open Shortest-Path First (OSPF)
    Ospf,
    /// Routing Information Protocol (RIP)
    Rip,
    /// Intermediate Systems --- Intermediate Systems (IS-IS)
    IsIs,
    /// Border Gateway Protocol (BGP)
    Bgp,
    /// Enhanced Interior Gateway Routing Protocol (EIGRIP)
    Eigrip,
    /// Next Hop Resolution Protocol (NHRP)
    Nhrp,
    /// This origin is unknown
    Table,
    /// Network Virtualization Overlays (NVC)
    Vnc,
    /// Network Virtualization Overlays (NVC), direct
    VncDirect,
    /// Babel (Interior gateway protocol for mesh networks)
    Babel,
    /// used for testing purposes
    Sharp,
    /// Policy Based Routing (PBR)
    Pbr,
    /// OpenFabric (similar to IS-IS)
    OpenFabric,
}

impl RteOrigin {
    /// Parse the origin type, based on the first character of the route.
    pub fn from(s: impl AsRef<str>) -> Result<Self, Box<dyn Error>> {
        match s.as_ref().chars().next() {
            Some('K') => Ok(Self::Kernel),
            Some('C') => Ok(Self::Connected),
            Some('S') => Ok(Self::Static),
            Some('R') => Ok(Self::Rip),
            Some('O') => Ok(Self::Ospf),
            Some('I') => Ok(Self::IsIs),
            Some('B') => Ok(Self::Bgp),
            Some('E') => Ok(Self::Eigrip),
            Some('N') => Ok(Self::Nhrp),
            Some('T') => Ok(Self::Table),
            Some('v') => Ok(Self::Vnc),
            Some('V') => Ok(Self::VncDirect),
            Some('A') => Ok(Self::Babel),
            Some('D') => Ok(Self::Sharp),
            Some('F') => Ok(Self::Pbr),
            Some('f') => Ok(Self::OpenFabric),
            Some(c) => Err(format!("Unknown Route type character: {}", c).into()),
            None => Err("Route Type character received an empty string!".into()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::physical_network::{BgpSessionInfo, IfaceInfo, IpAddr, StaticRouteInfo};
    use gns3::*;
    use snowcap::netsim::*;

    const TEST_PROJECT_NAME: &str = "FrrConnTestProject";

    #[test]
    fn ospf_routing_table() {
        let ospf_route_str = "O[1]>* 10.1.6.0/24 [110/4] via 10.1.5.2, eth1, 00:01:54";
        let ospf_route = match RoutingTableEntry::from(ospf_route_str) {
            Ok(r) => r,
            Err(pos) => panic!(
                "Error\n{}\n{}^\n",
                ospf_route_str,
                std::iter::repeat(' ').take(pos).collect::<String>()
            ),
        };
        assert_eq!(
            ospf_route,
            RoutingTableEntry {
                origin: RteOrigin::Ospf,
                instance_id: 1,
                selected: true,
                fib_route: true,
                queued: false,
                rejected: false,
                network: IpAddr::new("10.1.6.0", 24),
                administrative_distance: 110,
                metric: 4,
                next_hop_addr: Some("10.1.5.2".into()),
                next_hop_iface: "eth1".into(),
                learned_from: None,
            }
        );
    }

    #[test]
    fn connected_routing_table() {
        let route_str = "C>* 10.1.5.0/24 is directly connected, eth1, 00:02:41";
        let route = match RoutingTableEntry::from(route_str) {
            Ok(r) => r,
            Err(pos) => panic!(
                "Error\n{}\n{}^\n",
                route_str,
                std::iter::repeat(' ').take(pos).collect::<String>()
            ),
        };
        assert_eq!(
            route,
            RoutingTableEntry {
                origin: RteOrigin::Connected,
                instance_id: 0,
                selected: true,
                fib_route: true,
                queued: false,
                rejected: false,
                network: IpAddr::new("10.1.5.0", 24),
                administrative_distance: 0,
                metric: 0,
                next_hop_addr: None,
                next_hop_iface: "eth1".into(),
                learned_from: None,
            }
        );
    }

    #[test]
    fn bgp_routing_table() {
        let route_str = "B>  109.0.0.0/8 [200/0] via 10.1.12.2 (recursive), 00:01:42 *                       via 10.1.5.2, eth1, 00:01:42";
        let route = match RoutingTableEntry::from(route_str) {
            Ok(r) => r,
            Err(pos) => panic!(
                "Error\n{}\n{}^\n",
                route_str,
                std::iter::repeat(' ').take(pos).collect::<String>()
            ),
        };
        assert_eq!(
            route,
            RoutingTableEntry {
                origin: RteOrigin::Bgp,
                instance_id: 0,
                selected: true,
                fib_route: true,
                queued: false,
                rejected: false,
                network: IpAddr::new("109.0.0.0", 8),
                administrative_distance: 200,
                metric: 0,
                next_hop_addr: Some("10.1.12.2".into()),
                next_hop_iface: "eth1".into(),
                learned_from: Some("10.1.5.2".into()),
            }
        );
    }

    #[test]
    fn multiline_routing_table() {
        let table_str = "\
B>  109.0.0.0/8 [200/0] via 10.1.12.2 (recursive), 00:01:42
 *                       via 10.1.5.2, eth1, 00:01:42
B>  109.0.0.0/8 [200/0] via 10.1.12.2 (recursive), 00:01:42 *                       via 10.1.5.2, eth1, 00:01:42
B   109.0.0.0/8 [20/0] via 10.1.12.2, eth1, 00:01:42
";
        let table = RoutingTable::from(table_str).unwrap();
        assert_eq!(
            table.entries,
            vec![
                RoutingTableEntry {
                    origin: RteOrigin::Bgp,
                    instance_id: 0,
                    selected: true,
                    fib_route: true,
                    queued: false,
                    rejected: false,
                    network: IpAddr::new("109.0.0.0", 8),
                    administrative_distance: 200,
                    metric: 0,
                    next_hop_addr: Some("10.1.12.2".into()),
                    next_hop_iface: "eth1".into(),
                    learned_from: Some("10.1.5.2".into()),
                },
                RoutingTableEntry {
                    origin: RteOrigin::Bgp,
                    instance_id: 0,
                    selected: true,
                    fib_route: true,
                    queued: false,
                    rejected: false,
                    network: IpAddr::new("109.0.0.0", 8),
                    administrative_distance: 200,
                    metric: 0,
                    next_hop_addr: Some("10.1.12.2".into()),
                    next_hop_iface: "eth1".into(),
                    learned_from: Some("10.1.5.2".into()),
                },
                RoutingTableEntry {
                    origin: RteOrigin::Bgp,
                    instance_id: 0,
                    selected: false,
                    fib_route: false,
                    queued: false,
                    rejected: false,
                    network: IpAddr::new("109.0.0.0", 8),
                    administrative_distance: 20,
                    metric: 0,
                    next_hop_addr: Some("10.1.12.2".into()),
                    next_hop_iface: "eth1".into(),
                    learned_from: None,
                },
            ]
        )
    }

    #[test]
    fn test() {
        let mut server = match GNS3Server::new("localhost", 3080) {
            Ok(s) => s,
            Err(_) => return, // skip the test
        };
        delete_test_project(&mut server, TEST_PROJECT_NAME);
        let project = server.create_project(TEST_PROJECT_NAME).unwrap();

        // get the FRR template
        let frr =
            server.get_templates().unwrap().into_iter().find(|t| t.name == "FRR 7.3.1").unwrap();

        // create two nodes
        let node = server.create_node("node", &frr.id).unwrap();
        let n1 = server.create_node("neighbor1", &frr.id).unwrap();
        let n2 = server.create_node("neighbor2", &frr.id).unwrap();
        let n3 = server.create_node("neighbor3", &frr.id).unwrap();

        server.create_link(&node, 0, &n1, 0).unwrap();
        server.create_link(&node, 1, &n2, 0).unwrap();
        server.create_link(&node, 2, &n3, 0).unwrap();

        // start all nodes
        server.start_all_nodes().unwrap();

        let mut c = FrrConnection::new(node.port).unwrap();
        c.logging = true;

        // apply example config
        let router = PhysicalRouter {
            router_id: 0.into(),
            name: String::from("node"),
            gns_node: node,
            loopback_addr: IpAddr::new("10.0.0.1", 24),
            as_id: AsId(65001),
            is_internal: true,
            ifaces: vec![
                IfaceInfo {
                    neighbor: 1.into(),
                    neighbor_addr: IpAddr::new("10.1.0.2", 24),
                    iface_addr: IpAddr::new("10.1.0.1", 24),
                    gns_interface: GNS3Interface {
                        adapter_number: 0,
                        port_number: 0,
                        name: "eth0".to_string(),
                        short_name: "eth0".to_string(),
                        link_type: "ethernet".to_string(),
                    },
                    enabled: true,
                    cost: Some(10),
                    link_id: 0,
                },
                IfaceInfo {
                    neighbor: 2.into(),
                    neighbor_addr: IpAddr::new("10.2.0.2", 24),
                    iface_addr: IpAddr::new("10.2.0.1", 24),
                    gns_interface: GNS3Interface {
                        adapter_number: 1,
                        port_number: 0,
                        name: "eth1".to_string(),
                        short_name: "eth1".to_string(),
                        link_type: "ethernet".to_string(),
                    },
                    enabled: true,
                    cost: None,
                    link_id: 0,
                },
                IfaceInfo {
                    neighbor: 3.into(),
                    neighbor_addr: IpAddr::new("10.3.0.2", 24),
                    iface_addr: IpAddr::new("10.3.0.1", 24),
                    gns_interface: GNS3Interface {
                        adapter_number: 2,
                        port_number: 0,
                        name: "eth2".to_string(),
                        short_name: "eth2".to_string(),
                        link_type: "ethernet".to_string(),
                    },
                    enabled: false,
                    cost: None,
                    link_id: 0,
                },
            ],
            bgp_sessions: vec![
                BgpSessionInfo {
                    neighbor: 1.into(),
                    neighbor_addr: IpAddr::new("10.0.0.2", 24),
                    neighbor_as_id: AsId(65001),
                    is_rr_client: true,
                    internal_session: true,
                },
                BgpSessionInfo {
                    neighbor: 2.into(),
                    neighbor_addr: IpAddr::new("10.0.0.3", 24),
                    neighbor_as_id: AsId(65002),
                    is_rr_client: false,
                    internal_session: false,
                },
            ],
            route_maps: vec![],
            static_routes: vec![StaticRouteInfo {
                addr: IpAddr::new("99.0.1.0", 24),
                next_hop: String::from("eth1"),
            }],
            advertise_route: Some(IpAddr::new(String::from("10.0.0.0"), 24)),
        };

        c.initialize_config(&router).unwrap();

        let config = get_config(&mut c).unwrap();

        assert!(config.contains("hostname node"));
        assert_eq!(
            config,
            "!
frr version 7.3.1
frr defaults traditional
hostname frr
hostname node
service integrated-vtysh-config
!
ip route 99.0.1.0/24 eth1
!
interface eth0
 ip address 10.1.0.1/24
 ip ospf 1 area 0
 ip ospf cost 10
!
interface eth1
 ip address 10.2.0.1/24
!
interface lo
 ip address 10.0.0.1/32
!
router bgp 65001
 bgp router-id 10.0.0.1
 bgp log-neighbor-changes
 bgp route-reflector allow-outbound-policy
 bgp bestpath compare-routerid
 neighbor external peer-group
 neighbor internal peer-group
 neighbor 10.0.0.3 remote-as 65002
 neighbor 10.0.0.3 peer-group external
 neighbor 10.0.0.3 update-source eth1
 neighbor 10.0.0.2 remote-as 65001
 neighbor 10.0.0.2 peer-group internal
 neighbor 10.0.0.2 update-source lo
 !
 address-family ipv4 unicast
  network 10.0.0.0/24
  neighbor 10.0.0.2 route-reflector-client
 exit-address-family
!
router ospf 1
 ospf router-id 10.0.0.1
 redistribute connected
!
line vty
!"
        );

        server.delete_project(project.id).unwrap();
    }

    fn delete_test_project(server: &mut GNS3Server, name: &'static str) {
        if let Some(project) = server.get_projects().unwrap().into_iter().find(|p| p.name == name) {
            server.delete_project(project.id).unwrap();
        }
    }

    /// Stores the running config as startup config and returns the entire configuration file
    fn get_config(c: &mut FrrConnection) -> Result<String, Box<dyn std::error::Error>> {
        let result = c.send_wait("write terminal\n")?;
        Ok(result
            .lines()
            .collect::<Vec<_>>()
            .into_iter()
            .skip(4)
            .rev()
            .skip(2)
            .rev()
            .collect::<Vec<_>>()
            .join("\n"))
    }
}
