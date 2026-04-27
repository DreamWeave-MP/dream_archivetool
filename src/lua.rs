use std::path::PathBuf;

use mlua::{Error as LuaError, Lua, Result as LuaResult, String as LuaString, Table};

use crate::{
    AddOptions, ArchiveFormat, ArchivePlanAction, ArchivePlanOperation, ArchiveTool,
    Ba2ArchiveKind, Ba2Version, CreateOptions, DiffComparison, DiffOptions, ExtractAllOptions,
    ExtractOptions, ExtractPlanAction, ExtractPlanOperation, OverwriteMode, Tes4Version,
    VerifyOptions,
};

/// Create a Lua table for common [`ArchiveTool`] operations.
///
/// The returned table contains tool-policy operations: `info`, `verify`, `diff`, `extract`,
/// `extract_by_path_hex`, `extract_hex`, `extract_all`, `plan_extract_all`, `create`,
/// `plan_create`, `add`, and `plan_add`. Archive-format primitives such as listing and payload
/// reads belong to `dream_archive`'s Lua API instead. Archive entry arguments are Lua byte strings,
/// so `dream_archive` entry paths can be passed to `extract` without a UTF-8 boundary. The table is
/// not registered globally unless [`register`] is called.
pub fn create_module(lua: &Lua) -> LuaResult<Table> {
    let module = lua.create_table_with_capacity(0, 12)?;

    module.set(
        "info",
        lua.create_function(|lua, path: String| {
            let info = ArchiveTool::info(path).map_err(LuaError::external)?;
            archive_info_table(lua, info)
        })?,
    )?;
    register_report_functions(lua, &module)?;
    register_entry_functions(lua, &module)?;
    register_write_functions(lua, &module)?;

    Ok(module)
}

fn register_entry_functions(lua: &Lua, module: &Table) -> LuaResult<()> {
    module.set(
        "extract",
        lua.create_function(
            |lua, (path, entry, opts): (String, LuaString, Option<Table>)| {
                let options = extract_options(opts)?;
                let entry = entry.as_bytes();
                let summary = ArchiveTool::extract_by_path_bytes(path, entry.as_ref(), &options)
                    .map_err(LuaError::external)?;
                summary_table(lua, summary.extracted, summary.skipped)
            },
        )?,
    )?;
    module.set(
        "extract_hex",
        lua.create_function(
            |lua, (path, entry_hex, opts): (String, String, Option<Table>)| {
                let entry =
                    crate::path::decode_archive_path_hex(&entry_hex).map_err(LuaError::external)?;
                let options = extract_options(opts)?;
                let summary = ArchiveTool::extract_by_path_bytes(path, &entry, &options)
                    .map_err(LuaError::external)?;
                summary_table(lua, summary.extracted, summary.skipped)
            },
        )?,
    )?;
    module.set(
        "extract_by_path_hex",
        lua.create_function(
            |lua, (path, entry_hex, opts): (String, String, Option<Table>)| {
                let entry =
                    crate::path::decode_archive_path_hex(&entry_hex).map_err(LuaError::external)?;
                let options = extract_options(opts)?;
                let summary = ArchiveTool::extract_by_path_bytes(path, &entry, &options)
                    .map_err(LuaError::external)?;
                summary_table(lua, summary.extracted, summary.skipped)
            },
        )?,
    )?;
    Ok(())
}

fn register_report_functions(lua: &Lua, module: &Table) -> LuaResult<()> {
    module.set(
        "verify",
        lua.create_function(|lua, (path, opts): (String, Option<Table>)| {
            let options = verify_options(opts)?;
            let report = ArchiveTool::verify(path, &options).map_err(LuaError::external)?;
            verify_report_table(lua, report)
        })?,
    )?;
    module.set(
        "diff",
        lua.create_function(|lua, (old, new, opts): (String, String, Option<Table>)| {
            let options = diff_options(opts)?;
            let report = ArchiveTool::diff(old, new, &options).map_err(LuaError::external)?;
            diff_report_table(lua, report)
        })?,
    )?;
    Ok(())
}

