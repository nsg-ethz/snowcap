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

//! # GNS3 Server API
//!
//! This is a very simple crate to interact with the GNS3 server, creating projects, nodes and links
//! automatically.
//!
//! ```
//! use gns3::GNS3Server;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // connect to the server
//!     let mut server = match GNS3Server::new("localhost", 3080) {
//!         Ok(s) => s,
//!         Err(e) => {
//!             eprintln!("Cannot connect to the server: {}", e);
//! # return Ok(());
//!             return Err(e.into());
//!         }
//!     };
//!
//!     // crate a new project
//!     let project = server.create_project("DocumentationExample")?;
//!
//!     // get the FRR template
//!     let frr = server
//!         .get_templates()?
//!         .into_iter()
//!         .filter(|t| t.name == "FRR 7.3.1")
//!         .next()
//!         .unwrap();
//!
//!     // create two nodes
//!     let node_a = server.create_node("node_a", &frr.id)?;
//!     let node_b = server.create_node("node_b", &frr.id)?;
//!
//!     // create the link between them
//!     server.create_link(&node_a, 0, &node_b, 0)?;
//!
//!     // start all nodes
//!     server.start_all_nodes()?;
//!
//!     # server.close_project()?;
//!     # server.delete_project(project.id)?;
//!     Ok(())
//! }
//! ```
#![deny(missing_docs)]

mod server;
mod types;
pub use server::GNS3Server;
pub use types::*;

use thiserror::Error;

/// # GNS3 Error type
#[derive(Debug, Error)]
pub enum Error {
    /// Error during handling of the HTTP request
    #[allow(clippy::upper_case_acronyms)]
    #[error("HTTP Error: {0}")]
    HTTPError(#[from] isahc::Error),
    /// Cannot deserialize the response
    #[error("Cannot parse JSON response: {0}")]
    JsonError(#[from] serde_json::error::Error),
    /// IO Error
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
    /// GNS3 Error
    #[allow(clippy::upper_case_acronyms)]
    #[error("GNS3 Error: {id}: {message}")]
    GNS3Error {
        /// Error ID
        id: u32,
        /// Error message
        message: String,
    },
    /// HTTP Response Error
    #[error("HTTP Response Error: {0}. Message:\n{1}")]
    ResponseError(u16, String),
    /// No project is selected
    #[error("No project is opened!")]
    NoProjectOpened,
}

/// GNS3 Result type
type Result<T> = core::result::Result<T, Error>;
