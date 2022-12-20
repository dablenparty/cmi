#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::uninlined_format_args)]

use std::path::PathBuf;

use clap::Parser;
use curse::CurseModpack;

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
}

#[tokio::main]
async fn main() -> crate::error::Result<()> {
    dotenv::dotenv().unwrap_or_else(|e| {
        panic!("Failed to load .env file: {}", e);
    });

    let args = CommandLineArgs::parse();

    let mut modpack = CurseModpack::load(&args.modpack_zip).await?;
    modpack.install_to(&args.target).await?;

    Ok(())
}