fn register_write_functions(lua: &Lua, module: &Table) -> LuaResult<()> {
    module.set(
        "extract_all",
        lua.create_function(|lua, (path, opts): (String, Option<Table>)| {
            let options = extract_all_options(opts)?;
            let summary = ArchiveTool::extract_all(path, &options).map_err(LuaError::external)?;
            summary_table(lua, summary.extracted, summary.skipped)
        })?,
    )?;
    module.set(
        "plan_extract_all",
        lua.create_function(|lua, (path, opts): (String, Option<Table>)| {
            let options = extract_all_options(opts)?;
            let plan = ArchiveTool::plan_extract_all(path, &options).map_err(LuaError::external)?;
            extract_all_plan_table(lua, plan)
        })?,
    )?;
    module.set(
        "create",
        lua.create_function(
            |_, (output, input, opts): (String, String, Option<Table>)| {
                let options = create_options(opts)?;
                ArchiveTool::create(output, input, &options).map_err(LuaError::external)
            },
        )?,
    )?;
    module.set(
        "plan_create",
        lua.create_function(
            |lua, (output, input, opts): (String, String, Option<Table>)| {
                let options = create_options(opts)?;
                let plan = ArchiveTool::plan_create(output, input, &options)
                    .map_err(LuaError::external)?;
                create_plan_table(lua, plan)
            },
        )?,
    )?;
    module.set(
        "add",
        lua.create_function(|_, (archive, opts): (String, Table)| {
            let options = add_options(&opts)?;
            ArchiveTool::add(archive, &options).map_err(LuaError::external)
        })?,
    )?;
    module.set(
        "plan_add",
        lua.create_function(|lua, (archive, opts): (String, Table)| {
            let options = add_options(&opts)?;
            let plan = ArchiveTool::plan_add(archive, &options).map_err(LuaError::external)?;
            add_plan_table(lua, plan)
        })?,
    )?;

    Ok(())
}

/// Register the Lua API table as the global `dream_archivetool` value.
pub fn register(lua: &Lua) -> LuaResult<()> {
    let module = create_module(lua)?;
    lua.globals().set("dream_archivetool", module)
}

fn format_name(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Tes3 => "tes3",
        ArchiveFormat::Tes4 => "tes4",
        ArchiveFormat::Ba2 => "ba2",
    }
}

fn create_options(opts: Option<Table>) -> LuaResult<CreateOptions> {
    let Some(opts) = opts else {
        return Ok(CreateOptions::default());
    };
    let format = parse_format(opts.get::<Option<String>>("format")?.as_deref())?;
    let tes4_version = opts.get::<Option<String>>("tes4_version")?;
    let ba2_kind = opts.get::<Option<String>>("ba2_kind")?;
    let ba2_version = opts.get::<Option<String>>("ba2_version")?;
    match format {
        ArchiveFormat::Tes3 => {
            reject_irrelevant_create_option("tes4_version", tes4_version.is_some(), format)?;
            reject_irrelevant_create_option("ba2_kind", ba2_kind.is_some(), format)?;
            reject_irrelevant_create_option("ba2_version", ba2_version.is_some(), format)?;
        }
        ArchiveFormat::Tes4 => {
            reject_irrelevant_create_option("ba2_kind", ba2_kind.is_some(), format)?;
            reject_irrelevant_create_option("ba2_version", ba2_version.is_some(), format)?;
        }
        ArchiveFormat::Ba2 => {
            reject_irrelevant_create_option("tes4_version", tes4_version.is_some(), format)?;
        }
    }
    Ok(CreateOptions {
        format,
        tes4_version: parse_tes4_version(tes4_version.as_deref())?,
        ba2_kind: parse_ba2_kind(ba2_kind.as_deref())?,
        ba2_version: parse_ba2_version(ba2_version.as_deref())?,
        fsync: opts.get::<Option<bool>>("fsync")?.unwrap_or(false),
        follow_symlinks: opts
            .get::<Option<bool>>("follow_symlinks")?
            .unwrap_or(false),
    })
}

