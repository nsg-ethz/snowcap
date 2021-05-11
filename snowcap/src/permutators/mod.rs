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

//! # Permutators
//!
//! This module contains all different iterators which iterate over all permutations. The iterators
//! differ from each other by the order in which the permutations are yielded.
//!
//! ## Different Permutators
//! - **[`HeapsPermutator`]**: Simple permutator, implemented using the Heaps algorithm. This
//!   permutator does not re-implement `fail_pos`, and thus does not make use of the feedback
//!   functionality for permutators. The HeapsPermutaor is implemented for any type, which
//!   implements the `Copy` trait.
//!
//! - **[`LexicographicPermutator`]**: Simple permutator, which returns all permutations in a
//!   lexicographic ordering. It is implemented for any type, which implements the `Copy` trait, and
//!   requires a [`CompleteOrdering`](crate::modifier_ordering::ModifierOrdering) for the chosen
//!   type. This permutator does not re-implement `fail_pos`, and thus does not make use of the
//!   feedback functionality for permutators.
//!
//! - **[`SJTPermutator`]**: Permutator implementing the Steinhaus-Johnson-Trotter algorithm. It is
//!   implemented for any type, which implements the `Copy` trait. This permutator does not
//!   re-implement `fail_pos`, and thus, does not make use of the feedback functionality for
//!   permutators.
//!
//! - **[`MultipleSwapPermutator`]**: This is a meta-permutator that consists of a different
//!   permutator. Every time `next` is called, it in terms calls `next` multiple times on the
//!   specified permutator, resulting in multiple swaps at once. The numebr of times to call `next`
//!   is chosen such that every permutation is created exactly once (by choosing the smallest prime
//!   number larger than the number of elements in the permutation series). It does not re-implement
//!   `fail_pos`, and thus, does not make use of the feedback functionality for permutators.
//!
//! - **[`TreePermutator`]**: This permutator is based on the
//!   [`TreeStrategy`](crate::strategies::TreeStrategy). The permutator generates a sequence
//!   identical to the `LexicographicPermutator`, but it does not require a `CompleteOrdering`.
//!   Instead, it builds a datastructure which does not require the comparison of single elements.
//!   Additionally, this permutator re-implements `fail_pos`, and makes use of the fallback
//!   funcitonality to reduce the number of permutations for dependencies with an *immediate*
//!   *effect*.
//!
//! - **[`RandomTreePermutator`]**: This permutator is very similar to the [`TreePermutator`].
//!   However, the main difference is that the ordering of the remaining elements is always
//!   shuffled, every time a new branch in the tree is entered. As for the `TreePermutator`, this
//!   permutator re-implements `fail_pos` to reduce the number of permutations for dependencies with
//!   an *immediate effect*.

mod heaps;
pub use heaps::HeapsPermutator;

mod lexicographic;
pub use lexicographic::LexicographicPermutator;

mod sjt;
pub use sjt::SJTPermutator;

mod multiple_swap;
pub use multiple_swap::MultipleSwapPermutator;

mod tree;
pub use tree::TreePermutator;

mod random_tree;
pub use random_tree::RandomTreePermutator;

/// Permutator trait
pub trait Permutator<T>
where
    Self: Iterator,
    Self::Item: PermutatorItem<T>,
{
    /// Creates the permutator from the sequence of `T`
    fn new(input: Vec<T>) -> Self;

    /// This function may reduce the number of permutations, by skipping permutations which start
    /// the exact same way as the last call to `next`, up to the position `pos`. Not every
    /// permutator has this funciton implemented.
    fn fail_pos(&mut self, _pos: usize) {}
}

/// This is an empty trait to tell the compiler which types can be returned by the Permutator
/// Iterator.
pub trait PermutatorItem<T> {
    /// Transforms the item to a vector of `T`. This is in order to allow a `Permutator` to be
    /// passed to a function dynamically.
    fn as_patches(self) -> Vec<T>;
}

impl<T> PermutatorItem<T> for Vec<T> {
    fn as_patches(self) -> Vec<T> {
        self
    }
}
