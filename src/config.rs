use std::env;
use std::path::PathBuf;
use crate::error::BotError;

#[derive(Clone, Debug)]
pub struct Config {
    pub bot_token: String,
    pub proxy: Option<String>,
    pub db_path: PathBuf,
}

impl Config {
    pub fn from_env() -> Result<Self, BotError> {
        let _ = dotenvy::dotenv();

        let bot_token = env::var("BOT_TOKEN")
            .map_err(|_| BotError::Config("BOT_TOKEN is required in .env".to_string()))?;

        if bot_token.trim().is_empty() {
            return Err(BotError::Config("BOT_TOKEN cannot be empty".to_string()));
        }

        let proxy = env::var("PROXY")
            .or_else(|_| env::var("HTTPS_PROXY"))
            .or_else(|_| env::var("https_proxy"))
            .or_else(|_| env::var("HTTP_PROXY"))
            .or_else(|_| env::var("http_proxy"))
            .ok()
            .filter(|p| !p.trim().is_empty());

        let db_path = env::var("DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("bot.db"));

        Ok(Self {
            bot_token,
            proxy,
            db_path,
        })
    }
}