fn reject_irrelevant_create_option(
    option: &str,
    supplied: bool,
    format: ArchiveFormat,
) -> LuaResult<()> {
    if supplied {
        return Err(LuaError::external(format!(
            "{option} is not valid with format {}",
            format_name(format)
        )));
    }
    Ok(())
}

fn add_options(opts: &Table) -> LuaResult<AddOptions> {
    let inputs: Table = opts.get("inputs")?;
    let mut paths = Vec::new();
    for value in inputs.sequence_values::<String>() {
        paths.push(PathBuf::from(value?));
    }
    Ok(AddOptions {
        inputs: paths,
        output: PathBuf::from(opts.get::<String>("output")?),
        fsync: opts.get::<Option<bool>>("fsync")?.unwrap_or(false),
        follow_symlinks: opts
            .get::<Option<bool>>("follow_symlinks")?
            .unwrap_or(false),
    })
}

fn verify_options(opts: Option<Table>) -> LuaResult<VerifyOptions> {
    let Some(opts) = opts else {
        return Ok(VerifyOptions::default());
    };
    Ok(VerifyOptions {
        read_payloads: opts.get::<Option<bool>>("read_payloads")?.unwrap_or(false),
    })
}

fn diff_options(opts: Option<Table>) -> LuaResult<DiffOptions> {
    let Some(opts) = opts else {
        return Ok(DiffOptions::default());
    };
    Ok(DiffOptions {
        fingerprint_payloads: opts
            .get::<Option<bool>>("fingerprint_payloads")?
            .unwrap_or(false),
    })
}

fn extract_options(opts: Option<Table>) -> LuaResult<ExtractOptions> {
    let Some(opts) = opts else {
        return Ok(ExtractOptions::default());
    };
    Ok(ExtractOptions {
        output: opts.get::<Option<String>>("output")?.map(PathBuf::from),
        overwrite: parse_overwrite(opts.get::<Option<String>>("overwrite")?.as_deref())?,
        preserve_paths: opts.get::<Option<bool>>("preserve_paths")?.unwrap_or(true),
        fsync: opts.get::<Option<bool>>("fsync")?.unwrap_or(false),
    })
}

fn extract_all_options(opts: Option<Table>) -> LuaResult<ExtractAllOptions> {
    let Some(opts) = opts else {
        return Ok(ExtractAllOptions::default());
    };
    Ok(ExtractAllOptions {
        output: opts.get::<Option<String>>("output")?.map(PathBuf::from),
        overwrite: parse_overwrite(opts.get::<Option<String>>("overwrite")?.as_deref())?,
        fsync: opts.get::<Option<bool>>("fsync")?.unwrap_or(false),
    })
}

fn parse_format(value: Option<&str>) -> LuaResult<ArchiveFormat> {
    match value.unwrap_or("tes3") {
        "tes3" => Ok(ArchiveFormat::Tes3),
        "tes4" => Ok(ArchiveFormat::Tes4),
        "ba2" => Ok(ArchiveFormat::Ba2),
        value => Err(LuaError::external(format!(
            "unknown archive format: {value}"
        ))),
    }
}

fn parse_tes4_version(value: Option<&str>) -> LuaResult<Tes4Version> {
    match value.unwrap_or("oblivion") {
        "oblivion" => Ok(Tes4Version::Oblivion),
        "fallout3" | "fallout-3" => Ok(Tes4Version::Fallout3),
        "skyrim" => Ok(Tes4Version::Skyrim),
        "skyrim-se" | "sse" => Ok(Tes4Version::SkyrimSe),
        value => Err(LuaError::external(format!("unknown TES4 version: {value}"))),
    }
}

