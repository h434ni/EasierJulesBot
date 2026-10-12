use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tracing::debug;
use crate::error::BotError;

#[derive(Clone, Debug)]
pub struct TelegramExtClient {
    client: Client,
    base_url: String,
}

impl TelegramExtClient {
    pub fn new(client: Client, bot_token: &str) -> Self {
        Self {
            client,
            base_url: format!("https://api.telegram.org/bot{bot_token}"),
        }
    }

    pub async fn call_api<T: DeserializeOwned>(
        &self,
        method: &str,
        payload: Option<&Value>,
    ) -> Result<T, BotError> {
        let url = format!("{}/{}", self.base_url, method);
        let mut req = self.client.post(&url).header("Content-Type", "application/json");

        if let Some(data) = payload {
            req = req.json(data);
        }

        let resp = self.client.execute(req.build()?).await?;
        let json_val: Value = resp.json().await?;

        let ok = json_val.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
        if !ok {
            let code = json_val.get("error_code").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let desc = json_val
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown Telegram API error")
                .to_string();
            return Err(BotError::Custom(format!(
                "Telegram API Error [{code}]: {desc}"
            )));
        }

        let result = json_val.get("result").cloned().unwrap_or(Value::Null);
        serde_json::from_value(result).map_err(BotError::Json)
    }

    pub async fn send_rich_message(
        &self,
        chat_id: i64,
        message_thread_id: Option<i32>,
        markdown_text: &str,
        reply_markup: Option<&Value>,
        disable_notification: bool,
    ) -> Result<Value, BotError> {
        let mut payload = json!({
            "chat_id": chat_id,
            "rich_message": {
                "markdown": markdown_text
            },
            "disable_notification": disable_notification
        });

        if let Some(thread_id) = message_thread_id {
            payload["message_thread_id"] = json!(thread_id);
        }
        if let Some(rm) = reply_markup {
            payload["reply_markup"] = rm.clone();
        }

        // Try sendRichMessage first
        match self.call_api::<Value>("sendRichMessage", Some(&payload)).await {
            Ok(res) => Ok(res),
            Err(e) => {
                debug!("sendRichMessage failed, falling back to sendMessage: {e}");
                // Fallback to standard sendMessage with HTML or plain Markdown
                let mut fallback_payload = json!({
                    "chat_id": chat_id,
                    "text": markdown_text,
                    "disable_notification": disable_notification
                });
                if let Some(thread_id) = message_thread_id {
                    fallback_payload["message_thread_id"] = json!(thread_id);
                }
                if let Some(rm) = reply_markup {
                    fallback_payload["reply_markup"] = rm.clone();
                }

                // Try with Markdown parse_mode first, if fails try plain text
                fallback_payload["parse_mode"] = json!("Markdown");
                match self.call_api::<Value>("sendMessage", Some(&fallback_payload)).await {
                    Ok(res) => Ok(res),
                    Err(_) => {
                        fallback_payload.as_object_mut().unwrap().remove("parse_mode");
                        self.call_api::<Value>("sendMessage", Some(&fallback_payload)).await
                    }
                }
            }
        }
    }

    pub async fn edit_rich_message(
        &self,
        chat_id: i64,
        message_id: i32,
        markdown_text: &str,
        reply_markup: Option<&Value>,
    ) -> Result<Value, BotError> {
        let mut payload = json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "rich_message": {
                "markdown": markdown_text
            }
        });
        if let Some(rm) = reply_markup {
            payload["reply_markup"] = rm.clone();
        }

        match self.call_api::<Value>("editMessageText", Some(&payload)).await {
            Ok(res) => Ok(res),
            Err(e) => {
                debug!("editMessageText with rich_message failed, falling back to standard edit: {e}");
                let mut fallback_payload = json!({
                    "chat_id": chat_id,
                    "message_id": message_id,
                    "text": markdown_text,
                });
                if let Some(rm) = reply_markup {
                    fallback_payload["reply_markup"] = rm.clone();
                }

                fallback_payload["parse_mode"] = json!("Markdown");
                match self.call_api::<Value>("editMessageText", Some(&fallback_payload)).await {
                    Ok(res) => Ok(res),
                    Err(_) => {
                        fallback_payload.as_object_mut().unwrap().remove("parse_mode");
                        self.call_api::<Value>("editMessageText", Some(&fallback_payload)).await
                    }
                }
            }
        }
    }

    pub async fn create_forum_topic(&self, chat_id: i64, name: &str) -> Result<Value, BotError> {
        let payload = json!({
            "chat_id": chat_id,
            "name": name
        });
        self.call_api("createForumTopic", Some(&payload)).await
    }

    pub async fn delete_forum_topic(
        &self,
        chat_id: i64,
        message_thread_id: i32,
    ) -> Result<Value, BotError> {
        let payload = json!({
            "chat_id": chat_id,
            "message_thread_id": message_thread_id
        });
        self.call_api("deleteForumTopic", Some(&payload)).await
    }

    pub async fn reopen_forum_topic(
        &self,
        chat_id: i64,
        message_thread_id: i32,
    ) -> Result<Value, BotError> {
        let payload = json!({
            "chat_id": chat_id,
            "message_thread_id": message_thread_id
        });
        self.call_api("reopenForumTopic", Some(&payload)).await
    }

    pub async fn pin_chat_message(
        &self,
        chat_id: i64,
        message_id: i32,
    ) -> Result<Value, BotError> {
        let payload = json!({
            "chat_id": chat_id,
            "message_id": message_id
        });
        self.call_api("pinChatMessage", Some(&payload)).await
    }

    pub async fn leave_chat(&self, chat_id: i64) -> Result<Value, BotError> {
        let payload = json!({ "chat_id": chat_id });
        self.call_api("leaveChat", Some(&payload)).await
    }
}
