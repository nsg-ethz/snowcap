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

//! # Heaps Permutator
//!
//! This is the naive permutator, which just returns all permutations in a non-specific ordering.

use super::Permutator;
use crate::modifier_ordering::{ModifierOrdering, NoOrdering};
use crate::netsim::config::ConfigModifier;
use std::marker::PhantomData;

/// # Heaps Permutator
///
/// The permutator will not try to find an intelligent ordering. It implements the Heaps algorithm.
/// Before the algorithm starts, the data is sorted by the given ordering.
///
/// The `HeapsPermutator` is only implemented for `ConfigModifier` and `usize`.
pub struct HeapsPermutator<O = NoOrdering, T = ConfigModifier> {
    data: Vec<T>,
    state: Vec<usize>,
    i: usize,
    len: usize,
    started: bool,
    ordering: PhantomData<O>,
}

impl<O, T> Permutator<T> for HeapsPermutator<O, T>
where
    O: ModifierOrdering<T>,
    T: Clone,
{
    fn new(mut input: Vec<T>) -> Self {
        // sort the input after the given ordering
        O::sort(&mut input);
        let input_len = input.len();
        let mut state: Vec<usize> = Vec::with_capacity(input_len);
        for _ in 0..input_len {
            state.push(0)
        }
        HeapsPermutator {
            data: input,
            state,
            i: 0,
            len: input_len,
            started: false,
            ordering: PhantomData,
        }
    }
}

impl<O, T> Iterator for HeapsPermutator<O, T>
where
    T: Clone,
{
    type Item = Vec<T>;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.started {
            self.started = true;
            return Some(self.data.clone());
        }
        loop {
            if self.i >= self.len {
                break None;
            } else if self.state[self.i] < self.i {
                if self.i % 2 == 0 {
                    // i is even
                    self.data.swap(0, self.i);
                } else {
                    // i is odd
                    self.data.swap(self.state[self.i], self.i);
                }
                self.state[self.i] += 1;
                self.i = 0;
                break Some(self.data.clone());
            } else {
                self.state[self.i] = 0;
                self.i += 1;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::modifier_ordering::{NoOrdering, SimpleOrdering};
    use crate::netsim::config::{
        ConfigExpr::BgpSession,
        ConfigModifier::{self, Insert},
    };
    use crate::netsim::BgpSessionType::EBgp;

    #[test]
    fn test_heaps_permutator_0() {
        // create an example vector
        let data: Vec<ConfigModifier> = Vec::new();
        let permutations: Vec<Vec<ConfigModifier>> =
            HeapsPermutator::<NoOrdering>::new(data).collect();
        assert_eq!(permutations, vec![vec![]]);
    }

    #[test]
    fn test_heaps_permutator_1() {
        // create an example vector
        let mut data: Vec<ConfigModifier> = Vec::new();
        let p = Insert(BgpSession { source: 0.into(), target: 0.into(), session_type: EBgp });
        data.push(p.clone());
        let permutations: Vec<Vec<ConfigModifier>> =
            HeapsPermutator::<NoOrdering>::new(data).collect();
        assert_eq!(permutations, vec![vec![p.clone()]]);
    }

    #[test]
    fn test_heaps_permutator_2() {
        // create an example vector
        let mut data: Vec<ConfigModifier> = Vec::new();
        let p1 = Insert(BgpSession { source: 2.into(), target: 2.into(), session_type: EBgp });
        let p2 = Insert(BgpSession { source: 1.into(), target: 1.into(), session_type: EBgp });
        data.push(p1.clone());
        data.push(p2.clone());
        let permutations: Vec<Vec<ConfigModifier>> =
            HeapsPermutator::<NoOrdering>::new(data).collect();
        assert_eq!(permutations, vec![vec![p1.clone(), p2.clone()], vec![p2.clone(), p1.clone()],]);
    }

    #[test]
    fn test_heaps_permutator_3() {
        // create an example vector
        let mut data: Vec<ConfigModifier> = Vec::new();
        let p1 = Insert(BgpSession { source: 3.into(), target: 3.into(), session_type: EBgp });
        let p2 = Insert(BgpSession { source: 2.into(), target: 2.into(), session_type: EBgp });
        let p3 = Insert(BgpSession { source: 1.into(), target: 1.into(), session_type: EBgp });
        data.push(p1.clone());
        data.push(p2.clone());
        data.push(p3.clone());
        let permutations: Vec<Vec<ConfigModifier>> =
            HeapsPermutator::<NoOrdering>::new(data).collect();
        assert_eq!(
            permutations,
            vec![
                vec![p1.clone(), p2.clone(), p3.clone()],
                vec![p2.clone(), p1.clone(), p3.clone()],
                vec![p3.clone(), p1.clone(), p2.clone()],
                vec![p1.clone(), p3.clone(), p2.clone()],
                vec![p2.clone(), p3.clone(), p1.clone()],
                vec![p3.clone(), p2.clone(), p1.clone()],
            ]
        );
    }

    #[test]
    fn test_heaps_ordered_permutator_0() {
        // create an example vector
        let data: Vec<ConfigModifier> = Vec::new();
        let permutations: Vec<Vec<ConfigModifier>> =
            HeapsPermutator::<SimpleOrdering>::new(data).collect();
        assert_eq!(permutations, vec![vec![]]);
    }

    #[test]
    fn test_heaps_ordered_permutator_1() {
        // create an example vector
        let mut data: Vec<ConfigModifier> = Vec::new();
        let p = Insert(BgpSession { source: 0.into(), target: 0.into(), session_type: EBgp });
        data.push(p.clone());
        let permutations: Vec<Vec<ConfigModifier>> =
            HeapsPermutator::<SimpleOrdering>::new(data).collect();
        assert_eq!(permutations, vec![vec![p.clone()]]);
    }

    #[test]
    fn test_heaps_ordered_permutator_2() {
        // create an example vector
        let mut data: Vec<ConfigModifier> = Vec::new();
        let p1 = Insert(BgpSession { source: 1.into(), target: 1.into(), session_type: EBgp });
        let p2 = Insert(BgpSession { source: 2.into(), target: 2.into(), session_type: EBgp });
        data.push(p2.clone());
        data.push(p1.clone());
        let permutations: Vec<Vec<ConfigModifier>> =
            HeapsPermutator::<SimpleOrdering>::new(data).collect();
        assert_eq!(permutations, vec![vec![p1.clone(), p2.clone()], vec![p2.clone(), p1.clone()],]);
    }

    #[test]
    fn test_heaps_ordered_permutator_3() {
        // create an example vector
        let mut data: Vec<ConfigModifier> = Vec::new();
        let p1 = Insert(BgpSession { source: 1.into(), target: 1.into(), session_type: EBgp });
        let p2 = Insert(BgpSession { source: 2.into(), target: 2.into(), session_type: EBgp });
        let p3 = Insert(BgpSession { source: 3.into(), target: 3.into(), session_type: EBgp });
        data.push(p3.clone());
        data.push(p2.clone());
        data.push(p1.clone());
        let permutations: Vec<Vec<ConfigModifier>> =
            HeapsPermutator::<SimpleOrdering>::new(data).collect();
        assert_eq!(
            permutations,
            vec![
                vec![p1.clone(), p2.clone(), p3.clone()],
                vec![p2.clone(), p1.clone(), p3.clone()],
                vec![p3.clone(), p1.clone(), p2.clone()],
                vec![p1.clone(), p3.clone(), p2.clone()],
                vec![p2.clone(), p3.clone(), p1.clone()],
                vec![p3.clone(), p2.clone(), p1.clone()],
            ]
        );
    }
}
