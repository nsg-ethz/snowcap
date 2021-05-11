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

//! # ModifierOrdering
//!
//! This module defines different orderings for
//! [`ConfigModifier`](crate::netsim::config::ConfigModifier), and a trait as an interface.
//!
//! Orderings can be [`CompleteOrdering`], which means that the comparison `cmp(a, b)` only returns
//! `std::cmp::Ordering::Equal` if `a == b`.

mod simple;
pub use simple::SimpleOrdering;

mod simple_reverse;
pub use simple_reverse::SimpleReverseOrdering;

mod unordered;
pub use unordered::NoOrdering;

mod random;
pub use random::RandomOrdering;

use std::cmp::Ordering;

/// # ModifierOrdering
/// Trait for defining different orderings, used in `Permutators` and `Strategies`.
pub trait ModifierOrdering<T> {
    /// Sort a sequence of config modifiers
    fn sort(modifiers: &mut Vec<T>) {
        modifiers.sort_by(|a, b| Self::order(a, b))
    }

    /// Order two config modifiers
    fn order(a: &T, b: &T) -> Ordering;
}

/// # Complete Ordering
/// This trait should only be implemented by `ModifierOrderings` if all members of the
/// `ConfigModifier` are compared, and when `Ordering::Equal` is returned only if both
/// modifiers are actually equal.
pub trait CompleteOrdering {}

/// # StdOrdering
/// Standard ordering defined by the rust standard library.
pub struct StdOrdering<T: Ord> {
    marker: std::marker::PhantomData<T>,
}

impl<T> ModifierOrdering<T> for StdOrdering<T>
where
    T: Ord,
{
    fn order(a: &T, b: &T) -> Ordering {
        a.cmp(b)
    }
}

impl<T> CompleteOrdering for StdOrdering<T> where T: Ord {}
