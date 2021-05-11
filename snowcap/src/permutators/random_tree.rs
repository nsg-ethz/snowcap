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

//! # Tree Permutator

use super::Permutator;
use crate::netsim::config::ConfigModifier;
use rand::prelude::*;

/// # Random Tree Permutator
///
/// The random tree permutator is similar to the [`TreePermutator`](super::TreePermutator), but
/// with the difference that it is shuffled every time a new branch of the tree is entered.
pub struct RandomTreePermutator<T = ConfigModifier> {
    data: Vec<T>,
    state: Vec<usize>,
    /// This stores the remaining choices in reverse order, which means that we can pop the last
    /// element from the back to get the next one.
    remaining: Vec<Vec<usize>>,
    len: usize,
    started: bool,
    rng: ThreadRng,
}

impl<T> Permutator<T> for RandomTreePermutator<T>
where
    T: Clone,
{
    fn new(mut input: Vec<T>) -> Self {
        let mut rng = rand::thread_rng();
        // shuffle the input
        input.shuffle(&mut rng);
        let input_len = input.len();
        let mut state: Vec<usize> = Vec::with_capacity(input_len);
        let mut remaining: Vec<Vec<usize>> = Vec::with_capacity(input_len);
        for i in 0..input_len {
            state.push(i);
            remaining.push(((i + 1)..input_len).rev().collect());
        }
        RandomTreePermutator { data: input, state, remaining, len: input_len, started: false, rng }
    }

    fn fail_pos(&mut self, pos: usize) {
        for i in (pos + 1)..self.len {
            self.remaining[i].clear();
        }
    }
}

impl<T> Iterator for RandomTreePermutator<T>
where
    T: Clone,
{
    type Item = Vec<T>;
    fn next(&mut self) -> Option<Self::Item> {
        // handle the first value
        if !self.started {
            self.started = true;
            return Some(
                self.state.iter().map(|idx| self.data.get(*idx).unwrap()).cloned().collect(),
            );
        }
        // go from the back of the remaining array, and get the first position where there is still
        // something remaining
        let change_pos =
            match self.remaining.iter().enumerate().rev().find(|(_, rem)| !rem.is_empty()) {
                Some((pos, _)) => pos,
                None => return None, // when nothing was found, we have tried all permutations
            };

        // build the new remaining vector for the positions further down in the tree by collecting
        // all elements from change_pos + 1
        let mut working_rem: Vec<usize> = self.state.iter().skip(change_pos).cloned().collect();

        // change the state of the change_pos to have the next required element
        let new_element = self.remaining[change_pos].pop().unwrap();
        self.state[change_pos] = new_element;

        // remove the new_element from the working_rem
        working_rem.remove(working_rem.iter().position(|x| *x == new_element).unwrap());

        // sort the working_rem in order to get the lexicographic ordering. Sorting is done in
        // reverse order
        working_rem.shuffle(&mut self.rng);

        // Now, we have working_rem as all elements which can still be chosen by any of the next
        // positions. Thus, we build the next elements up iteratively:
        for pos in (change_pos + 1)..self.len {
            self.state[pos] = working_rem.pop().unwrap();
            self.remaining[pos] = working_rem.clone();
        }

        Some(self.state.iter().map(|idx| self.data.get(*idx).unwrap()).cloned().collect())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Debug)]
    enum Elems {
        A,
        B,
        C,
        D,
    }

    use Elems::*;

    type CurrentPermutator = RandomTreePermutator<Elems>;

    #[test]
    fn test_tree_0() {
        let data: Vec<Elems> = vec![];
        let permutations: Vec<Vec<Elems>> = CurrentPermutator::new(data).collect();
        assert_eq!(permutations.len(), 1);
        assert!(permutations.iter().all(|p| p.len() == 0));
    }

    #[test]
    fn test_tree_1() {
        let data: Vec<Elems> = vec![A];
        let permutations: Vec<Vec<Elems>> = CurrentPermutator::new(data).collect();
        assert_eq!(permutations.len(), 1);
        assert!(permutations.iter().all(|p| p.len() == 1));
    }

    #[test]
    fn test_tree_2() {
        let data: Vec<Elems> = vec![A, B];
        let permutations: Vec<Vec<Elems>> = CurrentPermutator::new(data).collect();
        assert_eq!(permutations.len(), 2);
        assert!(permutations.iter().all(|p| p.len() == 2));
    }

    #[test]
    fn test_tree_3() {
        let data: Vec<Elems> = vec![A, B, C];
        let permutations: Vec<Vec<Elems>> = CurrentPermutator::new(data).collect();
        assert_eq!(permutations.len(), 6);
        assert!(permutations.iter().all(|p| p.len() == 3));
    }

    #[test]
    fn test_tree_4() {
        let data: Vec<Elems> = vec![A, B, C, D];
        let permutations: Vec<Vec<Elems>> = CurrentPermutator::new(data).collect();
        assert_eq!(permutations.len(), 24);
        assert!(permutations.iter().all(|p| p.len() == 4));
    }

    #[test]
    fn test_tree_skip() {
        let data: Vec<Elems> = vec![A, B, C, D];
        let mut permutator = CurrentPermutator::new(data);
        let mut permutations: Vec<Vec<Elems>> = Vec::new();
        // push ABCD
        permutations.push(permutator.next().unwrap());
        // tell that A* does not work
        permutator.fail_pos(0);
        // push BACD
        permutations.push(permutator.next().unwrap());
        // tell that BA* does not work
        permutator.fail_pos(1);
        // push BCAD
        permutations.push(permutator.next().unwrap());
        // tell that BCA* does not work
        permutator.fail_pos(2);
        // push BCDA
        permutations.push(permutator.next().unwrap());
        // tell that BCDA does not work
        permutator.fail_pos(3);
        //push all remaining elements
        while let Some(e) = permutator.next() {
            permutations.push(e);
        }

        // compare
        assert_eq!(permutations.len(), 18);
        assert!(permutations.iter().all(|p| p.len() == 4));
    }
}
