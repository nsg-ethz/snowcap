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

//! # Random Modifier Ordering

use super::ModifierOrdering;
use crate::netsim::config::ConfigModifier;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::cmp::Ordering;
use std::marker::PhantomData;

/// # RandomOrdering
///
/// This returns a random ordering when calling sort. But every item is still treated as equal
/// in terms of ordering.
pub struct RandomOrdering<T = ConfigModifier> {
    phantom: PhantomData<T>,
}

impl<T> ModifierOrdering<T> for RandomOrdering<T> {
    fn sort(modifiers: &mut Vec<T>) {
        modifiers.shuffle(&mut thread_rng());
    }

    fn order(_a: &T, _b: &T) -> Ordering {
        Ordering::Equal
    }
}
