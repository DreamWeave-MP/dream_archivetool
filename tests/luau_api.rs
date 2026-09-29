// SPDX-License-Identifier: GPL-3.0-or-later

//! The `dream.archivetool` extension composed with `dream.archive`: the plan's types check
//! and a strict script requiring both modules type checks (under the `luau-analysis`
//! feature), the augmentation puts the policy methods on `dream.archive.Archive`, and the
//! behaviour contracts ported from the mlua bindings hold.

#[cfg(feature = "luau-analysis")]
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use dream_archive::luau::{ARCHIVE_KEY, ArchiveExtension};
use dream_archivetool::luau::ArchivetoolExtension;
use dream_archivetool::{ArchiveFormat, ArchiveTool, CreateOptions};
use l3i::Runtime;
#[cfg(feature = "luau-analysis")]
use l3i::analysis::{Mode, ModuleConfig, SourceCode, SourceProvider};
use l3i::extension::{RuntimePlan, RuntimePolicy};

fn plan() -> Rc<RuntimePlan> {
    RuntimePlan::builder()
        .policy(
            RuntimePolicy::new()
                .compat_global("@dream/archive", "dreamArchive")
                .compat_global("@dream/archivetool", "dreamArchivetool"),
        )
        // Registered out of dependency order on purpose: the plan sorts them.
        .extension(ArchivetoolExtension)
        .extension(ArchiveExtension)
        .finalize()
        .unwrap()
}

fn runtime(globals: &[(&str, &Path)]) -> Runtime {
    let runtime = Runtime::from_plan(&plan()).unwrap();
    for (name, path) in globals {
        runtime
            .set_global(name, path.to_string_lossy().as_ref())
            .unwrap();
    }
    runtime
}

fn unique_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "dream_archivetool-luau-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
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

const FAILS_WITH: &str = r"
    local function fails_with(needle, f, ...)
        local ok, err = pcall(f, ...)
        assert(not ok, 'expected an error containing ' .. needle)
        assert(string.find(tostring(err), needle, 1, true), tostring(err))
    end
";

#[cfg(feature = "luau-analysis")]
struct Scripts(HashMap<&'static str, &'static str>);

#[cfg(feature = "luau-analysis")]
impl SourceProvider for Scripts {
    fn read_source(&self, name: &str) -> Option<SourceCode> {
        self.0.get(name).map(|text| SourceCode {
            text: (*text).to_owned(),
            is_script: true,
        })
    }
    fn resolve_module(&self, _requirer: &str, _required: &str) -> Option<String> {
        None
    }
    fn module_config(&self, _name: &str) -> ModuleConfig {
        ModuleConfig {
            mode: Mode::Strict,
            ..ModuleConfig::default()
        }
    }
}

#[cfg(feature = "luau-analysis")]
const STRICT_SCRIPT: &str = "--!strict
local dreamArchive = require('@dream/archive')
local tool = require('@dream/archivetool')
local archive = dreamArchive.openPath('a.bsa')
local other = dreamArchive.openPath('b.bsa')
local report = archive:verify({ readPayloads = true })
-- Every view declares its element type: `#`, `[i]`, and `for` type check without `:toTable()`.
local firstIssue: dream_archivetool_PathIssue? = report.duplicateNormalizedPaths[1]
local warnings: number = #report.warnings
for _, warning in report.warnings do
    local text: string = warning
    warnings += #text
end
for _, unsafe in report.unsafePaths do
    local path: string = unsafe.path
    warnings += #path
end
local diff = archive:diff(other, { fingerprintPayloads = true })
local change: dream_archivetool_DiffChange? = diff.changed[1]
for i, added in diff.added do
    local position: number = i
    local path: string = added.path
    warnings += position + #path
end
local plan = archive:planExtract(archive:entries(), { output = 'out', preservePaths = false })
local row: dream_archivetool_ExtractPlanRow? = plan.entries[1]
local planned: number = #plan.entries + #diff.removed
for _, planRow in plan.entries do
    local action: string = planRow.action
    planned += #action
end
local summary = archive:extractMany({ 'textures/a.dds', 'textures/b.dds' }, { overwrite = 'skip' })
local n: number = summary.extracted + summary.skipped + report.fileCount + diff.unchanged + warnings + planned
if firstIssue and change and row then
    local size: number? = change.old.size
    local fp: integer? = change.new.payloadFingerprint
    print(firstIssue.path, size, fp, row.action, row.target)
