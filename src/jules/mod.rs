pub mod models;

use reqwest::{Client, StatusCode};
use serde_json::Value;
use crate::error::BotError;
use models::*;

pub const JULES_BASE_URL: &str = "https://jules.googleapis.com/v1alpha";

#[derive(Clone, Debug)]
pub struct JulesApiClient {
    client: Client,
    api_key: String,
}

impl JulesApiClient {
    pub fn new(client: Client, api_key: String) -> Self {
        Self { client, api_key }
    }

    fn normalize_id(prefix: &str, id: &str) -> String {
        if id.starts_with(prefix) {
            id.to_string()
        } else {
            format!("{prefix}{id}")
        }
    }

    async fn request<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        endpoint: &str,
        params: Option<&[(&str, &str)]>,
        body: Option<&Value>,
    ) -> Result<T, BotError> {
        let url = format!("{JULES_BASE_URL}/{endpoint}");
        let mut req = self
            .client
            .request(method.clone(), &url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json");

        if let Some(p) = params {
            req = req.query(p);
        }

        if let Some(b) = body {
            req = req.json(b);
        }

        let resp = req.send().await?;
        let status = resp.status();

        if method == reqwest::Method::DELETE
            && (status == StatusCode::OK
                || status == StatusCode::ACCEPTED
                || status == StatusCode::NO_CONTENT)
        {
            return serde_json::from_str("{}").map_err(BotError::Json);
        }

        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(BotError::Jules(format!("API Error ({status}): {text}")));
        }

        let text = resp.text().await?;
        if text.trim().is_empty() {
            return serde_json::from_str("{}").map_err(BotError::Json);
        }

        serde_json::from_str(&text).map_err(|e| {
            BotError::Jules(format!("Failed to parse response JSON: {e}; raw: {text}"))
        })
    }

    pub async fn list_sources(
        &self,
        page_size: Option<u32>,
        page_token: Option<&str>,
    ) -> Result<ListSourcesResponse, BotError> {
        let size_str = page_size.unwrap_or(30).to_string();
        let mut params = vec![("pageSize", size_str.as_str())];
        if let Some(token) = page_token {
            params.push(("pageToken", token));
        }

        self.request(reqwest::Method::GET, "sources", Some(&params), None)
            .await
    }

    pub async fn get_source(&self, source_id: &str) -> Result<Source, BotError> {
        let endpoint = Self::normalize_id("sources/", source_id);
        self.request(reqwest::Method::GET, &endpoint, None, None)
            .await
    }

    pub async fn create_session(
        &self,
        prompt: &str,
        source: &str,
        branch: &str,
        auto_pr: bool,
    ) -> Result<Session, BotError> {
        let mut payload = serde_json::json!({
            "prompt": prompt,
            "sourceContext": {
                "source": source,
                "githubRepoContext": {
                    "startingBranch": branch
                }
            }
        });

        if auto_pr {
            payload["automationMode"] = serde_json::json!("AUTO_CREATE_PR");
        } else {
            payload["requirePlanApproval"] = serde_json::json!(false);
        }

        self.request(
            reqwest::Method::POST,
            "sessions",
            None,
            Some(&payload),
        )
        .await
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Session, BotError> {
        let endpoint = Self::normalize_id("sessions/", session_id);
        self.request(reqwest::Method::GET, &endpoint, None, None)
            .await
    }

    pub async fn list_sessions(
        &self,
        page_size: Option<u32>,
        page_token: Option<&str>,
    ) -> Result<ListSessionsResponse, BotError> {
        let size_str = page_size.unwrap_or(30).to_string();
        let mut params = vec![("pageSize", size_str.as_str())];
        if let Some(token) = page_token {
            params.push(("pageToken", token));
        }

        self.request(reqwest::Method::GET, "sessions", Some(&params), None)
            .await
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<(), BotError> {
        let endpoint = Self::normalize_id("sessions/", session_id);
        let _: Value = self
            .request(reqwest::Method::DELETE, &endpoint, None, None)
            .await?;
        Ok(())
    }

    pub async fn send_message(&self, session_id: &str, message: &str) -> Result<Value, BotError> {
        let normalized = Self::normalize_id("sessions/", session_id);
        let endpoint = format!("{normalized}:sendMessage");
        let payload = serde_json::json!({ "prompt": message });

        self.request(reqwest::Method::POST, &endpoint, None, Some(&payload))
            .await
    }

    pub async fn list_activities(
        &self,
        session_id: &str,
        page_size: Option<u32>,
        page_token: Option<&str>,
    ) -> Result<ListActivitiesResponse, BotError> {
        let normalized = Self::normalize_id("sessions/", session_id);
        let endpoint = format!("{normalized}/activities");
        let size_str = page_size.unwrap_or(50).to_string();
        let mut params = vec![("pageSize", size_str.as_str())];
        if let Some(token) = page_token {
            params.push(("pageToken", token));
        }

        self.request(reqwest::Method::GET, &endpoint, Some(&params), None)
            .await
    }
}
