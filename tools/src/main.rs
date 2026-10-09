// -*- mode: rust; coding: utf-8-unix -*-

// SPDX-License-Identifier: MIT
//
// SPDX-FileCopyrightText: Copyright Kristóf Ralovich (C) 2025-2026.
// All rights reserved.

use clap::Parser;
use log::{info, warn};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};
use u3d::common::*;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Sets the file to describe
    //#[arg(short, long, value_name = "FILE")]
    file_name: PathBuf,
}

fn main() -> ExitCode {
    // RUST_LOG=trace cargo run
    pretty_env_logger::init();

    let args = Args::parse();

    let file_name = args.file_name.into_os_string().into_string().unwrap();

    info!(
        "The current time is {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    info!(
        "The current time is {}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let parsed = u3d_parse_file(&file_name);
    match parsed {
        Ok(_data) => ExitCode::SUCCESS,
        Err(why) => {
            warn!("u3d-describe_file FAILED: {}", why);
            ExitCode::from(101)
        }
    }
}
