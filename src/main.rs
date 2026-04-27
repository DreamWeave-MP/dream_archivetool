#[cfg(feature = "cli")]
use std::io;
use std::process::ExitCode;

#[cfg(feature = "cli")]
mod cli;

#[cfg(feature = "cli")]
fn main() -> ExitCode {
    match cli::run_from_env(&mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("ERROR: {err}");
            ExitCode::from(1)
        }
    }
}

#[cfg(not(feature = "cli"))]
fn main() -> ExitCode {
    eprintln!(
        "dream-archivetool was built without the 'cli' feature; enable default features or build with --features cli"
    );
    ExitCode::from(1)
}
