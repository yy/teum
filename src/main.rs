use clap::{Parser, Subcommand};

use teum::commands;
use teum::config::Config;

#[derive(Parser)]
#[command(name = "teum", about = "Minimal time tracker", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start tracking time
    Start {
        /// Use a preset from config
        #[arg(short, long)]
        preset: Option<String>,
        /// Start time override (HH:MM)
        #[arg(short = 'a', long = "at")]
        at: Option<String>,
        /// @project #tags description
        args: Vec<String>,
    },
    /// Stop the running interval
    Stop {
        /// Stop time override (HH:MM)
        #[arg(short = 'a', long = "at")]
        at: Option<String>,
        /// Energy level (1-5)
        #[arg(short, long)]
        energy: Option<u8>,
    },
    /// Show current tracking status
    Status {
        /// Emit machine-readable JSON (for scripts and the dial timer)
        #[arg(long)]
        json: bool,
    },
    /// Resume the last completed interval
    Resume {
        /// Start time override (HH:MM)
        #[arg(short = 'a', long = "at")]
        at: Option<String>,
    },
    /// Cancel the running interval
    Cancel,
    /// Show time entries
    Log {
        /// Period: today, yesterday, week, last-week, month
        #[arg(default_value = "today")]
        period: String,
    },
    /// Show time summary by project
    Summary {
        /// Period: today, week, last-week, month
        #[arg(default_value = "week")]
        period: String,
        /// Report group from config
        #[arg(short, long)]
        group: Option<String>,
    },
    /// Weekly analysis: category buckets, table, and an HTML chart report
    Report {
        /// Period: all (default), week, last-week, month, year
        #[arg(default_value = "all")]
        period: String,
        /// Also write a self-contained HTML report. Optional path; defaults
        /// to ~/.config/teum/report.html when given with no value.
        #[arg(long, num_args = 0..=1)]
        html: Option<Option<String>>,
        /// Open the HTML report in the browser (implies --html)
        #[arg(long)]
        open: bool,
    },
    /// Open data file in $EDITOR
    Edit {
        /// Which week: current (default) or YYYY-wWW
        #[arg(default_value = "current")]
        target: String,
    },
    /// Fill the gap since the last entry ended (start = last end, end = now)
    Fill {
        /// Use a preset from config
        #[arg(short, long)]
        preset: Option<String>,
        /// Keep the new entry running
        #[arg(long = "continue")]
        cont: bool,
        /// Description and optional @project #tags
        args: Vec<String>,
    },
    /// Inject a past interval, trimming the previous entry
    Inject {
        /// Duration (e.g., 30m, 1h, 1h30m)
        duration: String,
        /// Use a preset from config
        #[arg(short, long)]
        preset: Option<String>,
        /// Keep the new entry running
        #[arg(long = "continue")]
        cont: bool,
        /// Description and optional @project #tags
        args: Vec<String>,
    },
    /// Add a past interval manually
    Add {
        /// Full interval line
        line: Vec<String>,
    },
    /// Git sync: add, commit, pull, push
    Sync,
    /// Initialize data directory and config
    Init,
}

fn main() {
    let cli = Cli::parse();
    let config = Config::load();

    let result = match cli.command {
        Command::Start { preset, at, args } => {
            commands::start(&config, preset.as_deref(), at.as_deref(), &args)
        }
        Command::Stop { at, energy } => commands::stop(&config, at.as_deref(), energy),
        Command::Status { json } => commands::status(&config, json),
        Command::Resume { at } => commands::resume(&config, at.as_deref()),
        Command::Cancel => commands::cancel(&config),
        Command::Log { period } => commands::log(&config, &period),
        Command::Summary { period, group } => commands::summary(&config, &period, group.as_deref()),
        Command::Report { period, html, open } => commands::report(&config, &period, html, open),
        Command::Edit { target } => commands::edit(&config, &target),
        Command::Fill { preset, cont, args } => {
            commands::fill(&config, preset.as_deref(), cont, &args)
        }
        Command::Inject {
            duration,
            preset,
            cont,
            args,
        } => commands::inject(&config, preset.as_deref(), &duration, cont, &args),
        Command::Add { line } => commands::add(&config, &line.join(" ")),
        Command::Sync => commands::sync(&config),
        Command::Init => commands::init(&config),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
