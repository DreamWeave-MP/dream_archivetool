use std::borrow::Cow;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use dream_archivetool::{
    AddOptions, ArchiveFormat, ArchiveTool, Ba2ArchiveKind, Ba2Version, CreateOptions, DiffOptions,
    ExtractAllOptions, ExtractOptions, OverwriteMode, Result, Tes4Version, VerifyOptions,
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
        /// Include entry sizes; JSON always includes available size fields
        #[arg(short, long, conflicts_with = "json")]
        long: bool,
        /// Write JSON to stdout
        #[arg(long)]
        json: bool,
    },
    /// Verify archive structure and optional payload readability
    Verify {
        /// Archive path
        archive: PathBuf,
        /// Stream every named payload to a sink
        #[arg(long)]
        read_payloads: bool,
        /// Write JSON report to stdout
        #[arg(long)]
        json: bool,
    },
    /// Compare two archives by normalized path bytes
    Diff {
        /// Old archive path
        old: PathBuf,
        /// New archive path
        new: PathBuf,
        /// Hash payload bytes instead of comparing only metadata
        #[arg(long)]
        hash: bool,
        /// Write JSON report to stdout
        #[arg(long)]
        json: bool,
    },
    /// Extract one archive entry
    Extract {
        /// Archive path
        archive: PathBuf,
        /// Entry path inside the archive. Non-UTF-8 Unix bytes are accepted.
        #[arg(required_unless_present = "entry_hex", conflicts_with = "entry_hex")]
        entry: Option<OsString>,
        /// Hex-encoded normalized entry path bytes from `list --json` `path_bytes_hex`
        #[arg(long, value_name = "HEX", conflicts_with = "entry")]
        entry_hex: Option<String>,
        /// Output directory. Defaults to the current directory.
        #[arg(short, long, conflicts_with = "stdout")]
        output: Option<PathBuf>,
        /// Write file bytes to stdout
        #[arg(
            long,
            conflicts_with_all = ["output", "flat", "overwrite", "skip_existing", "fsync"]
        )]
        stdout: bool,
        /// Sync file contents and parent directory after writing
        #[arg(long, conflicts_with = "stdout")]
        fsync: bool,
        /// Discard archive directories and write only the basename
        #[arg(long, conflicts_with = "stdout")]
        flat: bool,
        /// Replace existing files
        #[arg(long, conflicts_with_all = ["skip_existing", "stdout"])]
        overwrite: bool,
        /// Leave existing files untouched
        #[arg(long, conflicts_with_all = ["overwrite", "stdout"])]
        skip_existing: bool,
        /// Write JSON summary to stdout
        #[arg(long, conflicts_with = "stdout")]
        json: bool,
    },
    /// Extract every archive entry
    ExtractAll {
        /// Archive path
        archive: PathBuf,
        /// Output directory. Defaults to the current directory.
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
        /// Print extraction plan without writing files
        #[arg(long)]
        dry_run: bool,
        /// Sync file contents and parent directory after writing
        #[arg(long)]
        fsync: bool,
    },
    /// Create a new archive from a file or directory
    Create {
        /// Output archive path
        archive: PathBuf,
        /// Input file or directory. Directory contents are stored relative to that root.
        input: PathBuf,
        /// Archive format to write
        #[arg(long, value_enum)]
        format: ArchiveFormat,
        /// TES4 BSA version; only valid with --format tes4
        #[arg(long, value_enum)]
        tes4_version: Option<Tes4Version>,
        /// BA2 BA2 archive kind; only valid with --format ba2. GNMF update/create is rejected.
        #[arg(long, value_enum)]
        ba2_kind: Option<Ba2ArchiveKind>,
        /// BA2 BA2 version; only valid with --format ba2
        #[arg(long, value_enum)]
        ba2_version: Option<Ba2Version>,
        /// Write JSON summary to stdout
        #[arg(long)]
        json: bool,
        /// Print archive creation plan without writing files
        #[arg(long)]
        dry_run: bool,
        /// Sync file contents and parent directory after writing the archive
        #[arg(long)]
        fsync: bool,
    },
    /// Add or update entries by writing a new archive
    Add {
        /// Input archive path
        archive: PathBuf,
        /// Files or directories to add. Directory contents are stored relative to each root.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Output archive path
        #[arg(short, long)]
        output: PathBuf,
        /// Write JSON summary to stdout
        #[arg(long)]
        json: bool,
        /// Print archive update plan without writing files
        #[arg(long)]
        dry_run: bool,
        /// Sync file contents and parent directory after writing the archive
        #[arg(long)]
        fsync: bool,
    },
}

