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

//! # NoOrdering
//!
//! This represents the unordered ordering

use super::ModifierOrdering;
use std::cmp::Ordering;

/// # NoOrdering
///
/// This makes every `ConfigModifier` to be treated equal to any other `ConfigModifier` (Equal in
/// terms of `Ordering::Equal`).
pub struct NoOrdering {}

impl<T> ModifierOrdering<T> for NoOrdering {
    fn sort(_modifiers: &mut Vec<T>) {}

    fn order(_a: &T, _b: &T) -> Ordering {
        Ordering::Equal
    }
}