fn parse_ba2_kind(value: Option<&str>) -> LuaResult<Ba2ArchiveKind> {
    match value.unwrap_or("gnrl") {
        "gnrl" => Ok(Ba2ArchiveKind::Gnrl),
        "dx10" => Ok(Ba2ArchiveKind::Dx10),
        "gnmf" => Ok(Ba2ArchiveKind::Gnmf),
        value => Err(LuaError::external(format!("unknown BA2 kind: {value}"))),
    }
}

fn parse_ba2_version(value: Option<&str>) -> LuaResult<Ba2Version> {
    match value.unwrap_or("fallout4") {
        "fallout4" | "fallout-4" => Ok(Ba2Version::Fallout4),
        "starfield" => Ok(Ba2Version::Starfield),
        "fallout4-next-gen" | "fallout-4-next-gen" => Ok(Ba2Version::Fallout4NextGen),
        value => Err(LuaError::external(format!("unknown BA2 version: {value}"))),
    }
}

fn parse_overwrite(value: Option<&str>) -> LuaResult<OverwriteMode> {
    match value.unwrap_or("fail") {
        "fail" => Ok(OverwriteMode::Fail),
        "overwrite" => Ok(OverwriteMode::Overwrite),
        "skip" => Ok(OverwriteMode::Skip),
        value => Err(LuaError::external(format!(
            "unknown overwrite mode: {value}"
        ))),
    }
}

fn summary_table(lua: &Lua, extracted: usize, skipped: usize) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(0, 2)?;
    table.set("extracted", extracted)?;
    table.set("skipped", skipped)?;
    Ok(table)
}

fn archive_info_table(lua: &Lua, info: crate::ArchiveInfo) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(0, 9)?;
    table.set("path", info.path)?;
    table.set("format", format_name(info.format))?;
    table.set("file_count", info.file_count)?;
    table.set("named_entry_count", info.named_entry_count)?;
    table.set("has_unnameable_entries", info.has_unnameable_entries)?;
    table.set("rewritable", info.rewritable)?;
    table.set("rewrite_blocker", info.rewrite_blocker)?;
    table.set("tes4", optional_tes4_info_table(lua, info.tes4)?)?;
    table.set("ba2", optional_ba2_info_table(lua, info.ba2)?)?;
    Ok(table)
}

fn optional_tes4_info_table(lua: &Lua, info: Option<crate::Tes4Info>) -> LuaResult<Option<Table>> {
    info.map(|info| {
        let table = lua.create_table_with_capacity(0, 7)?;
        table.set("version", info.version)?;
        table.set("archive_types", info.archive_types)?;
        table.set("archive_types_bits", info.archive_types_bits)?;
        table.set("archive_flags", string_array(lua, info.archive_flags)?)?;
        table.set("archive_flags_bits", info.archive_flags_bits)?;
        table.set(
            "unsupported_archive_flags_bits",
            info.unsupported_archive_flags_bits,
        )?;
        table.set("name_mode", info.name_mode)?;
        Ok(table)
    })
    .transpose()
}

fn optional_ba2_info_table(lua: &Lua, info: Option<crate::Ba2Info>) -> LuaResult<Option<Table>> {
    info.map(|info| {
        let table = lua.create_table_with_capacity(0, 4)?;
        table.set("version", info.version)?;
        table.set("payload_format", info.payload_format)?;
        table.set("compression_format", info.compression_format)?;
        table.set("strings", info.strings)?;
        Ok(table)
    })
    .transpose()
}

fn verify_report_table(lua: &Lua, report: crate::VerifyReport) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(0, 11)?;
    table.set("path", report.path)?;
    table.set("format", format_name(report.format))?;
    table.set("file_count", report.file_count)?;
    table.set("named_entry_count", report.named_entry_count)?;
    table.set("unnameable_entries", report.unnameable_entries)?;
    table.set("rewritable", report.rewritable)?;
    table.set("rewrite_blocker", report.rewrite_blocker)?;
    table.set(
        "duplicate_normalized_paths",
        verify_path_issue_array(lua, report.duplicate_normalized_paths)?,
    )?;
    table.set(
        "unsafe_paths",
        verify_path_issue_array(lua, report.unsafe_paths)?,
    )?;
    table.set("payloads_read", report.payloads_read)?;
    table.set("warnings", string_array(lua, report.warnings)?)?;
    Ok(table)
}

