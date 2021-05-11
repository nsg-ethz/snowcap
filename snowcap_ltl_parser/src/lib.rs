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

//! # LTL Parser
//!
//! This module provides macros to generate LTL expressions more easily.
//!

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse_macro_input, BinOp, Error, Expr, ExprBinary, ExprCall, ExprLit, ExprParen, ExprPath,
    ExprUnary, Lit, Result, UnOp,
};

/// # Generate LTL Expressions from the provided tokens
///
/// The result will be wrapped into a `LTLModal::Now` structure.
///
/// ## Allowed Tokens
/// - Literals, like `true`, `false`, and numbers t index propositional variables
/// - `!`, `-`: `LTLBoolean::Not`
/// - `+`, `||`, `|`: `LTLBoolean::Or`
/// - `*`, `&&`, `&`: `LTLBoolean::And`
/// - `^`: `LTLBoolean::Xor`
/// - `==`: `LTLBoolean::Iff`
/// - `>>`, `>`: `LTLBoolean::Implies`
/// - `<<`, `<`, `<=`: `LTLBoolean::Implies`, but in reverse
/// - `Not(_)`, `not(_)`: `LTLBoolean::Not`
/// - `Or(_, ..)`, `or(_, ..)`: `LTLBoolean::Or`
/// - `And(_, ..)`, `and(_, ..)`: `LTLBoolean::And`
/// - `Xor(_, _)`, `xor(_, _)`: `LTLBoolean::Xor`
/// - `Implies(_, _)`, `implies(_, _)`: `LTLBoolean::Implies`
/// - `Iff(_, _)`, `iff(_, _)`: `LTLBoolean::Iff`
/// - `X(_)`, `x(_)`, `N(_)`, `n(_)`, `Next(_)`, `next(_)`: `LTLModal::Next`
/// - `F(_)`, `f(_)`, `Finally(_)`, `finally(_)`: `LTLModal::Finally`
/// - `G(_)`, `f(_)`, `Globally(_)`, `globally(_)`: `LTLModal::Globally`
/// - `U(_, _)`, `u(_, _)`, `Until(_, _)`, `until(_, _)`: `LTLModal::Until`
/// - `R(_, _)`, `r(_, _)`, `Release(_, _)`, `release(_, _)`: `LTLModal::Release`
/// - `W(_, _)`, `w(_, _)`, `WeakUntil(_, _)`: `LTLModal::WeakUntil`
/// - `M(_, _)`, `m(_, _)`, `StrongRelease(_, _)`: `LTLModal::StrongRelease`
#[proc_macro]
pub fn ltl(input: TokenStream) -> TokenStream {
    let e = parse_macro_input!(input as Expr);

    match parse_recursive(e) {
        Ok(result) => TokenStream::from(quote! {snowcap::hard_policies::LTLModal::Now(#result)}),
        Err(e) => e.to_compile_error().into(),
    }
}

fn parse_recursive(e: Expr) -> Result<TokenStream2> {
    match e {
        Expr::Lit(ExprLit {
            lit: Lit::Int(i), ..
        }) => Ok(quote! {Box::new(#i)}.into()),
        Expr::Lit(ExprLit {
            lit: Lit::Bool(b), ..
        }) => Ok(quote! {Box::new(#b)}.into()),
        Expr::Unary(ExprUnary {
            op: UnOp::Neg(_),
            expr,
            ..
        })
        | Expr::Unary(ExprUnary {
            op: UnOp::Not(_),
            expr,
            ..
        }) => {
            let content = parse_recursive(*expr)?;
            Ok(quote! {Box::new(snowcap::hard_policies::LTLBoolean::Not(#content))})
        }
        Expr::Binary(ExprBinary {
            op,
            left,
            right,
            attrs,
        }) => {
            let l = parse_recursive(*left.clone())?;
            let r = parse_recursive(*right.clone())?;
            match op {
                BinOp::Add(_) | BinOp::Or(_) | BinOp::BitOr(_) => Ok(quote! {
                    Box::new(snowcap::hard_policies::LTLBoolean::Or(vec![#l, #r]))
                }),
                BinOp::Mul(_) | BinOp::And(_) | BinOp::BitAnd(_) => Ok(quote! {
                    Box::new(snowcap::hard_policies::LTLBoolean::And(vec![#l, #r]))
                }),
                BinOp::BitXor(_) => Ok(quote! {
                    Box::new(snowcap::hard_policies::LTLBoolean::Xor(#l, #r))
                }),
                BinOp::Eq(_) => Ok(quote! {
                    Box::new(snowcap::hard_policies::LTLBoolean::Iff(#l, #r))
                }),
                BinOp::Shr(_) | BinOp::Gt(_) => Ok(quote! {
                    Box::new(snowcap::hard_policies::LTLBoolean::Implies(#l, #r))
                }),
                BinOp::Shl(_) | BinOp::Lt(_) | BinOp::Le(_) => Ok(quote! {
                    Box::new(snowcap::hard_policies::LTLBoolean::Implies(#r, #l))
                }),
                _ => Err(Error::new_spanned(
                    ExprBinary {
                        attrs,
                        left,
                        op,
                        right,
                    },
                    format!("Unknown binary operator: {:?}", op),
                )),
            }
        }
        Expr::Paren(ExprParen { expr, .. }) => parse_recursive(*expr),
        Expr::Call(ExprCall { func, args, .. }) => {
            // check the function name
            let func_ident = if let Expr::Path(ExprPath { path, .. }) = *func.clone() {
                if let Some(ident) = path.get_ident() {
                    ident.to_string()
                } else {
                    return Err(Error::new_spanned(
                        path.clone(),
                        format!("Invalid function: {:?}", path),
                    ));
                }
            } else {
                return Err(Error::new_spanned(
                    func.clone(),
                    format!("Invalid function: {:?}", func),
                ));
            };
            let args = args
                .iter()
                .map(|e| parse_recursive(e.clone()))
                .collect::<Result<Vec<_>>>()?;

            let args_len = args.len();

            match func_ident.as_str() {
                "X" | "x" | "N" | "n" | "Next" | "next" => {
                    if args_len != 1 {
                        Err(Error::new_spanned(
                            func.clone(),
                            "Invalid number of arguments for \"Next\"",
                        ))
                    } else {
                        let a = args[0].clone();
                        Ok(quote! {Box::new(snowcap::hard_policies::LTLModal::Next(#a))})
                    }
                }
                "F" | "f" | "Finally" | "finally" => {
                    if args_len != 1 {
                        Err(Error::new_spanned(
                            func.clone(),
                            "Invalid number of arguments for \"Finally\"",
                        ))
                    } else {
                        let a = args[0].clone();
                        Ok(quote! {Box::new(snowcap::hard_policies::LTLModal::Finally(#a))})
                    }
                }
                "G" | "g" | "Globally" | "globally" => {
                    if args_len != 1 {
                        Err(Error::new_spanned(
                            func.clone(),
                            "Invalid number of arguments for \"Globally\"",
                        ))
                    } else {
                        let a = args[0].clone();
                        Ok(quote! {Box::new(snowcap::hard_policies::LTLModal::Globally(#a))})
                    }
                }
                "U" | "u" | "Until" | "until" => {
                    if args_len != 2 {
                        Err(Error::new_spanned(
                            func.clone(),
                            "Invalid number of arguments for \"Until\"",
                        ))
                    } else {
                        let a = args[0].clone();
                        let b = args[1].clone();
                        Ok(quote! {Box::new(snowcap::hard_policies::LTLModal::Until(#a, #b))})
                    }
                }
                "R" | "r" | "Release" | "release" => {
                    if args_len != 2 {
                        Err(Error::new_spanned(
                            func.clone(),
                            "Invalid number of arguments for \"Release\"",
                        ))
                    } else {
                        let a = args[0].clone();
                        let b = args[1].clone();
                        Ok(quote! {Box::new(snowcap::hard_policies::LTLModal::Release(#a, #b))})
                    }
                }
                "W" | "w" | "WeakUntil" => {
                    if args_len != 2 {
                        Err(Error::new_spanned(
                            func.clone(),
                            "Invalid number of arguments for \"WeakUntil\"",
                        ))
                    } else {
                        let a = args[0].clone();
                        let b = args[1].clone();
                        Ok(
                            quote! {Box::new(snowcap::hard_policies::LTLModal::WeakUntil(#a, #b))},
                        )
                    }
                }
                "M" | "m" | "StrongRelease" => {
                    if args_len != 2 {
                        Err(Error::new_spanned(
                            func.clone(),
                            "Invalid number of arguments for \"StrongRelease\"",
                        ))
                    } else {
                        let a = args[0].clone();
                        let b = args[1].clone();
                        Ok(
                            quote! {Box::new(snowcap::hard_policies::LTLModal::StrongRelease(#a, #b))},
                        )
                    }
                }
                "Not" | "not" => {
                    if args_len != 1 {
                        Err(Error::new_spanned(
                            func.clone(),
                            "Invalid number of arguments for \"Not\"",
                        ))
                    } else {
                        let a = args[0].clone();
                        Ok(quote! {Box::new(snowcap::hard_policies::LTLBoolean::Not(#a))})
                    }
                }
                "And" | "and" => {
                    if args_len == 0 {
                        Err(Error::new_spanned(
                            func.clone(),
                            "Invalid number of arguments for \"And\"",
                        ))
                    } else if args_len == 1 {
                        Ok(args[0].clone())
                    } else {
                        Ok(
                            quote! {Box::new(snowcap::hard_policies::LTLBoolean::And(vec![#(#args),*]))},
                        )
                    }
                }
                "Or" | "or" => {
                    if args_len == 0 {
                        Err(Error::new_spanned(
                            func.clone(),
                            "Invalid number of arguments for \"Or\"",
                        ))
                    } else if args_len == 1 {
                        Ok(args[0].clone())
                    } else {
                        Ok(
                            quote! {Box::new(snowcap::hard_policies::LTLBoolean::Or(vec![#(#args),*]))},
                        )
                    }
                }
                "Xor" | "xor" => {
                    if args_len != 2 {
                        Err(Error::new_spanned(
                            func.clone(),
                            "Invalid number of arguments for \"Xor\"",
                        ))
                    } else {
                        let a = args[0].clone();
                        let b = args[1].clone();
                        Ok(quote! {Box::new(snowcap::hard_policies::LTLBoolean::Xor(#a, #b))})
                    }
                }
                "Implies" | "implies" => {
                    if args_len != 2 {
                        Err(Error::new_spanned(
                            func.clone(),
                            "Invalid number of arguments for \"Implies\"",
                        ))
                    } else {
                        let a = args[0].clone();
                        let b = args[1].clone();
                        Ok(
                            quote! {Box::new(snowcap::hard_policies::LTLBoolean::Implies(#a, #b))},
                        )
                    }
                }
                "Iff" | "iff" => {
                    if args_len != 2 {
                        Err(Error::new_spanned(
                            func.clone(),
                            "Invalid number of arguments for \"Iff\"",
                        ))
                    } else {
                        let a = args[0].clone();
                        let b = args[1].clone();
                        Ok(quote! {Box::new(snowcap::hard_policies::LTLBoolean::Iff(#a, #b))})
                    }
                }
                _ => Err(Error::new_spanned(
                    func.clone(),
                    format!("Invalid function name: {}", func_ident),
                )),
            }
        }
        e => Err(Error::new_spanned(
            e.clone(),
            format!("Invalid expression: {:?}", e),
        )),
    }
}
