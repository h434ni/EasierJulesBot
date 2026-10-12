use std::sync::Arc;
use std::time::Duration;
use teloxide::prelude::*;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use crate::bot::keyboards::get_topic_closed_keyboard;
use crate::error::BotError;
use crate::jules::JulesApiClient;
use crate::state::AppState;
use crate::telegram_ext::TelegramExtClient;

pub async fn handle_forum_topic_closed(
    _bot: Bot,
    msg: Message,
    state: Arc<AppState>,
    ext: TelegramExtClient,
) -> Result<(), BotError> {
    let thread_id = match msg.thread_id.map(|t| t.0 .0) {
        Some(id) if id != 0 => id,
        _ => return Ok(()),
    };

    let topic_data = match state.db.get_topic(thread_id)? {
        Some(t) => t,
        None => return Ok(()),
    };

    if topic_data.session_id.is_none() {
        return Ok(());
    }

    state.db.update_topic_state(thread_id, "closed")?;

    let kb = get_topic_closed_keyboard();
    let prompt_payload = serde_json::json!({
        "chat_id": msg.chat.id.0,
        "message_thread_id": thread_id,
        "text": "You closed this topic. Do you want to archive/delete the associated Jules session as well?\n(If no response in 5 minutes, I will reopen the topic)",
        "reply_markup": kb
    });

    let sent_msg: serde_json::Value = ext
        .call_api("sendMessage", Some(&prompt_payload))
        .await?;
    let prompt_msg_id = sent_msg
        .get("message_id")
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;

    // Create cancellation token for 5-minute timer
    let cancel_token = CancellationToken::new();
    state
        .pending_deletions
        .insert(thread_id, cancel_token.clone());

    let state_clone = state.clone();
    let ext_clone = ext.clone();
    let chat_id = msg.chat.id.0;

    tokio::spawn(async move {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(300)) => {
                state_clone.pending_deletions.remove(&thread_id);
                info!("5-minute timeout expired for topic {thread_id}, reopening topic");
                match ext_clone.reopen_forum_topic(chat_id, thread_id).await {
                    Ok(_) => {
                        let _ = state_clone.db.update_topic_state(thread_id, "open");
                        if prompt_msg_id != 0 {
                            let edit_payload = serde_json::json!({
                                "chat_id": chat_id,
                                "message_id": prompt_msg_id,
                                "text": "Time's up! Reopened the topic."
                            });
                            let _ = ext_clone.call_api::<serde_json::Value>("editMessageText", Some(&edit_payload)).await;
                        }
                    }
                    Err(e) => {
                        error!("Failed to reopen forum topic {thread_id}: {e}");
                    }
                }
            }
            _ = cancel_token.cancelled() => {
                info!("Topic close timer for {thread_id} was cancelled");
            }
        }
    });

    Ok(())
}

pub async fn handle_forum_topic_reopened(
    _bot: Bot,
    msg: Message,
    state: Arc<AppState>,
) -> Result<(), BotError> {
    let thread_id = match msg.thread_id.map(|t| t.0 .0) {
        Some(id) if id != 0 => id,
        _ => return Ok(()),
    };

    if let Some((_, token)) = state.pending_deletions.remove(&thread_id) {
        token.cancel();
    }

    state.db.update_topic_state(thread_id, "open")?;
    Ok(())
}

