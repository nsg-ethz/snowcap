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

//! # GNS3 Types

use serde::Deserialize;
use std::fmt;

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct GNS3ResponseVersion {
    pub version: String,
    pub local: bool,
}

/// Project Information
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct GNS3Project {
    /// ID of the project
    #[serde(rename = "project_id")]
    pub id: String,
    /// Name of the project
    pub name: String,
    /// Path of the project
    pub path: String,
    /// filename of the project
    pub filename: String,
    /// Status of the project
    pub status: GNS3ProjectStatus,
}

#[allow(clippy::upper_case_acronyms)]
/// Project Status
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum GNS3ProjectStatus {
    /// Open status
    #[serde(rename = "opened")]
    Opened,
    /// Close status
    #[serde(rename = "closed")]
    Closed,
}

#[allow(clippy::upper_case_acronyms)]
/// Node Information
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct GNS3Node {
    /// ID of the node
    #[serde(rename = "node_id")]
    pub id: String,
    /// name of the node
    pub name: String,
    /// type of the node (e.g., qemu)
    pub node_type: String,
    /// Name of the node
    #[serde(rename = "console")]
    pub port: u16,
    /// Status of the node
    pub status: GNS3NodeStatus,
    /// Links
    #[serde(rename = "ports")]
    pub interfaces: Vec<GNS3Interface>,
}

/// Project Status
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum GNS3NodeStatus {
    /// Node is stopped
    #[serde(rename = "stopped")]
    Stopped,
    /// Node is started
    #[serde(rename = "started")]
    Started,
    /// Node is suspended
    #[serde(rename = "suspended")]
    Suspended,
}

impl GNS3NodeStatus {
    /// Returns true if the node is started
    pub fn is_started(&self) -> bool {
        matches!(self, Self::Started)
    }
    /// Returns true if the node is stopped
    pub fn is_stopped(&self) -> bool {
        matches!(self, Self::Stopped)
    }
    /// Returns true if the node is suspended
    pub fn is_suspended(&self) -> bool {
        matches!(self, Self::Suspended)
    }
}

/// Interface Information
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct GNS3Interface {
    /// adapter number
    pub adapter_number: u32,
    /// port number
    pub port_number: u32,
    /// Name of the interface
    pub name: String,
    /// Short name of the interface
    pub short_name: String,
    /// Link type (Ethernet, etc...)
    pub link_type: String,
}

/// GNS3 Template
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct GNS3Template {
    /// ID of the template
    #[serde(rename = "template_id")]
    pub id: String,
    /// name of the template
    pub name: String,
    /// device category
    pub category: String,
    /// Type of the template (e.g. qemu).
    pub template_type: String,
    /// Disk image of the device (only for qemu)
    pub hda_disk_image: Option<String>,
}

/// Link data
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct GNS3Link {
    /// ID of the link
    #[serde(rename = "link_id")]
    pub id: String,
    /// nodes which the link connects
    pub nodes: [GNS3LinkEndpoint; 2],
    /// pcap file name containing the capture
    pub capture_file_name: Option<String>,
    /// pcap file containing the capture
    pub capture_file_path: Option<String>,
    /// pcap file containing the capture
    pub capturing: bool,
}

/// Endpoint of a link
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct GNS3LinkEndpoint {
    /// ID of the node for which the link is configured
    pub node_id: String,
    /// adapter number
    pub adapter_number: u32,
    /// port number
    pub port_number: u32,
}

impl GNS3LinkEndpoint {
    /// Create a link endpoint from a node, and the index of the link.
    pub fn from_node(node: &GNS3Node, iface_id: usize) -> Self {
        Self {
            node_id: node.id.clone(),
            adapter_number: node.interfaces.get(iface_id).unwrap().adapter_number,
            port_number: node.interfaces.get(iface_id).unwrap().port_number,
        }
    }
}

impl fmt::Display for GNS3LinkEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{ \"node_id\": \"{}\", \"adapter_number\": {}, \"port_number\": {} }}",
            self.node_id, self.adapter_number, self.port_number
        )
    }
}
