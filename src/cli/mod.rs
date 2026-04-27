use std::borrow::Cow;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

use clap::{CommandFactory, Parser};
use dream_archivetool::{
    AddOptions, ArchiveFormat, ArchiveTool, Ba2ArchiveKind, Ba2Version, CreateOptions, DiffOptions,
    ExtractAllOptions, ExtractOptions, OverwriteMode, Result, Tes4Version, VerifyOptions,
};

mod args;

use args::{Cli, CliArchiveFormat, CliBa2ArchiveKind, CliBa2Version, CliTes4Version, Command};

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
        command @ (Command::Extract { .. } | Command::ExtractAll { .. }) => {
            handle_extraction_command(stdout, command)
        }
        command @ (Command::Create { .. } | Command::Add { .. }) => {
            handle_mutation_command(stdout, command)
        }
    }
}

fn handle_extraction_command(stdout: &mut dyn Write, command: Command) -> Result<()> {
    match command {
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
        } => handle_extract_command(
            stdout,
            ExtractCliCommand {
                archive,
                entry,
                entry_hex,
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
        } => handle_extract_all_command(
            stdout,
            ExtractAllCliCommand {
                archive,
                output,
                overwrite: overwrite_mode(overwrite, skip_existing),
                json,
                dry_run,
                fsync,
            },
        ),
        _ => unreachable!("non-extraction command routed to extraction handler"),
    }
}

fn handle_mutation_command(stdout: &mut dyn Write, command: Command) -> Result<()> {
    match command {
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
            follow_symlinks,
        } => handle_create_command(
            stdout,
            CreateCommand {
                archive,
                input,
                format,
                tes4_version,
                ba2_kind,
                ba2_version,
                output: CommandOutput { json, dry_run },
                fsync,
                input_collection: InputCollection { follow_symlinks },
            },
        ),
        Command::Add {
            archive,
            inputs,
            output,
            json,
            dry_run,
            fsync,
            follow_symlinks,
        } => handle_add_command(
            stdout,
            archive,
            &AddOptions {
                inputs,
                output,
                fsync,
                follow_symlinks,
            },
            json,
            dry_run,
        ),
        _ => unreachable!("non-mutation command routed to mutation handler"),
    }
}

struct CreateCommand {
    archive: PathBuf,
    input: PathBuf,
    format: CliArchiveFormat,
    tes4_version: Option<CliTes4Version>,
    ba2_kind: Option<CliBa2ArchiveKind>,
    ba2_version: Option<CliBa2Version>,
    output: CommandOutput,
    fsync: bool,
    input_collection: InputCollection,
}

struct CommandOutput {
    json: bool,
    dry_run: bool,
}

struct InputCollection {
    follow_symlinks: bool,
}

fn handle_create_command(stdout: &mut dyn Write, command: CreateCommand) -> Result<()> {
    let options = create_options(
        command.format,
        command.tes4_version,
        command.ba2_kind,
        command.ba2_version,
        command.fsync,
        command.input_collection.follow_symlinks,
    )?;
    write_create(
        stdout,
        command.archive,
        command.input,
        &options,
        command.output.json,
        command.output.dry_run,
    )
}