fn verify_path_issue_array(lua: &Lua, issues: Vec<crate::VerifyPathIssue>) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(issues.len(), 0)?;
    for (index, issue) in issues.into_iter().enumerate() {
        let issue_table = lua.create_table_with_capacity(0, 4)?;
        issue_table.set("path", issue.path)?;
        issue_table.set("path_bytes_hex", issue.path_bytes_hex)?;
        issue_table.set("raw_path_bytes_hex", issue.raw_path_bytes_hex)?;
        issue_table.set(
            "colliding_raw_path_bytes_hex",
            issue.colliding_raw_path_bytes_hex,
        )?;
        table.set(index + 1, issue_table)?;
    }
    Ok(table)
}

fn diff_report_table(lua: &Lua, report: crate::DiffReport) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(0, 8)?;
    table.set("old", report.old)?;
    table.set("new", report.new)?;
    table.set("comparison", diff_comparison_name(report.comparison))?;
    table.set("fingerprint_payloads", report.fingerprint_payloads)?;
    table.set("added", diff_entry_array(lua, report.added)?)?;
    table.set("removed", diff_entry_array(lua, report.removed)?)?;
    table.set("changed", diff_change_array(lua, report.changed)?)?;
    table.set("unchanged", report.unchanged)?;
    Ok(table)
}

fn diff_entry_array(lua: &Lua, entries: Vec<crate::DiffEntry>) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(entries.len(), 0)?;
    for (index, entry) in entries.into_iter().enumerate() {
        table.set(index + 1, diff_entry_table(lua, entry)?)?;
    }
    Ok(table)
}

fn diff_entry_table(lua: &Lua, entry: crate::DiffEntry) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(0, 5)?;
    table.set("path", entry.path)?;
    table.set("path_bytes_hex", entry.path_bytes_hex)?;
    table.set("size", entry.size)?;
    table.set("compressed_size", entry.compressed_size)?;
    table.set("payload_fingerprint", entry.payload_fingerprint)?;
    Ok(table)
}

fn diff_change_array(lua: &Lua, changes: Vec<crate::DiffChange>) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(changes.len(), 0)?;
    for (index, change) in changes.into_iter().enumerate() {
        let change_table = lua.create_table_with_capacity(0, 4)?;
        change_table.set("path", change.path)?;
        change_table.set("path_bytes_hex", change.path_bytes_hex)?;
        change_table.set("old", diff_entry_state_table(lua, change.old)?)?;
        change_table.set("new", diff_entry_state_table(lua, change.new)?)?;
        table.set(index + 1, change_table)?;
    }
    Ok(table)
}

fn diff_entry_state_table(lua: &Lua, state: crate::DiffEntryState) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(0, 3)?;
    table.set("size", state.size)?;
    table.set("compressed_size", state.compressed_size)?;
    table.set("payload_fingerprint", state.payload_fingerprint)?;
    Ok(table)
}

fn extract_all_plan_table(lua: &Lua, plan: crate::ExtractAllPlan) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(0, 4)?;
    table.set("operation", extract_plan_operation_name(plan.operation))?;
    table.set("archive", plan.archive)?;
    table.set("output", plan.output)?;
    table.set("entries", extract_plan_entry_array(lua, plan.entries)?)?;
    Ok(table)
}

fn extract_plan_entry_array(lua: &Lua, entries: Vec<crate::ExtractPlanEntry>) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(entries.len(), 0)?;
    for (index, entry) in entries.into_iter().enumerate() {
        let entry_table = lua.create_table_with_capacity(0, 4)?;
        entry_table.set("action", extract_plan_action_name(entry.action))?;
        entry_table.set("path", entry.path)?;
        entry_table.set("path_bytes_hex", entry.path_bytes_hex)?;
        entry_table.set("target", entry.target)?;
        table.set(index + 1, entry_table)?;
    }
    Ok(table)
}

