+++
title = "ArchivetoolExtension"
description = "The Rust side of the Luau API: the l3i extension a host composes, its constants, and the userdata types behind reports, plans and rows."
weight = 50

[extra]
kind = "api"
+++

Rust, in the `luau` module, behind the `luau` feature. [Embedding Luau](@/docs/luau-hosts.md)
shows it in a host.

## ArchivetoolExtension

{{ api_signature(value="struct ArchivetoolExtension") }}

The `dream.archivetool` extension: an `l3i::extension::Extension` that describes the
`@dream/archivetool` module, the report, plan and row types, and the members it adds to
`dream.archive.Archive`. It holds no state; `Clone`, `Copy`, `Debug`, `Default`. Add it to a
plan with `RuntimePlan::builder().extension(ArchivetoolExtension)`, beside dream_archive's
`ArchiveExtension`.

It declares that it requires `dream.archive`: a plan without dream_archive's extension fails to
finalize, and the plan installs `dream.archive` first whichever order the host adds them in. The
archive methods are an augmentation of the type `dream.archive` owns, merged into its metatable
when the plan is built, so there is no registration order to get right. A member name both
extensions declared would fail the plan; none does.

It never creates a runtime, installs no global, and checks no capabilities. The row types take
l3i userdata tags, from the budget the plan shares with dream_archive; the reports, plans and
views do not.

## Constants

{{ api_signature(value='const EXTENSION_ID: &str = "dream.archivetool"') }}

The extension's id in the plan.

{{ api_signature(value='const MODULE: &str = "@dream/archivetool"') }}

The path scripts `require`. A host that wants the conventional global adds
`RuntimePolicy::new().compat_global(MODULE, "dreamArchivetool")`.

## Reports and plans

Each wraps one `Rc` of the Rust result; the Luau fields read from it. `Clone`.

{{ api_signature(value="struct VerifyReport") }}

`dream.archivetool.VerifyReport`, over a [`crate::VerifyReport`](@/docs/api/inspecting.md#verifyreport).

{{ api_signature(value="fn report(&self) -> &crate::VerifyReport") }}

{{ api_signature(value="struct DiffReport") }}

`dream.archivetool.DiffReport`, over a [`crate::DiffReport`](@/docs/api/inspecting.md#diffreport).

{{ api_signature(value="fn report(&self) -> &crate::DiffReport") }}

{{ api_signature(value="struct ExtractPlan") }}

`dream.archivetool.ExtractPlan`, over an [`ExtractAllPlan`](@/docs/api/extracting.md#extractallplan).

{{ api_signature(value="fn plan(&self) -> &crate::ExtractAllPlan") }}

{{ api_signature(value="struct CreatePlan") }}

`dream.archivetool.CreatePlan`, over a [`crate::CreatePlan`](@/docs/api/creating.md#createplan).

{{ api_signature(value="fn plan(&self) -> &crate::CreatePlan") }}

{{ api_signature(value="struct AddPlan") }}

`dream.archivetool.AddPlan`, over a [`crate::AddPlan`](@/docs/api/creating.md#addplan).

{{ api_signature(value="fn plan(&self) -> &crate::AddPlan") }}

A host function that receives one of these from a script reads the Rust result through
`report()` or `plan()`, without converting anything.

## Views and rows

The views are `l3i::sequence::SequenceSource` implementations, pushed to Luau as sequences; the
rows are userdata holding the result's `Rc` and an index. Their fields are private: they exist
to be handed to Luau, which reads them as the [reports and plans](@/docs/luau/results.md) page
describes.

| View | Luau name | Rows |
|---|---|---|
| `PathIssues` | `dream.archivetool.PathIssues` | `PathIssue`, `dream.archivetool.PathIssue` |
| `Warnings` | `dream.archivetool.Warnings` | `String` |
| `DiffEntries` | `dream.archivetool.DiffEntries` | `DiffEntry`, `dream.archivetool.DiffEntry` |
| `DiffChanges` | `dream.archivetool.DiffChanges` | `DiffChange`, `dream.archivetool.DiffChange` |
| `ExtractPlanRows` | `dream.archivetool.ExtractPlanRows` | `ExtractPlanRow`, `dream.archivetool.ExtractPlanRow` |
| `ArchivePlanRows` | `dream.archivetool.ArchivePlanRows` | `ArchivePlanRow`, `dream.archivetool.ArchivePlanRow` |

{{ api_signature(value="struct DiffState") }}

`dream.archivetool.DiffState`: one side of a `DiffChange`, its `old` or `new`. `Clone`.