fn create_options(
    format: CliArchiveFormat,
    tes4_version: Option<CliTes4Version>,
    ba2_kind: Option<CliBa2ArchiveKind>,
    ba2_version: Option<CliBa2Version>,
    fsync: bool,
    follow_symlinks: bool,
) -> Result<CreateOptions> {
    let format = ArchiveFormat::from(format);
    match format {
        ArchiveFormat::Tes3 => {
            reject_irrelevant_create_option("--tes4-version", tes4_version.is_some(), format)?;
            reject_irrelevant_create_option("--ba2-kind", ba2_kind.is_some(), format)?;
            reject_irrelevant_create_option("--ba2-version", ba2_version.is_some(), format)?;
            Ok(CreateOptions {
                format,
                fsync,
                follow_symlinks,
                ..Default::default()
            })
        }
        ArchiveFormat::Tes4 => {
            reject_irrelevant_create_option("--ba2-kind", ba2_kind.is_some(), format)?;
            reject_irrelevant_create_option("--ba2-version", ba2_version.is_some(), format)?;
            Ok(CreateOptions {
                format,
                tes4_version: tes4_version.map_or(Tes4Version::Oblivion, Tes4Version::from),
                fsync,
                follow_symlinks,
                ..Default::default()
            })
        }
        ArchiveFormat::Ba2 => {
            reject_irrelevant_create_option("--tes4-version", tes4_version.is_some(), format)?;
            Ok(CreateOptions {
                format,
                ba2_kind: ba2_kind.map_or(Ba2ArchiveKind::Gnrl, Ba2ArchiveKind::from),
                ba2_version: ba2_version.map_or(Ba2Version::Fallout4, Ba2Version::from),
                fsync,
                follow_symlinks,
                ..Default::default()
            })
        }
        _ => Err(dream_archivetool::ArchiveError::Archive(
            "unsupported archive format selected by CLI".to_string(),
        )),
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
    if !hex.len().is_multiple_of(2) {
        return Err(dream_archivetool::ArchiveError::Archive(
            "--entry-hex must contain an even number of hexadecimal digits".to_string(),
        ));
    }
    dream_archivetool::decode_archive_path_hex(hex).map_err(|_| {
        dream_archivetool::ArchiveError::Archive(
            "--entry-hex contains a non-hexadecimal digit".to_string(),
        )
    })
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
            fingerprint_payloads: hash,
        },
    )?;
    if json {
        write_json(stdout, &report)?;
    } else {
        writeln!(
            stdout,
            "comparison: {}",
            diff_comparison_name(report.comparison)
        )?;
        writeln!(stdout, "added: {}", report.added.len())?;
        writeln!(stdout, "removed: {}", report.removed.len())?;
        writeln!(stdout, "changed: {}", report.changed.len())?;
        writeln!(stdout, "unchanged: {}", report.unchanged)?;
    }
    Ok(())
}

fn diff_comparison_name(comparison: dream_archivetool::DiffComparison) -> &'static str {
    match comparison {
        dream_archivetool::DiffComparison::MetadataOnly => "metadata-only",
        dream_archivetool::DiffComparison::PayloadFingerprint => "payload-fingerprint",
        _ => "unknown",
    }
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

struct ExtractCliCommand {
    archive: PathBuf,
    entry: Option<OsString>,
    entry_hex: Option<String>,
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

fn handle_extract_command(stdout: &mut dyn Write, command: ExtractCliCommand) -> Result<()> {
    write_extract(
        stdout,
        ExtractCommandOptions {
            archive: command.archive,
            entry: entry_selector(command.entry, command.entry_hex)?,
            destination: command.destination,
            fsync: command.fsync,
            preserve_paths: command.preserve_paths,
            overwrite: command.overwrite,
            json: command.json,
        },
    )
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

struct ExtractAllCliCommand {
    archive: PathBuf,
    output: Option<PathBuf>,
    overwrite: OverwriteMode,
    json: bool,
    dry_run: bool,
    fsync: bool,
}

fn handle_extract_all_command(stdout: &mut dyn Write, command: ExtractAllCliCommand) -> Result<()> {
    write_extract_all(
        stdout,
        command.archive,
        command.output,
        command.overwrite,
        command.json,
        command.dry_run,
        command.fsync,
    )
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

fn handle_add_command(
    stdout: &mut dyn Write,
    archive: PathBuf,
    options: &AddOptions,
    json: bool,
    dry_run: bool,
) -> Result<()> {
    if options.inputs.is_empty() {
        return Err(dream_archivetool::ArchiveError::Archive(
            "no input files supplied".to_string(),
        ));
    }
    if dry_run {
        let plan = ArchiveTool::plan_add(&archive, options)?;
        write_json(stdout, &plan)?;
        return Ok(());
    }
    let count = ArchiveTool::add(archive, options)?;
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
        _ => "unknown",
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
        let mut stdout = Vec::new();

        run(
            Cli::parse_from([
                "dream-archivetool",
                "add",
                archive.to_str().unwrap(),
                added.to_str().unwrap(),
                "--json",
            ]),
            &mut stdout,
        )
        .unwrap();

        let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(value["files"], 2);
        assert_eq!(
            ArchiveTool::read_entry(&archive, "added.txt").unwrap(),
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