end
local info = tool.info('a.bsa')
local created = tool.create('c.bsa', 'input', { format = 'tes3' })
local addPlan = tool.planAdd('c.bsa', { inputs = { 'more' } })
local addRow: dream_archivetool_ArchivePlanRow? = addPlan.entries[1]
for _, member in addPlan.entries do
    local source: string? = member.source
    local memberPath: string = member.path
    n += #memberPath
end
local all = tool.planExtractAll('a.bsa', { output = 'out' })
print(n, info.fileCount, created.files, addPlan.added, addRow, all.operation, archive:toolInfo().rewritable)
print(plan:toTable(), diff:toTable(), report:toTable(), addPlan:toTable())
";

#[test]
fn the_composition_augments_the_archive_type() {
    let plan = plan();
    assert_eq!(
        plan.installation_order(),
        ["dream.archive", "dream.archivetool", "dream.net"]
    );
    let archive = plan.userdata_by_key(ARCHIVE_KEY).unwrap();
    assert_eq!(archive.owner, "dream.archive");
    let contributors: Vec<(&str, &str)> = archive
        .members
        .iter()
        .filter(|member| {
            ["verify", "diff", "extractMany", "planExtractAll"].contains(&member.name.as_str())
        })
        .map(|member| (member.name.as_str(), member.contributor))
        .collect();
    assert_eq!(
        contributors,
        [
            ("verify", "dream.archivetool"),
            ("diff", "dream.archivetool"),
            ("extractMany", "dream.archivetool"),
            ("planExtractAll", "dream.archivetool")
        ]
    );
    assert!(
        archive
            .members
            .iter()
            .any(|m| m.name == "readInto" && m.contributor == "dream.archive")
    );
    let tagged: Vec<&str> = plan
        .userdata()
        .iter()
        .filter(|u| u.tag.is_some() && u.owner == "dream.archivetool")
        .map(|u| u.key.as_str())
        .collect();
    assert_eq!(
        tagged,
        [
            "dream.archivetool.ArchivePlanRow",
            "dream.archivetool.DiffChange",
            "dream.archivetool.DiffEntry",
            "dream.archivetool.ExtractPlanRow",
            "dream.archivetool.PathIssue"
        ],
        "only the row handles take tags"
    );
    let definitions = plan.type_definitions();
    for fallback in ["(self, ...any): any", "(...any) -> ...any", ": any,\n"] {
        assert!(
            !definitions.contains(fallback),
            "{fallback:?} in:\n{definitions}"
        );
    }
}

#[cfg(feature = "luau-analysis")]
#[test]
fn the_definitions_and_the_strict_script_type_check() {
    let plan = plan();
    plan.check_definitions().unwrap();
    let definitions = plan.type_definitions();
    let options = l3i::analysis::AnalysisOptions {
        definitions: vec![l3i::analysis::Definitions {
            name: "dream.d.luau".to_owned(),
            source: definitions.clone(),
        }],
        ..Default::default()
    };
    let sources = plan.analysis_sources(Scripts(HashMap::from([("strict", STRICT_SCRIPT)])));
    let analysis = l3i::analysis::Analysis::new(sources, options).unwrap();
    let report = analysis.check("strict", false);
    let text: Vec<String> = report
        .diagnostics
        .iter()
        .map(|d| {
            format!(
                "strict:{}:{}: {}",
                d.span.begin_line + 1,
                d.span.begin_column + 1,
                d.text
            )
        })
        .collect();
    assert!(report.is_clean(), "{}\n---\n{definitions}", text.join("\n"));
}

