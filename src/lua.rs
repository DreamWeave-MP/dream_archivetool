// SPDX-License-Identifier: GPL-3.0-or-later

//! Luau bindings for the `dream_archivetool` policy layer.
//!
//! This module deliberately does not mirror `dream_archive`'s archive primitives. Use
//! `dream_archive` to open archives, list entries, read payloads, and use path helpers; use this
//! module for policy-heavy operations such as safe filesystem extraction, create/add planning,
//! verification, and diffing. Register both modules in the same [`Lua`] state when scripts need the
//! full stack.
//!
//! Version 0.2.0 moved the bindings from `LuaJIT` to Luau, following `dream_archive` 0.2, and
//! renamed the whole Lua-facing surface to Luau conventions: functions, methods, option keys,
//! report fields, and enum-like string values are camelCase (`extractMany`, `planExtractAll`,
//! `preservePaths`, `pathBytesHex`, `"bsaTes4"`, `"payloadFingerprint"`). The conventional global
//! name is `dreamArchivetool`, next to `dreamArchive` and `dreamPath`. [`register`] sets that
//! global; to make `require("@dreamArchivetool")` work instead, pass [`create_module`]'s table to
//! [`mlua::Lua::register_module`] yourself.
//!
//! Lua string boundaries are part of the API contract. Filesystem paths are UTF-8 host paths.
//! Archive entry paths are byte strings, so `dreamArchive` entry paths can be passed straight to
//! [`dreamArchivetool.extract`](create_module) without pretending arbitrary archive bytes are text.
//! Report display `path` fields are for humans; `pathBytesHex` is the stable normalized lookup
//! key, not raw archive-name identity. Wide archive sizes are exposed as decimal strings because
//! Luau numbers are doubles and cannot carry every u64 exactly.

use std::path::PathBuf;

use mlua::{
    AnyUserData, Error as LuaError, Function, Lua, LuaString, Result as LuaResult, Table,
    UserDataMethods, UserDataRegistry, Value,
};

use crate::{
    AddOptions, ArchiveFormat, ArchivePlanAction, ArchivePlanOperation, ArchiveTool,
    Ba2ArchiveKind, Ba2Version, CreateOptions, DiffComparison, DiffOptions, ExtractAllOptions,
    ExtractOptions, ExtractPlanAction, ExtractPlanOperation, OverwriteMode, Tes4Version,
    VerifyOptions,
};

/// Create a `dreamArchive` Luau module with `dream_archivetool` policy methods attached to
/// `dreamArchive.open*` archive userdata.
///
/// Call this before any `dream_archive::lua::LuaArchive` userdata is created in the same Lua
/// state. The returned table is the lower-level `dreamArchive` module; register
/// [`create_module`] separately when scripts also need path-based create/add helpers.
///
/// Userdata methods operate on the supplied `dream_archive` handle instead of reopening the
/// archive path in this policy layer. For `dreamArchive.openBytes(...)`, that handle is an
/// immutable byte snapshot. For `dreamArchive.openPath(...)`, payload reads follow
/// `dream_archive`'s path/source semantics; this policy layer does not promise that path-backed
/// bytes are pinned after the handle is created.
///
/// # Errors
///
/// Returns a Lua error if the lower-level archive module table or userdata method registration
/// cannot be created.
pub fn create_dream_archive_module(lua: &Lua) -> LuaResult<Table> {
    dream_archive::lua::create_module_with_archive_methods(lua, add_archive_tool_methods)
}

/// Register `dream_archivetool` policy methods on `dream_archive::lua::LuaArchive`.
///
/// This must run before any archive userdata is created in the Lua state. Use
/// [`create_dream_archive_module`] when possible; it also makes `dreamArchive.open*` create
/// userdata through `mlua`'s type registry so the added methods are visible.
///
/// Userdata methods operate on the supplied `dream_archive` handle instead of reopening the
/// archive path in this policy layer. For `dreamArchive.openBytes(...)`, that handle is an
/// immutable byte snapshot. For `dreamArchive.openPath(...)`, payload reads follow
/// `dream_archive`'s path/source semantics; this policy layer does not promise that path-backed
/// bytes are pinned after the handle is created.
///
/// # Errors
///
/// Returns a Lua error if userdata method registration fails.
pub fn register_dream_archive_methods(lua: &Lua) -> LuaResult<()> {
    dream_archive::lua::register_archive_methods(lua, add_archive_tool_methods)
}

#[expect(
    clippy::too_many_lines,
    reason = "Lua userdata method registration is a flat API table"
)]
fn add_archive_tool_methods(methods: &mut UserDataRegistry<dream_archive::lua::LuaArchive>) {
    methods.add_method("toolInfo", |lua, this, ()| {
        archive_info_table(
            lua,
            crate::archive::archive_info_ref(&lua_archive_label(this), lua_archive_ref(this)),
        )
    });
    methods.add_method("verify", |lua, this, opts: Option<Table>| {
        let options = verify_options(opts)?;
        let report = crate::verify::verify_loaded_archive(
            &lua_archive_label(this),
            lua_archive_ref(this),
            options,
        )
        .map_err(LuaError::external)?;
        verify_report_table(lua, report)
    });
    methods.add_method(
        "diff",
        |lua, this, (other, opts): (AnyUserData, Option<Table>)| {
            let other = other.borrow::<dream_archive::lua::LuaArchive>()?;
            let options = diff_options(opts)?;
            let report = crate::diff::diff_loaded_archives(
                &lua_archive_label(this),
                lua_archive_ref(this),
                &lua_archive_label(&other),
                lua_archive_ref(&other),
                options,
            )
            .map_err(LuaError::external)?;
            diff_report_table(lua, report)
        },
    );
    methods.add_method(
        "extract",
        |lua, this, (entry, opts): (LuaString, Option<Table>)| {
            let options = extract_options(opts, "archive:extract")?;
            let summary = crate::extract::extract_entry_by_path_from_loaded_archive(
                &lua_archive_label(this),
                lua_archive_ref(this),
                entry.as_bytes().as_ref(),
                &options,
            )
            .map_err(LuaError::external)?;
            summary_table(lua, summary.extracted, summary.skipped)
        },
    );
    methods.add_method(
        "extractMany",
        |lua, this, (entries, opts): (Table, Option<Table>)| {
            let entries = lua_byte_string_array(&entries, "archive:extractMany", "entries")?;
            let options = extract_options(opts, "archive:extractMany")?;
            let summary = crate::extract::extract_entries_by_path_from_loaded_archive(
                &lua_archive_label(this),
                lua_archive_ref(this),
                &entries,
                &options,
            )
            .map_err(LuaError::external)?;
            summary_table(lua, summary.extracted, summary.skipped)
        },
    );
    methods.add_method(
        "planExtract",
        |lua, this, (entries, opts): (Table, Option<Table>)| {
            let entries = lua_byte_string_array(&entries, "archive:planExtract", "entries")?;
            let options = extract_options(opts, "archive:planExtract")?;
            let plan = crate::extract::plan_extract_entries_by_path_from_loaded_archive(
                &lua_archive_label(this),
                &entries,
                &options,
            )
            .map_err(LuaError::external)?;
            extract_all_plan_table(lua, plan)
        },
    );
    methods.add_method(
        "extractByPathHex",
        |lua, this, (entry_hex, opts): (LuaString, Option<Table>)| {
            let entry = lua_path_hex(&entry_hex, "archive:extractByPathHex")?;
            let options = extract_options(opts, "archive:extractByPathHex")?;
            let summary = crate::extract::extract_entry_by_path_from_loaded_archive(
                &lua_archive_label(this),
                lua_archive_ref(this),
                &entry,
                &options,
            )
            .map_err(LuaError::external)?;
            summary_table(lua, summary.extracted, summary.skipped)
        },
    );
    methods.add_method(
        "extractManyByPathHex",
        |lua, this, (entries, opts): (Table, Option<Table>)| {
            let entries =
                lua_hex_string_array(&entries, "archive:extractManyByPathHex", "entries")?;
            let options = extract_options(opts, "archive:extractManyByPathHex")?;
            let summary = crate::extract::extract_entries_by_path_from_loaded_archive(
                &lua_archive_label(this),
                lua_archive_ref(this),
                &entries,
                &options,
            )
            .map_err(LuaError::external)?;
            summary_table(lua, summary.extracted, summary.skipped)
        },
    );
    methods.add_method(
        "planExtractByPathHex",
        |lua, this, (entries, opts): (Table, Option<Table>)| {
            let entries =
                lua_hex_string_array(&entries, "archive:planExtractByPathHex", "entries")?;
            let options = extract_options(opts, "archive:planExtractByPathHex")?;
            let plan = crate::extract::plan_extract_entries_by_path_from_loaded_archive(
                &lua_archive_label(this),
                &entries,
                &options,
            )
            .map_err(LuaError::external)?;
            extract_all_plan_table(lua, plan)
        },
    );
    methods.add_method("extractAll", |lua, this, opts: Option<Table>| {
        let options = extract_all_options(opts, "archive:extractAll")?;
        let summary = crate::extract::extract_all_from_loaded_archive(
            &lua_archive_label(this),
            lua_archive_ref(this),
            &options,
        )
        .map_err(LuaError::external)?;
        summary_table(lua, summary.extracted, summary.skipped)
    });
    methods.add_method("planExtractAll", |lua, this, opts: Option<Table>| {
        let options = extract_all_options(opts, "archive:planExtractAll")?;
        let plan = crate::extract::plan_extract_all_from_loaded_archive(
            &lua_archive_label(this),
            lua_archive_ref(this),
            &options,
        )
        .map_err(LuaError::external)?;
        extract_all_plan_table(lua, plan)
    });
}

