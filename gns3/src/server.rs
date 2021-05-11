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

//! # GNS3 Server

use crate::types::*;
use crate::{Error, Result};

use isahc::prelude::*;
use regex::Regex;

/// # GNS3 Server Handle
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, PartialEq, Clone)]
pub struct GNS3Server {
    address: String,
    version: String,
    project: Option<String>,
}

impl GNS3Server {
    /// Create a new instance of a server handler
    pub fn new(address: impl AsRef<str>, port: u32) -> Result<Self> {
        let address = format!("http://{}:{}", address.as_ref(), port);
        let version_addr = format!("{}/v2/version", address);
        let v: GNS3ResponseVersion = serde_json::from_str(&isahc::get(&version_addr)?.text()?)?;
        Ok(Self {
            address,
            version: v.version,
            project: None,
        })
    }

    /// Get the version
    pub fn version(&self) -> &str {
        self.version.as_ref()
    }

    /// Returns all project informations
    pub fn get_projects(&self) -> Result<Vec<GNS3Project>> {
        Ok(serde_json::from_str(&self.request_get("projects")?)?)
    }

    /// Opens a project, and returns the new information
    pub fn open_project(&mut self, project_id: impl AsRef<str>) -> Result<GNS3Project> {
        // first, get the status to check if the project is already opened
        match {
            let project_info: GNS3Project = serde_json::from_str(
                &self.request_get(format!("projects/{}", project_id.as_ref()))?,
            )?;
            if project_info.status == GNS3ProjectStatus::Closed {
                Ok(serde_json::from_str(&self.request_post(
                    format!("projects/{}/open", project_id.as_ref()),
                    String::from("{}"),
                )?)?)
            } else {
                Ok(project_info)
            }
        } {
            Ok(project_info) => {
                self.project = Some(project_info.id.clone());
                Ok(project_info)
            }
            Err(e) => {
                self.project = None;
                Err(e)
            }
        }
    }

    /// Opens a project, and returns the new information
    pub fn close_project(&mut self) -> Result<GNS3Project> {
        let project_id: String = self.project.take().ok_or(Error::NoProjectOpened)?;
        // first, get the status to check if the project is already opened
        let project_info: GNS3Project =
            serde_json::from_str(&self.request_get(format!("projects/{}", project_id))?)?;
        if project_info.status == GNS3ProjectStatus::Opened {
            self.request_post(format!("projects/{}/close", project_id), String::from("{}"))?;
        }
        Ok(serde_json::from_str(
            &self.request_get(format!("projects/{}", project_id))?,
        )?)
    }

    /// Create a new project with the given name, and open it
    pub fn create_project(&mut self, project_name: impl AsRef<str>) -> Result<GNS3Project> {
        let project: GNS3Project = serde_json::from_str(&self.request_post(
            "projects",
            format!("{{\"name\": \"{}\"}}", project_name.as_ref()),
        )?)?;
        self.project = Some(project.id.clone());
        Ok(project)
    }

    /// Returns all available templates
    pub fn get_templates(&self) -> Result<Vec<GNS3Template>> {
        Ok(serde_json::from_str(&self.request_get("templates")?)?)
    }

    /// Create a new node from a template
    pub fn create_node(
        &self,
        name: impl AsRef<str>,
        template_id: impl AsRef<str>,
    ) -> Result<GNS3Node> {
        let project_id: String = self.project.as_ref().ok_or(Error::NoProjectOpened)?.clone();
        let node: GNS3Node = serde_json::from_str(&self.request_post(
            format!("projects/{}/templates/{}", project_id, template_id.as_ref()),
            format!("{{\"name\": \"{}\", \"x\": 0, \"y\": 0}}", name.as_ref()),
        )?)?;
        self.modify_node(node.id, Some(name.as_ref().to_string()), None)
    }

    /// Create a new VPCS on the local compute
    pub fn create_vpcs(&self, name: impl AsRef<str>) -> Result<GNS3Node> {
        let project_id: String = self.project.as_ref().ok_or(Error::NoProjectOpened)?.clone();
        Ok(serde_json::from_str(&self.request_post(
            format!("projects/{}/nodes", project_id),
            format!("{{\"name\": \"{}\", \"x\": 0, \"y\": 0, \"node_type\": \"vpcs\", \"compute_id\": \"local\"}}", name.as_ref()),
        )?)?)
    }

