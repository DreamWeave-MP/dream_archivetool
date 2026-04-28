// SPDX-License-Identifier: GPL-3.0-or-later

use std::io;
use std::process::ExitCode;

mod cli;

fn main() -> ExitCode {
    match cli::run_from_env(&mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("ERROR: {err}");
            ExitCode::from(1)
        }
    }
}
