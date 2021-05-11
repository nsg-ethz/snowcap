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

//! Utilities for telnet interactions to a Python Docker Container

use crate::physical_network::IpAddr;

use log::*;
use regex::Regex;
use telnet::{Telnet, TelnetEvent};

use std::thread::sleep;

use std::error::Error;
use std::str;
use std::time::{Duration, SystemTime};

/// Connection to a docker container, running inside GNS3, to which we can connect via telnet, and
/// which contains a python interpreter. This struct can be used to write a python program to the
/// client, and execute it.
pub struct PythonConnection {
    c: Telnet,
    prompt_re: Regex,
    logging: bool,
}

impl std::fmt::Debug for PythonConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PythonConnection")
    }
}

impl PythonConnection {
    /// create a new connection to a device
    pub fn new(port: u16) -> Result<Self, Box<dyn Error>> {
        let prompt_re = Regex::new(r"root@[a-zA-Z0-9_\-.,]+:[a-zA-Z0-9_\-./~()]+# $").unwrap();

        let mut c = Telnet::connect(("localhost", port), 2048)?;
        // receive all initial events
        while let Ok(event) = c.read_timeout(Duration::from_millis(1)) {
            if matches!(event, TelnetEvent::TimedOut) {
                break;
            }
        }

        c.write("\n\n".as_bytes())?;

        let now = SystemTime::now();

        let mut result = String::new();
        loop {
            let event = c.read_nonblocking()?;
            match event {
                telnet::TelnetEvent::NoData => {
                    if now.elapsed()? > Duration::from_secs(20) {
                        error!("Could not connect to Python container at port: {}", port);
                        return Err("Could not connect to python container".into());
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
    ///
    /// After this, the node needs to be restarted!
    pub fn configure(&mut self, addr: &IpAddr, gateway: &IpAddr) -> Result<(), Box<dyn Error>> {
        // enter config mode
        let cfg = "/etc/network/interfaces";
        self.send_wait(format!("echo \"\" >> {}\n\n", cfg))?;
        self.send_wait(format!("echo \"auto eth0\" >> {}\n\n", cfg))?;
        self.send_wait(format!("echo \"iface eth0 inet static\" >> {}\n\n", cfg))?;
        self.send_wait(format!("echo \"        address {}\" >> {}\n\n", addr.addr, cfg))?;
        self.send_wait(format!("echo \"        netmask {}\" >> {}\n\n", addr.repr_mask(), cfg))?;
        self.send_wait(format!("echo \"        gateway {}\" >> {}\n\n", gateway.addr, cfg))?;
        Ok(())
    }

    /// Create a file on the client. Any existing file with the same name will be overwritten!
    pub fn create_file(
        &mut self,
        filename: impl AsRef<str>,
        content: impl AsRef<str>,
    ) -> Result<(), Box<dyn Error>> {
        let filename = filename.as_ref();
        let content = content.as_ref();
        // create the folder
        let folder_parts = filename.split('/').collect::<Vec<_>>();
        let num_folder_parts = folder_parts.len();
        let foldername = folder_parts[..num_folder_parts - 1].join("/");
        if !foldername.is_empty() {
            self.send_wait(format!("mkdir -p {}\n\n", foldername))?;
        }

        // create an empty file
        self.send_wait(format!("echo \"\" > {}\n\n", filename))?;

        for line in content.lines() {
            self.send_wait(format!("echo \"{}\" >> {}\n\n", line, filename))?;
        }

        Ok(())
    }

    /// Execute a command, without waiting for the response
    pub fn run_program(&mut self, command: impl AsRef<str>) -> Result<(), Box<dyn Error>> {
        let cmd = format!("{}\n", command.as_ref());
        self.c.write(cmd.as_str().as_bytes())?;
        Ok(())
    }

    /// Send ctrl-c to the client, stopping any currently running command
    pub fn ctrl_c(&mut self) -> Result<(), Box<dyn Error>> {
        self.c.write(&[0x03])?;
        self.send_wait("\n")?;
        Ok(())
    }

    fn send_wait(&mut self, data: impl AsRef<str>) -> Result<String, Box<dyn Error>> {
        self.c.write(data.as_ref().as_bytes())?;
        self.receive_until_prompt()
    }

    fn receive_until_prompt(&mut self) -> Result<String, Box<dyn Error>> {
        let mut result = String::new();
        let now = SystemTime::now();
        loop {
            let event = self.c.read_nonblocking()?;
            match event {
                telnet::TelnetEvent::NoData => {
                    if now.elapsed()? > Duration::from_secs(10) {
                        if self.logging {
                            eprintln!("\n");
                        }
                        return Err("Took longer than 10 second to receive an answer!".into());
                    }
                    sleep(Duration::from_millis(10));
                }
                telnet::TelnetEvent::Data(d) => {
                    let s = str::from_utf8(&d)?;
                    if self.logging {
                        eprint!("{}", s);
                    }
                    result.push_str(s);
                    // first, check if the bytes end with some wierd ending
                    if self.prompt_re.is_match(&result) {
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

    const TEST_PROJECT_NAME: &str = "PythonConnTestProject";

    #[test]
    fn python_init() {
        let mut server = match GNS3Server::new("localhost", 3080) {
            Ok(s) => s,
            Err(_) => return, // skip the test
        };
        delete_test_project(&mut server, TEST_PROJECT_NAME);
        let project = server.create_project(TEST_PROJECT_NAME).unwrap();

        // get the python template
        let python_template = server
            .get_templates()
            .unwrap()
            .into_iter()
            .find(|t| t.name == "Python, Go, Perl, PHP")
            .unwrap();

        // create two nodes
        let client = server.create_node("client", python_template.id).unwrap();

        // start all nodes
        server.start_all_nodes().unwrap();

        let mut c = PythonConnection::new(client.port).unwrap();
        c.logging = true;

        c.configure(&IpAddr::new("10.0.0.1", 24), &IpAddr::new("10.0.0.2", 24)).unwrap();

        server.stop_all_nodes().unwrap();
        server.start_all_nodes().unwrap();

        let mut c = PythonConnection::new(client.port).unwrap();
        c.logging = true;

        let ip_response = c.send_wait("ifconfig\n").unwrap();
        assert!(ip_response.contains("inet addr:10.0.0.1"));
        assert!(ip_response.contains("Mask:255.255.255.0"));

        server.delete_project(project.id).unwrap();
    }

    #[test]
    fn python_create_file() {
        let mut server = match GNS3Server::new("localhost", 3080) {
            Ok(s) => s,
            Err(_) => return, // skip the test
        };
        delete_test_project(&mut server, TEST_PROJECT_NAME);
        let project = server.create_project(TEST_PROJECT_NAME).unwrap();

        // get the python template
        let python_template = server
            .get_templates()
            .unwrap()
            .into_iter()
            .find(|t| t.name == "Python, Go, Perl, PHP")
            .unwrap();

        // create two nodes
        let client = server.create_node("client", python_template.id).unwrap();

        // start all nodes
        server.start_all_nodes().unwrap();

        let mut c = PythonConnection::new(client.port).unwrap();
        c.logging = true;

        c.create_file(
            "/root/test/file/file.txt",
            "This is the new file
It has multiple lines
    send(a, 'b')",
        )
        .unwrap();

        let response = c.send_wait("cat /root/test/file/file.txt\n\n").unwrap();
        assert!(response.contains(
            "This is the new file
It has multiple lines
    send(a, 'b')"
        ));

        server.delete_project(project.id).unwrap();
    }

    #[test]
    fn python_run_python() {
        let mut server = match GNS3Server::new("localhost", 3080) {
            Ok(s) => s,
            Err(_) => return, // skip the test
        };
        delete_test_project(&mut server, TEST_PROJECT_NAME);
        let project = server.create_project(TEST_PROJECT_NAME).unwrap();

        // get the python template
        let python_template = server
            .get_templates()
            .unwrap()
            .into_iter()
            .find(|t| t.name == "Python, Go, Perl, PHP")
            .unwrap();

        // create two nodes
        let client = server.create_node("client", python_template.id).unwrap();

        // start all nodes
        server.start_all_nodes().unwrap();

        let mut c = PythonConnection::new(client.port).unwrap();
        c.logging = true;

        c.create_file(
            "/root/test.py",
            "
import time
with open('result', 'w') as f:
    f.write('something')
while true:
    time.sleep(1)
",
        )
        .unwrap();

        c.run_program("python /root/test.py").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        c.ctrl_c().unwrap();

        let response = c.send_wait("cat /root/result\n\n").unwrap();
        assert!(response.contains("something"));

        server.delete_project(project.id).unwrap();
    }

    fn delete_test_project(server: &mut GNS3Server, name: &'static str) {
        if let Some(project) = server.get_projects().unwrap().into_iter().find(|p| p.name == name) {
            server.delete_project(project.id).unwrap();
        }
    }
}
