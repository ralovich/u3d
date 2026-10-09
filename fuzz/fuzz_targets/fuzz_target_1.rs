// -*- mode: rust; coding: utf-8-unix -*-

// SPDX-License-Identifier: MIT
//
// SPDX-FileCopyrightText: Copyright Kristóf Ralovich (C) 2025-2026.
// All rights reserved.

#![no_main]

use libfuzzer_sys::fuzz_target;

extern crate u3d;

fuzz_target!(|data: &[u8]| {
    let file_name = "fuzz_target_1.u3d".to_owned();
    // fuzzed code goes here
    let _ = u3d::common::u3d_parse(data, &file_name);
});
