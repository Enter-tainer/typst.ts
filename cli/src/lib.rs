use std::path::PathBuf;

pub mod compile;
pub mod export;
pub mod font;
pub mod query;
pub mod query_repl;
pub mod tracing;
pub mod utils;
pub mod version;

use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use typst_ts_core::build_info::VERSION;
use version::VersionFormat;

#[derive(Debug, Parser)]
#[clap(name = "typst-ts-cli", version = VERSION)]
pub struct Opts {
    /// Print Version
    #[arg(short = 'V', long, group = "version-dump")]
    pub version: bool,

    /// Print Version in format
    #[arg(long = "VV", alias = "version-fmt", group = "version-dump", default_value_t = VersionFormat::None)]
    pub vv: VersionFormat,

    #[clap(subcommand)]
    pub sub: Option<Subcommands>,
}

#[derive(Debug, Subcommand)]
#[clap(
    about = "The cli for typst.ts.",
    after_help = "",
    next_display_order = None
)]
#[allow(clippy::large_enum_variant)]
pub enum Subcommands {
    #[clap(visible_alias = "c", about = "Run compiler.")]
    Compile(CompileArgs),

    /// Processes an input file to extract provided metadata
    Query(QueryArgs),

    QueryRepl(QueryReplArgs),

    #[clap(about = "Generate shell completion script.")]
    Completion(CompletionArgs),

    #[clap(about = "Dump Client Environment.")]
    Env(EnvArgs),

    #[clap(subcommand)]
    Font(FontSubCommands),

    #[clap(subcommand)]
    Package(PackageSubCommands),
}

#[derive(Debug, Subcommand)]
#[clap(
    about = "Commands about font for typst.",
    after_help = "",
    next_display_order = None
)]
#[allow(clippy::large_enum_variant)]
pub enum FontSubCommands {
    /// List all discovered fonts in system and custom font paths
    List(ListFontsArgs),
    /// Measure fonts and generate a profile file for compiler
    Measure(MeasureFontsArgs),
}

#[derive(Debug, Subcommand)]
#[clap(
    about = "Commands about package for typst.",
    after_help = "",
    next_display_order = None
)]
#[allow(clippy::large_enum_variant)]
pub enum PackageSubCommands {
    /// List all discovered packages in data and cache paths
    List(ListPackagesArgs),
    /// Link a package to local data path
    Link(LinkPackagesArgs),
    /// Unlink a package from local data path
    Unlink(LinkPackagesArgs),
    /// Generate documentation for a package
    Doc(GenPackagesDocArgs),
}

/// Shared arguments for font related commands
#[derive(Default, Debug, Clone, Parser)]
pub struct FontArgs {
    /// Add additional directories to search for fonts
    #[clap(
        long = "font-path",
        env = "TYPST_FONT_PATHS", 
        value_name = "DIR",
        action = ArgAction::Append,
    )]
    pub paths: Vec<PathBuf>,
}

#[derive(Default, Debug, Clone, Parser)]
#[clap(next_help_heading = "Compile options")]
pub struct CompileOnceArgs {
    /// Shared arguments for font related commands.
    #[clap(flatten)]
    pub font: FontArgs,

    /// Path to typst workspace.
    #[clap(long, short, default_value = ".")]
    pub workspace: String,

    /// Entry file.
    #[clap(long, short, required = true)]
    pub entry: String,

    /// Output to directory, default in the same directory as the entry file.
    #[clap(long, short, default_value = "")]
    pub output: String,
}

#[derive(Default, Debug, Clone, Parser)]
#[clap(next_help_heading = "Compile options")]
pub struct CompileArgs {
    /// compile arguments before query.
    #[clap(flatten)]
    pub compile: CompileOnceArgs,

    /// Watch mode.
    #[clap(long)]
    pub watch: bool,

    /// Generate dynamic layout representation.
    /// Note: this is an experimental feature and will be merged as
    ///   format `dyn-svg` in the future.
    #[clap(long)]
    pub dynamic_layout: bool,

    /// Output formats, possible values: `ast`, `pdf`, `svg`, and, `svg_html`.
    #[clap(long)]
    pub format: Vec<String>,

    /// Enable tracing.
    /// Possible usage: --trace=verbosity={0..3}
    ///   where verbosity: {0..3} -> {warning, info, debug, trace}
    #[clap(long)]
    pub trace: Option<String>,
}

/// Processes an input file to extract provided metadata
///
/// Examples:
/// ```shell
/// # query elements with selector "heading"
/// query --selector "heading"
/// # query elements with selector "heading" which is of level 1
/// query --selector "heading.where(level: 1)"
/// # query first element with selector "heading" which is of level 1
/// query --selector "heading.where(level: 1)" --one
/// ```
#[derive(Debug, Clone, Parser)]
pub struct QueryArgs {
    /// compile arguments before query.
    #[clap(flatten)]
    pub compile: CompileArgs,

    /// Define what elements to retrieve
    #[clap(long = "selector")]
    pub selector: String,

    /// Extract just one field from all retrieved elements
    #[clap(long = "field")]
    pub field: Option<String>,

    /// Expect and retrieve exactly one element
    #[clap(long = "one", default_value = "false")]
    pub one: bool,
}

/// TODO: Repl Doc
#[derive(Debug, Clone, Parser)]
pub struct QueryReplArgs {
    /// compile arguments before query.
    #[clap(flatten)]
    pub compile: CompileOnceArgs,
}

/// List all discovered fonts in system and custom font paths
#[derive(Debug, Clone, Parser)]
pub struct ListFontsArgs {
    /// Shared arguments for font related commands.
    #[clap(flatten)]
    pub font: FontArgs,

    /// Also list style variants of each font family
    #[arg(long)]
    pub variants: bool,
}

/// Measure fonts and generate a profile file for compiler
#[derive(Debug, Clone, Parser)]
pub struct MeasureFontsArgs {
    /// Shared arguments for font related commands.
    #[clap(flatten)]
    pub font: FontArgs,

    /// Path to output profile file
    #[arg(long, required = true)]
    pub output: PathBuf,

    /// Exclude system font paths
    #[arg(long)]
    pub no_system_fonts: bool,
}

#[derive(ValueEnum, Debug, Clone)]
pub enum EnvKey {
    Features,
}

/// Generate shell completion script.
#[derive(Debug, Clone, Parser)]
pub struct CompletionArgs {
    /// Completion script kind.
    #[clap(value_enum)]
    pub shell: clap_complete::Shell,
}

/// Dump Client Environment.
#[derive(Debug, Clone, Parser)]
pub struct EnvArgs {
    /// The key of environment kind.
    #[clap(value_name = "KEY")]
    pub key: EnvKey,
}

#[derive(Debug, Clone, Parser)]
pub struct ListPackagesArgs {
    /// Also list other information of each package
    #[arg(short)]
    pub long: bool,
}

#[derive(Debug, Clone, Parser)]
pub struct LinkPackagesArgs {
    /// Path to package manifest file
    #[arg(long)]
    pub manifest: String,
}

#[derive(Debug, Clone, Parser)]
pub struct GenPackagesDocArgs {
    /// Path to package manifest file
    #[arg(long)]
    pub manifest: String,

    /// Path to output directory
    #[arg(long, short, default_value = "")]
    pub output: String,

    /// Generate dynamic layout representation.
    /// Note: this is an experimental feature and will be merged as
    ///   format `dyn-svg` in the future.
    #[clap(long)]
    pub dynamic_layout: bool,
}
