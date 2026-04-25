use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use rome_archivetool::{ArchiveTool, Result};

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Inspect and manipulate Bethesda BSA and BA2 archives"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print archive metadata
    Info {
        /// Archive path
        archive: PathBuf,
        /// Write JSON to stdout
        #[arg(long)]
        json: bool,
    },
    /// List archive entries
    List {
        /// Archive path
        archive: PathBuf,
        /// Include entry sizes
        #[arg(short, long)]
        long: bool,
        /// Write JSON to stdout
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse(), &mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("ERROR: {err}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli, stdout: &mut dyn Write) -> Result<()> {
    match cli.command {
        Command::Info { archive, json } => {
            let info = ArchiveTool::info(archive)?;
            if json {
                serde_json::to_writer_pretty(&mut *stdout, &info)
                    .map_err(|err| rome_archivetool::ArchiveError::Archive(err.to_string()))?;
                writeln!(stdout)?;
            } else {
                writeln!(stdout, "format: {:?}", info.format)?;
                writeln!(stdout, "files: {}", info.file_count)?;
            }
        }
        Command::List {
            archive,
            long,
            json,
        } => {
            let entries = ArchiveTool::list(archive)?;
            if json {
                serde_json::to_writer_pretty(&mut *stdout, &entries)
                    .map_err(|err| rome_archivetool::ArchiveError::Archive(err.to_string()))?;
                writeln!(stdout)?;
            } else {
                for entry in entries {
                    if long {
                        let size = entry
                            .size
                            .map_or_else(|| "-".to_string(), |size| size.to_string());
                        writeln!(stdout, "{size:>10} {}", entry.path)?;
                    } else {
                        writeln!(stdout, "{}", entry.path)?;
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use clap::Parser;

    use super::*;

    #[test]
    fn list_command_writes_entry_names() {
        let dir = std::env::temp_dir().join(format!(
            "rome-archivetool-cli-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        let archive: ba2::tes3::Archive = [(
            ba2::tes3::ArchiveKey::from(b"icons/example.dds".as_slice()),
            ba2::tes3::File::from(b"payload".as_slice()),
        )]
        .into_iter()
        .collect();
        let mut output = fs::File::create(&archive_path).unwrap();
        archive.write(&mut output).unwrap();
        let mut stdout = Vec::new();

        run(
            Cli::parse_from(["rome-archivetool", "list", archive_path.to_str().unwrap()]),
            &mut stdout,
        )
        .unwrap();

        assert_eq!(String::from_utf8(stdout).unwrap(), "icons/example.dds\n");

        fs::remove_dir_all(dir).unwrap();
    }
}
