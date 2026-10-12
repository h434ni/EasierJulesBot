use std::sync::Arc;
use teloxide::prelude::*;
use tracing::error;
use crate::bot::keyboards::*;
use crate::error::BotError;
use crate::jules::JulesApiClient;
use crate::state::{AppState, TopicSetupState};
use crate::telegram_ext::TelegramExtClient;

pub async fn handle_forum_topic_created(
    _bot: Bot,
    msg: Message,
    state: Arc<AppState>,
    ext: TelegramExtClient,
) -> Result<(), BotError> {
    if msg.forum_topic_created().is_none() {
        return Ok(());
    }

    let owner_id = state.db.get_setting("owner_id")?;
    let sender_id = msg.from.as_ref().map(|u| u.id.0.to_string());

    if owner_id.is_some() && sender_id != owner_id {
        return Ok(());
    }

    let thread_id = msg.thread_id.map(|t| t.0 .0).unwrap_or(0);
    if thread_id == 0 {
        return Ok(());
    }

    state.db.create_topic(thread_id)?;
    state
        .topic_setups
        .insert(thread_id, TopicSetupState::default());

    let menu_payload = serde_json::json!({
        "chat_id": msg.chat.id.0,
        "message_thread_id": thread_id,
        "text": "Please configure this topic:",
        "reply_markup": get_topic_setup_keyboard(false)
    });
    let _ = ext.call_api::<serde_json::Value>("sendMessage", Some(&menu_payload)).await;

    Ok(())
}

