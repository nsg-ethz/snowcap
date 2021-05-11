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

//! # DepGroups
//!
//! This module contains the code required for the `DepGroupsStrategy` and the `DepGroupsOptimizer`.

pub(crate) mod optimizer;
pub(crate) mod optimizer_trta;
pub(crate) mod strategy;
pub(crate) mod strategy_trta;
mod utils;

const TIME_FRACTION: u32 = 30;
const DO_EXPANSION: bool = true;
const EXPANSION_CHECK_ERRORS: bool = true;
const REDUCTION_CHECK_ERRORS: bool = true;
