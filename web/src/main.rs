use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{Parser, Subcommand};
use config::Config;
use progscrape_application::{
    MemIndex, PersistLocation, Storage, StorageWriter, StoryEvaluator, StoryIndex,
};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::EnvFilter;
use web::WebError;

use crate::auth::Auth;
use crate::index::Index;

mod auth;
mod config;
mod cron;
mod filters;
mod index;
mod resource;
mod serve_static_files;
mod static_files;
mod web;

pub enum Engine {}

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        value_name = "LOG",
        help = "Logging filter (overrides SERVER_LOG environment variable)"
    )]
    log: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Serve {
        #[arg(long, value_name = "DIR", value_hint = clap::ValueHint::DirPath, help = "Persistence path")]
        persist_path: Option<PathBuf>,

        #[arg(long, value_name = "DIR", value_hint = clap::ValueHint::DirPath, help = "Root path")]
        root: Option<PathBuf>,

        #[arg(
            long,
            value_name = "HEADER",
            help = "Header to extract authorization from"
        )]
        auth_header: Option<String>,

        #[arg(
            long,
            value_name = "HEADER",
            help = "Fixed authorization value for testing purposes"
        )]
        fixed_auth_value: Option<String>,
    },
    Initialize {
        #[arg(long, value_name = "DIR", value_hint = clap::ValueHint::DirPath, help = "Persistence path")]
        persist_path: PathBuf,

        #[arg(long, value_name = "DIR", value_hint = clap::ValueHint::DirPath, help = "Root path")]
        root: Option<PathBuf>,
    },
}

/// Our entry point.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    go().await?;
    Ok(())
}

async fn go() -> Result<(), WebError> {
    let args = Args::parse();

    // We ask for more detailed tracing in debug mode
    let default_directive = if cfg!(debug_assertions) {
        LevelFilter::DEBUG.into()
    } else {
        LevelFilter::INFO.into()
    };

    // Initialize logging using either the environment variable or --log option
    let env_filter = if let Some(log) = args.log {
        EnvFilter::builder()
            .with_default_directive(default_directive)
            .parse(log)?
    } else {
        EnvFilter::builder()
            .with_default_directive(default_directive)
            .with_env_var("SERVER_LOG")
            .from_env()?
    };

    tracing_subscriber::fmt().with_env_filter(env_filter).init();
    tracing::info!("Logging initialized");

    match args.command {
        Command::Serve {
            root,
            persist_path,
            auth_header,
            fixed_auth_value,
        } => {
            let persist_path = persist_path
                .unwrap_or("target/index".into())
                .canonicalize()?;
            let index = Index::initialize_with_persistence(persist_path)?;
            let root_path = root.unwrap_or(".".into()).canonicalize()?;

            let auth = match (auth_header, fixed_auth_value) {
                (Some(auth_header), None) => Auth::FromHeader(auth_header),
                (None, Some(fixed_auth_value)) => Auth::Fixed(fixed_auth_value),
                (None, None) => Auth::None,
                _ => {
                    return Err(WebError::ArgumentsInvalid(
                        "Invalid auth header parameter".into(),
                    ));
                }
            };
            web::start_server(&root_path, index, auth).await?;
        }
        Command::Initialize { root, persist_path } => {
            if persist_path.exists() {
                return Err(WebError::ArgumentsInvalid(format!(
                    "Path {} must not exist",
                    persist_path.to_string_lossy()
                )));
            };
            std::fs::create_dir_all(&persist_path)?;
            let resource_path = root.unwrap_or(".".into()).canonicalize()?.join("resource");
            let reader = BufReader::new(File::open(resource_path.join("config/config.json"))?);
            let config: Config = serde_json::from_reader(reader)?;
            let eval = StoryEvaluator::new(&config.tagger, &config.score, &config.scrape);

            let start = Instant::now();

            let import_start = Instant::now();
            let scrapes = progscrape_scrapers::import_legacy(Path::new("."))?;
            let import_time = import_start.elapsed();

            // First, build an in-memory index quickly
            let memindex_start = Instant::now();
            let mut memindex = MemIndex::default();
            memindex.insert_scrapes(scrapes.into_iter())?;
            let memindex_time = memindex_start.elapsed();

            // Now, import those stories
            let story_start = Instant::now();
            let mut index = StoryIndex::new(PersistLocation::Path(persist_path))?;
            index.insert_scrape_collections(&eval, memindex.get_all_stories())?;
            let story_index_time = story_start.elapsed();

            let count = index.story_count()?;
            tracing::info!("Shard   | Count");
            for (shard, count) in &count.by_shard {
                tracing::info!("{} | {}", shard, count);
            }

            tracing::info!(
                "Completed init in {}s (import={}s, memindex={}s, storyindex={}s)",
                start.elapsed().as_secs(),
                import_time.as_secs(),
                memindex_time.as_secs(),
                story_index_time.as_secs()
            );
        }
    };
    Ok(())
}