    /// Modify the name or the port ID of a node
    pub fn modify_node(
        &self,
        node_id: impl AsRef<str>,
        name: Option<String>,
        port: Option<u32>,
    ) -> Result<GNS3Node> {
        let project_id: String = self.project.as_ref().ok_or(Error::NoProjectOpened)?.clone();
        let mut options_vec: Vec<String> = Vec::new();
        if let Some(name) = name {
            options_vec.push(format!("\"name\": \"{}\"", name));
        }
        if let Some(port) = port {
            options_vec.push(format!("\"console\": {}", port));
        }
        Ok(serde_json::from_str(&self.request_put(
            format!("projects/{}/nodes/{}", project_id, node_id.as_ref()),
            format!("{{ {} }}", options_vec.join(", ")),
        )?)?)
    }

    /// Return all nodes in the project
    pub fn get_nodes(&self) -> Result<Vec<GNS3Node>> {
        let project_id: String = self.project.as_ref().ok_or(Error::NoProjectOpened)?.clone();
        Ok(serde_json::from_str(
            &self.request_get(format!("projects/{}/nodes", project_id))?,
        )?)
    }

    /// Create a new link. The interface is used as an index into the interfaces of the
    /// corresponding node.
    pub fn create_link(
        &self,
        node_a: &GNS3Node,
        iface_a: usize,
        node_b: &GNS3Node,
        iface_b: usize,
    ) -> Result<GNS3Link> {
        let project_id: String = self.project.as_ref().ok_or(Error::NoProjectOpened)?.clone();
        Ok(serde_json::from_str(&self.request_post(
            format!("projects/{}/links", project_id),
            format!(
                "{{ \"nodes\": [ {}, {} ] }}",
                GNS3LinkEndpoint::from_node(node_a, iface_a),
                GNS3LinkEndpoint::from_node(node_b, iface_b)
            ),
        )?)?)
    }

    /// Return all links in the project
    pub fn get_links(&self) -> Result<Vec<GNS3Link>> {
        let project_id: String = self.project.as_ref().ok_or(Error::NoProjectOpened)?.clone();
        Ok(serde_json::from_str(
            &self.request_get(format!("projects/{}/links", project_id))?,
        )?)
    }

    /// Start the capture on a specific link
    pub fn start_capture(&self, link: impl AsRef<str>) -> Result<GNS3Link> {
        let project_id: String = self.project.as_ref().ok_or(Error::NoProjectOpened)?.clone();
        Ok(serde_json::from_str(&self.request_post(
            format!(
                "projects/{}/links/{}/start_capture",
                project_id,
                link.as_ref()
            ),
            "{}".to_string(),
        )?)?)
    }

    /// Stop the capture on a specific link
    pub fn stop_capture(&self, link: impl AsRef<str>) -> Result<GNS3Link> {
        let project_id: String = self.project.as_ref().ok_or(Error::NoProjectOpened)?.clone();
        Ok(serde_json::from_str(&self.request_post(
            format!(
                "projects/{}/links/{}/stop_capture",
                project_id,
                link.as_ref()
            ),
            "{}".to_string(),
        )?)?)
    }

    /// Stop the capture on a specific link
    pub fn clear_capture_file(&self, link: impl AsRef<str>) -> Result<GNS3Link> {
        self.stop_capture(link.as_ref())?;
        self.start_capture(link.as_ref())
    }

    /// Start all nodes in the project
    pub fn start_all_nodes(&self) -> Result<()> {
        let project_id: String = self.project.as_ref().ok_or(Error::NoProjectOpened)?.clone();
        self.request_post(
            format!("projects/{}/nodes/start", project_id),
            String::from("{}"),
        )?;
        Ok(())
    }

    /// Stop all nodes in the project
    pub fn stop_all_nodes(&self) -> Result<()> {
        let project_id: String = self.project.as_ref().ok_or(Error::NoProjectOpened)?.clone();
        self.request_post(
            format!("projects/{}/nodes/stop", project_id),
            String::from("{}"),
        )?;
        Ok(())
    }

    /// Start a specific node
    pub fn start_node(&self, node_id: impl AsRef<str>) -> Result<GNS3Node> {
        let project_id: String = self.project.as_ref().ok_or(Error::NoProjectOpened)?.clone();
        Ok(serde_json::from_str(&self.request_post(
            format!("projects/{}/nodes/{}/start", project_id, node_id.as_ref()),
            String::from("{}"),
        )?)?)
    }

