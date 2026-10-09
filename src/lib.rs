// -*- mode: rust; coding: utf-8-unix -*-

// SPDX-License-Identifier: MIT
//
// SPDX-FileCopyrightText: Copyright Kristóf Ralovich (C) 2025-2026.
// All rights reserved.

mod blocks;
pub mod common;
mod compression;
pub mod interface;

#[allow(unused)]
const LIBU3D_VERSION: &str = env!("CARGO_PKG_VERSION");
#[allow(unused)]
const LIBU3D_VERSION_MAJOR: &str = env!("CARGO_PKG_VERSION_MAJOR");
#[allow(unused)]
const LIBU3D_VERSION_MINOR: &str = env!("CARGO_PKG_VERSION_MINOR");
#[allow(unused)]
const LIBU3D_VERSION_PATCH: &str = env!("CARGO_PKG_VERSION_PATCH");