pub(crate) fn run_from_env(stdout: &mut dyn Write) -> Result<()> {
    run(Cli::parse(), stdout)
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
        Command::Verify {
            archive,
            read_payloads,
            json,
        } => write_verify(stdout, archive, read_payloads, json),
        Command::Diff {
            old,
            new,
            hash,
            json,
        } => write_diff(stdout, old, new, hash, json),
        Command::Extract {
            archive,
            entry,
            entry_hex,
            output,
            stdout: stdout_mode,
            fsync,
            flat,
            overwrite,
            skip_existing,
            json,
        } => write_extract(
            stdout,
            ExtractCommandOptions {
                archive,
                entry: entry_selector(entry, entry_hex)?,
                destination: if stdout_mode {
                    ExtractDestination::Stdout
                } else {
                    ExtractDestination::Disk(output)
                },
                fsync,
                preserve_paths: !flat,
                overwrite: overwrite_mode(overwrite, skip_existing),
                json,
            },
        ),
        Command::ExtractAll {
            archive,
            output,
            overwrite,
            skip_existing,
            json,
            dry_run,
            fsync,
        } => write_extract_all(
            stdout,
            archive,
            output,
            overwrite_mode(overwrite, skip_existing),
            json,
            dry_run,
            fsync,
        ),
        Command::Create {
            archive,
            input,
            format,
            tes4_version,
            ba2_kind,
            ba2_version,
            json,
            dry_run,
            fsync,
        } => {
            let options = create_options(format, tes4_version, ba2_kind, ba2_version, fsync)?;
            write_create(stdout, archive, input, &options, json, dry_run)
        }
        Command::Add {
            archive,
            inputs,
            output,
            json,
            dry_run,
            fsync,
        } => write_add(stdout, archive, inputs, output, json, dry_run, fsync),
    }
}

fn create_options(
    format: ArchiveFormat,
    tes4_version: Option<Tes4Version>,
    ba2_kind: Option<Ba2ArchiveKind>,
    ba2_version: Option<Ba2Version>,
    fsync: bool,
) -> Result<CreateOptions> {
    match format {
        ArchiveFormat::Tes3 => {
            reject_irrelevant_create_option("--tes4-version", tes4_version.is_some(), format)?;
            reject_irrelevant_create_option("--ba2-kind", ba2_kind.is_some(), format)?;
            reject_irrelevant_create_option("--ba2-version", ba2_version.is_some(), format)?;
            Ok(CreateOptions {
                format,
                fsync,
                ..Default::default()
            })
        }
        ArchiveFormat::Tes4 => {
            reject_irrelevant_create_option("--ba2-kind", ba2_kind.is_some(), format)?;
            reject_irrelevant_create_option("--ba2-version", ba2_version.is_some(), format)?;
            Ok(CreateOptions {
                format,
                tes4_version: tes4_version.unwrap_or(Tes4Version::Oblivion),
                fsync,
                ..Default::default()
            })
        }
        ArchiveFormat::Ba2 => {
            reject_irrelevant_create_option("--tes4-version", tes4_version.is_some(), format)?;
            Ok(CreateOptions {
                format,
                ba2_kind: ba2_kind.unwrap_or(Ba2ArchiveKind::Gnrl),
                ba2_version: ba2_version.unwrap_or(Ba2Version::Fallout4),
                fsync,
                ..Default::default()
            })
        }
    }
}

fn reject_irrelevant_create_option(
    option: &str,
    supplied: bool,
    format: ArchiveFormat,
) -> Result<()> {
    if supplied {
        return Err(dream_archivetool::ArchiveError::Archive(format!(
            "{option} is not valid with --format {}",
            format_name(format)
        )));
    }
    Ok(())
}

