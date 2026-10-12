use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::MessageId;
use tracing::{error, info};
use crate::bot::keyboards::*;
use crate::error::BotError;
use crate::state::{AppState, UserState};
use crate::telegram_ext::TelegramExtClient;

pub async fn handle_start_command(
    bot: Bot,
    msg: Message,
    state: Arc<AppState>,
) -> Result<(), BotError> {
    if !msg.chat.is_private() {
        return Ok(());
    }

    let user_id = match msg.from.as_ref() {
        Some(u) => u.id.0 as i64,
        None => return Ok(()),
    };
    let user_id_str = user_id.to_string();

    let owner_id = state.db.get_setting("owner_id")?;
    match owner_id {
        None => {
            info!("Claiming bot ownership for user {user_id_str}");
            state.db.set_setting("owner_id", &user_id_str)?;
        }
        Some(ref id) if id != &user_id_str => {
            bot.send_message(
                msg.chat.id,
                "Sorry, this bot is already claimed by another user.",
            )
            .await?;
            return Ok(());
        }
        _ => {}
    }

    let api_key = state.db.get_setting("api_key")?;
    let is_connected = api_key.is_some() && !api_key.as_ref().unwrap().trim().is_empty();
    let group_id = state.db.get_setting("group_id")?;

    let keyboard = get_start_keyboard(is_connected, group_id.as_deref());

    bot.send_message(
        msg.chat.id,
        "Welcome to the Jules API Telegram Bot!\n\nUse the buttons below to configure the bot.",
    )
    .reply_markup(keyboard)
    .await?;

    Ok(())
}

pub async fn handle_start_callbacks(
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

    let chat_id = msg.chat().id;
    let message_id = msg.id();
    let user_id = q.from.id.0 as i64;

    match data {
        "connect_account" => {
            bot.edit_message_text(chat_id, message_id, "Please send me your Jules API Key.")
                .reply_markup(get_cancel_connect_keyboard())
                .await?;

            state.user_states.insert(
                user_id,
                UserState::WaitingForApiKey {
                    prompt_msg_id: Some(message_id.0),
                },
            );
            bot.answer_callback_query(q.id).await?;
        }
        "cancel_connect" => {
            state.user_states.remove(&user_id);
            let api_key = state.db.get_setting("api_key")?;
            let is_connected =
                api_key.is_some() && !api_key.as_ref().unwrap().trim().is_empty();
            let group_id = state.db.get_setting("group_id")?;

            let keyboard = get_start_keyboard(is_connected, group_id.as_deref());

            bot.edit_message_text(
                chat_id,
                message_id,
                "Welcome to the Jules API Telegram Bot!\n\nUse the buttons below to configure the bot.",
            )
            .reply_markup(keyboard)
            .await?;

            bot.answer_callback_query(q.id).await?;
        }
        "setup_group" => {
            state.db.set_setting("ready_for_group", "true")?;

            // Send with markdown formatting via ext client for backwards compatibility
            let text = "Bot is now ready to be added to a group!\n\n(If you are moving me to a new group, I will try to leave the old one automatically. If I fail, please remove me manually.)\n\nPlease add me to a single group chat, and make sure the group has *Topics (Forum mode) enabled*.";
            let payload = serde_json::json!({
                "chat_id": chat_id.0,
                "message_id": message_id.0,
                "text": text,
                "parse_mode": "Markdown",
                "reply_markup": get_cancel_connect_keyboard()
            });
            let _ = ext.call_api::<serde_json::Value>("editMessageText", Some(&payload)).await;

            bot.answer_callback_query(q.id).await?;
        }
        "new_task" => {
            let group_id_str = state.db.get_setting("group_id")?;
            let group_id = match group_id_str {
                Some(ref gid) => match gid.parse::<i64>() {
                    Ok(id) => id,
                    Err(_) => {
                        bot.answer_callback_query(q.id)
                            .text("Configured group ID is invalid.")
                            .show_alert(true)
                            .await?;
                        return Ok(());
                    }
                },
                None => {
                    bot.answer_callback_query(q.id)
                        .text("Please add me to a group first!")
                        .show_alert(true)
                        .await?;
                    return Ok(());
                }
            };

            match ext.create_forum_topic(group_id, "New Task").await {
                Ok(topic_val) => {
                    let thread_id = topic_val
                        .get("message_thread_id")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0) as i32;
                    let topic_name = topic_val
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("New Task");

                    bot.answer_callback_query(q.id)
                        .text(format!("Created new topic: {topic_name}"))
                        .await?;

                    state.db.create_topic(thread_id)?;

                    // Send topic setup menu in that forum topic
                    let menu_payload = serde_json::json!({
                        "chat_id": group_id,
                        "message_thread_id": thread_id,
                        "text": "Please configure this topic:",
                        "reply_markup": get_topic_setup_keyboard(false)
                    });
                    let _ = ext.call_api::<serde_json::Value>("sendMessage", Some(&menu_payload)).await;

                    // Edit private message
                    let gid_str = group_id_str.as_deref().unwrap();
                    let nav_kb = get_new_task_created_keyboard(gid_str, thread_id);
                    bot.edit_message_text(
                        chat_id,
                        message_id,
                        "new task created. go to task to continue",
                    )
                    .reply_markup(nav_kb)
                    .await?;
                }
                Err(e) => {
                    error!("Failed to create forum topic: {e}");
                    bot.answer_callback_query(q.id)
                        .text("Failed to create topic. Make sure I have permissions to manage topics.")
                        .show_alert(true)
                        .await?;
                }
            }
        }
        _ => {}
    }

    Ok(())
}

pub async fn handle_private_text(
    bot: Bot,
    msg: Message,
    state: Arc<AppState>,
) -> Result<(), BotError> {
    if !msg.chat.is_private() {
        return Ok(());
    }

    let user_id = match msg.from.as_ref() {
        Some(u) => u.id.0 as i64,
        None => return Ok(()),
    };

    let text = match msg.text() {
        Some(t) => t.trim(),
        None => return Ok(()),
    };

    if text.starts_with('/') {
        return Ok(());
    }

    if let Some((_, user_state)) = state.user_states.remove(&user_id) {
        match user_state {
            UserState::WaitingForApiKey { prompt_msg_id } => {
                if let Some(p_id) = prompt_msg_id {
                    let _ = bot
                        .edit_message_reply_markup(msg.chat.id, MessageId(p_id))
                        .reply_markup(teloxide::types::InlineKeyboardMarkup::default())
                        .await;
                }

                state.db.set_setting("api_key", text)?;
                let group_id = state.db.get_setting("group_id")?;
                let keyboard = get_start_keyboard(true, group_id.as_deref());

                bot.send_message(
                    msg.chat.id,
                    "API Key saved! You can now proceed to setup a group.",
                )
                .reply_markup(keyboard)
                .await?;
            }
        }
    }

    Ok(())
}
