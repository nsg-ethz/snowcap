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

use crate::physical_network::IPAddr;

use log::*;
use regex::Regex;
use telnet::{Telnet, TelnetEvent};

use std::thread::sleep;

use std::error::Error;
use std::str;
use std::time::{Duration, SystemTime};

/// Virtual PC System (VPCS) Connection
#[derive(Debug)]
pub struct VpcsConnection {
    c: Telnet,
    prompt_re: Regex,
    logging: bool,
}

impl VpcsConnection {
    /// create a new connection to a device
    pub fn new(port: u16) -> Result<Self, Box<dyn Error>> {
        let prompt_re = Regex::new(r"(?m)[a-zA-Z0-9_\-.():~/]+> \z").unwrap();

        let mut c = Telnet::connect(("localhost", port), 2048)?;
        // receive all initial events
        while let Ok(event) = c.read_timeout(Duration::from_millis(1)) {
            match event {
                TelnetEvent::TimedOut => break,
                _ => {}
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
            if prompt_re.is_match(&result) {
                break;
            }
        }

        Ok(Self { c, prompt_re, logging: false })
    }

    /// Reconfigure a specific option in the router. if the configuration needs to happen inside a
    /// nested group, use the first element in the expr to navigate into this position, and the
    /// last to set the actual configuration.
    pub fn set_ip(&mut self, addr: &IPAddr, gateway: &IPAddr) -> Result<(), Box<dyn Error>> {
        // enter config mode
        self.send_wait(format!("ip {} {} {}\n", addr.addr, addr.mask, gateway.addr))?;
        Ok(())
    }

    fn send_wait(&mut self, data: impl AsRef<str>) -> Result<String, Box<dyn Error>> {
        self.c.write(data.as_ref().as_bytes())?;
        Ok(self.receive_until_prompt()?)
    }

    fn receive_until_prompt(&mut self) -> Result<String, Box<dyn Error>> {
        let mut result = String::new();
        let now = SystemTime::now();
        loop {
            let event = self.c.read_nonblocking()?;
            match event {
                telnet::TelnetEvent::NoData => {
                    if now.elapsed()? > Duration::from_secs(10) {
                        eprintln!("{}", result);
                        return Err("Took longer than 10 second to receive an answer!".into());
                    }
                    sleep(Duration::from_millis(10));
                }
                telnet::TelnetEvent::Data(d) => {
                    result.push_str(str::from_utf8(&d)?);
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

#[cfg(test)]
mod test {
    use super::*;
    use gns3::*;

    const TEST_PROJECT_NAME: &'static str = "VPCSConnTestProject";

    #[test]
    fn test_vpsc() {
        let mut server = match GNS3Server::new("localhost", 3080) {
            Ok(s) => s,
            Err(_) => return, // skip the test
        };
        delete_test_project(&mut server, TEST_PROJECT_NAME);
        let project = server.create_project(TEST_PROJECT_NAME).unwrap();

        // create two nodes
        let pc0 = server.create_vpcs("pc0").unwrap();
        let pc1 = server.create_vpcs("pc1").unwrap();

        server.create_link(&pc0, 0, &pc1, 0).unwrap();

        // start all nodes
        server.start_all_nodes().unwrap();

        let mut c0 = VpcsConnection::new(pc0.port).unwrap();
        let mut c1 = VpcsConnection::new(pc1.port).unwrap();
        c0.logging = true;
        c1.logging = true;

        c0.set_ip(&IPAddr::new("10.0.0.1", 24), &IPAddr::new("10.0.0.2", 24)).unwrap();
        c1.set_ip(&IPAddr::new("10.0.0.2", 24), &IPAddr::new("10.0.0.1", 24)).unwrap();

        server.delete_project(project.id).unwrap();
    }

    fn delete_test_project(server: &mut GNS3Server, name: &'static str) {
        if let Some(project) =
            server.get_projects().unwrap().into_iter().filter(|p| p.name == name).next()
        {
            server.delete_project(project.id).unwrap();
        }
    }
}
