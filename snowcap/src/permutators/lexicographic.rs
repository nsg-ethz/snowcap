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

//! # Ordered Permutator
//!
//! The `LexicographicPermutator` generates all permutations in lexicographic order.

use super::Permutator;
use crate::modifier_ordering::{CompleteOrdering, ModifierOrdering};
use crate::netsim::config::ConfigModifier;
use std::cmp::Ordering;
use std::marker::PhantomData;

/// # Ordered Permutator
/// The `LexicographicPermutator` generates all permutations in lexicographic order. The ordering
/// can be chosen using [`ModifierOrdering`](crate::modifier_ordering::ModifierOrdering),
/// which must be a [`CompleteOrdering`](crate::modifier_ordering::CompleteOrdering).
///
/// The `LexicographicPermutator` is only implemented for `ConfigModifier`.
pub struct LexicographicPermutator<O, T = ConfigModifier> {
    data: Vec<T>,
    len: usize,
    start: bool,
    ordering: PhantomData<O>,
}

impl<O, T> Permutator<T> for LexicographicPermutator<O, T>
where
    O: ModifierOrdering<T> + CompleteOrdering,
    T: Clone + PartialEq + std::fmt::Debug,
{
    fn new(mut input: Vec<T>) -> Self {
        let len = input.len();
        // sort the input vector
        O::sort(&mut input);
        // make sure that the items are not equal
        if len > 1 {
            for i in 0..(len - 1) {
                assert_ne!(input[i], input[i + 1]);
            }
        }
        LexicographicPermutator { data: input, len, start: true, ordering: PhantomData }
    }
}

impl<O, T> Iterator for LexicographicPermutator<O, T>
where
    O: ModifierOrdering<T> + CompleteOrdering,
    T: Clone,
{
    type Item = Vec<T>;

    fn next(&mut self) -> Option<Self::Item> {
        // handle the start
        if self.start {
            self.start = false;
            return Some(self.data.clone());
        }

        // step 1: find the largest index k, such that data[k] < data[k + 1]
        let mut k: Option<usize> = None;
        if self.len > 1 {
            for i in 0..(self.len - 1) {
                if O::order(&self.data[i], &self.data[i + 1]) == Ordering::Less {
                    k = Some(i);
                }
            }
        }

        let k = k?;

        // step 2: find the largest index l > k, such that data[k] < data[l]
        let mut l: usize = k + 1;
        for i in (k + 2)..self.len {
            if O::order(&self.data[k], &self.data[i]) == Ordering::Less {
                l = i
            }
        }

        // step 3: Swap data[k] and data[l]
        self.data.swap(k, l);

        // step 4: Reverse the order from k+1..n
        reverse_vec_part(&mut self.data, k + 1);

        Some(self.data.clone())
    }
}

/// Reorders a vector in the range [k..end]
fn reverse_vec_part<T>(v: &mut Vec<T>, k: usize) {
    let n = v.len();
    let num_swap = (n - k) / 2;
    for i in 0..num_swap {
        v.swap(k + i, n - 1 - i);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::modifier_ordering::SimpleOrdering;
    use crate::netsim::config::{ConfigExpr::BgpSession, ConfigModifier::*};
    use crate::netsim::BgpSessionType::EBgp;

    #[test]
    fn test_reverse_vec_part() {
        let mut v = vec![0, 1, 2, 3, 4];
        reverse_vec_part(&mut v, 5);
        assert_eq!(v, vec![0, 1, 2, 3, 4]);

        let mut v = vec![0, 1, 2, 3, 4];
        reverse_vec_part(&mut v, 4);
        assert_eq!(v, vec![0, 1, 2, 3, 4]);

        let mut v = vec![0, 1, 2, 3, 4];
        reverse_vec_part(&mut v, 3);
        assert_eq!(v, vec![0, 1, 2, 4, 3]);

        let mut v = vec![0, 1, 2, 3, 4];
        reverse_vec_part(&mut v, 2);
        assert_eq!(v, vec![0, 1, 4, 3, 2]);

        let mut v = vec![0, 1, 2, 3, 4];
        reverse_vec_part(&mut v, 1);
        assert_eq!(v, vec![0, 4, 3, 2, 1]);

        let mut v = vec![0, 1, 2, 3, 4];
        reverse_vec_part(&mut v, 0);
        assert_eq!(v, vec![4, 3, 2, 1, 0]);
    }

    #[test]
    fn test_ordered_permutation_0() {
        let data: Vec<ConfigModifier> = Vec::new();
        let permutations: Vec<Vec<ConfigModifier>> =
            LexicographicPermutator::<SimpleOrdering>::new(data).collect();
        assert_eq!(permutations, vec![vec![]]);
    }

    #[test]
    fn test_ordered_permutator_1() {
        // create an example vector
        let mut data: Vec<ConfigModifier> = Vec::new();
        let p = Insert(BgpSession { source: 0.into(), target: 0.into(), session_type: EBgp });
        data.push(p.clone());
        let permutations: Vec<Vec<ConfigModifier>> =
            LexicographicPermutator::<SimpleOrdering>::new(data).collect();
        assert_eq!(permutations, vec![vec![p.clone()]]);
    }

    #[test]
    fn test_ordered_permutator_2() {
        // create an example vector
        let mut data: Vec<ConfigModifier> = Vec::new();
        let p1 = Insert(BgpSession { source: 1.into(), target: 1.into(), session_type: EBgp });
        let p2 = Insert(BgpSession { source: 2.into(), target: 2.into(), session_type: EBgp });
        data.push(p2.clone());
        data.push(p1.clone());
        let permutations: Vec<Vec<ConfigModifier>> =
            LexicographicPermutator::<SimpleOrdering>::new(data).collect();
        assert_eq!(permutations, vec![vec![p1.clone(), p2.clone()], vec![p2.clone(), p1.clone()],]);
    }

    #[test]
    fn test_ordered_permutator_3() {
        // create an example vector
        let mut data: Vec<ConfigModifier> = Vec::new();
        let p1 = Insert(BgpSession { source: 1.into(), target: 1.into(), session_type: EBgp });
        let p2 = Insert(BgpSession { source: 2.into(), target: 2.into(), session_type: EBgp });
        let p3 = Remove(BgpSession { source: 3.into(), target: 3.into(), session_type: EBgp });
        data.push(p3.clone());
        data.push(p1.clone());
        data.push(p2.clone());
        let permutations: Vec<Vec<ConfigModifier>> =
            LexicographicPermutator::<SimpleOrdering>::new(data).collect();
        assert_eq!(
            permutations,
            vec![
                vec![p1.clone(), p2.clone(), p3.clone()],
                vec![p1.clone(), p3.clone(), p2.clone()],
                vec![p2.clone(), p1.clone(), p3.clone()],
                vec![p2.clone(), p3.clone(), p1.clone()],
                vec![p3.clone(), p1.clone(), p2.clone()],
                vec![p3.clone(), p2.clone(), p1.clone()],
            ]
        );
    }
}
