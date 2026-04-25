use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use rome_archivetool::{
    AddOptions, ArchiveFormat, ArchiveTool, CreateOptions, ExtractAllOptions, ExtractOptions,
    Fo4ArchiveKind, Fo4Version, OverwriteMode, Result, Tes4Version,
};

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Inspect and manipulate Bethesda BSA and BA2 archives"
)]
struct Cli {
    /// Generate shell completion script to stdout
    #[arg(long, value_name = "SHELL", conflicts_with = "generate_manpage")]
    generate_completion: Option<Shell>,
    /// Generate roff manpage to stdout
    #[arg(long, conflicts_with = "generate_completion")]
    generate_manpage: bool,
    #[command(subcommand)]
    command: Option<Command>,
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
    /// Extract one archive entry
    Extract {
        /// Archive path
        archive: PathBuf,
        /// Entry path inside the archive
        entry: String,
        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Write file bytes to stdout
        #[arg(long)]
        stdout: bool,
        /// Discard archive directories and write only the basename
        #[arg(long)]
        flat: bool,
        /// Replace existing files
        #[arg(long, conflicts_with = "skip_existing")]
        overwrite: bool,
        /// Leave existing files untouched
        #[arg(long, conflicts_with = "overwrite")]
        skip_existing: bool,
    },
    /// Extract every archive entry
    ExtractAll {
        /// Archive path
        archive: PathBuf,
        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Replace existing files
        #[arg(long, conflicts_with = "skip_existing")]
        overwrite: bool,
        /// Leave existing files untouched
        #[arg(long, conflicts_with = "overwrite")]
        skip_existing: bool,
        /// Write JSON summary to stdout
        #[arg(long)]
        json: bool,
    },
    /// Create a new archive from a file or directory
    Create {
        /// Output archive path
        archive: PathBuf,
        /// Input file or directory
        input: PathBuf,
        /// Archive format to write
        #[arg(long, value_enum)]
        format: ArchiveFormat,
        /// TES4 BSA version
        #[arg(long, value_enum, default_value_t = Tes4Version::Oblivion)]
        tes4_version: Tes4Version,
        /// FO4 BA2 archive kind
        #[arg(long, value_enum, default_value_t = Fo4ArchiveKind::Gnrl)]
        ba2_kind: Fo4ArchiveKind,
        /// FO4 BA2 version
        #[arg(long, value_enum, default_value_t = Fo4Version::Fallout4)]
        ba2_version: Fo4Version,
        /// Write JSON summary to stdout
        #[arg(long)]
        json: bool,
    },
    /// Add or update entries by writing a new archive
    Add {
        /// Input archive path
        archive: PathBuf,
        /// Files or directories to add
        inputs: Vec<PathBuf>,
        /// Output archive path
        #[arg(short, long)]
        output: PathBuf,
        /// Write JSON summary to stdout
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
    if let Some(shell) = cli.generate_completion {
        clap_complete::generate(shell, &mut Cli::command(), "rome-archivetool", stdout);
        return Ok(());
    }
    if cli.generate_manpage {
        clap_mangen::Man::new(Cli::command()).render(stdout)?;
        return Ok(());
    }

    let Some(command) = cli.command else {
        write!(stdout, "{}", Cli::command().render_help())?;
        return Ok(());
    };

    match command {
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
        Command::Extract {
            archive,
            entry,
            output,
            stdout: stdout_mode,
            flat,
            overwrite,
            skip_existing,
        } => {
            if stdout_mode {
                let bytes = ArchiveTool::read_entry(&archive, &entry)?;
                stdout.write_all(&bytes)?;
                return Ok(());
            }
            let options = ExtractOptions {
                output,
                overwrite: overwrite_mode(overwrite, skip_existing),
                preserve_paths: !flat,
            };
            let summary = ArchiveTool::extract(archive, &entry, &options)?;
            writeln!(stdout, "extracted: {}", summary.extracted)?;
            if summary.skipped > 0 {
                writeln!(stdout, "skipped: {}", summary.skipped)?;
            }
        }
        Command::ExtractAll {
            archive,
            output,
            overwrite,
            skip_existing,
            json,
        } => {
            let options = ExtractAllOptions {
                output,
                overwrite: overwrite_mode(overwrite, skip_existing),
            };
            let summary = ArchiveTool::extract_all(archive, &options)?;
            if json {
                serde_json::to_writer_pretty(&mut *stdout, &summary)
                    .map_err(|err| rome_archivetool::ArchiveError::Archive(err.to_string()))?;
                writeln!(stdout)?;
            } else {
                writeln!(stdout, "extracted: {}", summary.extracted)?;
                if summary.skipped > 0 {
                    writeln!(stdout, "skipped: {}", summary.skipped)?;
                }
            }
        }
        Command::Create {
            archive,
            input,
            format,
            tes4_version,
            ba2_kind,
            ba2_version,
            json,
        } => {
            let count = ArchiveTool::create(
                archive,
                input,
                &CreateOptions {
                    format,
                    tes4_version,
                    fo4_kind: ba2_kind,
                    fo4_version: ba2_version,
                },
            )?;
            write_count(stdout, count, json)?;
        }
        Command::Add {
            archive,
            inputs,
            output,
            json,
        } => {
            if inputs.is_empty() {
                return Err(rome_archivetool::ArchiveError::Archive(
                    "no input files supplied".to_string(),
                ));
            }
            let count = ArchiveTool::add(archive, &AddOptions { inputs, output })?;
            write_count(stdout, count, json)?;
        }
    }
    Ok(())
}

fn write_count(stdout: &mut dyn Write, count: usize, json: bool) -> Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *stdout, &serde_json::json!({ "files": count }))
            .map_err(|err| rome_archivetool::ArchiveError::Archive(err.to_string()))?;
        writeln!(stdout)?;
    } else {
        writeln!(stdout, "files: {count}")?;
    }
    Ok(())
}

