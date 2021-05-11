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

//! # Multiple-Swap Permutator
//!
//! This is a permutator that is based on another permutator, calling the next method multiple times
//! in order to have larger differences between each iteration.

use super::{Permutator, PermutatorItem};
use crate::netsim::config::ConfigModifier;
use primal::Primes;

/// # Multiple-Swap Permutator
///
/// This is a permutator that is based on another permutator, calling the next method multiple times
/// in order to have larger differences between each iteration. The number of times to call next,
/// before we return the element, is determined by taking the next prime number, larger than the
/// total number of elements to produce the permutator. This way, we are guaranteed that we return
/// each element exactly once.
pub struct MultipleSwapPermutator<P, T = ConfigModifier> {
    /// Data, needed for generating the permutator multiple times
    data: Vec<T>,
    /// Current permutator
    permutator: P,
    /// number of times to wait until we yield the next element. `num` is set to 1 if `len <= 3`,
    /// since for small `len`, the next prime number is larger than, or equal to `len - 1`, in which
    /// case it makes no sense to wait that long. `num` is always a prime number.
    num: usize,
    /// The first iteration is always returned.
    started: bool,
    /// The first teration, which gets `None` at a point where we would like to return it, we know
    /// that we have iterated over all combinations. At this point, we need to always return None.
    finished: bool,
}

impl<P, T> Permutator<T> for MultipleSwapPermutator<P, T>
where
    P: Permutator<T> + Iterator,
    P::Item: PermutatorItem<T>,
    T: Clone,
{
    fn new(input: Vec<T>) -> Self {
        let len = input.len();
        Self {
            data: input.clone(),
            permutator: P::new(input),
            num: if len <= 3 { 1 } else { Primes::all().find(|p| *p > len).unwrap() },
            started: false,
            finished: false,
        }
    }
}

impl<P, T> Iterator for MultipleSwapPermutator<P, T>
where
    P: Permutator<T> + Iterator,
    P::Item: PermutatorItem<T>,
    T: Clone,
{
    type Item = P::Item;

    fn next(&mut self) -> Option<Self::Item> {
        // handle the start
        if !self.started {
            self.started = true;
            return self.permutator.next();
        }

        // if finished, always return None
        if self.finished {
            return None;
        }

        // repeat for num - 1 times, to take the next, and rebuild the permutator if it fails
        for _ in 0..(self.num - 1) {
            if self.permutator.next().is_none() {
                self.permutator = P::new(self.data.clone());
                self.permutator.next();
            }
        }

        // now, take the permutator result, and check if it is now done.
        match self.permutator.next() {
            Some(elem) => Some(elem),
            None => {
                self.finished = true;
                None
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::modifier_ordering::NoOrdering;
    use crate::permutators::SJTPermutator;

    #[derive(Clone, Copy, PartialEq, Debug)]
    enum Elems {
        A,
        B,
        C,
        D,
    }

    use Elems::*;

    type CurrentPermutator = MultipleSwapPermutator<SJTPermutator<NoOrdering, Elems>, Elems>;

    #[test]
    fn test_multiple_swap_permutator_0() {
        let data: Vec<Elems> = vec![];
        let permutations: Vec<Vec<Elems>> = CurrentPermutator::new(data).collect();
        assert_eq!(permutations, vec![vec![]]);
    }

    #[test]
    fn test_multiple_swap_permutator_1() {
        let data: Vec<Elems> = vec![A];
        let permutations: Vec<Vec<Elems>> = CurrentPermutator::new(data).collect();
        assert_eq!(permutations, vec![vec![A]]);
    }

    #[test]
    fn test_multiple_swap_permutator_2() {
        let data: Vec<Elems> = vec![A, B];
        let permutations: Vec<Vec<Elems>> = CurrentPermutator::new(data).collect();
        assert_eq!(permutations, vec![vec![A, B], vec![B, A]]);
    }

    #[test]
    fn test_multiple_swap_permutator_3() {
        let data: Vec<Elems> = vec![A, B, C];
        let permutations: Vec<Vec<Elems>> = CurrentPermutator::new(data).collect();
        assert_eq!(
            permutations,
            vec![
                vec![A, B, C],
                vec![A, C, B],
                vec![C, A, B],
                vec![C, B, A],
                vec![B, C, A],
                vec![B, A, C]
            ]
        );
    }

    #[test]
    fn test_multiple_swap_permutator_4() {
        let data: Vec<Elems> = vec![A, B, C, D];
        let permutations: Vec<Vec<Elems>> = CurrentPermutator::new(data).collect();
        assert_eq!(
            permutations,
            vec![
                vec![A, B, C, D],
                vec![A, D, C, B],
                vec![C, D, A, B],
                vec![C, B, A, D],
                vec![D, B, A, C],
                vec![A, B, D, C],
                vec![A, C, D, B],
                vec![D, C, A, B],
                vec![B, C, A, D],
                vec![B, D, A, C],
                vec![A, D, B, C],
                vec![A, C, B, D],
                vec![D, C, B, A],
                vec![B, C, D, A],
                vec![B, A, D, C],
                vec![D, A, B, C],
                vec![C, A, B, D],
                vec![C, D, B, A],
                vec![B, D, C, A],
                vec![B, A, C, D],
                vec![D, A, C, B],
                vec![C, A, D, B],
                vec![C, B, D, A],
                vec![D, B, C, A],
            ]
        );
    }

    #[test]
    fn test_first_prime_bigger() {
        assert_eq!(Primes::all().skip_while(|p| *p <= 0).next().unwrap(), 2);
        assert_eq!(Primes::all().skip_while(|p| *p <= 1).next().unwrap(), 2);
        assert_eq!(Primes::all().skip_while(|p| *p <= 2).next().unwrap(), 3);
        assert_eq!(Primes::all().skip_while(|p| *p <= 3).next().unwrap(), 5);
        assert_eq!(Primes::all().skip_while(|p| *p <= 4).next().unwrap(), 5);
        assert_eq!(Primes::all().skip_while(|p| *p <= 5).next().unwrap(), 7);
    }
}