fn create_plan_table(lua: &Lua, plan: crate::CreatePlan) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(0, 5)?;
    table.set("operation", archive_plan_operation_name(plan.operation))?;
    table.set("format", format_name(plan.format))?;
    table.set("output", plan.output)?;
    table.set("files", plan.files)?;
    table.set("entries", archive_plan_entry_array(lua, plan.entries)?)?;
    Ok(table)
}

fn add_plan_table(lua: &Lua, plan: crate::AddPlan) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(0, 9)?;
    table.set("operation", archive_plan_operation_name(plan.operation))?;
    table.set("archive", plan.archive)?;
    table.set("output", plan.output)?;
    table.set("format", format_name(plan.format))?;
    table.set("files", plan.files)?;
    table.set("added", plan.added)?;
    table.set("replaced", plan.replaced)?;
    table.set("preserved", plan.preserved)?;
    table.set("entries", archive_plan_entry_array(lua, plan.entries)?)?;
    Ok(table)
}

fn archive_plan_entry_array(lua: &Lua, entries: Vec<crate::ArchivePlanEntry>) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(entries.len(), 0)?;
    for (index, entry) in entries.into_iter().enumerate() {
        let entry_table = lua.create_table_with_capacity(0, 5)?;
        entry_table.set("action", archive_plan_action_name(entry.action))?;
        entry_table.set("source", entry.source)?;
        entry_table.set("path", entry.path)?;
        entry_table.set("path_bytes_hex", entry.path_bytes_hex)?;
        entry_table.set("size", entry.size)?;
        table.set(index + 1, entry_table)?;
    }
    Ok(table)
}

fn string_array(lua: &Lua, values: Vec<String>) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(values.len(), 0)?;
    for (index, value) in values.into_iter().enumerate() {
        table.set(index + 1, value)?;
    }
    Ok(table)
}

fn diff_comparison_name(comparison: DiffComparison) -> &'static str {
    match comparison {
        DiffComparison::MetadataOnly => "metadata-only",
        DiffComparison::PayloadFingerprint => "payload-fingerprint",
    }
}

fn archive_plan_operation_name(operation: ArchivePlanOperation) -> &'static str {
    match operation {
        ArchivePlanOperation::Create => "create",
        ArchivePlanOperation::Add => "add",
    }
}

fn archive_plan_action_name(action: ArchivePlanAction) -> &'static str {
    match action {
        ArchivePlanAction::Add => "add",
        ArchivePlanAction::Replace => "replace",
        ArchivePlanAction::Preserve => "preserve",
    }
}

fn extract_plan_operation_name(operation: ExtractPlanOperation) -> &'static str {
    match operation {
        ExtractPlanOperation::ExtractAll => "extract-all",
    }
}

