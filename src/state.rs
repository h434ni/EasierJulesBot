use std::sync::Arc;
use dashmap::DashMap;
use reqwest::Client;
use tokio_util::sync::CancellationToken;
use crate::config::Config;
use crate::db::Db;

#[derive(Debug, Clone, Default)]
pub struct TopicSetupState {
    pub auto_pr: bool,
    pub selected_repo: Option<String>,
    pub selected_branch: Option<String>,
    pub waiting_for_prompt: bool,
}

#[derive(Debug, Clone)]
pub enum UserState {
    WaitingForApiKey { prompt_msg_id: Option<i32> },
}

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub config: Config,
    pub http_client: Client,
    pub user_states: Arc<DashMap<i64, UserState>>,
    pub topic_setups: Arc<DashMap<i32, TopicSetupState>>,
    pub pending_deletions: Arc<DashMap<i32, CancellationToken>>,
    pub last_seen_times: Arc<DashMap<String, String>>,
}

impl AppState {
    pub fn new(db: Arc<Db>, config: Config, http_client: Client) -> Self {
        Self {
            db,
            config,
            http_client,
            user_states: Arc::new(DashMap::new()),
            topic_setups: Arc::new(DashMap::new()),
            pending_deletions: Arc::new(DashMap::new()),
            last_seen_times: Arc::new(DashMap::new()),
        }
    }
}