fn entry_selector(entry: Option<OsString>, entry_hex: Option<String>) -> Result<Vec<u8>> {
    if let Some(hex) = entry_hex {
        return decode_hex_entry(&hex);
    }
    let entry = entry.ok_or_else(|| {
        dream_archivetool::ArchiveError::Archive(
            "entry path or --entry-hex is required".to_string(),
        )
    })?;
    Ok(archive_entry_bytes(&entry).into_owned())
}

fn decode_hex_entry(hex: &str) -> Result<Vec<u8>> {
    let bytes = hex.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return Err(dream_archivetool::ArchiveError::Archive(
            "--entry-hex must contain an even number of hexadecimal digits".to_string(),
        ));
    }
    let mut decoded = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        let high = hex_nibble(chunk[0])?;
        let low = hex_nibble(chunk[1])?;
        decoded.push((high << 4) | low);
    }
    Ok(decoded)
}

fn hex_nibble(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(dream_archivetool::ArchiveError::Archive(
            "--entry-hex contains a non-hexadecimal digit".to_string(),
        )),
    }
}

#[cfg(unix)]
fn archive_entry_bytes(entry: &std::ffi::OsStr) -> Cow<'_, [u8]> {
    use std::os::unix::ffi::OsStrExt;
    Cow::Borrowed(entry.as_bytes())
}

#[cfg(not(unix))]
fn archive_entry_bytes(entry: &std::ffi::OsStr) -> Cow<'_, [u8]> {
    Cow::Owned(entry.to_string_lossy().into_owned().into_bytes())
}