fn lua_archive_ref(
    archive: &dream_archive::lua::LuaArchive,
) -> crate::loaded::LoadedArchiveRef<'_> {
    crate::loaded::LoadedArchiveRef::from_archive(archive.archive())
}

fn lua_archive_label(archive: &dream_archive::lua::LuaArchive) -> String {
    archive
        .path()
        .map_or_else(|| "<memory>".to_string(), |path| path.display().to_string())
}

fn lua_path_hex(entry_hex: &LuaString, context: &str) -> LuaResult<Vec<u8>> {
    let entry_hex = entry_hex.to_str().map_err(|_| {
        LuaError::external(format!("{context}.pathBytesHex must be a UTF-8 string"))
    })?;
    crate::path::decode_archive_path_hex(entry_hex.as_ref())
        .map_err(|err| LuaError::external(format!("{context}: invalid pathBytesHex: {err}")))
}

/// Create a Luau table for common [`ArchiveTool`] operations.
///
/// The returned table contains tool-policy operations: `info`, `verify`, `diff`, `extract`,
/// `extractByPathHex`, `extractHex`, `extractMany`, `extractManyByPathHex`, `planExtract`,
/// `planExtractByPathHex`, `extractAll`, `planExtractAll`, `create`, `planCreate`, `add`, and
/// `planAdd`. Archive-format primitives such as listing and payload reads belong to
/// `dreamArchive`'s Luau API instead. Archive entry arguments are Lua byte strings, so
/// `dreamArchive` entry paths can be passed to extraction functions without a UTF-8 boundary.
/// The table is not registered globally unless [`register`] is called; pass it to
/// [`mlua::Lua::register_module`] as `"@dreamArchivetool"` to make it `require`-able instead.
///
/// # Errors
///
/// Returns a Lua error if table/function creation fails.
pub fn create_module(lua: &Lua) -> LuaResult<Table> {
    let module = lua.create_table_with_capacity(0, 16)?;

    module.set(
        "info",
        lua.create_function(|lua, path: LuaString| {
            let path = path
                .to_str()
                .map_err(|_| LuaError::external("info.path must be a UTF-8 host path string"))?;
            let info = ArchiveTool::info(path.as_ref()).map_err(LuaError::external)?;
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
            |lua, (path, entry, opts): (LuaString, LuaString, Option<Table>)| {
                let path = path.to_str().map_err(|_| {
                    LuaError::external("extract.path must be a UTF-8 host path string")
                })?;
                let options = extract_options(opts, "extract")?;
                let entry = entry.as_bytes();
                let summary =
                    ArchiveTool::extract_by_path_bytes(path.as_ref(), entry.as_ref(), &options)
                        .map_err(LuaError::external)?;
                summary_table(lua, summary.extracted, summary.skipped)
            },
        )?,
    )?;
    module.set(
        "extractMany",
        lua.create_function(
            |lua, (path, entries, opts): (LuaString, Table, Option<Table>)| {
                let path = path.to_str().map_err(|_| {
                    LuaError::external("extractMany.path must be a UTF-8 host path string")
                })?;
                let entries = lua_byte_string_array(&entries, "extractMany", "entries")?;
                let options = extract_options(opts, "extractMany")?;
                let summary =
                    ArchiveTool::extract_many_by_path_bytes(path.as_ref(), &entries, &options)
                        .map_err(LuaError::external)?;
                summary_table(lua, summary.extracted, summary.skipped)
            },
        )?,
    )?;
    module.set(
        "planExtract",
        lua.create_function(
            |lua, (path, entries, opts): (LuaString, Table, Option<Table>)| {
                let path = path.to_str().map_err(|_| {
                    LuaError::external("planExtract.path must be a UTF-8 host path string")
                })?;
                let entries = lua_byte_string_array(&entries, "planExtract", "entries")?;
                let options = extract_options(opts, "planExtract")?;
                let plan =
                    ArchiveTool::plan_extract_many_by_path_bytes(path.as_ref(), &entries, &options)
                        .map_err(LuaError::external)?;
                extract_all_plan_table(lua, plan)
            },
        )?,
    )?;
    module.set(
        "extractManyByPathHex",
        lua.create_function(
            |lua, (path, entries, opts): (LuaString, Table, Option<Table>)| {
                let path = path.to_str().map_err(|_| {
                    LuaError::external("extractManyByPathHex.path must be a UTF-8 host path string")
                })?;
                let entries = lua_hex_string_array(&entries, "extractManyByPathHex", "entries")?;
                let options = extract_options(opts, "extractManyByPathHex")?;
                let summary =
                    ArchiveTool::extract_many_by_path_bytes(path.as_ref(), &entries, &options)
                        .map_err(LuaError::external)?;
                summary_table(lua, summary.extracted, summary.skipped)
            },
        )?,
    )?;
    module.set(
        "planExtractByPathHex",
        lua.create_function(
            |lua, (path, entries, opts): (LuaString, Table, Option<Table>)| {
                let path = path.to_str().map_err(|_| {
                    LuaError::external("planExtractByPathHex.path must be a UTF-8 host path string")
                })?;
                let entries = lua_hex_string_array(&entries, "planExtractByPathHex", "entries")?;
                let options = extract_options(opts, "planExtractByPathHex")?;
                let plan =
                    ArchiveTool::plan_extract_many_by_path_bytes(path.as_ref(), &entries, &options)
                        .map_err(LuaError::external)?;
                extract_all_plan_table(lua, plan)
            },
        )?,
    )?;
    module.set(
        "extractHex",
        extract_by_path_hex_function(lua, "extractHex")?,
    )?;
    module.set(
        "extractByPathHex",
        extract_by_path_hex_function(lua, "extractByPathHex")?,
    )?;
    Ok(())
}

fn extract_by_path_hex_function(lua: &Lua, context: &'static str) -> LuaResult<Function> {
    lua.create_function(
        move |lua, (path, entry_hex, opts): (LuaString, LuaString, Option<Table>)| {
            let path = path.to_str().map_err(|_| {
                LuaError::external(format!("{context}.path must be a UTF-8 host path string"))
            })?;
            let entry_hex = entry_hex.to_str().map_err(|_| {
                LuaError::external(format!("{context}.pathBytesHex must be a UTF-8 string"))
            })?;
            let entry =
                crate::path::decode_archive_path_hex(entry_hex.as_ref()).map_err(|err| {
                    LuaError::external(format!("{context}: invalid pathBytesHex: {err}"))
                })?;
            let options = extract_options(opts, context)?;
            let summary = ArchiveTool::extract_by_path_bytes(path.as_ref(), &entry, &options)
                .map_err(LuaError::external)?;
            summary_table(lua, summary.extracted, summary.skipped)
        },
    )
}

fn register_report_functions(lua: &Lua, module: &Table) -> LuaResult<()> {
    module.set(
        "verify",
        lua.create_function(|lua, (path, opts): (LuaString, Option<Table>)| {
            let path = path
                .to_str()
                .map_err(|_| LuaError::external("verify.path must be a UTF-8 host path string"))?;
            let options = verify_options(opts)?;
            let report =
                ArchiveTool::verify(path.as_ref(), &options).map_err(LuaError::external)?;
            verify_report_table(lua, report)
        })?,
    )?;
    module.set(
        "diff",
        lua.create_function(
            |lua, (old, new, opts): (LuaString, LuaString, Option<Table>)| {
                let old = old
                    .to_str()
                    .map_err(|_| LuaError::external("diff.old must be a UTF-8 host path string"))?;
                let new = new
                    .to_str()
                    .map_err(|_| LuaError::external("diff.new must be a UTF-8 host path string"))?;
                let options = diff_options(opts)?;
                let report = ArchiveTool::diff(old.as_ref(), new.as_ref(), &options)
                    .map_err(LuaError::external)?;
                diff_report_table(lua, report)
            },
        )?,
    )?;
    Ok(())
}

fn register_write_functions(lua: &Lua, module: &Table) -> LuaResult<()> {
    module.set(
        "extractAll",
        lua.create_function(|lua, (path, opts): (LuaString, Option<Table>)| {
            let path = path.to_str().map_err(|_| {
                LuaError::external("extractAll.path must be a UTF-8 host path string")
            })?;
            let options = extract_all_options(opts, "extractAll")?;
            let summary =
                ArchiveTool::extract_all(path.as_ref(), &options).map_err(LuaError::external)?;
            summary_table(lua, summary.extracted, summary.skipped)
        })?,
    )?;
    module.set(
        "planExtractAll",
        lua.create_function(|lua, (path, opts): (LuaString, Option<Table>)| {
            let path = path.to_str().map_err(|_| {
                LuaError::external("planExtractAll.path must be a UTF-8 host path string")
            })?;
            let options = extract_all_options(opts, "planExtractAll")?;
            let plan = ArchiveTool::plan_extract_all(path.as_ref(), &options)
                .map_err(LuaError::external)?;
            extract_all_plan_table(lua, plan)
        })?,
    )?;
    module.set(
        "create",
        lua.create_function(
            |lua, (output, input, opts): (LuaString, LuaString, Option<Table>)| {
                let output = output.to_str().map_err(|_| {
                    LuaError::external("create.output must be a UTF-8 host path string")
                })?;
                let input = input.to_str().map_err(|_| {
                    LuaError::external("create.input must be a UTF-8 host path string")
                })?;
                let options = create_options(opts, "create")?;
                let files = ArchiveTool::create(output.as_ref(), input.as_ref(), &options)
                    .map_err(LuaError::external)?;
                files_table(lua, files)
            },
        )?,
    )?;
    module.set(
        "planCreate",
        lua.create_function(
            |lua, (output, input, opts): (LuaString, LuaString, Option<Table>)| {
                let output = output.to_str().map_err(|_| {
                    LuaError::external("planCreate.output must be a UTF-8 host path string")
                })?;
                let input = input.to_str().map_err(|_| {
                    LuaError::external("planCreate.input must be a UTF-8 host path string")
                })?;
                let options = create_options(opts, "planCreate")?;
                let plan = ArchiveTool::plan_create(output.as_ref(), input.as_ref(), &options)
                    .map_err(LuaError::external)?;
                create_plan_table(lua, plan)
            },
        )?,
    )?;
    module.set(
        "add",
        lua.create_function(|lua, (archive, opts): (LuaString, Table)| {
            let archive = archive
                .to_str()
                .map_err(|_| LuaError::external("add.path must be a UTF-8 host path string"))?;
            let options = add_options(&opts, "add")?;
            let files = ArchiveTool::add(archive.as_ref(), &options).map_err(LuaError::external)?;
            files_table(lua, files)
        })?,
    )?;
    module.set(
        "planAdd",
        lua.create_function(|lua, (archive, opts): (LuaString, Table)| {
            let archive = archive
                .to_str()
                .map_err(|_| LuaError::external("planAdd.path must be a UTF-8 host path string"))?;
            let options = add_options(&opts, "planAdd")?;
            let plan =
                ArchiveTool::plan_add(archive.as_ref(), &options).map_err(LuaError::external)?;
            add_plan_table(lua, plan)
        })?,
    )?;

    Ok(())
}

/// Register the Luau API table as the global `dreamArchivetool` value.
///
/// # Errors
///
/// Returns a Lua error if the module table cannot be created or assigned to globals.
pub fn register(lua: &Lua) -> LuaResult<()> {
    let module = create_module(lua)?;
    lua.globals().set("dreamArchivetool", module)
}

fn format_name(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Tes3 => "bsaTes3",
        ArchiveFormat::Tes4 => "bsaTes4",
        ArchiveFormat::Ba2 => "ba2",
    }
}

