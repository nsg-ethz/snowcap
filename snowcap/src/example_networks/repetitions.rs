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

//! Repetitions for the diffiult gadget

/// Trait for encoding the number of repetitions as a type.
pub trait Repetitions {
    /// Get the number of repetitions
    fn get_count() -> usize;
}

/// One repetition
pub struct Repetition1 {}

impl Repetitions for Repetition1 {
    fn get_count() -> usize {
        1
    }
}

/// Two repetitions
pub struct Repetition2 {}

impl Repetitions for Repetition2 {
    fn get_count() -> usize {
        2
    }
}

/// Three repetitions
pub struct Repetition3 {}

impl Repetitions for Repetition3 {
    fn get_count() -> usize {
        3
    }
}

/// Four repetitions
pub struct Repetition4 {}

impl Repetitions for Repetition4 {
    fn get_count() -> usize {
        4
    }
}

/// Five repetitions
pub struct Repetition5 {}

impl Repetitions for Repetition5 {
    fn get_count() -> usize {
        5
    }
}

/// Six repetitions
pub struct Repetition6 {}

impl Repetitions for Repetition6 {
    fn get_count() -> usize {
        6
    }
}

/// Seven repetitions
pub struct Repetition7 {}

impl Repetitions for Repetition7 {
    fn get_count() -> usize {
        7
    }
}

/// Eight repetitions
pub struct Repetition8 {}

impl Repetitions for Repetition8 {
    fn get_count() -> usize {
        8
    }
}

/// Nine repetitions
pub struct Repetition9 {}

impl Repetitions for Repetition9 {
    fn get_count() -> usize {
        9
    }
}

/// 10 repetitions
pub struct Repetition10 {}

impl Repetitions for Repetition10 {
    fn get_count() -> usize {
        10
    }
}

/// 11 repetition
pub struct Repetition11 {}

impl Repetitions for Repetition11 {
    fn get_count() -> usize {
        11
    }
}

/// 12 repetitions
pub struct Repetition12 {}

impl Repetitions for Repetition12 {
    fn get_count() -> usize {
        12
    }
}

/// 13 repetitions
pub struct Repetition13 {}

impl Repetitions for Repetition13 {
    fn get_count() -> usize {
        13
    }
}

/// 14 repetitions
pub struct Repetition14 {}

impl Repetitions for Repetition14 {
    fn get_count() -> usize {
        14
    }
}

/// 15 repetitions
pub struct Repetition15 {}

impl Repetitions for Repetition15 {
    fn get_count() -> usize {
        15
    }
}

/// 16 repetitions
pub struct Repetition16 {}

impl Repetitions for Repetition16 {
    fn get_count() -> usize {
        16
    }
}

/// 17 repetitions
pub struct Repetition17 {}

impl Repetitions for Repetition17 {
    fn get_count() -> usize {
        17
    }
}

/// 18 repetitions
pub struct Repetition18 {}

impl Repetitions for Repetition18 {
    fn get_count() -> usize {
        18
    }
}

/// 19 repetitions
pub struct Repetition19 {}

impl Repetitions for Repetition19 {
    fn get_count() -> usize {
        19
    }
}

/// 20 repetitions
pub struct Repetition20 {}

impl Repetitions for Repetition20 {
    fn get_count() -> usize {
        20
    }
}

/// 30 repetitions
pub struct Repetition30 {}

impl Repetitions for Repetition30 {
    fn get_count() -> usize {
        30
    }
}

/// 40 repetitions
pub struct Repetition40 {}

impl Repetitions for Repetition40 {
    fn get_count() -> usize {
        40
    }
}

/// 50 repetitions
pub struct Repetition50 {}

impl Repetitions for Repetition50 {
    fn get_count() -> usize {
        50
    }
}

/// 60 repetitions
pub struct Repetition60 {}

impl Repetitions for Repetition60 {
    fn get_count() -> usize {
        60
    }
}

/// 70 repetitions
pub struct Repetition70 {}

impl Repetitions for Repetition70 {
    fn get_count() -> usize {
        70
    }
}

/// 80 repetitions
pub struct Repetition80 {}

impl Repetitions for Repetition80 {
    fn get_count() -> usize {
        80
    }
}

/// 90 repetitions
pub struct Repetition90 {}

impl Repetitions for Repetition90 {
    fn get_count() -> usize {
        90
    }
}

/// 100 repetitions
pub struct Repetition100 {}

impl Repetitions for Repetition100 {
    fn get_count() -> usize {
        100
    }
}
