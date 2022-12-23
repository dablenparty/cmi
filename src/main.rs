#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::uninlined_format_args)]

use std::{env, path::PathBuf};

use clap::Parser;
use curse::CurseModpack;
use log::info;

mod curse;
mod error;
mod util;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct CommandLineArgs {
    /// The zip file to install from
    #[clap(required = true)]
    modpack_zip: PathBuf,
    /// The target directory to install to
    #[arg(required = true)]
    target: PathBuf,
    /// The log level to use
    /// Valid values are: error, warn, info, debug, trace
    #[clap(short, long, value_parser, default_value_t = log::LevelFilter::Info)]
    log_level: log::LevelFilter,
}

fn setup_logging(log_level: log::LevelFilter) -> crate::error::Result<()> {
    let current_exe = env::current_exe()?;
    let log_folder = current_exe.with_file_name("cmi-logs");
    let package_name = env!("CARGO_PKG_NAME");
    let latest_log_file = dablenutil::logging::rotate_logs(&log_folder, Some(package_name))?;
    dablenutil::logging::init_simple_logger(&latest_log_file, log_level)?;
    Ok(())
}

#[tokio::main]
async fn main() -> crate::error::Result<()> {
    dotenv::dotenv().unwrap_or_else(|e| {
        panic!("Failed to load .env file: {}", e);
    });

    let args = CommandLineArgs::parse();

    setup_logging(args.log_level)?;

    let mut modpack = CurseModpack::load(&args.modpack_zip).await?;
    info!("Loaded modpack: {}", modpack);
    modpack.install_to(&args.target).await?;

    info!("Done!");

    Ok(())
}