fn write_info(stdout: &mut dyn Write, archive: PathBuf, json: bool) -> Result<()> {
    let info = ArchiveTool::info(archive)?;
    if json {
        serde_json::to_writer_pretty(&mut *stdout, &info)
            .map_err(|err| dream_archivetool::ArchiveError::Archive(err.to_string()))?;
        writeln!(stdout)?;
    } else {
        writeln!(stdout, "format: {}", format_name(info.format))?;
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

fn write_verify(
    stdout: &mut dyn Write,
    archive: PathBuf,
    read_payloads: bool,
    json: bool,
) -> Result<()> {
    let report = ArchiveTool::verify(archive, &VerifyOptions { read_payloads })?;
    if json {
        serde_json::to_writer_pretty(&mut *stdout, &report)
            .map_err(|err| dream_archivetool::ArchiveError::Archive(err.to_string()))?;
        writeln!(stdout)?;
        return Ok(());
    }
    writeln!(stdout, "format: {}", format_name(report.format))?;
    writeln!(stdout, "files: {}", report.file_count)?;
    writeln!(stdout, "named: {}", report.named_entry_count)?;
    writeln!(stdout, "unnameable: {}", report.unnameable_entries)?;
    writeln!(stdout, "rewritable: {}", report.rewritable)?;
    if let Some(blocker) = report.rewrite_blocker {
        writeln!(stdout, "rewrite blocker: {blocker}")?;
    }
    if let Some(payloads) = report.payloads_read {
        writeln!(stdout, "payloads read: {payloads}")?;
    }
    for warning in report.warnings {
        writeln!(stdout, "warning: {warning}")?;
    }
    Ok(())
}

fn write_diff(
    stdout: &mut dyn Write,
    old: PathBuf,
    new: PathBuf,
    hash: bool,
    json: bool,
) -> Result<()> {
    let report = ArchiveTool::diff(
        old,
        new,
        &DiffOptions {
            hash_payloads: hash,
        },
    )?;
    if json {
        write_json(stdout, &report)?;
    } else {
        writeln!(stdout, "added: {}", report.added.len())?;
        writeln!(stdout, "removed: {}", report.removed.len())?;
        writeln!(stdout, "changed: {}", report.changed.len())?;
        writeln!(stdout, "unchanged: {}", report.unchanged)?;
    }
    Ok(())
}

struct ExtractCommandOptions {
    archive: PathBuf,
    entry: Vec<u8>,
    destination: ExtractDestination,
    fsync: bool,
    preserve_paths: bool,
    overwrite: OverwriteMode,
    json: bool,
}

enum ExtractDestination {
    Stdout,
    Disk(Option<PathBuf>),
}

fn write_extract(stdout: &mut dyn Write, options: ExtractCommandOptions) -> Result<()> {
    let ExtractDestination::Disk(output) = options.destination else {
        ArchiveTool::extract_entry_path_to_writer(&options.archive, &options.entry, stdout)?;
        return Ok(());
    };
    let extract_options = ExtractOptions {
        output,
        overwrite: options.overwrite,
        preserve_paths: options.preserve_paths,
        fsync: options.fsync,
    };
    let summary = ArchiveTool::extract_by_path(options.archive, &options.entry, &extract_options)?;
    if options.json {
        write_summary_json(stdout, &summary)
    } else {
        write_summary(stdout, &summary)
    }
}

fn write_extract_all(
    stdout: &mut dyn Write,
    archive: PathBuf,
    output: Option<PathBuf>,
    overwrite: OverwriteMode,
    json: bool,
    dry_run: bool,
    fsync: bool,
) -> Result<()> {
    let options = ExtractAllOptions {
        output,
        overwrite,
        fsync,
    };
    if dry_run {
        let plan = ArchiveTool::plan_extract_all(archive, &options)?;
        write_json(stdout, &plan)?;
        return Ok(());
    }
    let summary = ArchiveTool::extract_all(archive, &options)?;
    if json {
        write_summary_json(stdout, &summary)?;
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
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        let plan = ArchiveTool::plan_create(&archive, &input, options)?;
        write_json(stdout, &plan)?;
        return Ok(());
    }
    let count = ArchiveTool::create(archive, input, options)?;
    write_count(stdout, count, json)
}

fn write_add(
    stdout: &mut dyn Write,
    archive: PathBuf,
    inputs: Vec<PathBuf>,
    output: PathBuf,
    json: bool,
    dry_run: bool,
    fsync: bool,
) -> Result<()> {
    if inputs.is_empty() {
        return Err(dream_archivetool::ArchiveError::Archive(
            "no input files supplied".to_string(),
        ));
    }
    let options = AddOptions {
        inputs,
        output,
        fsync,
    };
    if dry_run {
        let plan = ArchiveTool::plan_add(&archive, &options)?;
        write_json(stdout, &plan)?;
        return Ok(());
    }
    let count = ArchiveTool::add(archive, &options)?;
    write_count(stdout, count, json)
}

fn write_json<T: serde::Serialize>(stdout: &mut dyn Write, value: &T) -> Result<()> {
    serde_json::to_writer_pretty(&mut *stdout, value)
        .map_err(|err| dream_archivetool::ArchiveError::Archive(err.to_string()))?;
    writeln!(stdout)?;
    Ok(())
}

fn format_name(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Tes3 => "tes3",
        ArchiveFormat::Tes4 => "tes4",
        ArchiveFormat::Ba2 => "ba2",
    }
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

fn write_summary_json(
    stdout: &mut dyn Write,
    summary: &dream_archivetool::ExtractSummary,
) -> Result<()> {
    serde_json::to_writer_pretty(&mut *stdout, summary)
        .map_err(|err| dream_archivetool::ArchiveError::Archive(err.to_string()))?;
    writeln!(stdout)?;
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
        assert_eq!(value["named_entry_count"], 2);
        assert_eq!(value["has_unnameable_entries"], false);
        assert_eq!(value["rewritable"], true);
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
        let entries = value.as_array().unwrap();
        let icon = entries
            .iter()
            .find(|entry| entry["path"] == "icons/example.dds")
            .unwrap();
        assert_eq!(icon["path_bytes_hex"], "69636f6e732f6578616d706c652e646473");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn diff_command_reports_added_and_changed_entries() {
        let dir = unique_dir("diff");
        let old_input = dir.join("old-input");
        let new_input = dir.join("new-input");
        fs::create_dir_all(&old_input).unwrap();
        fs::create_dir_all(&new_input).unwrap();
        fs::write(old_input.join("same.txt"), b"same").unwrap();
        fs::write(old_input.join("changed.txt"), b"old").unwrap();
        fs::write(new_input.join("same.txt"), b"same").unwrap();
        fs::write(new_input.join("changed.txt"), b"new").unwrap();
        fs::write(new_input.join("added.txt"), b"added").unwrap();
        let old_archive = dir.join("old.bsa");
        let new_archive = dir.join("new.bsa");
        ArchiveTool::create(&old_archive, &old_input, &CreateOptions::default()).unwrap();
        ArchiveTool::create(&new_archive, &new_input, &CreateOptions::default()).unwrap();
        let mut stdout = Vec::new();

        run(
            Cli::parse_from([
                "dream-archivetool",
                "diff",
                old_archive.to_str().unwrap(),
                new_archive.to_str().unwrap(),
                "--hash",
                "--json",
            ]),
            &mut stdout,
        )
        .unwrap();

        let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(value["added"].as_array().unwrap().len(), 1);
        assert_eq!(value["changed"].as_array().unwrap().len(), 1);
        assert_eq!(value["unchanged"], 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn verify_command_can_write_json() {
        let dir = unique_dir("verify-json");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);
        let mut stdout = Vec::new();

        run(
            Cli::parse_from([
                "dream-archivetool",
                "verify",
                archive_path.to_str().unwrap(),
                "--read-payloads",
                "--json",
            ]),
            &mut stdout,
        )
        .unwrap();

        let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(value["format"], "tes3");
        assert_eq!(value["file_count"], 2);
        assert_eq!(value["payloads_read"], 2);
        assert_eq!(
            value["duplicate_normalized_paths"]
                .as_array()
                .unwrap()
                .len(),
            0
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
    fn extract_command_can_write_json_summary() {
        let dir = unique_dir("extract-json");
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
                "--json",
            ]),
            &mut stdout,
        )
        .unwrap();

        let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(value["extracted"], 1);
        assert_eq!(value["skipped"], 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn info_command_writes_human_format_name() {
        let dir = unique_dir("info-human");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);
        let mut stdout = Vec::new();

        run(
            Cli::parse_from(["dream-archivetool", "info", archive_path.to_str().unwrap()]),
            &mut stdout,
        )
        .unwrap();

        assert!(
            String::from_utf8(stdout)
                .unwrap()
                .contains("format: tes3\n")
        );
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
    fn create_command_dry_run_reports_plan_without_writing() {
        let dir = unique_dir("create-dry-run");
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
                "--dry-run",
                "--json",
            ]),
            &mut stdout,
        )
        .unwrap();

        let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(value["operation"], "create");
        assert_eq!(value["files"], 1);
        assert!(!archive.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn add_command_dry_run_reports_add_replace_preserve() {
        let dir = unique_dir("add-dry-run");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("base.txt"), b"base").unwrap();
        fs::write(input.join("keep.txt"), b"keep").unwrap();
        let archive = dir.join("base.bsa");
        ArchiveTool::create(&archive, &input, &CreateOptions::default()).unwrap();
        let update = dir.join("update");
        fs::create_dir_all(&update).unwrap();
        fs::write(update.join("base.txt"), b"new").unwrap();
        fs::write(update.join("added.txt"), b"added").unwrap();
        let output = dir.join("updated.bsa");
        let mut stdout = Vec::new();

        run(
            Cli::parse_from([
                "dream-archivetool",
                "add",
                archive.to_str().unwrap(),
                update.to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
                "--dry-run",
                "--json",
            ]),
            &mut stdout,
        )
        .unwrap();

        let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(value["added"], 1);
        assert_eq!(value["replaced"], 1);
        assert_eq!(value["preserved"], 1);
        assert!(!output.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn extract_all_command_dry_run_reports_targets_without_writing() {
        let dir = unique_dir("extract-all-dry-run");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);
        let output = dir.join("out");
        let mut stdout = Vec::new();

        run(
            Cli::parse_from([
                "dream-archivetool",
                "extract-all",
                archive_path.to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
                "--dry-run",
                "--json",
            ]),
            &mut stdout,
        )
        .unwrap();

        let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(value["operation"], "extract-all");
        assert_eq!(value["entries"].as_array().unwrap().len(), 2);
        assert!(!output.exists());
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
        let err = Cli::try_parse_from([
            "dream-archivetool",
            "add",
            "base.bsa",
            "--output",
            "updated.bsa",
        ])
        .unwrap_err();

        assert!(err.to_string().contains("<INPUTS>"));
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

    #[test]
    fn create_command_rejects_irrelevant_format_options() {
        let err = run(
            Cli::parse_from([
                "dream-archivetool",
                "create",
                "out.bsa",
                "input",
                "--format",
                "tes3",
                "--ba2-kind",
                "dx10",
            ]),
            &mut Vec::new(),
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("--ba2-kind is not valid with --format tes3")
        );
    }

    #[test]
    fn list_json_conflicts_with_long() {
        let err =
            Cli::try_parse_from(["dream-archivetool", "list", "test.bsa", "--json", "--long"])
                .unwrap_err();

        assert!(err.to_string().contains("cannot be used with"));
    }

    #[test]
    fn extract_stdout_conflicts_with_disk_options() {
        let err = Cli::try_parse_from([
            "dream-archivetool",
            "extract",
            "test.bsa",
            "textures/a.dds",
            "--stdout",
            "--fsync",
        ])
        .unwrap_err();

        assert!(err.to_string().contains("cannot be used with"));
    }

    #[cfg(unix)]
    #[test]
    fn extract_command_accepts_non_utf8_entry_bytes_on_unix() {
        use std::os::unix::ffi::OsStringExt;

        let dir = unique_dir("extract-non-utf8-entry");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        let mut builder = dream_archive::Tes3BsaBuilder::new();
        builder
            .add_bytes(b"textures/invalid-\xff.dds", b"bytes")
            .unwrap();
        builder.write_path(&archive_path).unwrap();
        let mut stdout = Vec::new();

        run(
            Cli::parse_from([
                OsString::from("dream-archivetool"),
                OsString::from("extract"),
                archive_path.into_os_string(),
                OsString::from_vec(b"textures/invalid-\xff.dds".to_vec()),
                OsString::from("--stdout"),
            ]),
            &mut stdout,
        )
        .unwrap();

        assert_eq!(stdout, b"bytes");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn extract_command_accepts_entry_hex() {
        let dir = unique_dir("extract-entry-hex");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);
        let mut stdout = Vec::new();

        run(
            Cli::parse_from([
                "dream-archivetool",
                "extract",
                archive_path.to_str().unwrap(),
                "--entry-hex",
                "69636f6e732f6578616d706c652e646473",
                "--stdout",
            ]),
            &mut stdout,
        )
        .unwrap();

        assert_eq!(stdout, b"payload");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn entry_hex_conflicts_with_positional_entry() {
        let err = Cli::try_parse_from([
            "dream-archivetool",
            "extract",
            "test.bsa",
            "icons/example.dds",
            "--entry-hex",
            "00",
        ])
        .unwrap_err();

        assert!(err.to_string().contains("cannot be used with"));
    }

    #[test]
    fn invalid_entry_hex_is_rejected() {
        let err = run(
            Cli::parse_from([
                "dream-archivetool",
                "extract",
                "test.bsa",
                "--entry-hex",
                "abc",
                "--stdout",
            ]),
            &mut Vec::new(),
        )
        .unwrap_err();

        assert!(err.to_string().contains("even number"));
    }

    #[test]
    fn no_subcommand_prints_help_successfully() {
        let mut stdout = Vec::new();

        run(Cli::parse_from(["dream-archivetool"]), &mut stdout).unwrap();

        assert!(
            String::from_utf8(stdout)
                .unwrap()
                .contains("Inspect and manipulate")
        );
    }

    #[test]
    fn missing_archive_error_includes_path_context() {
        let dir = unique_dir("missing-archive");
        let archive_path = dir.join("missing.bsa");
        let mut stdout = Vec::new();

        let err = run(
            Cli::parse_from(["dream-archivetool", "list", archive_path.to_str().unwrap()]),
            &mut stdout,
        )
        .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("failed to open archive"));
        assert!(message.contains("missing.bsa"));
    }
}
