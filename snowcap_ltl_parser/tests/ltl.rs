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

#![cfg(test)]

#[macro_use]
extern crate snowcap_ltl_parser;
use snowcap::hard_policies::*;

#[test]
fn now_bool() {
    assert_eq!(ltl!(true).repr(), "true");
    assert_eq!(ltl!(false).repr(), "false");
}

#[test]
fn now_num() {
    assert_eq!(ltl!(0).repr(), "x00");
    assert_eq!(ltl!(1).repr(), "x01");
}

#[test]
fn now_boolean() {
    assert_eq!(ltl!(!1).repr(), "!x01");
    assert_eq!(ltl!(0 || 1).repr(), "(x00 || x01)");
    assert_eq!(ltl!(1 && 2).repr(), "(x01 && x02)");
    assert_eq!(ltl!(1 ^ 2).repr(), "(x01 ^^ x02)");
    assert_eq!(ltl!(1 > 2).repr(), "(x01 => x02)");
    assert_eq!(ltl!(1 >> 2).repr(), "(x01 => x02)");
    assert_eq!(ltl!(1 < 2).repr(), "(x02 => x01)");
    assert_eq!(ltl!(1 << 2).repr(), "(x02 => x01)");
    assert_eq!(ltl!(1 <= 2).repr(), "(x02 => x01)");
    assert_eq!(ltl!(1 == 2).repr(), "(x01 <=> x02)");
}

#[test]
fn now_boolean_long() {
    assert_eq!(ltl!(0 || 1 || 2).repr(), "((x00 || x01) || x02)");
    assert_eq!(ltl!(0 || (1 || 2)).repr(), "(x00 || (x01 || x02))");
    assert_eq!(ltl!(1 && 2 && 3).repr(), "((x01 && x02) && x03)");
}

#[test]
fn now_named_boolean() {
    assert_eq!(ltl!(And(0)).repr(), "x00");
    assert_eq!(ltl!(And(0, 1)).repr(), "(x00 && x01)");
    assert_eq!(ltl!(And(0, 1, 2, 3)).repr(), "(x00 && x01 && x02 && x03)");
    assert_eq!(ltl!(Or(0, 1, 2, 3)).repr(), "(x00 || x01 || x02 || x03)");
    assert_eq!(ltl!(and(0, 1, 2, 3)).repr(), "(x00 && x01 && x02 && x03)");
    assert_eq!(ltl!(or(0, 1, 2, 3)).repr(), "(x00 || x01 || x02 || x03)");
    assert_eq!(ltl!(Xor(0, 1)).repr(), "(x00 ^^ x01)");
    assert_eq!(ltl!(xor(0, 1)).repr(), "(x00 ^^ x01)");
    assert_eq!(ltl!(Implies(0, 1)).repr(), "(x00 => x01)");
    assert_eq!(ltl!(implies(0, 1)).repr(), "(x00 => x01)");
    assert_eq!(ltl!(Iff(0, 1)).repr(), "(x00 <=> x01)");
    assert_eq!(ltl!(iff(0, 1)).repr(), "(x00 <=> x01)");
}

#[test]
fn now_boolean_combined() {
    assert_eq!(ltl!(0 || (1 && 2)).repr(), "(x00 || (x01 && x02))");
    assert_eq!(ltl!(0 || (1 && !2)).repr(), "(x00 || (x01 && !x02))");
    assert_eq!(ltl!(0 || !(1 == !2)).repr(), "(x00 || !(x01 <=> !x02))");
}

#[test]
fn modal_simple_unary() {
    assert_eq!(ltl!(Finally(1)).repr(), "(F x01)");
    assert_eq!(ltl!(finally(1)).repr(), "(F x01)");
    assert_eq!(ltl!(F(1)).repr(), "(F x01)");
    assert_eq!(ltl!(f(1)).repr(), "(F x01)");
    assert_eq!(ltl!(Globally(1)).repr(), "(G x01)");
    assert_eq!(ltl!(globally(1)).repr(), "(G x01)");
    assert_eq!(ltl!(G(1)).repr(), "(G x01)");
    assert_eq!(ltl!(g(1)).repr(), "(G x01)");
}

#[test]
fn modal_simple_binary() {
    assert_eq!(ltl!(Until(0, 1)).repr(), "(x00 U x01)");
    assert_eq!(ltl!(until(0, 1)).repr(), "(x00 U x01)");
    assert_eq!(ltl!(U(0, 1)).repr(), "(x00 U x01)");
    assert_eq!(ltl!(u(0, 1)).repr(), "(x00 U x01)");
    assert_eq!(ltl!(Release(0, 1)).repr(), "(x00 R x01)");
    assert_eq!(ltl!(release(0, 1)).repr(), "(x00 R x01)");
    assert_eq!(ltl!(R(0, 1)).repr(), "(x00 R x01)");
    assert_eq!(ltl!(r(0, 1)).repr(), "(x00 R x01)");
    assert_eq!(ltl!(WeakUntil(0, 1)).repr(), "(x00 W x01)");
    assert_eq!(ltl!(W(0, 1)).repr(), "(x00 W x01)");
    assert_eq!(ltl!(w(0, 1)).repr(), "(x00 W x01)");
    assert_eq!(ltl!(StrongRelease(0, 1)).repr(), "(x00 M x01)");
    assert_eq!(ltl!(M(0, 1)).repr(), "(x00 M x01)");
    assert_eq!(ltl!(m(0, 1)).repr(), "(x00 M x01)");
}

#[test]
fn modal_complex() {
    assert_eq!(ltl!(U(1, G(2))).repr(), "(x01 U (G x02))");
    assert_eq!(ltl!(U(1, G(2 == 3))).repr(), "(x01 U (G (x02 <=> x03)))");
    assert_eq!(
        ltl!(U(F(1) || R(0, !1), G(2 == 3))).repr(),
        "(((F x01) || (x00 R !x01)) U (G (x02 <=> x03)))"
    );
}