    /// Stop a specific node
    pub fn stop_node(&self, node_id: impl AsRef<str>) -> Result<GNS3Node> {
        let project_id: String = self.project.as_ref().ok_or(Error::NoProjectOpened)?.clone();
        Ok(serde_json::from_str(&self.request_post(
            format!("projects/{}/nodes/{}/stop", project_id, node_id.as_ref()),
            String::from("{}"),
        )?)?)
    }

    /// Delete an existing project
    pub fn delete_project(&mut self, project_id: impl AsRef<str>) -> Result<()> {
        if self.project == Some(project_id.as_ref().to_string()) {
            self.project = None;
        }
        self.request_delete(format!("projects/{}", project_id.as_ref()))
    }

    fn request_get(&self, key: impl AsRef<str>) -> Result<String> {
        let addr = format!("{}/v2/{}", self.address, key.as_ref());
        //eprintln!("GET  {} {}", addr);
        self.handle_response(isahc::get(&addr)?)
    }

    fn request_post(&self, key: impl AsRef<str>, data: String) -> Result<String> {
        let addr = format!("{}/v2/{}", self.address, key.as_ref());
        //eprintln!("POST {} {}", addr, data);
        self.handle_response(isahc::post(&addr, data)?)
    }

    fn request_put(&self, key: impl AsRef<str>, data: String) -> Result<String> {
        let addr = format!("{}/v2/{}", self.address, key.as_ref());
        //eprintln!("PUT  {} {}", addr, data);
        self.handle_response(isahc::put(&addr, data)?)
    }

