// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io;
use std::process::ExitCode;

use dream_archivetool::ArchiveError;

mod cli;

fn main() -> ExitCode {
    match cli::run_from_env(&mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        // The reader went away, as `| head` does: the output is no longer wanted.
        Err(ArchiveError::Io(err)) if err.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("ERROR: {}", cli::error_message(&err));
            ExitCode::from(1)
        }
    }
}
