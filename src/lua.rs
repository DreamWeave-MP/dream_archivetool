use std::path::PathBuf;

use mlua::{Error as LuaError, Lua, Result as LuaResult, Table};

use crate::{
    AddOptions, ArchiveFormat, ArchiveTool, Ba2ArchiveKind, Ba2Version, CreateOptions,
    ExtractAllOptions, ExtractOptions, OverwriteMode, Tes4Version,
};

/// Create a Lua table for common [`ArchiveTool`] operations.
///
/// The returned table contains `guess_format`, `info`, `list`, `read_entry`, `read_entry_hex`,
/// `extract`, `extract_hex`, `extract_all`, `create`, and `add` functions. The table is not
/// registered globally unless [`register`] is called.
pub fn create_module(lua: &Lua) -> LuaResult<Table> {
    let module = lua.create_table()?;

    module.set(
        "guess_format",
        lua.create_function(|_, path: String| {
            ArchiveTool::guess_format(path)
                .map(format_name)
                .map_err(LuaError::external)
        })?,
    )?;
    module.set(
        "info",
        lua.create_function(|lua, path: String| {
            let info = ArchiveTool::info(path).map_err(LuaError::external)?;
            let table = lua.create_table()?;
            table.set("path", info.path)?;
            table.set("format", format_name(info.format))?;
            table.set("file_count", info.file_count)?;
            Ok(table)
        })?,
    )?;
    module.set(
        "list",
        lua.create_function(|lua, path: String| {
            let entries = ArchiveTool::list(path).map_err(LuaError::external)?;
            let table = lua.create_table()?;
            for (index, entry) in entries.into_iter().enumerate() {
                let entry_table = lua.create_table()?;
                entry_table.set("path", entry.path)?;
                entry_table.set("path_bytes_hex", entry.path_bytes_hex)?;
                entry_table.set("size", entry.size)?;
                entry_table.set("compressed_size", entry.compressed_size)?;
                table.set(index + 1, entry_table)?;
            }
            Ok(table)
        })?,
    )?;
    register_entry_functions(lua, &module)?;
    register_write_functions(lua, &module)?;

    Ok(module)
}

fn register_entry_functions(lua: &Lua, module: &Table) -> LuaResult<()> {
    module.set(
        "read_entry",
        lua.create_function(|lua, (path, entry): (String, String)| {
            let bytes = ArchiveTool::read_entry(path, &entry).map_err(LuaError::external)?;
            lua.create_string(&bytes)
        })?,
    )?;
    module.set(
        "read_entry_hex",
        lua.create_function(|lua, (path, entry_hex): (String, String)| {
            let entry =
                crate::path::decode_archive_path_hex(&entry_hex).map_err(LuaError::external)?;
            let bytes =
                ArchiveTool::read_entry_by_path_bytes(path, &entry).map_err(LuaError::external)?;
            lua.create_string(&bytes)
        })?,
    )?;
    module.set(
        "extract",
        lua.create_function(
            |lua, (path, entry, opts): (String, String, Option<Table>)| {
                let options = extract_options(opts)?;
                let summary =
                    ArchiveTool::extract(path, &entry, &options).map_err(LuaError::external)?;
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
        "create",
        lua.create_function(
            |_, (output, input, opts): (String, String, Option<Table>)| {
                let options = create_options(opts)?;
                ArchiveTool::create(output, input, &options).map_err(LuaError::external)
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
    let table = lua.create_table()?;
    table.set("extracted", extracted)?;
    table.set("skipped", skipped)?;
    Ok(table)
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
    fn lua_module_lists_and_reads_created_archive() {
        let dir = unique_dir("list-read");
        let archive = create_test_archive(&dir);
        let lua = Lua::new();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();

        let path: String = lua
            .load("return dream_archivetool.list(archive_path)[1].path")
            .eval()
            .unwrap();
        let bytes: String = lua
            .load("return dream_archivetool.read_entry(archive_path, 'textures/example.dds')")
            .eval()
            .unwrap();

        assert_eq!(path, "textures/example.dds");
        assert_eq!(bytes.as_bytes(), b"hello");
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

        let bytes: String = lua
            .load(
                r"
                local entry_hex = dream_archivetool.list(archive_path)[1].path_bytes_hex
                return dream_archivetool.read_entry_hex(archive_path, entry_hex)
            ",
            )
            .eval()
            .unwrap();
        let extracted: usize = lua
            .load(
                r"
                local entry_hex = dream_archivetool.list(archive_path)[1].path_bytes_hex
                local summary = dream_archivetool.extract_hex(archive_path, entry_hex, {
                    output = output_path,
                    preserve_paths = false,
                })
                return summary.extracted
            ",
            )
            .eval()
            .unwrap();

        assert_eq!(bytes.as_bytes(), b"hello");
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
