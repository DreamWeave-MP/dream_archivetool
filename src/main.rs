use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use dream_archivetool::{
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
        clap_complete::generate(shell, &mut Cli::command(), "dream-archivetool", stdout);
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

    handle_command(command, stdout)
}

fn handle_command(command: Command, stdout: &mut dyn Write) -> Result<()> {
    match command {
        Command::Info { archive, json } => write_info(stdout, archive, json),
        Command::List {
            archive,
            long,
            json,
        } => write_list(stdout, archive, long, json),
        Command::Extract {
            archive,
            entry,
            output,
            stdout: stdout_mode,
            flat,
            overwrite,
            skip_existing,
        } => write_extract(
            stdout,
            ExtractCommandOptions {
                archive,
                entry,
                output,
                stdout_mode,
                preserve_paths: !flat,
                overwrite: overwrite_mode(overwrite, skip_existing),
            },
        ),
        Command::ExtractAll {
            archive,
            output,
            overwrite,
            skip_existing,
            json,
        } => write_extract_all(stdout, archive, output, overwrite, skip_existing, json),
        Command::Create {
            archive,
            input,
            format,
            tes4_version,
            ba2_kind,
            ba2_version,
            json,
        } => write_create(
            stdout,
            archive,
            input,
            &CreateOptions {
                format,
                tes4_version,
                fo4_kind: ba2_kind,
                fo4_version: ba2_version,
            },
            json,
        ),
        Command::Add {
            archive,
            inputs,
            output,
            json,
        } => write_add(stdout, archive, inputs, output, json),
    }
}

fn write_info(stdout: &mut dyn Write, archive: PathBuf, json: bool) -> Result<()> {
    let info = ArchiveTool::info(archive)?;
    if json {
        serde_json::to_writer_pretty(&mut *stdout, &info)
            .map_err(|err| dream_archivetool::ArchiveError::Archive(err.to_string()))?;
        writeln!(stdout)?;
    } else {
        writeln!(stdout, "format: {:?}", info.format)?;
        writeln!(stdout, "files: {}", info.file_count)?;
    }
    Ok(())
}

fn write_list(stdout: &mut dyn Write, archive: PathBuf, long: bool, json: bool) -> Result<()> {
    let entries = ArchiveTool::list(archive)?;
    if json {
        serde_json::to_writer_pretty(&mut *stdout, &entries)
            .map_err(|err| dream_archivetool::ArchiveError::Archive(err.to_string()))?;
        writeln!(stdout)?;
        return Ok(());
    }
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
    Ok(())
}

struct ExtractCommandOptions {
    archive: PathBuf,
    entry: String,
    output: Option<PathBuf>,
    stdout_mode: bool,
    preserve_paths: bool,
    overwrite: OverwriteMode,
}

fn write_extract(stdout: &mut dyn Write, options: ExtractCommandOptions) -> Result<()> {
    if options.stdout_mode {
        let bytes = ArchiveTool::read_entry(&options.archive, &options.entry)?;
        stdout.write_all(&bytes)?;
        return Ok(());
    }
    let extract_options = ExtractOptions {
        output: options.output,
        overwrite: options.overwrite,
        preserve_paths: options.preserve_paths,
    };
    let summary = ArchiveTool::extract(options.archive, &options.entry, &extract_options)?;
    write_summary(stdout, &summary)
}

fn write_extract_all(
    stdout: &mut dyn Write,
    archive: PathBuf,
    output: Option<PathBuf>,
    overwrite: bool,
    skip_existing: bool,
    json: bool,
) -> Result<()> {
    let options = ExtractAllOptions {
        output,
        overwrite: overwrite_mode(overwrite, skip_existing),
    };
    let summary = ArchiveTool::extract_all(archive, &options)?;
    if json {
        serde_json::to_writer_pretty(&mut *stdout, &summary)
            .map_err(|err| dream_archivetool::ArchiveError::Archive(err.to_string()))?;
        writeln!(stdout)?;
    } else {
        write_summary(stdout, &summary)?;
    }
    Ok(())
}

fn write_create(
    stdout: &mut dyn Write,
    archive: PathBuf,
    input: PathBuf,
    options: &CreateOptions,
    json: bool,
) -> Result<()> {
    let count = ArchiveTool::create(archive, input, options)?;
    write_count(stdout, count, json)
}

fn write_add(
    stdout: &mut dyn Write,
    archive: PathBuf,
    inputs: Vec<PathBuf>,
    output: PathBuf,
    json: bool,
) -> Result<()> {
    if inputs.is_empty() {
        return Err(dream_archivetool::ArchiveError::Archive(
            "no input files supplied".to_string(),
        ));
    }
    let count = ArchiveTool::add(archive, &AddOptions { inputs, output })?;
    write_count(stdout, count, json)
}

fn write_summary(
    stdout: &mut dyn Write,
    summary: &dream_archivetool::ExtractSummary,
) -> Result<()> {
    writeln!(stdout, "extracted: {}", summary.extracted)?;
    if summary.skipped > 0 {
        writeln!(stdout, "skipped: {}", summary.skipped)?;
    }
    Ok(())
}

fn write_count(stdout: &mut dyn Write, count: usize, json: bool) -> Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *stdout, &serde_json::json!({ "files": count }))
            .map_err(|err| dream_archivetool::ArchiveError::Archive(err.to_string()))?;
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
            "dream-archivetool-cli-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_tes3_archive(path: &Path) {
        let mut builder = dream_archive::Tes3BsaBuilder::new();
        builder.add_bytes("icons/example.dds", b"payload").unwrap();
        builder.add_bytes("meshes/example.nif", b"mesh").unwrap();
        builder.write_path(path).unwrap();
    }

    #[test]
    fn list_command_writes_entry_names() {
        let dir = unique_dir("list");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);
        let mut stdout = Vec::new();

        run(
            Cli::parse_from(["dream-archivetool", "list", archive_path.to_str().unwrap()]),
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
                "dream-archivetool",
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
                "dream-archivetool",
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
                "dream-archivetool",
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
                "dream-archivetool",
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
                "dream-archivetool",
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
                "dream-archivetool",
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
                "dream-archivetool",
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
                "dream-archivetool",
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
            Cli::parse_from(["dream-archivetool", "--generate-completion", "bash"]),
            &mut stdout,
        )
        .unwrap();
        assert!(
            String::from_utf8(stdout)
                .unwrap()
                .contains("dream-archivetool")
        );

        let mut stdout = Vec::new();
        run(
            Cli::parse_from(["dream-archivetool", "--generate-manpage"]),
            &mut stdout,
        )
        .unwrap();
        assert!(
            String::from_utf8(stdout)
                .unwrap()
                .contains("dream-archivetool")
        );
    }
}