fn overwrite_mode(overwrite: bool, skip_existing: bool) -> OverwriteMode {
    if overwrite {
        OverwriteMode::Overwrite
    } else if skip_existing {
        OverwriteMode::Skip
    } else {
        OverwriteMode::Fail
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use clap::Parser;

    use super::*;

    fn unique_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rome-archivetool-cli-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_tes3_archive(path: &Path) {
        let archive: ba2::tes3::Archive = [
            (
                ba2::tes3::ArchiveKey::from(b"icons/example.dds".as_slice()),
                ba2::tes3::File::from(b"payload".as_slice()),
            ),
            (
                ba2::tes3::ArchiveKey::from(b"meshes/example.nif".as_slice()),
                ba2::tes3::File::from(b"mesh".as_slice()),
            ),
        ]
        .into_iter()
        .collect();
        let mut output = fs::File::create(path).unwrap();
        archive.write(&mut output).unwrap();
    }

    #[test]
    fn list_command_writes_entry_names() {
        let dir = unique_dir("list");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);
        let mut stdout = Vec::new();

        run(
            Cli::parse_from(["rome-archivetool", "list", archive_path.to_str().unwrap()]),
            &mut stdout,
        )
        .unwrap();

        let output = String::from_utf8(stdout).unwrap();
        assert!(output.contains("icons/example.dds\n"));
        assert!(output.contains("meshes/example.nif\n"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn info_command_can_write_json() {
        let dir = unique_dir("info-json");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);
        let mut stdout = Vec::new();

        run(
            Cli::parse_from([
                "rome-archivetool",
                "info",
                archive_path.to_str().unwrap(),
                "--json",
            ]),
            &mut stdout,
        )
        .unwrap();

        let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(value["format"], "tes3");
        assert_eq!(value["file_count"], 2);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn list_command_can_write_json() {
        let dir = unique_dir("list-json");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);
        let mut stdout = Vec::new();

        run(
            Cli::parse_from([
                "rome-archivetool",
                "list",
                archive_path.to_str().unwrap(),
                "--json",
            ]),
            &mut stdout,
        )
        .unwrap();

        let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(value.as_array().unwrap().len(), 2);
        assert!(
            value
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["path"] == "icons/example.dds")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn extract_command_can_write_bytes_to_stdout() {
        let dir = unique_dir("extract-stdout");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);
        let mut stdout = Vec::new();

        run(
            Cli::parse_from([
                "rome-archivetool",
                "extract",
                archive_path.to_str().unwrap(),
                "icons/example.dds",
                "--stdout",
            ]),
            &mut stdout,
        )
        .unwrap();

        assert_eq!(stdout, b"payload");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn extract_command_can_flatten_paths() {
        let dir = unique_dir("extract-flat");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        let output = dir.join("out");
        write_tes3_archive(&archive_path);
        let mut stdout = Vec::new();

        run(
            Cli::parse_from([
                "rome-archivetool",
                "extract",
                archive_path.to_str().unwrap(),
                "icons/example.dds",
                "--output",
                output.to_str().unwrap(),
                "--flat",
            ]),
            &mut stdout,
        )
        .unwrap();

        assert_eq!(fs::read(output.join("example.dds")).unwrap(), b"payload");
        assert!(!output.join("icons/example.dds").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn extract_all_command_can_skip_existing_and_write_json() {
        let dir = unique_dir("extract-all-json");
        let output = dir.join("out");
        fs::create_dir_all(output.join("icons")).unwrap();
        fs::write(output.join("icons/example.dds"), b"existing").unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);
        let mut stdout = Vec::new();

        run(
            Cli::parse_from([
                "rome-archivetool",
                "extract-all",
                archive_path.to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
                "--skip-existing",
                "--json",
            ]),
            &mut stdout,
        )
        .unwrap();

        let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(value["extracted"], 1);
        assert_eq!(value["skipped"], 1);
        assert_eq!(
            fs::read(output.join("icons/example.dds")).unwrap(),
            b"existing"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn create_command_writes_tes3_archive() {
        let dir = unique_dir("create");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("hello.txt"), b"hello").unwrap();
        let archive = dir.join("out.bsa");
        let mut stdout = Vec::new();

        run(
            Cli::parse_from([
                "rome-archivetool",
                "create",
                archive.to_str().unwrap(),
                input.to_str().unwrap(),
                "--format",
                "tes3",
            ]),
            &mut stdout,
        )
        .unwrap();

        assert_eq!(String::from_utf8(stdout).unwrap(), "files: 1\n");
        assert_eq!(
            ArchiveTool::read_entry(&archive, "hello.txt").unwrap(),
            b"hello"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn add_command_can_write_json() {
        let dir = unique_dir("add-json");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("base.txt"), b"base").unwrap();
        let archive = dir.join("base.bsa");
        ArchiveTool::create(&archive, &input, &CreateOptions::default()).unwrap();
        let added = dir.join("added.txt");
        fs::write(&added, b"added").unwrap();
        let output = dir.join("updated.bsa");
        let mut stdout = Vec::new();

        run(
            Cli::parse_from([
                "rome-archivetool",
                "add",
                archive.to_str().unwrap(),
                added.to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
                "--json",
            ]),
            &mut stdout,
        )
        .unwrap();

        let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(value["files"], 2);
        assert_eq!(
            ArchiveTool::read_entry(&output, "added.txt").unwrap(),
            b"added"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn add_command_rejects_empty_inputs() {
        let dir = unique_dir("add-empty");
        fs::create_dir_all(&dir).unwrap();
        let archive = dir.join("base.bsa");
        write_tes3_archive(&archive);
        let output = dir.join("updated.bsa");
        let mut stdout = Vec::new();

        let err = run(
            Cli::parse_from([
                "rome-archivetool",
                "add",
                archive.to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
            ]),
            &mut stdout,
        )
        .unwrap_err();

        assert!(err.to_string().contains("no input files supplied"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn generation_options_do_not_require_subcommand() {
        let mut stdout = Vec::new();
        run(
            Cli::parse_from(["rome-archivetool", "--generate-completion", "bash"]),
            &mut stdout,
        )
        .unwrap();
        assert!(
            String::from_utf8(stdout)
                .unwrap()
                .contains("rome-archivetool")
        );

        let mut stdout = Vec::new();
        run(
            Cli::parse_from(["rome-archivetool", "--generate-manpage"]),
            &mut stdout,
        )
        .unwrap();
        assert!(
            String::from_utf8(stdout)
                .unwrap()
                .contains("rome-archivetool")
        );
    }
}