pub async fn handle_session_callbacks(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<AppState>,
    ext: TelegramExtClient,
) -> Result<(), BotError> {
    let data = match q.data.as_deref() {
        Some(d) => d,
        None => return Ok(()),
    };

    let msg = match q.message.as_ref() {
        Some(m) => m,
        None => return Ok(()),
    };

    let chat_id = msg.chat().id.0;
    let message_id = msg.id().0;
    let thread_id = msg
        .regular_message()
        .and_then(|rm| rm.thread_id)
        .map(|t| t.0 .0)
        .unwrap_or(0);

    if !data.starts_with("delete_session_") {
        return Ok(());
    }

    if thread_id == 0 {
        return Ok(());
    }

    // Cancel pending 5-min timer
    if let Some((_, token)) = state.pending_deletions.remove(&thread_id) {
        token.cancel();
    }

    let choice = data.strip_prefix("delete_session_").unwrap_or("");

    if choice == "yes" {
        let topic_data = state.db.get_topic(thread_id)?;
        let session_id = topic_data.and_then(|t| t.session_id);

        if let Some(s_id) = session_id {
            let api_key = match state.db.get_setting("api_key")? {
                Some(k) if !k.trim().is_empty() => k,
                _ => return Ok(()),
            };

            let client = JulesApiClient::new(state.http_client.clone(), api_key);
            match client.delete_session(&s_id).await {
                Ok(_) => {
                    let edit_payload = serde_json::json!({
                        "chat_id": chat_id,
                        "message_id": message_id,
                        "text": "Session has been deleted. The topic will remain closed."
                    });
                    let _ = ext.call_api::<serde_json::Value>("editMessageText", Some(&edit_payload)).await;
                    state.db.update_topic_state(thread_id, "deleted")?;
                }
                Err(e) => {
                    error!("Failed to delete session {s_id}: {e}");
                    let edit_payload = serde_json::json!({
                        "chat_id": chat_id,
                        "message_id": message_id,
                        "text": "Failed to delete session via API."
                    });
                    let _ = ext.call_api::<serde_json::Value>("editMessageText", Some(&edit_payload)).await;
                }
            }
        }
    } else {
        match ext.reopen_forum_topic(chat_id, thread_id).await {
            Ok(_) => {
                state.db.update_topic_state(thread_id, "open")?;
                let edit_payload = serde_json::json!({
                    "chat_id": chat_id,
                    "message_id": message_id,
                    "text": "Topic reopened."
                });
                let _ = ext.call_api::<serde_json::Value>("editMessageText", Some(&edit_payload)).await;
            }
            Err(e) => {
                error!("Failed to reopen topic {thread_id}: {e}");
                let edit_payload = serde_json::json!({
                    "chat_id": chat_id,
                    "message_id": message_id,
                    "text": "Failed to reopen topic."
                });
                let _ = ext.call_api::<serde_json::Value>("editMessageText", Some(&edit_payload)).await;
            }
        }
    }

    bot.answer_callback_query(q.id).await?;
    Ok(())
}

pub async fn handle_topic_session_message(
    _bot: Bot,
    msg: Message,
    state: Arc<AppState>,
    ext: TelegramExtClient,
) -> Result<(), BotError> {
    let thread_id = match msg.thread_id.map(|t| t.0 .0) {
        Some(id) if id != 0 => id,
        _ => return Ok(()),
    };

    let text = match msg.text() {
        Some(t) if !t.starts_with('/') => t,
        _ => return Ok(()),
    };

    let owner_id = state.db.get_setting("owner_id")?;
    let sender_id = msg.from.as_ref().map(|u| u.id.0.to_string());
    if owner_id.is_some() && sender_id != owner_id {
        return Ok(());
    }

    let topic_data = match state.db.get_topic(thread_id)? {
        Some(t) => t,
        None => return Ok(()),
    };

    let session_id = match topic_data.session_id {
        Some(s) if topic_data.state == "open" => s,
        _ => return Ok(()),
    };

    let api_key = match state.db.get_setting("api_key")? {
        Some(k) if !k.trim().is_empty() => k,
        _ => return Ok(()),
    };

    let client = JulesApiClient::new(state.http_client.clone(), api_key);
    if let Err(e) = client.send_message(&session_id, text).await {
        error!("Failed to send message to Jules session {session_id}: {e}");
        let payload = serde_json::json!({
            "chat_id": msg.chat.id.0,
            "message_thread_id": thread_id,
            "text": "Failed to send message to Jules."
        });
        let _ = ext.call_api::<serde_json::Value>("sendMessage", Some(&payload)).await;
    }

    Ok(())
}