    fn request_delete(&self, key: impl AsRef<str>) -> Result<()> {
        let addr = format!("{}/v2/{}", self.address, key.as_ref());
        //eprintln!("DEL  {} {}", addr);
        match self.handle_response(isahc::delete(&addr)?) {
            Ok(_) => Ok(()),
            Err(Error::GNS3Error { id, .. }) if (200..300).contains(&id) => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn handle_response(&self, mut response: Response<Body>) -> Result<String> {
        let status = response.status();
        if !status.is_success() {
            return Err(Error::ResponseError(status.as_u16(), response.text()?));
        }
        let response = response.text()?;
        let error_re = Regex::new(r"^(\d*): (.*)$").unwrap();
        if let Some(captures) = error_re.captures(&response) {
            if captures.len() == 3 {
                let error_id: u32 = captures.get(1).unwrap().as_str().parse().unwrap();
                let error_text: String = captures.get(2).unwrap().as_str().to_string();
                Err(Error::GNS3Error {
                    id: error_id,
                    message: error_text,
                })
            } else {
                panic!("Unexpected Error Received! {}", response)
            }
        } else {
            Ok(response)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    const TEST_PROJECT_NAME: &str = "TestProjectForTestPurpose";

    #[test]
    fn new_server() {
        let server = match GNS3Server::new("localhost", 3080) {
            Ok(s) => s,
            Err(_) => return, // skip the test
        };
        assert_eq!(server.version(), "2.2.16");
    }

    #[test]
    fn list_projects() {
        let server = match GNS3Server::new("localhost", 3080) {
            Ok(s) => s,
            Err(_) => return, // skip the test
        };
        server.get_projects().unwrap();
    }

    #[test]
    fn create_close_open_delete_project() {
        let mut server = match GNS3Server::new("localhost", 3080) {
            Ok(s) => s,
            Err(_) => return, // skip the test
        };
        delete_test_project(&mut server, TEST_PROJECT_NAME);
        let project = server.create_project(TEST_PROJECT_NAME).unwrap();
        assert_eq!(project.status, GNS3ProjectStatus::Opened);
        assert_eq!(
            server.open_project(&project.id).unwrap().status,
            GNS3ProjectStatus::Opened
        );
        assert_eq!(
            server.close_project().unwrap().status,
            GNS3ProjectStatus::Closed
        );
        server.delete_project(&project.id).unwrap();
    }

    #[test]
    fn frr_template() {
        let server = match GNS3Server::new("localhost", 3080) {
            Ok(s) => s,
            Err(_) => return, // skip the test
        };
        let templates = server.get_templates().unwrap();
        assert!(templates.iter().any(|t| t.name == "FRR 7.3.1"));
    }

    #[test]
    fn create_node() {
        let mut server = match GNS3Server::new("localhost", 3080) {
            Ok(s) => s,
            Err(_) => return, // skip the test
        };
        delete_test_project(&mut server, TEST_PROJECT_NAME);
        let project = server.create_project(TEST_PROJECT_NAME).unwrap();
        let frr_id = server
            .get_templates()
            .unwrap()
            .iter()
            .find(|t| t.name == "FRR 7.3.1")
            .unwrap()
            .id
            .clone();
        let node = server.create_node("test_node", frr_id).unwrap();
        assert_eq!(node.name, "test_node");
        assert_eq!(server.get_nodes().unwrap(), vec![node]);
        server.close_project().unwrap();
        server.delete_project(&project.id).unwrap();
    }

    #[test]
    fn create_link() {
        let mut server = match GNS3Server::new("localhost", 3080) {
            Ok(s) => s,
            Err(_) => return, // skip the test
        };
        delete_test_project(&mut server, TEST_PROJECT_NAME);
        let project = server.create_project(TEST_PROJECT_NAME).unwrap();
        assert_eq!(server.get_links().unwrap().len(), 0);
        let frr_id = server
            .get_templates()
            .unwrap()
            .iter()
            .find(|t| t.name == "FRR 7.3.1")
            .unwrap()
            .id
            .clone();
        let node_a = server.create_node("node_a", &frr_id).unwrap();
        let node_b = server.create_node("node_b", &frr_id).unwrap();
        server.create_link(&node_a, 0, &node_b, 0).unwrap();
        assert_eq!(server.get_links().unwrap().len(), 1);
        server.close_project().unwrap();
        server.delete_project(&project.id).unwrap();
    }

    #[test]
    fn start_stop_node() {
        let mut server = match GNS3Server::new("localhost", 3080) {
            Ok(s) => s,
            Err(_) => return, // skip the test
        };
        delete_test_project(&mut server, TEST_PROJECT_NAME);
        let project = server.create_project(TEST_PROJECT_NAME).unwrap();
        assert_eq!(server.get_links().unwrap().len(), 0);
        let frr_id = server
            .get_templates()
            .unwrap()
            .iter()
            .find(|t| t.name == "FRR 7.3.1")
            .unwrap()
            .id
            .clone();
        let node_a = server.create_node("node_a", &frr_id).unwrap();
        assert!(node_a.status.is_stopped());
        assert!(server.start_node(&node_a.id).unwrap().status.is_started());
        server.stop_node(&node_a.id).unwrap();
        sleep(Duration::from_millis(100));
        assert!(server
            .get_nodes()
            .unwrap()
            .get(0)
            .unwrap()
            .status
            .is_stopped());
        server.close_project().unwrap();
        server.delete_project(&project.id).unwrap();
    }

    #[test]
    fn start_stop_all() {
        let mut server = match GNS3Server::new("localhost", 3080) {
            Ok(s) => s,
            Err(_) => return, // skip the test
        };
        delete_test_project(&mut server, TEST_PROJECT_NAME);
        let project = server.create_project(TEST_PROJECT_NAME).unwrap();
        assert_eq!(server.get_links().unwrap().len(), 0);
        let frr_id = server
            .get_templates()
            .unwrap()
            .iter()
            .find(|t| t.name == "FRR 7.3.1")
            .unwrap()
            .id
            .clone();
        server.create_node("node_a", &frr_id).unwrap();
        server.create_node("node_b", &frr_id).unwrap();
        assert!(server
            .get_nodes()
            .unwrap()
            .into_iter()
            .all(|n| n.status.is_stopped()));
        server.start_all_nodes().unwrap();
        assert!(server
            .get_nodes()
            .unwrap()
            .into_iter()
            .all(|n| n.status.is_started()));
        server.stop_all_nodes().unwrap();
        sleep(Duration::from_millis(100));
        assert!(server
            .get_nodes()
            .unwrap()
            .into_iter()
            .all(|n| n.status.is_stopped()));
        server.close_project().unwrap();
        server.delete_project(&project.id).unwrap();
    }

    fn delete_test_project(server: &mut GNS3Server, name: &'static str) {
        if let Some(project) = server
            .get_projects()
            .unwrap()
            .into_iter()
            .find(|p| p.name == name)
        {
            server.delete_project(project.id).unwrap();
        }
    }
}
