#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use std::path::Path;

use curse::CurseModpack;

mod curse;
mod error;
mod util;

#[tokio::main]
async fn main() {
    dotenv::dotenv().expect("Failed to load .env");
    // TODO: clap
    let target = std::env::args().nth(1).expect("target not specified");
    let zip = std::env::args().nth(2).expect("zip not specified");

    let target = Path::new(&target);
    let zip = Path::new(&zip);

    let mut modpack = CurseModpack::load(zip).await.unwrap();
    modpack.install_to(target).await.unwrap();
}
