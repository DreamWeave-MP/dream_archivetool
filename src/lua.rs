use std::path::PathBuf;

use mlua::{Error as LuaError, Lua, Result as LuaResult, Table};

use crate::{
    AddOptions, ArchiveFormat, ArchiveTool, CreateOptions, ExtractAllOptions, Fo4ArchiveKind,
    Fo4Version, OverwriteMode, Tes4Version,
};

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
                entry_table.set("size", entry.size)?;
                entry_table.set("compressed_size", entry.compressed_size)?;
                table.set(index + 1, entry_table)?;
            }
            Ok(table)
        })?,
    )?;
    module.set(
        "extract",
        lua.create_function(|lua, (path, entry): (String, String)| {
            let bytes = ArchiveTool::read_entry(path, &entry).map_err(LuaError::external)?;
            lua.create_string(&bytes)
        })?,
    )?;
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
        lua.create_function(
            |_, (archive, output, inputs, opts): (String, String, Table, Option<Table>)| {
                let mut paths = Vec::new();
                for value in inputs.sequence_values::<String>() {
                    paths.push(PathBuf::from(value?));
                }
                let options = AddOptions {
                    inputs: paths,
                    output: PathBuf::from(output),
                    create: create_options(opts)?,
                };
                ArchiveTool::add(archive, &options).map_err(LuaError::external)
            },
        )?,
    )?;

    Ok(module)
}

pub fn register(lua: &Lua) -> LuaResult<()> {
    let module = create_module(lua)?;
    lua.globals().set("rome_archivetool", module)
}

fn format_name(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Tes3 => "tes3",
        ArchiveFormat::Tes4 => "tes4",
        ArchiveFormat::Fo4 => "fo4",
    }
}

fn create_options(opts: Option<Table>) -> LuaResult<CreateOptions> {
    let Some(opts) = opts else {
        return Ok(CreateOptions::default());
    };
    Ok(CreateOptions {
        format: parse_format(opts.get::<Option<String>>("format")?.as_deref())?,
        tes4_version: parse_tes4_version(opts.get::<Option<String>>("tes4_version")?.as_deref())?,
        fo4_kind: parse_fo4_kind(opts.get::<Option<String>>("ba2_kind")?.as_deref())?,
        fo4_version: parse_fo4_version(opts.get::<Option<String>>("ba2_version")?.as_deref())?,
    })
}

fn extract_all_options(opts: Option<Table>) -> LuaResult<ExtractAllOptions> {
    let Some(opts) = opts else {
        return Ok(ExtractAllOptions::default());
    };
    Ok(ExtractAllOptions {
        output: opts.get::<Option<String>>("output")?.map(PathBuf::from),
        overwrite: parse_overwrite(opts.get::<Option<String>>("overwrite")?.as_deref())?,
    })
}

fn parse_format(value: Option<&str>) -> LuaResult<ArchiveFormat> {
    match value.unwrap_or("tes3") {
        "tes3" => Ok(ArchiveFormat::Tes3),
        "tes4" => Ok(ArchiveFormat::Tes4),
        "fo4" => Ok(ArchiveFormat::Fo4),
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

fn parse_fo4_kind(value: Option<&str>) -> LuaResult<Fo4ArchiveKind> {
    match value.unwrap_or("gnrl") {
        "gnrl" => Ok(Fo4ArchiveKind::Gnrl),
        "dx10" => Ok(Fo4ArchiveKind::Dx10),
        "gnmf" => Ok(Fo4ArchiveKind::Gnmf),
        value => Err(LuaError::external(format!("unknown BA2 kind: {value}"))),
    }
}

fn parse_fo4_version(value: Option<&str>) -> LuaResult<Fo4Version> {
    match value.unwrap_or("fallout4") {
        "fallout4" | "fallout-4" => Ok(Fo4Version::Fallout4),
        "starfield" => Ok(Fo4Version::Starfield),
        "fallout4-next-gen" | "fallout-4-next-gen" => Ok(Fo4Version::Fallout4NextGen),
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
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn lua_module_lists_created_archive() {
        let dir = std::env::temp_dir().join(format!(
            "rome-archivetool-lua-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("hello.txt"), b"hello").unwrap();
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
        let lua = Lua::new();
        register(&lua).unwrap();
        lua.globals()
            .set("archive_path", archive.to_str().unwrap())
            .unwrap();

        let path: String = lua
            .load("return rome_archivetool.list(archive_path)[1].path")
            .eval()
            .unwrap();

        assert_eq!(path, "hello.txt");
        fs::remove_dir_all(dir).unwrap();
    }
}