#[test]
fn archive_methods_verify_diff_and_plan() {
    let dir = unique_dir("archive-methods");
    let old_archive = dir.join("old.bsa");
    let new_archive = dir.join("new.bsa");
    write_tes3_archive(&old_archive, b"old");
    write_tes3_archive(&new_archive, b"new");
    let runtime = runtime(&[("old_path", &old_archive), ("new_path", &new_archive)]);
    runtime
        .exec(
            r#"
            local old = dreamArchive.openPath(old_path)
            local new = dreamArchive.openPath(new_path)
            local verify = old:verify({ readPayloads = true })
            assert(verify.payloadsRead == 1 and verify.fileCount == 1 and verify.rewritable)
            assert(#verify.warnings == 0 and #verify.duplicateNormalizedPaths == 0 and #verify.unsafePaths == 0)
            assert(verify.format == "bsaTes3" and verify.path == old_path)
            local t = verify:toTable()
            assert(t.payloadsRead == 1 and #t.warnings == 0 and t.rewriteBlocker == nil and type(t.duplicateNormalizedPaths) == "table")

            local diff = old:diff(new, { fingerprintPayloads = true })
            assert(diff.comparison == "payloadFingerprint" and diff.fingerprintPayloads)
            assert(#diff.changed == 1 and #diff.added == 0 and #diff.removed == 0 and diff.unchanged == 0)
            local change = diff.changed[1]
            assert(change.path == "textures/example.dds" and #change.pathBytesHex == 40)
            assert(change.old.size == 3 and change.new.size == 3, "sizes are numbers")
            assert(type(change.old.payloadFingerprint) == "integer", "fingerprints are integers")
            assert(change.old.payloadFingerprint ~= change.new.payloadFingerprint)
            assert(change.old.compressedSize == nil)
            for i, c in diff.changed do assert(i == 1 and c.path == change.path) end
            local d = diff:toTable()
            assert(d.changed[1].old.size == 3 and type(d.changed[1].new.payloadFingerprint) == "integer")
            assert(d.old == old_path and d.new == new_path and #d.added == 0)
            assert(tostring(diff):find("dream.archivetool.DiffReport", 1, true))

            local metadata = old:diff(new)
            assert(metadata.comparison == "metadataOnly" and metadata.unchanged == 1)

            local entry = old:entries()[1]
            local plan = old:planExtract({ entry.path }, { preservePaths = false })
            assert(plan.operation == "extract" and #plan.entries == 1 and plan.archive == old_path)
            assert(plan.entries[1].action == "extract" and plan.entries[1].path == "textures/example.dds")
            assert(plan.entries[1].target:find("example.dds", 1, true))
            local byHandle = old:planExtract({ entry }, { preservePaths = false })
            assert(byHandle.entries[1].target == plan.entries[1].target, "entry handles name their paths")
            local byView = old:planExtract(old:entries(), { preservePaths = false })
            assert(#byView.entries == 1 and byView.entries[1].pathBytesHex == plan.entries[1].pathBytesHex)
            local p = plan:toTable()
            assert(p.operation == "extract" and p.entries[1].action == "extract" and p.entries[1].target == plan.entries[1].target)
            "#,
        )
        .unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn open_bytes_handles_use_their_snapshot_and_report_a_memory_label() {
    let dir = unique_dir("archive-methods-stale-path");
    let archive = dir.join("archive.bsa");
    let output = dir.join("out");
    write_tes3_archive(&archive, b"old");
    let runtime = runtime(&[("out_path", &output)]);
    let bytes = fs::read(&archive).unwrap();
    runtime.set_global("archive_bytes", &bytes).unwrap();
    runtime
        .exec("archive = dreamArchive.openBytes(archive_bytes)")
        .unwrap();
    write_tes3_archive(&archive, b"new");
    runtime
        .exec(
            r#"
            assert(archive:verify().path == "<memory>")
            assert(archive:toolInfo().path == "<memory>")
            local summary = archive:extract("textures/example.dds", { output = out_path, preservePaths = false })
            assert(summary.extracted == 1 and summary.skipped == 0)
            "#,
        )
        .unwrap();
    assert_eq!(fs::read(output.join("example.dds")).unwrap(), b"old");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_module_exposes_tool_policy_only() {
    let dir = unique_dir("info-boundary");
    let archive = create_test_archive(&dir);
    let runtime = runtime(&[("archive_path", &archive)]);
    runtime
        .exec(
            r#"
            local info = dreamArchivetool.info(archive_path)
            assert(info.format == "bsaTes3" and info.rewritable and info.fileCount == 1)
            assert(info.namedEntryCount == 1 and not info.hasUnnameableEntries)
            assert(info.tes4 == nil and info.ba2 == nil and info.rewriteBlocker == nil)
            assert(dreamArchivetool.list == nil and dreamArchivetool.readEntry == nil)
            assert(dreamArchivetool.extract_many == nil)
            assert(require("@dream/archivetool") == dreamArchivetool)
            assert(not pcall(function() dreamArchivetool.info = nil end), "frozen")
            "#,
        )
        .unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn extract_writes_entries_with_options_and_hex_paths() {
    let dir = unique_dir("extract");
    let archive = create_test_archive(&dir);
    let output = dir.join("output");
    let hex_output = dir.join("hex");
    let many_output = dir.join("many");
    let runtime = runtime(&[
        ("archive_path", &archive),
        ("output_path", &output),
        ("hex_output", &hex_output),
        ("many_output", &many_output),
    ]);
    runtime
        .exec(
            r#"
            local summary = dreamArchivetool.extract(archive_path, 'textures/example.dds', {
                output = output_path,
                preservePaths = false,
            })
            assert(summary.extracted == 1)
            local entry_hex = '74657874757265732f6578616d706c652e646473'
            assert(dreamArchivetool.extractHex(archive_path, entry_hex, { output = hex_output, preservePaths = false }).extracted == 1)
            assert(dreamArchivetool.extractByPathHex(archive_path, entry_hex, { output = hex_output, preservePaths = false, overwrite = 'skip' }).skipped == 1)
            local archive = dreamArchive.openPath(archive_path)
            local entry = archive:entries()[1]
            local plan = dreamArchivetool.planExtract(archive_path, { entry.path }, { output = many_output, preservePaths = false })
            assert(plan.operation == "extract" and plan.entries[1].action == "extract")
            local hexPlan = dreamArchivetool.planExtractByPathHex(archive_path, { entry_hex }, { output = many_output })
            assert(hexPlan.entries[1].pathBytesHex == entry_hex)
            local many = dreamArchivetool.extractMany(archive_path, { entry }, { output = many_output, preservePaths = false })
            assert(many.extracted == 1)
            local again = dreamArchivetool.extractManyByPathHex(archive_path, { entry_hex }, { output = many_output, preservePaths = false, overwrite = 'overwrite' })
            assert(again.extracted == 1)
            assert(archive:extractByPathHex(entry_hex, { output = many_output, preservePaths = false, overwrite = 'skip' }).skipped == 1)
            assert(archive:extractManyByPathHex({ entry_hex }, { output = many_output, preservePaths = false, overwrite = 'skip' }).skipped == 1)
            assert(archive:planExtractByPathHex({ entry_hex }, { output = many_output, preservePaths = false }).entries[1].action == "conflict")
            "#,
        )
        .unwrap();
    assert_eq!(fs::read(output.join("example.dds")).unwrap(), b"hello");
    assert_eq!(fs::read(hex_output.join("example.dds")).unwrap(), b"hello");
    assert_eq!(fs::read(many_output.join("example.dds")).unwrap(), b"hello");
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn extracts_non_utf8_archive_path_bytes() {
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
    let runtime = runtime(&[
        ("archive_path", &archive),
        ("bridge_output", &bridge_output),
        ("raw_output", &raw_output),
        ("hex_output", &hex_output),
    ]);
    runtime
        .set_global("entry_bytes", &b"bad-\xff.dds".to_vec())
        .unwrap();
    runtime
        .exec(
            r"
            local archive = dreamArchive.openPath(archive_path)
            local entry = archive:entries()[1]
            assert(archive:extract(entry, { output = bridge_output, preservePaths = false }).extracted == 1)
            assert(dreamArchivetool.extract(archive_path, entry_bytes, { output = raw_output, preservePaths = false }).extracted == 1)
            assert(dreamArchivetool.extractByPathHex(archive_path, '6261642dff2e646473', { output = hex_output, preservePaths = false }).extracted == 1)
            ",
        )
        .unwrap();
    let output_name = OsString::from_vec(b"bad-\xff.dds".to_vec());
    for output in [&bridge_output, &raw_output, &hex_output] {
        assert_eq!(fs::read(output.join(&output_name)).unwrap(), b"bytes");
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn extract_all_and_overwrite_modes() {
    let dir = unique_dir("extract-all");
    let archive = create_test_archive(&dir);
    let output = dir.join("output");
    fs::create_dir_all(output.join("textures")).unwrap();
    fs::write(output.join("textures/example.dds"), b"existing").unwrap();
    let runtime = runtime(&[("archive_path", &archive), ("output_path", &output)]);
    runtime
        .exec(
            r"
            local skipped = dreamArchivetool.extractAll(archive_path, { output = output_path, overwrite = 'skip' })
            assert(skipped.skipped == 1 and skipped.extracted == 0)
            local plan = dreamArchivetool.planExtractAll(archive_path, { output = output_path })
            assert(plan.operation == 'extractAll' and plan.entries[1].action == 'conflict')
            local archive = dreamArchive.openPath(archive_path)
            assert(archive:planExtractAll({ output = output_path, overwrite = 'skip' }).entries[1].action == 'skip')
            assert(not pcall(function() return archive:extractAll({ output = output_path }) end), 'fail on conflict')
            local written = archive:extractAll({ output = output_path, overwrite = 'overwrite' })
            assert(written.extracted == 1)
            ",
        )
        .unwrap();
    assert_eq!(
        fs::read(output.join("textures/example.dds")).unwrap(),
        b"hello"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn create_and_add_use_option_tables_and_plans() {
    let dir = unique_dir("create-add");
    let input = dir.join("input");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("base.txt"), b"base").unwrap();
    let archive = dir.join("out.bsa");
    let added = dir.join("added");
    fs::create_dir_all(&added).unwrap();
    fs::write(added.join("added.txt"), b"added").unwrap();
    let ba2 = dir.join("out.ba2");
    let runtime = runtime(&[
        ("input_path", &input),
        ("archive_path", &archive),
        ("added_path", &added),
        ("ba2_path", &ba2),
    ]);
    runtime
        .exec(
            r"
            local plan = dreamArchivetool.planCreate(archive_path, input_path, { format = 'bsaTes3' })
            assert(plan.operation == 'create' and plan.format == 'bsaTes3' and plan.files == 1)
            assert(plan.entries[1].action == 'add' and plan.entries[1].path == 'base.txt' and plan.entries[1].size == 4)
            assert(plan.entries[1].source ~= nil and #plan.entries[1].pathBytesHex == 16)
            local t = plan:toTable()
            assert(t.entries[1].size == 4 and t.files == 1 and t.output == archive_path)
            local created = dreamArchivetool.create(archive_path, input_path, { format = 'bsaTes3' })
            local addPlan = dreamArchivetool.planAdd(archive_path, { inputs = { added_path } })
            assert(addPlan.operation == 'add' and addPlan.added == 1 and addPlan.preserved == 1 and addPlan.files == 2)
            local actions = {}
            for _, row in addPlan.entries do table.insert(actions, row.action) end
            table.sort(actions)
            assert(actions[1] == 'add' and actions[2] == 'preserve')
            local a = addPlan:toTable()
            assert(#a.entries == 2 and a.replaced == 0 and a.format == 'bsaTes3')
            local updated = dreamArchivetool.add(archive_path, { inputs = { added_path } })
            assert(created.files + updated.files == 3)
            local ba2 = dreamArchivetool.create(ba2_path, input_path, { format = 'ba2', ba2Kind = 'gnrl', ba2Version = 'starfield' })
            assert(ba2.files == 1)
            ",
        )
        .unwrap();
    let entries = ArchiveTool::list(&archive).unwrap();
    assert!(entries.iter().any(|entry| entry.path == "base.txt"));
    assert!(entries.iter().any(|entry| entry.path == "added.txt"));
    let opened = dream_archive::ba2::Archive::open_path(&ba2).unwrap();
    assert_eq!(
        opened.info().version,
        dream_archive::ba2::ArchiveVersion::v2
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn reports_invalid_and_unknown_options() {
    let dir = unique_dir("invalid-options");
    let archive = create_test_archive(&dir);
    let runtime = runtime(&[("archive_path", &archive)]);
    runtime
        .exec(&format!(
            r#"
            {FAILS_WITH}
            fails_with('unknown overwrite mode', dreamArchivetool.extract, archive_path, 'textures/example.dds', {{ overwrite = 'explode' }})
            fails_with('unknown archive format', dreamArchivetool.create, 'out.bsa', 'input', {{ format = 'unknown' }})
            fails_with('ba2Kind is not valid', dreamArchivetool.create, 'out.bsa', 'input', {{ format = 'tes3', ba2Kind = 'gnrl' }})
            fails_with('ba2Kind is not valid with format bsaTes4', dreamArchivetool.create, 'out.bsa', 'input', {{ format = 'bsaTes4', ba2Kind = 'gnrl' }})
            fails_with('at least one input path', dreamArchivetool.add, archive_path, {{ inputs = {{}} }})
            fails_with("missing required option 'inputs'", dreamArchivetool.add, archive_path, {{ output = 'out.bsa' }})
            fails_with("unknown option 'overwirte'", dreamArchivetool.extract, archive_path, 'textures/example.dds', {{ overwirte = 'skip' }})
            fails_with('extractHex: unknown option', dreamArchivetool.extractHex, archive_path, '74657874757265732f6578616d706c652e646473', {{ overwirte = 'skip' }})
            fails_with('extractHex: invalid pathBytesHex', dreamArchivetool.extractHex, archive_path, 'not-hex')
            fails_with('extractByPathHex: invalid pathBytesHex', dreamArchivetool.extractByPathHex, archive_path, 'not-hex')
            fails_with('dense 1-based array', dreamArchivetool.add, archive_path, {{ output = 'out.bsa', inputs = {{ [2] = 'file.txt' }} }})
            fails_with('add.inputs', dreamArchivetool.add, archive_path, {{ output = 'out.bsa', inputs = 'file.txt' }})
            fails_with('add.output', dreamArchivetool.add, archive_path, {{ output = 12, inputs = {{ 'file.txt' }} }})
            fails_with('extractManyByPathHex.entries[1]: invalid pathBytesHex', dreamArchivetool.extractManyByPathHex, archive_path, {{ 'not-hex' }})
            fails_with('extract.output', dreamArchivetool.extract, archive_path, 'textures/example.dds', {{ output = 12 }})
            fails_with('extractAll.output', dreamArchivetool.extractAll, archive_path, {{ output = 12 }})
            fails_with("planExtractAll: unknown option 'overwirte'", dreamArchivetool.planExtractAll, archive_path, {{ overwirte = 'skip' }})
            fails_with("create: unknown option 'followSymlink'", dreamArchivetool.create, 'out.bsa', 'input', {{ format = 'bsaTes3', followSymlink = true }})
            fails_with("planCreate: unknown option 'followSymlink'", dreamArchivetool.planCreate, 'out.bsa', 'input', {{ format = 'bsaTes3', followSymlink = true }})
            fails_with('unknown BA2 version', dreamArchivetool.planCreate, 'out.ba2', 'input', {{ format = 'ba2', ba2Version = 'fallout-4-next-gen' }})
            fails_with('unknown archive format', dreamArchivetool.planCreate, 'out.bsa', 'input', {{ format = 'bsa-tes3' }})
            local archive = dreamArchive.openPath(archive_path)
            fails_with('archive:extractMany: entries must be', archive.extractMany, archive, 'textures/example.dds')
            fails_with('dense 1-based array', archive.planExtract, archive, {{ [2] = 'x' }})
            fails_with('archive:extract: expected an archive path or an entry handle', archive.extract, archive, 7)
            fails_with('not found', archive.extract, archive, 'missing.dds', {{ output = 'x' }})
            fails_with('not found', archive.planExtract, archive, {{ 'textures/example.dds', 'missing.dds' }})
            fails_with('not found', archive.extractMany, archive, {{ 'textures/example.dds', 'missing.dds' }}, {{ output = 'x' }})
            fails_with('not found', dreamArchivetool.planExtract, archive_path, {{ 'missing.dds' }})
            "#,
        ))
        .unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn errors_name_the_luau_options() {
    let dir = unique_dir("luau-option-names");
    let input = dir.join("input");
    write_input_tree(&input);
    #[cfg(unix)]
    std::os::unix::fs::symlink(input.join("textures/example.dds"), input.join("link.dds")).unwrap();
    let archive = dir.join("out.bsa");
    let runtime = runtime(&[("input_path", &input), ("archive_path", &archive)]);
    runtime
        .exec(&format!(
            r"
            {FAILS_WITH}
            fails_with('compress is not valid with format bsaTes3', dreamArchivetool.create, archive_path, input_path, {{ format = 'bsaTes3', compress = true }})
            fails_with('compress is not valid with format bsaTes3', dreamArchivetool.planCreate, archive_path, input_path, {{ compress = true }})
            local ok, err = pcall(dreamArchivetool.create, archive_path, input_path, {{ format = 'tes3' }})
            if not ok then
                assert(string.find(err, 'set followSymlinks to opt in', 1, true), err)
                assert(not string.find(err, 'follow_symlinks', 1, true), err)
            end
            ",
        ))
        .unwrap();
    #[cfg(unix)]
    assert!(!archive.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn hash_only_entries_are_refused_as_extraction_targets() {
    let dir = unique_dir("hash-only");
    let output = dir.join("out");
    let runtime = runtime(&[("out_path", &output)]);
    runtime
        .exec(&format!(
            r"
            {FAILS_WITH}
            local builder = dreamArchive.bsa.tes4.Builder.new()
            builder:setNameMode('hashOnly')
            builder:addBytes('textures/foo.dds', 'payload')
            local archive = dreamArchive.openBytes(builder:toBytes())
            local entry = archive:entries()[1]
            assert(entry.path == nil)
            fails_with('has no path', archive.extract, archive, entry, {{ output = out_path }})
            fails_with('has no path', archive.planExtract, archive, archive:entries(), {{ output = out_path }})
            fails_with('without recoverable paths', archive.planExtractAll, archive, {{ output = out_path }})
            local info = archive:toolInfo()
            assert(info.hasUnnameableEntries and info.tes4.nameMode == 'hashOnly' and not info.rewritable)
            assert(type(info.rewriteBlocker) == 'string')
            local verify = archive:verify()
            assert(verify.unnameableEntries == 1 and not verify.rewritable and verify.rewriteBlocker ~= nil)
            ",
        ))
        .unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn tes4_info_uses_luau_spellings() {
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
    let output = dir.join("output");
    let runtime = runtime(&[("archive_path", &archive), ("output_path", &output)]);
    runtime
        .exec(
            r#"
            local dreamArchive = require("@dream/archive")
            local dreamArchivetool = require("@dream/archivetool")
            local info = dreamArchivetool.info(archive_path)
            assert(info.format == "bsaTes4")
            assert(info.tes4.nameMode == "strings")
            assert(table.find(info.tes4.archiveFlags, "directoryStrings") ~= nil)
            assert(table.find(info.tes4.archiveFlags, "fileStrings") ~= nil)
            assert(type(info.tes4.archiveTypesBits) == "number")

            local archive = dreamArchive.openPath(archive_path)
            assert(archive:toolInfo().fileCount == 1)
            local plan = archive:planExtractAll({ output = output_path })
            assert(plan.operation == "extractAll")
            assert(#plan.entries[1].pathBytesHex > 0)
            assert(tostring(plan):find("dream.archivetool.ExtractPlan", 1, true))
            "#,
        )
        .unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sizes_are_numbers_that_match_dream_archive() {
    let runtime = runtime(&[]);
    runtime
        .exec(
            r"
            local builder = dreamArchive.bsa.tes3.Builder.new()
            builder:addBytes('a.txt', string.rep('x', 70000))
            local a = dreamArchive.openBytes(builder:toBytes())
            local other = dreamArchive.bsa.tes3.Builder.new()
            other:addBytes('a.txt', 'short')
            other:addBytes('b.txt', 'added')
            local b = dreamArchive.openBytes(other:toBytes())
            local diff = a:diff(b)
            assert(diff.changed[1].old.size == 70000 and diff.changed[1].new.size == 5)
            assert(diff.added[1].path == 'b.txt' and diff.added[1].size == 5 and diff.added[1].payloadFingerprint == nil)
            assert(#diff.removed == 0)
            local t = diff:toTable()
            assert(t.added[1].size == 5 and t.changed[1].old.size == 70000)

            -- Sizes are plain numbers: arithmetic and ordering apply, and dream.archive's own
            -- Entry.size, also a number, equals the report's size for the same file.
            local size = diff.added[1].size
            assert(typeof(size) == 'number' and size < 6 and size + 1 == 6)
            local entry
            for _, candidate in b:entries() do
                if candidate.path == 'b.txt' then entry = candidate end
            end
            assert(typeof(entry.size) == 'number' and entry.size == size)
            ",
        )
        .unwrap();
}
