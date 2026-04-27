use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use dream_archivetool::{ArchiveFormat, Ba2ArchiveKind, Ba2Version, Tes4Version};

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Inspect and manipulate Bethesda BSA and BA2 archives"
)]
pub(super) struct Cli {
    /// Generate shell completion script to stdout
    #[arg(long, value_name = "SHELL", conflicts_with = "generate_manpage")]
    pub(super) generate_completion: Option<Shell>,
    /// Generate roff manpage to stdout
    #[arg(long, conflicts_with = "generate_completion")]
    pub(super) generate_manpage: bool,
    #[command(subcommand)]
    pub(super) command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub(super) enum Command {
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
        format: CliArchiveFormat,
        /// TES4 BSA version; only valid with --format tes4
        #[arg(long, value_enum)]
        tes4_version: Option<CliTes4Version>,
        /// BA2 archive kind; only valid with --format ba2. GNMF update/create is rejected.
        #[arg(long, value_enum)]
        ba2_kind: Option<CliBa2ArchiveKind>,
        /// BA2 version; only valid with --format ba2
        #[arg(long, value_enum)]
        ba2_version: Option<CliBa2Version>,
        /// Write JSON summary to stdout
        #[arg(long)]
        json: bool,
        /// Print archive creation plan without writing files
        #[arg(long)]
        dry_run: bool,
        /// Sync file contents and parent directory after writing the archive
        #[arg(long)]
        fsync: bool,
        /// Follow symbolic links while collecting input files
        #[arg(long)]
        follow_symlinks: bool,
    },
    /// Add or update entries by rewriting the archive
    Add {
        /// Input archive path
        archive: PathBuf,
        /// Files or directories to add. Directory contents are stored relative to each root.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Output archive path. Omit to replace the input archive after a successful rewrite.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Write JSON summary to stdout
        #[arg(long)]
        json: bool,
        /// Print archive update plan without writing files
        #[arg(long)]
        dry_run: bool,
        /// Sync file contents and parent directory after writing the archive
        #[arg(long)]
        fsync: bool,
        /// Follow symbolic links while collecting input files
        #[arg(long)]
        follow_symlinks: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(super) enum CliArchiveFormat {
    Tes3,
    Tes4,
    Ba2,
}

impl From<CliArchiveFormat> for ArchiveFormat {
    fn from(format: CliArchiveFormat) -> Self {
        match format {
            CliArchiveFormat::Tes3 => Self::Tes3,
            CliArchiveFormat::Tes4 => Self::Tes4,
            CliArchiveFormat::Ba2 => Self::Ba2,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(super) enum CliTes4Version {
    Oblivion,
    Fallout3,
    Skyrim,
    SkyrimSe,
}

impl From<CliTes4Version> for Tes4Version {
    fn from(version: CliTes4Version) -> Self {
        match version {
            CliTes4Version::Oblivion => Self::Oblivion,
            CliTes4Version::Fallout3 => Self::Fallout3,
            CliTes4Version::Skyrim => Self::Skyrim,
            CliTes4Version::SkyrimSe => Self::SkyrimSe,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(super) enum CliBa2ArchiveKind {
    Gnrl,
    Dx10,
    Gnmf,
}

impl From<CliBa2ArchiveKind> for Ba2ArchiveKind {
    fn from(kind: CliBa2ArchiveKind) -> Self {
        match kind {
            CliBa2ArchiveKind::Gnrl => Self::Gnrl,
            CliBa2ArchiveKind::Dx10 => Self::Dx10,
            CliBa2ArchiveKind::Gnmf => Self::Gnmf,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(super) enum CliBa2Version {
    Fallout4,
    Starfield,
    Fallout4NextGen,
}

impl From<CliBa2Version> for Ba2Version {
    fn from(version: CliBa2Version) -> Self {
        match version {
            CliBa2Version::Fallout4 => Self::Fallout4,
            CliBa2Version::Starfield => Self::Starfield,
            CliBa2Version::Fallout4NextGen => Self::Fallout4NextGen,
        }
    }
}
