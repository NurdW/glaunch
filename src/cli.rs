use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "glaunch",
    about = "Steam game launch wrapper with gamescope, gamemode, HDR, and per-game profiles"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Launch a game (drop-in replacement for Steam launch options)
    Run(RunArgs),
    /// Manage per-game profiles
    Profile(ProfileArgs),
    /// Manage launch history
    History(HistoryArgs),
    /// Interactive profile editor
    Tui,
    /// Show detected hardware info
    Info(InfoArgs),
}

#[derive(clap::Args)]
pub struct RunArgs {
    /// Enable HDR inverse tone mapping (for SDR games)
    #[arg(long)]
    pub itm: bool,

    /// Enable FSR4 RDNA3 upgrade
    #[arg(long)]
    pub fsr4: bool,

    /// Disable HDR entirely
    #[arg(long)]
    pub no_hdr: bool,

    /// Disable adaptive sync (VRR)
    #[arg(long)]
    pub no_vrr: bool,

    /// Skip gamescope entirely (just gamemoderun + env)
    #[arg(long)]
    pub no_gamescope: bool,

    /// Pin to V-Cache CCD (auto-detects X3D topology)
    #[arg(long)]
    pub vcache: bool,

    /// Disable V-Cache CCD pinning
    #[arg(long)]
    pub no_vcache: bool,

    /// Fix Steam mouse stutter (LD_PRELOAD gamemode)
    #[arg(long)]
    pub fix_mouse: bool,

    /// Enable vkBasalt post-processing with optional profile name
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    pub vkbasalt: Option<String>,

    /// Override width (default: 3840)
    #[arg(short, long)]
    pub width: Option<u32>,

    /// Override height (default: 2160)
    #[arg(short = 'H', long)]
    pub height: Option<u32>,

    /// Override refresh rate (default: 240)
    #[arg(short, long)]
    pub refresh_rate: Option<u32>,

    /// Force gamescope to a specific monitor by connector name (e.g. DP-1)
    #[arg(short = 'O', long)]
    pub output: Option<String>,

    /// Enable MangoHud with optional config preset name
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    pub mangohud: Option<String>,

    /// Load a saved profile by name or Steam app ID
    #[arg(long)]
    pub profile: Option<String>,

    /// Print the final command instead of executing it
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Log each decision to stderr
    #[arg(short, long)]
    pub verbose: bool,

    /// The game command (everything after --)
    #[arg(last = true, required = true)]
    pub command: Vec<String>,
}

#[derive(clap::Args)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub command: ProfileCommand,
}

#[derive(Subcommand)]
pub enum ProfileCommand {
    /// List all profiles
    List,
    /// Show a profile's settings
    Show { name: String },
    /// Create a new profile
    Create { name: String },
    /// Open a profile in $EDITOR
    Edit { name: String },
    /// Delete a profile
    Delete { name: String },
}

#[derive(clap::Args)]
pub struct InfoArgs {
    /// Force re-detection (bypass cache)
    #[arg(long)]
    pub refresh: bool,
}

#[derive(clap::Args)]
pub struct HistoryArgs {
    #[command(subcommand)]
    pub command: HistoryCommand,
}

#[derive(Subcommand)]
pub enum HistoryCommand {
    /// List all apps with saved launch history
    List,
    /// Show launch history for an app
    Show { slug: String },
    /// Create a profile from a saved launch
    Promote {
        slug: String,
        /// Which launch to promote (1=newest, default: 1)
        #[arg(long, default_value = "1")]
        launch: usize,
        /// Override the profile name (defaults to app slug)
        #[arg(long)]
        name: Option<String>,
    },
    /// Clear launch history
    Clear {
        /// Clear history for a specific app (omit to clear all)
        slug: Option<String>,
    },
}