pub async fn handle_topic_setup_callbacks(
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

    if data.starts_with("toggle_pr_") {
        if thread_id == 0 {
            return Ok(());
        }

        let cur_val = data.strip_prefix("toggle_pr_").unwrap_or("0") == "1";
        let new_val = !cur_val;

        {
            let mut entry = state.topic_setups.entry(thread_id).or_default();
            entry.auto_pr = new_val;
        }

        let payload = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "reply_markup": get_topic_setup_keyboard(new_val)
        });
        let _ = ext.call_api::<serde_json::Value>("editMessageReplyMarkup", Some(&payload)).await;
        bot.answer_callback_query(q.id).await?;
    } else if data == "cancel_setup" {
        if thread_id == 0 {
            return Ok(());
        }

        let _ = state.db.delete_topic(thread_id);
        state.topic_setups.remove(&thread_id);

        bot.answer_callback_query(q.id)
            .text("Deleting topic...")
            .await?;

        match ext.delete_forum_topic(chat_id, thread_id).await {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to delete topic {thread_id}: {e}");
                let _ = bot
                    .edit_message_text(
                        msg.chat().id,
                        msg.id(),
                        "Failed to delete topic. Please delete it manually.",
                    )
                    .await;
            }
        }
    } else if data == "proceed_setup_new" {
        let api_key = state.db.get_setting("api_key")?;
        let api_key = match api_key {
            Some(ref k) if !k.trim().is_empty() => k.clone(),
            _ => {
                bot.answer_callback_query(q.id)
                    .text("API Key not found. Connect account first.")
                    .show_alert(true)
                    .await?;
                return Ok(());
            }
        };

        let client = JulesApiClient::new(state.http_client.clone(), api_key);
        match client.list_sources(Some(100), None).await {
            Ok(sources_res) => {
                if sources_res.sources.is_empty() {
                    bot.answer_callback_query(q.id)
                        .text("No repositories found in your Jules account.")
                        .show_alert(true)
                        .await?;
                    return Ok(());
                }

                let mut list = Vec::new();
                for s in sources_res.sources {
                    let name = s.id.or(s.name).unwrap_or_default();
                    if name.is_empty() {
                        continue;
                    }
                    let short = name.replace("github-", "");
                    list.push((short, name));
                }

                let kb = get_repo_picker_keyboard(&list);
                bot.edit_message_text(msg.chat().id, msg.id(), "Select a repository:")
                    .reply_markup(kb)
                    .await?;
                bot.answer_callback_query(q.id).await?;
            }
            Err(e) => {
                error!("Error fetching sources: {e}");
                bot.answer_callback_query(q.id)
                    .text("Error fetching repositories.")
                    .show_alert(true)
                    .await?;
            }
        }
    } else if data.starts_with("repo_") {
        if thread_id == 0 {
            return Ok(());
        }

        let repo_name = data.strip_prefix("repo_").unwrap().to_string();
        {
            let mut entry = state.topic_setups.entry(thread_id).or_default();
            entry.selected_repo = Some(repo_name.clone());
        }

        let api_key = state.db.get_setting("api_key")?;
        let api_key = match api_key {
            Some(ref k) if !k.trim().is_empty() => k.clone(),
            _ => return Ok(()),
        };

        let client = JulesApiClient::new(state.http_client.clone(), api_key);
        match client.get_source(&repo_name).await {
            Ok(src) => {
                let branches = src
                    .github_repo
                    .map(|r| r.branches.into_iter().map(|b| b.display_name).collect::<Vec<_>>())
                    .unwrap_or_default();

                if branches.is_empty() {
                    bot.answer_callback_query(q.id)
                        .text("No branches found for this repository.")
                        .show_alert(true)
                        .await?;
                    return Ok(());
                }

                let kb = get_branch_picker_keyboard(&branches);
                bot.edit_message_text(msg.chat().id, msg.id(), "Select a branch:")
                    .reply_markup(kb)
                    .await?;
                bot.answer_callback_query(q.id).await?;
            }
            Err(e) => {
                error!("Error fetching branches: {e}");
                bot.answer_callback_query(q.id)
                    .text("Error fetching branches.")
                    .show_alert(true)
                    .await?;
            }
        }
    } else if data.starts_with("branch_") {
        if thread_id == 0 {
            return Ok(());
        }

        let branch_name = data.strip_prefix("branch_").unwrap().to_string();
        let repo_name = {
            let mut entry = state.topic_setups.entry(thread_id).or_default();
            entry.selected_branch = Some(branch_name.clone());
            entry.waiting_for_prompt = true;
            entry.selected_repo.clone().unwrap_or_else(|| "Unknown".to_string())
        };

        let short_repo = repo_name
            .split('/')
            .last()
            .unwrap_or(&repo_name)
            .replace("github-", "");

        let text = format!(
            "Selected Repo: `{short_repo}`\nSelected Branch: `{branch_name}`\n\nPlease send the initialization message (prompt) to start the session."
        );

        let payload = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "text": text,
            "parse_mode": "Markdown"
        });
        let _ = ext.call_api::<serde_json::Value>("editMessageText", Some(&payload)).await;
        bot.answer_callback_query(q.id).await?;
    } else if data == "back_to_setup" {
        if thread_id == 0 {
            return Ok(());
        }

        let auto_pr = state
            .topic_setups
            .get(&thread_id)
            .map(|s| s.auto_pr)
            .unwrap_or(false);

        bot.edit_message_text(msg.chat().id, msg.id(), "Please configure this topic:")
            .reply_markup(get_topic_setup_keyboard(auto_pr))
            .await?;
        bot.answer_callback_query(q.id).await?;
    } else if data == "no_pr_alert" {
        bot.answer_callback_query(q.id)
            .text("There is no PR yet.")
            .show_alert(true)
            .await?;
    }

    Ok(())
}

pub fn get_state_emoji(state: &str) -> &'static str {
    match state {
        "QUEUED" => "⏳",
        "PLANNING" => "🧠",
        "AWAITING_PLAN_APPROVAL" => "✋",
        "AWAITING_USER_FEEDBACK" => "💬",
        "IN_PROGRESS" => "🔄",
        "PAUSED" => "⏸️",
        "FAILED" => "❌",
        "COMPLETED" => "✅",
        _ => "🔵",
    }
}