fn extract_plan_action_name(action: ExtractPlanAction) -> &'static str {
    match action {
        ExtractPlanAction::Extract => "extract",
        ExtractPlanAction::Skip => "skip",
        ExtractPlanAction::Overwrite => "overwrite",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn unique_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dream-archivetool-lua-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_input_tree(input: &Path) {
        fs::create_dir_all(input.join("textures")).unwrap();
        fs::write(input.join("textures/example.dds"), b"hello").unwrap();
    }

    fn create_test_archive(dir: &Path) -> PathBuf {
        let input = dir.join("input");
        write_input_tree(&input);
        let archive = dir.join("out.bsa");
        ArchiveTool::create(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Tes3,
                ..Default::default()
            },
        )
        .unwrap();
        archive
    }

    #[test]
    fn lua_module_exposes_tool_policy_info_only() {
        let dir = unique_dir("info-boundary");
        let archive = create_test_archive(&dir);
        let lua = Lua::new();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();

        let (format, rewritable, list_is_absent, read_is_absent): (String, bool, bool, bool) = lua
            .load(
                r"
                local info = dream_archivetool.info(archive_path)
                return info.format, info.rewritable,
                    dream_archivetool.list == nil,
                    dream_archivetool.read_entry == nil
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(format, "tes3");
        assert!(rewritable);
        assert!(list_is_absent);
        assert!(read_is_absent);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_extract_writes_entry_with_options() {
        let dir = unique_dir("extract");
        let archive = create_test_archive(&dir);
        let output = dir.join("output");
        let lua = Lua::new();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("output_path", output.to_str().unwrap())
            .unwrap();

        let extracted: usize = lua
            .load(
                r"
                local summary = dream_archivetool.extract(archive_path, 'textures/example.dds', {
                    output = output_path,
                    preserve_paths = false,
                })
                return summary.extracted
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(extracted, 1);
        assert_eq!(fs::read(output.join("example.dds")).unwrap(), b"hello");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_can_round_trip_hex_entry_paths() {
        let dir = unique_dir("hex-paths");
        let archive = create_test_archive(&dir);
        let output = dir.join("output");
        let lua = Lua::new();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("output_path", output.to_str().unwrap())
            .unwrap();

        let extracted: usize = lua
            .load(
                r"
                local entry_hex = '74657874757265732f6578616d706c652e646473'
                local summary = dream_archivetool.extract_hex(archive_path, entry_hex, {
                    output = output_path,
                    preserve_paths = false,
                })
                return summary.extracted
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(extracted, 1);
        assert_eq!(fs::read(output.join("example.dds")).unwrap(), b"hello");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_extract_can_overwrite_existing_file() {
        let dir = unique_dir("extract-overwrite");
        let archive = create_test_archive(&dir);
        let output = dir.join("output");
        fs::create_dir_all(output.join("textures")).unwrap();
        fs::write(output.join("textures/example.dds"), b"existing").unwrap();
        let lua = Lua::new();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("output_path", output.to_str().unwrap())
            .unwrap();

        let extracted: usize = lua
            .load(
                r"
                local summary = dream_archivetool.extract(archive_path, 'textures/example.dds', {
                    output = output_path,
                    overwrite = 'overwrite',
                })
                return summary.extracted
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(extracted, 1);
        assert_eq!(
            fs::read(output.join("textures/example.dds")).unwrap(),
            b"hello"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_extract_all_can_skip_existing_files() {
        let dir = unique_dir("extract-all");
        let archive = create_test_archive(&dir);
        let output = dir.join("output");
        fs::create_dir_all(output.join("textures")).unwrap();
        fs::write(output.join("textures/example.dds"), b"existing").unwrap();
        let lua = Lua::new();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("output_path", output.to_str().unwrap())
            .unwrap();

        let skipped: usize = lua
            .load(
                r"
                local summary = dream_archivetool.extract_all(archive_path, {
                    output = output_path,
                    overwrite = 'skip',
                })
                return summary.skipped
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(skipped, 1);
        assert_eq!(
            fs::read(output.join("textures/example.dds")).unwrap(),
            b"existing"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_create_and_add_use_option_tables() {
        let dir = unique_dir("create-add");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("base.txt"), b"base").unwrap();
        let archive = dir.join("out.bsa");
        let added = dir.join("added");
        fs::create_dir_all(&added).unwrap();
        fs::write(added.join("added.txt"), b"added").unwrap();
        let updated = dir.join("updated.bsa");
        let lua = Lua::new();
        register(&lua).unwrap();
        lua.globals()
            .set("input_path", input.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("added_path", added.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("updated_path", updated.to_str().unwrap())
            .unwrap();

        let files: usize = lua
            .load(
                r"
                local created = dream_archivetool.create(archive_path, input_path, { format = 'tes3' })
                local updated = dream_archivetool.add(archive_path, {
                    output = updated_path,
                    inputs = { added_path },
                })
                return created + updated
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(files, 3);
        let entries = ArchiveTool::list(&updated).unwrap();
        assert!(entries.iter().any(|entry| entry.path == "base.txt"));
        assert!(entries.iter().any(|entry| entry.path == "added.txt"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_reports_verify_diff_and_plans() {
        let dir = unique_dir("reports-plans");
        let archive = create_test_archive(&dir);
        let input = dir.join("input2");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("added.txt"), b"added").unwrap();
        let updated = dir.join("updated.bsa");
        let output = dir.join("extract-output");
        let lua = Lua::new();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("input_path", input.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("updated_path", updated.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("output_path", output.to_str().unwrap())
            .unwrap();

        let (payloads_read, extract_action, create_action, add_action): (
            usize,
            String,
            String,
            String,
        ) = lua
            .load(
                r"
                local verify = dream_archivetool.verify(archive_path, { read_payloads = true })
                local extract_plan = dream_archivetool.plan_extract_all(archive_path, {
                    output = output_path,
                })
                local create_plan = dream_archivetool.plan_create(updated_path, input_path, {
                    format = 'tes3',
                })
                local add_plan = dream_archivetool.plan_add(archive_path, {
                    output = updated_path,
                    inputs = { input_path },
                })
                local add_action = nil
                for _, entry in ipairs(add_plan.entries) do
                    if entry.action == 'add' then
                        add_action = entry.action
                    end
                end
                return verify.payloads_read, extract_plan.entries[1].action,
                    create_plan.entries[1].action, add_action
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(payloads_read, 1);
        assert_eq!(extract_action, "extract");
        assert_eq!(create_action, "add");
        assert_eq!(add_action, "add");

        ArchiveTool::create(
            &updated,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Tes3,
                ..Default::default()
            },
        )
        .unwrap();
        lua.globals()
            .set("other_archive_path", updated.to_str().unwrap())
            .unwrap();
        let comparison: String = lua
            .load(
                r"
                local diff = dream_archivetool.diff(archive_path, other_archive_path, {
                    fingerprint_payloads = true,
                })
                return diff.comparison
            ",
            )
            .eval()
            .unwrap();
        assert_eq!(comparison, "payload-fingerprint");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_create_supports_ba2_options() {
        let dir = unique_dir("create-ba2");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("base.txt"), b"base").unwrap();
        let archive = dir.join("out.ba2");
        let lua = Lua::new();
        register(&lua).unwrap();
        lua.globals()
            .set("input_path", input.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();

        let files: usize = lua
            .load(
                r"
                return dream_archivetool.create(archive_path, input_path, {
                    format = 'ba2',
                    ba2_kind = 'gnrl',
                    ba2_version = 'starfield',
                })
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(files, 1);
        let archive = dream_archive::ba2::Archive::open_path(&archive).unwrap();
        assert_eq!(
            archive.info().version,
            dream_archive::ba2::ArchiveVersion::v2
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_reports_invalid_options() {
        let dir = unique_dir("invalid-options");
        let archive = create_test_archive(&dir);
        let lua = Lua::new();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();

        let err = lua
            .load(
                r"
                return dream_archivetool.extract(archive_path, 'textures/example.dds', {
                    overwrite = 'explode',
                })
            ",
            )
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(err.to_string().contains("unknown overwrite mode"));

        let err = lua
            .load("return dream_archivetool.create('out.bsa', 'input', { format = 'unknown' })")
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(err.to_string().contains("unknown archive format"));

        let err = lua
            .load(
                "return dream_archivetool.create('out.bsa', 'input', { format = 'tes3', ba2_kind = 'gnrl' })",
            )
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(err.to_string().contains("ba2_kind is not valid"));

        let err = lua
            .load("return dream_archivetool.add(archive_path, { inputs = {} })")
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(!err.to_string().is_empty());

        let err = lua
            .load("return dream_archivetool.add(archive_path, { output = 'out.bsa' })")
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(!err.to_string().is_empty());

        fs::remove_dir_all(dir).unwrap();
    }
}
