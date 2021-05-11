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

//! Module that contains definitios for the error class

use thiserror::Error;

use super::gml_parser::GmlError;

/// Error for ZooTopology
#[derive(Debug, Error)]
pub enum ZooTopologyError {
    /// Gml Parse Error
    #[error("Cannot parse GML file: {0}")]
    GmlParseError(#[from] GmlError),
    /// Too few internal routers present in the network to generate the topology
    #[error("Too few internal routers")]
    TooFewInternalRouters,
    /// Too few external routers present in the network to generate the topology
    #[error("Too few external routers")]
    TooFewExternalRouters,
    /// Too few border routers
    #[error("Too few boreder routers")]
    TooFewBorderRouters,
    /// Too few non-border routers
    #[error("Too few non-boreder routers")]
    TooFewNonBorderRouters,
    /// Specified name could not be found
    #[error("Name not found: {0}")]
    NameNotFound(String),
    /// NoClosestRootFound
    #[error("Cannot find a root closest to a router")]
    NoClosestRootFound,
    /// Multiple BGP sessions generated at the same time
    #[error("Cannot generate the Configuraiton, as mulitiple BGP sessions are generated!")]
    MultipleBgpSessions,
    /// Multiple Link weights configured
    #[error("Cannot generate the Configuraiton, as mutliple link weights are configured on the same link")]
    MultipleLinkWeights,
}