fn create_options(opts: Option<Table>, context: &str) -> LuaResult<CreateOptions> {
    let Some(opts) = opts else {
        return Ok(CreateOptions::default());
    };
    reject_unknown_options(
        &opts,
        context,
        &[
            "format",
            "tes4Version",
            "ba2Kind",
            "ba2Version",
            "compress",
            "fsync",
            "followSymlinks",
        ],
    )?;
    let format = parse_optional_format(optional_string_field(&opts, context, "format")?)?;
    let tes4_version = optional_string_field(&opts, context, "tes4Version")?;
    let ba2_kind = optional_string_field(&opts, context, "ba2Kind")?;
    let ba2_version = optional_string_field(&opts, context, "ba2Version")?;
    match format {
        ArchiveFormat::Tes3 => {
            reject_irrelevant_create_option("tes4Version", tes4_version.is_some(), format)?;
            reject_irrelevant_create_option("ba2Kind", ba2_kind.is_some(), format)?;
            reject_irrelevant_create_option("ba2Version", ba2_version.is_some(), format)?;
        }
        ArchiveFormat::Tes4 => {
            reject_irrelevant_create_option("ba2Kind", ba2_kind.is_some(), format)?;
            reject_irrelevant_create_option("ba2Version", ba2_version.is_some(), format)?;
        }
        ArchiveFormat::Ba2 => {
            reject_irrelevant_create_option("tes4Version", tes4_version.is_some(), format)?;
        }
    }
    Ok(CreateOptions {
        format,
        tes4_version: parse_optional_tes4_version(tes4_version)?,
        ba2_kind: parse_optional_ba2_kind(ba2_kind)?,
        ba2_version: parse_optional_ba2_version(ba2_version)?,
        compress: opts.get::<Option<bool>>("compress")?.unwrap_or(false),
        fsync: opts.get::<Option<bool>>("fsync")?.unwrap_or(false),
        follow_symlinks: opts.get::<Option<bool>>("followSymlinks")?.unwrap_or(false),
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

fn add_options(opts: &Table, context: &str) -> LuaResult<AddOptions> {
    reject_unknown_options(
        opts,
        context,
        &["output", "inputs", "fsync", "followSymlinks"],
    )?;
    let inputs = required_table_field(opts, context, "inputs")?;
    let len = validate_dense_string_array(&inputs, context, "inputs")?;
    if len == 0 {
        return Err(LuaError::external(format!(
            "{context} requires at least one input path"
        )));
    }
    let mut paths = Vec::with_capacity(len);
    for index in 1..=len {
        let value: LuaString = inputs.raw_get(index)?;
        let value = value.to_str().map_err(|_| {
            LuaError::external(format!(
                "{context}.inputs[{index}] must be a UTF-8 host path string"
            ))
        })?;
        paths.push(PathBuf::from(value.as_ref()));
    }
    Ok(AddOptions {
        inputs: paths,
        output: optional_path_field(opts, context, "output")?,
        fsync: opts.get::<Option<bool>>("fsync")?.unwrap_or(false),
        follow_symlinks: opts.get::<Option<bool>>("followSymlinks")?.unwrap_or(false),
    })
}

fn verify_options(opts: Option<Table>) -> LuaResult<VerifyOptions> {
    let Some(opts) = opts else {
        return Ok(VerifyOptions::default());
    };
    reject_unknown_options(&opts, "verify", &["readPayloads"])?;
    Ok(VerifyOptions {
        read_payloads: opts.get::<Option<bool>>("readPayloads")?.unwrap_or(false),
    })
}

fn diff_options(opts: Option<Table>) -> LuaResult<DiffOptions> {
    let Some(opts) = opts else {
        return Ok(DiffOptions::default());
    };
    reject_unknown_options(&opts, "diff", &["fingerprintPayloads"])?;
    Ok(DiffOptions {
        fingerprint_payloads: opts
            .get::<Option<bool>>("fingerprintPayloads")?
            .unwrap_or(false),
    })
}

fn extract_options(opts: Option<Table>, context: &str) -> LuaResult<ExtractOptions> {
    let Some(opts) = opts else {
        return Ok(ExtractOptions::default());
    };
    reject_unknown_options(
        &opts,
        context,
        &["output", "overwrite", "preservePaths", "fsync"],
    )?;
    Ok(ExtractOptions {
        output: optional_path_field(&opts, context, "output")?,
        overwrite: parse_optional_overwrite(optional_string_field(&opts, context, "overwrite")?)?,
        preserve_paths: opts.get::<Option<bool>>("preservePaths")?.unwrap_or(true),
        fsync: opts.get::<Option<bool>>("fsync")?.unwrap_or(false),
    })
}

fn extract_all_options(opts: Option<Table>, context: &str) -> LuaResult<ExtractAllOptions> {
    let Some(opts) = opts else {
        return Ok(ExtractAllOptions::default());
    };
    reject_unknown_options(&opts, context, &["output", "overwrite", "fsync"])?;
    Ok(ExtractAllOptions {
        output: optional_path_field(&opts, context, "output")?,
        overwrite: parse_optional_overwrite(optional_string_field(&opts, context, "overwrite")?)?,
        fsync: opts.get::<Option<bool>>("fsync")?.unwrap_or(false),
    })
}

fn reject_unknown_options(opts: &Table, function: &str, allowed: &[&str]) -> LuaResult<()> {
    for pair in opts.clone().pairs::<Value, Value>() {
        let (key, _) = pair?;
        match key {
            Value::String(key) => {
                let key = key.to_str()?;
                if !allowed.contains(&key.as_ref()) {
                    return Err(LuaError::external(format!(
                        "{function}: unknown option key: {key}"
                    )));
                }
            }
            Value::Integer(key) => {
                return Err(LuaError::external(format!(
                    "{function}: unknown option key: {key}"
                )));
            }
            other => {
                return Err(LuaError::external(format!(
                    "{function}: unknown option key: {other:?}"
                )));
            }
        }
    }
    Ok(())
}

fn validate_dense_string_array(inputs: &Table, context: &str, field: &str) -> LuaResult<usize> {
    let len = inputs.raw_len();
    let mut count = 0usize;
    for pair in inputs.clone().pairs::<Value, Value>() {
        let (key, value) = pair?;
        let Value::Integer(index) = key else {
            return Err(LuaError::external(format!(
                "{context}.{field} must be a dense array of strings"
            )));
        };
        if index < 1 || usize::try_from(index).map_or(true, |index| index > len) {
            return Err(LuaError::external(format!(
                "{context}.{field} must be a dense 1-based array of strings"
            )));
        }
        if !matches!(value, Value::String(_)) {
            return Err(LuaError::external(format!(
                "{context}.{field}[{index}] must be a string"
            )));
        }
        count += 1;
    }
    if count != len {
        return Err(LuaError::external(format!(
            "{context}.{field} must not contain holes"
        )));
    }
    Ok(len)
}

fn required_table_field(opts: &Table, context: &str, field: &str) -> LuaResult<Table> {
    match opts.get::<Value>(field)? {
        Value::Nil => Err(LuaError::external(format!(
            "{context} requires opts.{field}"
        ))),
        Value::Table(table) => Ok(table),
        _ => Err(LuaError::external(format!(
            "{context}.{field} must be a table"
        ))),
    }
}

fn optional_path_field(opts: &Table, context: &str, field: &str) -> LuaResult<Option<PathBuf>> {
    match opts.get::<Value>(field)? {
        Value::Nil => Ok(None),
        Value::String(value) => value
            .to_str()
            .map(|value| Some(PathBuf::from(value.as_ref())))
            .map_err(|_| {
                LuaError::external(format!(
                    "{context}.{field} must be a UTF-8 host path string"
                ))
            }),
        _ => Err(LuaError::external(format!(
            "{context}.{field} must be a UTF-8 host path string"
        ))),
    }
}

fn optional_string_field(opts: &Table, context: &str, field: &str) -> LuaResult<Option<LuaString>> {
    match opts.get::<Value>(field)? {
        Value::Nil => Ok(None),
        Value::String(value) => Ok(Some(value)),
        _ => Err(LuaError::external(format!(
            "{context}.{field} must be a string"
        ))),
    }
}

fn lua_byte_string_array(entries: &Table, context: &str, field: &str) -> LuaResult<Vec<Vec<u8>>> {
    let len = validate_dense_string_array(entries, context, field)?;
    let mut paths = Vec::with_capacity(len);
    for index in 1..=len {
        let value: LuaString = entries.raw_get(index)?;
        paths.push(value.as_bytes().to_vec());
    }
    Ok(paths)
}

fn lua_hex_string_array(entries: &Table, context: &str, field: &str) -> LuaResult<Vec<Vec<u8>>> {
    let len = validate_dense_string_array(entries, context, field)?;
    let mut paths = Vec::with_capacity(len);
    for index in 1..=len {
        let value: LuaString = entries.raw_get(index)?;
        let value = value.to_str().map_err(|_| {
            LuaError::external(format!(
                "{context}.{field}[{index}] must be a UTF-8 pathBytesHex string"
            ))
        })?;
        let path = crate::path::decode_archive_path_hex(value.as_ref()).map_err(|err| {
            LuaError::external(format!(
                "{context}.{field}[{index}]: invalid pathBytesHex: {err}"
            ))
        })?;
        paths.push(path);
    }
    Ok(paths)
}

fn parse_optional_format(value: Option<LuaString>) -> LuaResult<ArchiveFormat> {
    match value {
        Some(value) => parse_format(Some(value.to_str()?.as_ref())),
        None => parse_format(None),
    }
}

fn parse_optional_tes4_version(value: Option<LuaString>) -> LuaResult<Tes4Version> {
    match value {
        Some(value) => parse_tes4_version(Some(value.to_str()?.as_ref())),
        None => parse_tes4_version(None),
    }
}

fn parse_optional_ba2_kind(value: Option<LuaString>) -> LuaResult<Ba2ArchiveKind> {
    match value {
        Some(value) => parse_ba2_kind(Some(value.to_str()?.as_ref())),
        None => parse_ba2_kind(None),
    }
}

fn parse_optional_ba2_version(value: Option<LuaString>) -> LuaResult<Ba2Version> {
    match value {
        Some(value) => parse_ba2_version(Some(value.to_str()?.as_ref())),
        None => parse_ba2_version(None),
    }
}

fn parse_optional_overwrite(value: Option<LuaString>) -> LuaResult<OverwriteMode> {
    match value {
        Some(value) => parse_overwrite(Some(value.to_str()?.as_ref())),
        None => parse_overwrite(None),
    }
}

fn parse_format(value: Option<&str>) -> LuaResult<ArchiveFormat> {
    match value.unwrap_or("tes3") {
        "tes3" | "bsaTes3" => Ok(ArchiveFormat::Tes3),
        "tes4" | "bsaTes4" => Ok(ArchiveFormat::Tes4),
        "ba2" => Ok(ArchiveFormat::Ba2),
        value => Err(LuaError::external(format!(
            "unknown archive format: {value}"
        ))),
    }
}

fn parse_tes4_version(value: Option<&str>) -> LuaResult<Tes4Version> {
    match value.unwrap_or("oblivion") {
        "oblivion" => Ok(Tes4Version::Oblivion),
        "fallout3" => Ok(Tes4Version::Fallout3),
        "skyrim" => Ok(Tes4Version::Skyrim),
        "skyrimSe" | "sse" => Ok(Tes4Version::SkyrimSe),
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
        "fallout4" => Ok(Ba2Version::Fallout4),
        "starfield" => Ok(Ba2Version::Starfield),
        "fallout4NextGen" => Ok(Ba2Version::Fallout4NextGen),
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

fn files_table(lua: &Lua, files: usize) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(0, 1)?;
    table.set("files", files)?;
    Ok(table)
}

fn archive_info_table(lua: &Lua, info: crate::ArchiveInfo) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(0, 9)?;
    table.set("path", info.path)?;
    table.set("format", format_name(info.format))?;
    table.set("fileCount", info.file_count)?;
    table.set("namedEntryCount", info.named_entry_count)?;
    table.set("hasUnnameableEntries", info.has_unnameable_entries)?;
    table.set("rewritable", info.rewritable)?;
    table.set("rewriteBlocker", info.rewrite_blocker)?;
    table.set("tes4", optional_tes4_info_table(lua, info.tes4)?)?;
    table.set("ba2", optional_ba2_info_table(lua, info.ba2)?)?;
    Ok(table)
}

fn optional_tes4_info_table(lua: &Lua, info: Option<crate::Tes4Info>) -> LuaResult<Option<Table>> {
    info.map(|info| {
        let table = lua.create_table_with_capacity(0, 7)?;
        table.set("version", info.version)?;
        table.set("archiveTypes", info.archive_types)?;
        table.set("archiveTypesBits", info.archive_types_bits)?;
        let archive_flags = info
            .archive_flags
            .iter()
            .map(|flag| kebab_to_camel(flag))
            .collect();
        table.set("archiveFlags", string_array(lua, archive_flags)?)?;
        table.set("archiveFlagsBits", info.archive_flags_bits)?;
        table.set(
            "unsupportedArchiveFlagsBits",
            info.unsupported_archive_flags_bits,
        )?;
        table.set("nameMode", kebab_to_camel(&info.name_mode))?;
        Ok(table)
    })
    .transpose()
}

fn optional_ba2_info_table(lua: &Lua, info: Option<crate::Ba2Info>) -> LuaResult<Option<Table>> {
    info.map(|info| {
        let table = lua.create_table_with_capacity(0, 4)?;
        table.set("version", info.version)?;
        table.set("payloadFormat", info.payload_format)?;
        table.set("compressionFormat", info.compression_format)?;
        table.set("strings", info.strings)?;
        Ok(table)
    })
    .transpose()
}

fn verify_report_table(lua: &Lua, report: crate::VerifyReport) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(0, 11)?;
    table.set("path", report.path)?;
    table.set("format", format_name(report.format))?;
    table.set("fileCount", report.file_count)?;
    table.set("namedEntryCount", report.named_entry_count)?;
    table.set("unnameableEntries", report.unnameable_entries)?;
    table.set("rewritable", report.rewritable)?;
    table.set("rewriteBlocker", report.rewrite_blocker)?;
    table.set(
        "duplicateNormalizedPaths",
        verify_path_issue_array(lua, report.duplicate_normalized_paths)?,
    )?;
    table.set(
        "unsafePaths",
        verify_path_issue_array(lua, report.unsafe_paths)?,
    )?;
    table.set("payloadsRead", report.payloads_read)?;
    table.set("warnings", string_array(lua, report.warnings)?)?;
    Ok(table)
}

fn verify_path_issue_array(lua: &Lua, issues: Vec<crate::VerifyPathIssue>) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(issues.len(), 0)?;
    for (index, issue) in issues.into_iter().enumerate() {
        let issue_table = lua.create_table_with_capacity(0, 4)?;
        issue_table.set("path", issue.path)?;
        issue_table.set("pathBytesHex", issue.path_bytes_hex)?;
        issue_table.set("rawPathBytesHex", issue.raw_path_bytes_hex)?;
        issue_table.set(
            "collidingRawPathBytesHex",
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
    table.set("fingerprintPayloads", report.fingerprint_payloads)?;
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
    table.set("pathBytesHex", entry.path_bytes_hex)?;
    table.set("size", optional_u64_decimal(entry.size))?;
    table.set(
        "compressedSize",
        optional_u64_decimal(entry.compressed_size),
    )?;
    table.set("payloadFingerprint", entry.payload_fingerprint)?;
    Ok(table)
}

fn diff_change_array(lua: &Lua, changes: Vec<crate::DiffChange>) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(changes.len(), 0)?;
    for (index, change) in changes.into_iter().enumerate() {
        let change_table = lua.create_table_with_capacity(0, 4)?;
        change_table.set("path", change.path)?;
        change_table.set("pathBytesHex", change.path_bytes_hex)?;
        change_table.set("old", diff_entry_state_table(lua, change.old)?)?;
        change_table.set("new", diff_entry_state_table(lua, change.new)?)?;
        table.set(index + 1, change_table)?;
    }
    Ok(table)
}

fn diff_entry_state_table(lua: &Lua, state: crate::DiffEntryState) -> LuaResult<Table> {
    let table = lua.create_table_with_capacity(0, 3)?;
    table.set("size", optional_u64_decimal(state.size))?;
    table.set(
        "compressedSize",
        optional_u64_decimal(state.compressed_size),
    )?;
    table.set("payloadFingerprint", state.payload_fingerprint)?;
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
        entry_table.set("pathBytesHex", entry.path_bytes_hex)?;
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
        entry_table.set("pathBytesHex", entry.path_bytes_hex)?;
        entry_table.set("size", optional_u64_decimal(entry.size))?;
        table.set(index + 1, entry_table)?;
    }
    Ok(table)
}

/// Spell a kebab-case policy name (as the CLI reports it) the Luau way: `"hash-only"` becomes
/// `"hashOnly"`, matching `dreamArchive`'s own enum strings.
fn kebab_to_camel(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut upper = false;
    for ch in value.chars() {
        if ch == '-' {
            upper = true;
        } else if upper {
            out.push(ch.to_ascii_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn optional_u64_decimal(value: Option<u64>) -> Option<String> {
    value.map(|value| value.to_string())
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
        DiffComparison::MetadataOnly => "metadataOnly",
        DiffComparison::PayloadFingerprint => "payloadFingerprint",
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
        ExtractPlanOperation::Extract => "extract",
        ExtractPlanOperation::ExtractAll => "extractAll",
    }
}

fn extract_plan_action_name(action: ExtractPlanAction) -> &'static str {
    match action {
        ExtractPlanAction::Extract => "extract",
        ExtractPlanAction::Skip => "skip",
        ExtractPlanAction::Overwrite => "overwrite",
        ExtractPlanAction::Conflict => "conflict",
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
            "dream_archivetool-lua-{name}-{}",
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

    fn write_tes3_archive(path: &Path, payload: &[u8]) {
        let mut builder = dream_archive::Tes3BsaBuilder::new();
        builder.add_bytes("textures/example.dds", payload).unwrap();
        builder.write_path(path).unwrap();
    }

    #[test]
    fn lua_archive_userdata_methods_verify_diff_and_plan() {
        let dir = unique_dir("archive-methods");
        fs::create_dir_all(&dir).unwrap();
        let old_archive = dir.join("old.bsa");
        let new_archive = dir.join("new.bsa");
        write_tes3_archive(&old_archive, b"old");
        write_tes3_archive(&new_archive, b"new");

        let lua = Lua::new();
        let dream_archive_module = create_dream_archive_module(&lua).unwrap();
        lua.globals()
            .set("dreamArchive", dream_archive_module)
            .unwrap();
        lua.globals()
            .set("old_path", old_archive.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("new_path", new_archive.to_str().unwrap())
            .unwrap();

        let (payloads_read, comparison, changed, planned): (usize, String, usize, usize) = lua
            .load(
                r"
                local old = dreamArchive.openPath(old_path)
                local new = dreamArchive.openPath(new_path)
                local verify = old:verify({ readPayloads = true })
                local diff = old:diff(new, { fingerprintPayloads = true })
                local entry = old:entries()[1]
                local plan = old:planExtract({ entry.path }, { preservePaths = false })
                return verify.payloadsRead, diff.comparison, #diff.changed, #plan.entries
                ",
            )
            .eval()
            .unwrap();

        assert_eq!(payloads_read, 1);
        assert_eq!(comparison, "payloadFingerprint");
        assert_eq!(changed, 1);
        assert_eq!(planned, 1);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_archive_userdata_open_bytes_uses_snapshot_not_stale_path() {
        let dir = unique_dir("archive-methods-stale-path");
        fs::create_dir_all(&dir).unwrap();
        let archive = dir.join("archive.bsa");
        let output = dir.join("out");
        write_tes3_archive(&archive, b"old");

        let lua = Lua::new();
        let dream_archive_module = create_dream_archive_module(&lua).unwrap();
        lua.globals()
            .set("dreamArchive", dream_archive_module)
            .unwrap();
        let archive_bytes = fs::read(&archive).unwrap();
        lua.globals()
            .set("archive_bytes", lua.create_string(&archive_bytes).unwrap())
            .unwrap();
        lua.globals()
            .set("out_path", output.to_str().unwrap())
            .unwrap();
        lua.load("archive = dreamArchive.openBytes(archive_bytes)")
            .exec()
            .unwrap();

        write_tes3_archive(&archive, b"new");

        lua.load(
            r#"
            archive:extract("textures/example.dds", {
                output = out_path,
                preservePaths = false,
            })
            "#,
        )
        .exec()
        .unwrap();

        assert_eq!(fs::read(output.join("example.dds")).unwrap(), b"old");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_archive_userdata_open_bytes_reports_memory_label() {
        let dir = unique_dir("archive-methods-open-bytes");
        fs::create_dir_all(&dir).unwrap();
        let archive = dir.join("archive.bsa");
        write_tes3_archive(&archive, b"payload");
        let bytes = fs::read(&archive).unwrap();

        let lua = Lua::new();
        let dream_archive_module = create_dream_archive_module(&lua).unwrap();
        lua.globals()
            .set("dreamArchive", dream_archive_module)
            .unwrap();
        lua.globals()
            .set("archive_bytes", lua.create_string(&bytes).unwrap())
            .unwrap();

        let path: String = lua
            .load(
                r"
                local archive = dreamArchive.openBytes(archive_bytes)
                return archive:verify().path
                ",
            )
            .eval()
            .unwrap();

        assert_eq!(path, "<memory>");

        fs::remove_dir_all(dir).unwrap();
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
                local info = dreamArchivetool.info(archive_path)
                return info.format, info.rewritable,
                    dreamArchivetool.list == nil,
                    dreamArchivetool.readEntry == nil
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(format, "bsaTes3");
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
                local summary = dreamArchivetool.extract(archive_path, 'textures/example.dds', {
                    output = output_path,
                    preservePaths = false,
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
                local summary = dreamArchivetool.extractHex(archive_path, entry_hex, {
                    output = output_path,
                    preservePaths = false,
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
    fn lua_extracts_paths_from_dream_archive_entry() {
        let dir = unique_dir("archive-bridge");
        let archive = create_test_archive(&dir);
        let raw_output = dir.join("raw-output");
        let lua = Lua::new();
        lua.globals()
            .set(
                "dreamArchive",
                crate::dream_archive::lua::create_module(&lua).unwrap(),
            )
            .unwrap();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("raw_output", raw_output.to_str().unwrap())
            .unwrap();
        let raw_extracted: usize = lua
            .load(
                r"
                local archive = dreamArchive.openPath(archive_path)
                local entry = archive:entries()[1]
                local raw = dreamArchivetool.extract(archive_path, entry.path, {
                    output = raw_output,
                    preservePaths = false,
                })
                return raw.extracted
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(raw_extracted, 1);
        assert_eq!(fs::read(raw_output.join("example.dds")).unwrap(), b"hello");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_extract_many_uses_dream_archive_entry_paths() {
        let dir = unique_dir("extract-many");
        let archive = create_test_archive(&dir);
        let output = dir.join("output");
        let lua = Lua::new();
        lua.globals()
            .set(
                "dreamArchive",
                crate::dream_archive::lua::create_module(&lua).unwrap(),
            )
            .unwrap();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("output_path", output.to_str().unwrap())
            .unwrap();

        let (operation, action, extracted): (String, String, usize) = lua
            .load(
                r"
                local archive = dreamArchive.openPath(archive_path)
                local entry = archive:entries()[1]
                local plan = dreamArchivetool.planExtract(archive_path, { entry.path }, {
                    output = output_path,
                    preservePaths = false,
                })
                local summary = dreamArchivetool.extractMany(archive_path, { entry.path }, {
                    output = output_path,
                    preservePaths = false,
                })
                return plan.operation, plan.entries[1].action, summary.extracted
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(operation, "extract");
        assert_eq!(action, "extract");
        assert_eq!(extracted, 1);
        assert_eq!(fs::read(output.join("example.dds")).unwrap(), b"hello");
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn lua_extracts_non_utf8_archive_path_bytes() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let dir = unique_dir("non-utf8-path");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(
            input.join(OsString::from_vec(b"bad-\xff.dds".to_vec())),
            b"bytes",
        )
        .unwrap();
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
        let bridge_output = dir.join("bridge");
        let raw_output = dir.join("raw");
        let hex_output = dir.join("hex");
        let lua = Lua::new();
        lua.globals()
            .set(
                "dreamArchive",
                crate::dream_archive::lua::create_module(&lua).unwrap(),
            )
            .unwrap();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("bridge_output", bridge_output.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("raw_output", raw_output.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("hex_output", hex_output.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("entry_bytes", lua.create_string(b"bad-\xff.dds").unwrap())
            .unwrap();

        let (bridge_extracted, raw_extracted, hex_extracted): (usize, usize, usize) = lua
            .load(
                r"
                local archive = dreamArchive.openPath(archive_path)
                local entry = archive:entries()[1]
                local bridge = dreamArchivetool.extract(archive_path, entry.path, {
                    output = bridge_output,
                    preservePaths = false,
                })
                local raw = dreamArchivetool.extract(archive_path, entry_bytes, {
                    output = raw_output,
                    preservePaths = false,
                })
                local by_hex = dreamArchivetool.extractByPathHex(archive_path, '6261642dff2e646473', {
                    output = hex_output,
                    preservePaths = false,
                })
                return bridge.extracted, raw.extracted, by_hex.extracted
            ",
            )
            .eval()
            .unwrap();

        assert_eq!((bridge_extracted, raw_extracted, hex_extracted), (1, 1, 1));
        let output_name = OsString::from_vec(b"bad-\xff.dds".to_vec());
        assert_eq!(
            fs::read(bridge_output.join(&output_name)).unwrap(),
            b"bytes"
        );
        assert_eq!(fs::read(raw_output.join(&output_name)).unwrap(), b"bytes");
        assert_eq!(fs::read(hex_output.join(&output_name)).unwrap(), b"bytes");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_extract_many_can_use_hex_paths() {
        let dir = unique_dir("extract-many-hex");
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

        let (action, extracted): (String, usize) = lua
            .load(
                r"
                local entries = { '74657874757265732f6578616d706c652e646473' }
                local plan = dreamArchivetool.planExtractByPathHex(archive_path, entries, {
                    output = output_path,
                    preservePaths = false,
                })
                local summary = dreamArchivetool.extractManyByPathHex(archive_path, entries, {
                    output = output_path,
                    preservePaths = false,
                })
                return plan.entries[1].action, summary.extracted
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(action, "extract");
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
                local summary = dreamArchivetool.extract(archive_path, 'textures/example.dds', {
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
                local summary = dreamArchivetool.extractAll(archive_path, {
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
        let files: usize = lua
            .load(
                r"
                local created = dreamArchivetool.create(archive_path, input_path, { format = 'bsaTes3' })
                local updated = dreamArchivetool.add(archive_path, {
                    inputs = { added_path },
                })
                return created.files + updated.files
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(files, 3);
        let entries = ArchiveTool::list(&archive).unwrap();
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

        let (
            payloads_read,
            verify_format,
            create_format,
            add_format,
            extract_action,
            create_action,
            add_action,
        ): (usize, String, String, String, String, String, String) = lua
            .load(
                r"
                local verify = dreamArchivetool.verify(archive_path, { readPayloads = true })
                local extract_plan = dreamArchivetool.planExtractAll(archive_path, {
                    output = output_path,
                })
                local create_plan = dreamArchivetool.planCreate(updated_path, input_path, {
                    format = 'bsaTes3',
                })
                local add_plan = dreamArchivetool.planAdd(archive_path, {
                    output = updated_path,
                    inputs = { input_path },
                })
                local add_action = nil
                for _, entry in ipairs(add_plan.entries) do
                    if entry.action == 'add' then
                        add_action = entry.action
                    end
                end
                return verify.payloadsRead, verify.format, create_plan.format, add_plan.format,
                    extract_plan.entries[1].action, create_plan.entries[1].action, add_action
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(payloads_read, 1);
        assert_eq!(verify_format, "bsaTes3");
        assert_eq!(create_format, "bsaTes3");
        assert_eq!(add_format, "bsaTes3");
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
                local diff = dreamArchivetool.diff(archive_path, other_archive_path, {
                    fingerprintPayloads = true,
                })
                return diff.comparison
            ",
            )
            .eval()
            .unwrap();
        assert_eq!(comparison, "payloadFingerprint");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_reports_wide_sizes_as_decimal_strings() {
        let lua = Lua::new();
        let entry = crate::DiffEntry {
            path: "huge.bin".to_owned(),
            path_bytes_hex: "687567652e62696e".to_owned(),
            size: Some(u64::MAX),
            compressed_size: Some(9_007_199_254_740_993),
            payload_fingerprint: None,
        };
        let table = diff_entry_table(&lua, entry).unwrap();

        assert_eq!(table.get::<String>("size").unwrap(), u64::MAX.to_string());
        assert_eq!(
            table.get::<String>("compressedSize").unwrap(),
            "9007199254740993"
        );
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
                local created = dreamArchivetool.create(archive_path, input_path, {
                    format = 'ba2',
                    ba2Kind = 'gnrl',
                    ba2Version = 'starfield',
                })
                local plan = dreamArchivetool.planCreate(archive_path, input_path, {
                    format = 'ba2',
                    ba2Kind = 'gnrl',
                    ba2Version = 'starfield',
                })
                assert(plan.format == 'ba2')
                return created.files
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
                return dreamArchivetool.extract(archive_path, 'textures/example.dds', {
                    overwrite = 'explode',
                })
            ",
            )
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(err.to_string().contains("unknown overwrite mode"));

        let err = lua
            .load("return dreamArchivetool.create('out.bsa', 'input', { format = 'unknown' })")
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(err.to_string().contains("unknown archive format"));

        let err = lua
            .load(
                "return dreamArchivetool.create('out.bsa', 'input', { format = 'tes3', ba2Kind = 'gnrl' })",
            )
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(err.to_string().contains("ba2Kind is not valid"));

        let err = lua
            .load("return dreamArchivetool.add(archive_path, { inputs = {} })")
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(err.to_string().contains("at least one input path"));

        let err = lua
            .load("return dreamArchivetool.add(archive_path, { output = 'out.bsa' })")
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(err.to_string().contains("add requires opts.inputs"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_reports_unknown_option_contexts() {
        let dir = unique_dir("unknown-options");
        let archive = create_test_archive(&dir);
        let lua = Lua::new();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();

        let err = lua
            .load(
                r"
                return dreamArchivetool.extract(archive_path, 'textures/example.dds', {
                    overwirte = 'skip',
                })
            ",
            )
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("extract: unknown option key: overwirte")
        );

        let err = lua
            .load(
                r"
                return dreamArchivetool.extractHex(archive_path, '74657874757265732f6578616d706c652e646473', {
                    overwirte = 'skip',
                })
            ",
            )
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("extractHex: unknown option key: overwirte")
        );

        let err = lua
            .load("return dreamArchivetool.extractHex(archive_path, 'not-hex')")
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(err.to_string().contains("extractHex: invalid pathBytesHex"));

        let err = lua
            .load("return dreamArchivetool.extractByPathHex(archive_path, 'not-hex')")
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("extractByPathHex: invalid pathBytesHex")
        );

        let err = lua
            .load(
                r"
                return dreamArchivetool.add(archive_path, {
                    output = 'out.bsa',
                    inputs = { [2] = 'file.txt' },
                })
            ",
            )
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("add.inputs must be a dense 1-based array")
        );

        let err = lua
            .load("return dreamArchivetool.add(archive_path, { output = 'out.bsa', inputs = 'file.txt' })")
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(err.to_string().contains("add.inputs must be a table"));

        let err = lua
            .load("return dreamArchivetool.add(archive_path, { output = 12, inputs = { 'file.txt' } })")
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("add.output must be a UTF-8 host path string")
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_reports_hex_batch_and_output_path_contexts() {
        let dir = unique_dir("hex-output-contexts");
        let archive = create_test_archive(&dir);
        let lua = Lua::new();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();

        let err = lua
            .load("return dreamArchivetool.extractManyByPathHex(archive_path, { 'not-hex' })")
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("extractManyByPathHex.entries[1]: invalid pathBytesHex")
        );

        let err = lua
            .load(
                r"
                return dreamArchivetool.extract(archive_path, 'textures/example.dds', {
                    output = 12,
                })
            ",
            )
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("extract.output must be a UTF-8 host path string")
        );

        let err = lua
            .load(
                r"
                return dreamArchivetool.extractAll(archive_path, {
                    output = 12,
                })
            ",
            )
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("extractAll.output must be a UTF-8 host path string")
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_reports_plan_unknown_option_contexts() {
        let dir = unique_dir("plan-unknown-options");
        let archive = create_test_archive(&dir);
        let lua = Lua::new();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();

        let err = lua
            .load(
                r"
                return dreamArchivetool.planExtractAll(archive_path, {
                    overwirte = 'skip',
                })
            ",
            )
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("planExtractAll: unknown option key: overwirte")
        );

        let err = lua
            .load(
                r"
                return dreamArchivetool.create('out.bsa', 'input', {
                    format = 'bsaTes3',
                    followSymlink = true,
                })
            ",
            )
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("create: unknown option key: followSymlink")
        );

        let err = lua
            .load(
                r"
                return dreamArchivetool.planCreate('out.bsa', 'input', {
                    format = 'bsaTes3',
                    followSymlink = true,
                })
            ",
            )
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("planCreate: unknown option key: followSymlink")
        );

        let err = lua
            .load("return dreamArchivetool.create('out.bsa', 'input', { format = 'bsaTes4', ba2Kind = 'gnrl' })")
            .eval::<mlua::Value>()
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("ba2Kind is not valid with format bsaTes4")
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_module_can_be_required_and_uses_luau_names() {
        let dir = unique_dir("require-luau-names");
        let input = dir.join("input");
        write_input_tree(&input);
        let archive = dir.join("out.bsa");
        ArchiveTool::create(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Tes4,
                ..Default::default()
            },
        )
        .unwrap();
        let lua = Lua::new();
        lua.register_module("@dreamArchive", create_dream_archive_module(&lua).unwrap())
            .unwrap();
        lua.register_module("@dreamArchivetool", create_module(&lua).unwrap())
            .unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();
        lua.globals()
            .set("output_path", dir.join("output").to_str().unwrap())
            .unwrap();

        lua.load(
            r#"
            local dreamArchive = require("@dreamArchive")
            local dreamArchivetool = require("@dreamArchivetool")
            local info = dreamArchivetool.info(archive_path)
            assert(info.format == "bsaTes4")
            assert(info.tes4.nameMode == "strings")
            assert(table.find(info.tes4.archiveFlags, "directoryStrings") ~= nil)
            assert(table.find(info.tes4.archiveFlags, "fileStrings") ~= nil)
            assert(dreamArchivetool.extract_many == nil)

            local archive = dreamArchive.openPath(archive_path)
            assert(archive:toolInfo().fileCount == 1)
            local plan = archive:planExtractAll({ output = output_path })
            assert(plan.operation == "extractAll")
            assert(#plan.entries[1].pathBytesHex > 0)

            local ok, err = pcall(dreamArchivetool.planCreate, "out.bsa", "input", { format = "bsa-tes3" })
            assert(not ok and string.find(tostring(err), "unknown archive format", 1, true))
            ok, err = pcall(dreamArchivetool.planCreate, "out.ba2", "input", {
                format = "ba2",
                ba2Version = "fallout-4-next-gen",
            })
            assert(not ok and string.find(tostring(err), "unknown BA2 version", 1, true))
            "#,
        )
        .exec()
        .unwrap();

        fs::remove_dir_all(dir).unwrap();
    }
}
