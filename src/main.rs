#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::uninlined_format_args)]

use std::{env, path::PathBuf};

use clap::Parser;
use curse::CurseModpack;
use log::info;

mod curse;
mod error;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct CommandLineArgs {
    /// The zip file to install from
    #[clap(required = true)]
    modpack_zip: PathBuf,
    /// The target directory to install to
    #[arg(required = true)]
    target: PathBuf,
    /// The log level to use: error, warn, info, debug, or trace
    #[clap(short, long, value_parser, default_value_t = log::LevelFilter::Info)]
    log_level: log::LevelFilter,
    /// Use the PolyMC API key instead of the Curse API key.
    /// Note that by using this, you are technically violating Curse's ToS.
    /// This will override the CURSE_API_KEY environment variable, so it is
    /// only required once.
    #[clap(long, default_value_t = false)]
    use_poly_api_key: bool,
}

fn setup_logging(log_level: log::LevelFilter) -> crate::error::Result<()> {
    let current_exe = env::current_exe()?;
    let log_folder = current_exe.with_file_name("cmi-logs");
    let logging_config = dablenutil::logging::LoggingConfig::new(log_folder)
        .file_level_filter(log_level)
        .term_level_filter(log_level);
    dablenutil::logging::rotate_logs(&logging_config)?;
    dablenutil::logging::init_simple_logger(&logging_config)?;
    Ok(())
}

async fn get_poly_key() -> crate::error::Result<String> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://cf.polymc.org/api")
        .send()
        .await?
        .error_for_status()?;
    let json: serde_json::Value = response.json().await?;
    json.get("token")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            crate::error::Error::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Failed to get PolyMC API key",
            ))
        })
}

async fn set_poly_key_dotenv() -> crate::error::Result<()> {
    let token = get_poly_key().await?;
    let token_string = format!("CURSE_API_KEY='{}'\n", token);
    let path = PathBuf::from(".env");
    let contents = if path.exists() {
        // removes the CURSE_API_KEY line if it exists, then appends the new one
        let contents = tokio::fs::read_to_string(".env").await?;
        let mut new_contents = contents
            .lines()
            .filter(|line| !line.starts_with("CURSE_API_KEY"))
            .fold(String::with_capacity(contents.len()), |mut acc, line| {
                acc.push_str(line);
                acc.push('\n');
                acc
            });
        new_contents.push_str(&token_string);
        new_contents
    } else {
        token_string
    };
    tokio::fs::write(path, contents).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> crate::error::Result<()> {
    let args = CommandLineArgs::parse();

    if args.use_poly_api_key {
        set_poly_key_dotenv().await?;
    }

    dotenv::dotenv().unwrap_or_else(|e| {
        panic!("Failed to load .env file: {}", e);
    });

    let curse_api_key = std::env::var_os("CURSE_API_KEY");
    assert!(curse_api_key.is_some(), "CURSE_API_KEY not set.\nIf you'd like to fetch the key used by PolyMC, rerun this program with the --use-poly-api-key flag.\nOtherwise, set the CURSE_API_KEY environment variable to your Curse API key (.env files are supported).");

    setup_logging(args.log_level)?;

    let mut modpack = CurseModpack::load(&args.modpack_zip)?;
    info!("Loaded modpack: {}", modpack);
    modpack.install_to(&args.target).await?;

    info!("Done!");

    Ok(())
}
