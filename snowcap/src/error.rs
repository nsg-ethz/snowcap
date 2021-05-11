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

//! Module containing all error types

use crate::netsim::{config::ConfigModifier, ConfigError, NetworkError};
use crate::topology_zoo::ZooTopologyError;
use thiserror::Error;

/// Main error type
#[derive(Debug, Error)]
pub enum Error {
    /// Error propagated from `netsim`
    #[error("Network Error: {0}")]
    NetworkError(#[from] NetworkError),
    /// No safe ordering can be found
    #[error("No safe ordering can be found!")]
    NoSafeOrdering,
    /// No safe ordering can be found using the chosen strategy, but there might be different
    /// strategies that may find a solution.
    #[error("No safe ordering can be found using the chosen strategy!")]
    ProbablyNoSafeOrdering,
    /// Global Optimum was not found using the GlobalOptimizer.
    #[error("Global optimum was not found: Best solution yet has cost {1}")]
    GlobalOptimumNotFound(Vec<ConfigModifier>, f64),
    /// The initial state of the network or the configuration is invalid
    #[error("Invalid initial state or configuration")]
    InvalidInitialState,
    /// The maximum number of backtracks are reached
    #[error("The configured max backtrack level was reached!")]
    ReachedMaxBacktrack,
    /// Used up all of the time budget
    #[error("The time budget was used up without finding any solution")]
    Timeout,
    /// On an operation abort
    #[error("The operation was aborted")]
    Abort,
    /// Topology Zoo Error
    #[error("Topology Zoo Error: {0}")]
    ZooTopologyError(#[from] ZooTopologyError),
}

impl From<ConfigError> for Error {
    fn from(cause: ConfigError) -> Self {
        Self::NetworkError(NetworkError::ConfigError(cause))
    }
}
