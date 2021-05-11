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

//! # Steinhaus-Johnson-Trotter Permutator

use super::Permutator;
use crate::modifier_ordering::ModifierOrdering;
use crate::netsim::config::ConfigModifier;

use std::cmp::Ordering;
use std::marker::PhantomData;

/// # Steinhaus-Johnson-Trotter Permutator
///
/// This is an implementation of the Steinhaus-Johnson-Trotter Permutator Algorithm as described
/// [here](https://en.wikipedia.org/wiki/Steinhaus%E2%80%93Johnson%E2%80%93Trotter_algorithm). It
/// is implemented using Even's speedup. Additionally, it is implemented for any type `T`, which
/// implements `Copy`.
pub struct SJTPermutator<O, T = ConfigModifier> {
    data: Vec<T>,
    indices: Vec<usize>,
    dirs: Vec<Direction>,
    len: usize,
    started: bool,
    phantom: PhantomData<O>,
}

impl<O, T> Permutator<T> for SJTPermutator<O, T>
where
    O: ModifierOrdering<T>,
    T: Clone,
{
    fn new(mut input: Vec<T>) -> Self {
        O::sort(&mut input);
        let len = input.len();
        let mut dirs: Vec<Direction> = Vec::with_capacity(len);
        for i in 0..len {
            if i == 0 {
                dirs.push(Direction::None);
            } else {
                dirs.push(Direction::Left);
            }
        }
        Self {
            data: input,
            indices: (0..len).collect(),
            dirs,
            len,
            started: false,
            phantom: PhantomData,
        }
    }
}

impl<O, T> Iterator for SJTPermutator<O, T>
where
    O: ModifierOrdering<T>,
    T: Clone,
{
    type Item = Vec<T>;

    fn next(&mut self) -> Option<Self::Item> {
        // handle the start
        if !self.started {
            self.started = true;
            return Some(self.data.clone());
        }

        // search for the greatest (in terms of index) non-zero direction
        let current_pos = self
            .indices
            .iter()
            .zip(self.dirs.iter())
            .enumerate()
            .filter(|(_, (_, dir))| **dir != Direction::None)
            .max_by(|(_, (i, _)), (_, (j, _))| i.cmp(j))
            .map(|(pos, _)| pos);

        match current_pos {
            Some(current_pos) => {
                // get the target position
                let current_dir = self.dirs[current_pos];
                let current_idx = self.indices[current_pos];
                let target_pos = match current_dir {
                    Direction::Left => current_pos - 1,
                    Direction::Right => current_pos + 1,
                    Direction::None => unreachable!(),
                };

                // the direction must be set to zero if either the targegt pos is at the border, or
                // if the element after the target pos in the same direction is greater than the
                // current element
                if target_pos == 0 || target_pos + 1 == self.len {
                    self.dirs[current_pos] = Direction::None;
                } else {
                    let one_after_pos = match current_dir {
                        Direction::Left => target_pos - 1,
                        Direction::Right => target_pos + 1,
                        Direction::None => unreachable!(),
                    };
                    if current_idx < self.indices[one_after_pos] {
                        self.dirs[current_pos] = Direction::None;
                    }
                }

                // swap the elements
                self.indices.swap(current_pos, target_pos);
                self.dirs.swap(current_pos, target_pos);

                // set all elements greater than the chosen element to have their direction set
                // towards the chosen element:
                let directions_to_set: Vec<(usize, Direction)> = self
                    .indices
                    .iter()
                    .enumerate()
                    .filter(|(_, x)| **x > current_idx)
                    .map(|(i, _)| (i, Direction::towards(i, target_pos)))
                    .collect();
                directions_to_set.into_iter().for_each(|(i, new_dir)| self.dirs[i] = new_dir);

                // return the vector built from the indices
                Some(self.indices.iter().map(|pos| self.data.get(*pos).unwrap().clone()).collect())
            }

            None => {
                // Iteration finished
                None
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Direction {
    Left,
    None,
    Right,
}

impl Direction {
    /// Returns the direction for the element at pos to move towards the current position
    pub fn towards(pos: usize, current: usize) -> Direction {
        match pos.cmp(&current) {
            Ordering::Less => Direction::Right,
            Ordering::Greater => Direction::Left,
            Ordering::Equal => Direction::None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::modifier_ordering::NoOrdering;

    #[derive(Clone, Copy, PartialEq, Debug)]
    enum Elems {
        A,
        B,
        C,
        D,
    }

    use Elems::*;

    type CurrentPermutator = SJTPermutator<NoOrdering, Elems>;

    #[test]
    fn test_sjt_permutator_0() {
        let data: Vec<Elems> = vec![];
        let permutations: Vec<Vec<Elems>> = CurrentPermutator::new(data).collect();
        assert_eq!(permutations, vec![vec![]]);
    }

    #[test]
    fn test_sjt_permutator_1() {
        let data: Vec<Elems> = vec![A];
        let permutations: Vec<Vec<Elems>> = CurrentPermutator::new(data).collect();
        assert_eq!(permutations, vec![vec![A]]);
    }

    #[test]
    fn test_sjt_permutator_2() {
        let data: Vec<Elems> = vec![A, B];
        let permutations: Vec<Vec<Elems>> = CurrentPermutator::new(data).collect();
        assert_eq!(permutations, vec![vec![A, B], vec![B, A]]);
    }

    #[test]
    fn test_sjt_permutator_3() {
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
    fn test_sjt_permutator_4() {
        let data: Vec<Elems> = vec![A, B, C, D];
        let permutations: Vec<Vec<Elems>> = CurrentPermutator::new(data).collect();
        assert_eq!(
            permutations,
            vec![
                vec![A, B, C, D],
                vec![A, B, D, C],
                vec![A, D, B, C],
                vec![D, A, B, C],
                vec![D, A, C, B],
                vec![A, D, C, B],
                vec![A, C, D, B],
                vec![A, C, B, D],
                vec![C, A, B, D],
                vec![C, A, D, B],
                vec![C, D, A, B],
                vec![D, C, A, B],
                vec![D, C, B, A],
                vec![C, D, B, A],
                vec![C, B, D, A],
                vec![C, B, A, D],
                vec![B, C, A, D],
                vec![B, C, D, A],
                vec![B, D, C, A],
                vec![D, B, C, A],
                vec![D, B, A, C],
                vec![B, D, A, C],
                vec![B, A, D, C],
                vec![B, A, C, D],
            ]
        );
    }
}
