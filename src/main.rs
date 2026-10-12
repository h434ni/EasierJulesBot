use std::sync::Arc;
use reqwest::Proxy;
use teloxide::prelude::*;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use easier_jules_lib::bot::build_dispatcher;
use easier_jules_lib::config::Config;
use easier_jules_lib::db::Db;
use easier_jules_lib::error::BotError;
use easier_jules_lib::poller::start_activity_poller;
use easier_jules_lib::state::AppState;
use easier_jules_lib::telegram_ext::TelegramExtClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize structured logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting EasierJules Telegram Bot (Rust Edition)...");

    // 2. Load configuration
    let config = Config::from_env().map_err(|e| {
        error!("Failed to load configuration: {e}");
        e
    })?;

    // 3. Build HTTP client (with proxy if configured)
    let mut http_builder = reqwest::Client::builder();
    if let Some(ref proxy_url) = config.proxy {
        info!("Configuring HTTP proxy: {proxy_url}");
        let proxy = Proxy::all(proxy_url).map_err(|e| {
            BotError::Config(format!("Invalid proxy configuration '{proxy_url}': {e}"))
        })?;
        http_builder = http_builder.proxy(proxy);
    }
    let http_client = http_builder.build()?;

    // 4. Initialize SQLite database
    let db = Arc::new(Db::open(&config.db_path).map_err(|e| {
        error!("Failed to open database at {:?}: {e}", config.db_path);
        e
    })?);
    info!("SQLite database initialized at {:?}", config.db_path);

    // 5. Initialize Telegram clients
    let bot = Bot::with_client(&config.bot_token, http_client.clone());
    let ext_client = TelegramExtClient::new(http_client.clone(), &config.bot_token);

    // 6. Build shared application state
    let state = Arc::new(AppState::new(db, config.clone(), http_client.clone()));

    // 7. Spawn background activity poller
    let poller_state = state.clone();
    let poller_ext = ext_client.clone();
    tokio::spawn(async move {
        start_activity_poller(poller_state, poller_ext).await;
    });

    // 8. Build dispatcher and run Telegram poller
    let mut dispatcher = build_dispatcher(bot, state, ext_client);

    info!("Bot is polling for updates...");
    dispatcher.dispatch().await;

    Ok(())
}