pub async fn handle_topic_prompt(
    _bot: Bot,
    msg: Message,
    state: Arc<AppState>,
    ext: TelegramExtClient,
) -> Result<bool, BotError> {
    let thread_id = match msg.thread_id.map(|t| t.0 .0) {
        Some(id) if id != 0 => id,
        _ => return Ok(false),
    };

    let is_waiting = state
        .topic_setups
        .get(&thread_id)
        .map(|s| s.waiting_for_prompt)
        .unwrap_or(false);

    if !is_waiting {
        return Ok(false);
    }

    let text = match msg.text() {
        Some(t) => t.trim(),
        None => return Ok(false),
    };

    let topic_setup = state.topic_setups.get(&thread_id).unwrap().clone();
    let repo_name = topic_setup.selected_repo.unwrap_or_default();
    let branch_name = topic_setup.selected_branch.unwrap_or_default();
    let auto_pr = topic_setup.auto_pr;

    let api_key = match state.db.get_setting("api_key")? {
        Some(k) if !k.trim().is_empty() => k,
        _ => return Ok(false),
    };

    let client = JulesApiClient::new(state.http_client.clone(), api_key);

    // Send temporary "Starting session..." status message
    let status_payload = serde_json::json!({
        "chat_id": msg.chat.id.0,
        "message_thread_id": thread_id,
        "text": "Starting session..."
    });
    let status_msg: serde_json::Value = ext
        .call_api("sendMessage", Some(&status_payload))
        .await?;
    let status_msg_id = status_msg
        .get("message_id")
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;

    match client.create_session(text, &repo_name, &branch_name, auto_pr).await {
        Ok(session_data) => {
            let session_id = session_data.name;
            state.db.update_topic_session(thread_id, &session_id)?;
            state.db.update_topic_auto_pr(thread_id, auto_pr)?;

            let short_repo = repo_name
                .split('/')
                .last()
                .unwrap_or(&repo_name)
                .replace("github-", "");

            let clean_id = session_id.split('/').last().unwrap_or(&session_id);
            let jules_url = session_data
                .url
                .unwrap_or_else(|| format!("https://jules.google.com/session/{clean_id}"));

            let auto_pr_str = if auto_pr { "`ON`" } else { "`OFF`" };
            let state_str = session_data.state.unwrap_or_else(|| "QUEUED".to_string());
            let state_emoji = get_state_emoji(&state_str);

            let rich_md = format!(
                "\n# {session_id}\n\n| **Status** | **Auto PR** | **Repo** | **Branch** | **State** |\n|:---|:---|:---|:---|:---|\n| `Active` | {auto_pr_str} | `{short_repo}` | `{branch_name}` | {state_emoji} `{state_str}` |\n"
            );

            let kb = get_status_card_keyboard(&jules_url, None);

            let pinned_msg = ext
                .send_rich_message(
                    msg.chat.id.0,
                    Some(thread_id),
                    &rich_md,
                    Some(&kb),
                    false,
                )
                .await?;

            let pinned_msg_id = pinned_msg
                .get("message_id")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;

            if pinned_msg_id != 0 {
                let _ = ext.pin_chat_message(msg.chat.id.0, pinned_msg_id).await;
                state
                    .db
                    .update_topic_pinned_message(thread_id, pinned_msg_id)?;
            }

            // Delete temporary status message
            let del_payload = serde_json::json!({
                "chat_id": msg.chat.id.0,
                "message_id": status_msg_id
            });
            let _ = ext.call_api::<serde_json::Value>("deleteMessage", Some(&del_payload)).await;

            state.topic_setups.remove(&thread_id);
        }
        Err(e) => {
            error!("Error creating session: {e}");
            let edit_payload = serde_json::json!({
                "chat_id": msg.chat.id.0,
                "message_id": status_msg_id,
                "text": format!("Error creating session: {e}")
            });
            let _ = ext.call_api::<serde_json::Value>("editMessageText", Some(&edit_payload)).await;
        }
    }

    Ok(true)
}
