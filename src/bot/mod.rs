pub mod handlers;
pub mod keyboards;
pub mod middleware;

use std::sync::Arc;
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::dptree;
use teloxide::prelude::*;
use teloxide::types::{CallbackQuery, ChatMemberUpdated, Message, Update};
use tracing::error;
use crate::error::BotError;
use crate::state::AppState;
use crate::telegram_ext::TelegramExtClient;

pub fn build_dispatcher(
    bot: Bot,
    state: Arc<AppState>,
    ext: TelegramExtClient,
) -> Dispatcher<Bot, BotError, teloxide::dispatching::DefaultKey> {
    let handler = dptree::entry()
        .branch(
            Update::filter_my_chat_member().endpoint(
                |bot: Bot, event: ChatMemberUpdated, state: Arc<AppState>, ext: TelegramExtClient| async move {
                    handlers::group::handle_my_chat_member(bot, event, state, ext).await
                },
            ),
        )
        .branch(
            Update::filter_callback_query().endpoint(
                |bot: Bot, q: CallbackQuery, state: Arc<AppState>, ext: TelegramExtClient| async move {
                    if let Some(ref m) = q.message {
                        if !middleware::is_authorized_chat(m.chat(), &state, &ext).await {
                            return Ok(());
                        }
                    }

                    if let Some(ref d) = q.data {
                        if d == "connect_account"
                            || d == "cancel_connect"
                            || d == "setup_group"
                            || d == "new_task"
                        {
                            return handlers::start::handle_start_callbacks(bot, q, state, ext).await;
                        }

                        if d.starts_with("admin_") || d == "forum_not_enabled" {
                            return handlers::group::handle_group_callbacks(bot, q, state, ext).await;
                        }

                        if d.starts_with("toggle_pr_")
                            || d == "cancel_setup"
                            || d == "proceed_setup_new"
                            || d.starts_with("repo_")
                            || d.starts_with("branch_")
                            || d == "back_to_setup"
                            || d == "no_pr_alert"
                        {
                            return handlers::topic_setup::handle_topic_setup_callbacks(bot, q, state, ext).await;
                        }

                        if d.starts_with("delete_session_") {
                            return handlers::session_msg::handle_session_callbacks(bot, q, state, ext).await;
                        }
                    }

                    bot.answer_callback_query(q.id).await?;
                    Ok(())
                },
            ),
        )
        .branch(
            Update::filter_message().endpoint(
                |bot: Bot, msg: Message, state: Arc<AppState>, ext: TelegramExtClient| async move {
                    if !middleware::is_authorized_chat(&msg.chat, &state, &ext).await {
                        return Ok(());
                    }

                    // 1. Topic lifecycle service messages
                    if msg.forum_topic_created().is_some() {
                        return handlers::topic_setup::handle_forum_topic_created(bot, msg, state, ext).await;
                    }

                    if msg.forum_topic_closed().is_some() {
                        return handlers::session_msg::handle_forum_topic_closed(bot, msg, state, ext).await;
                    }

                    if msg.forum_topic_reopened().is_some() {
                        return handlers::session_msg::handle_forum_topic_reopened(bot, msg, state).await;
                    }

                    // 2. Private chat flows
                    if msg.chat.is_private() {
                        if let Some(text) = msg.text() {
                            if text == "/start" || text.starts_with("/start ") {
                                return handlers::start::handle_start_command(bot, msg, state).await;
                            }
                        }
                        return handlers::start::handle_private_text(bot, msg, state).await;
                    }

                    // 3. Forum Topic group messages
                    if msg.thread_id.is_some() {
                        // Check if topic is waiting for prompt
                        if handlers::topic_setup::handle_topic_prompt(bot.clone(), msg.clone(), state.clone(), ext.clone()).await? {
                            return Ok(());
                        }

                        // Regular session message forwarded to Jules
                        return handlers::session_msg::handle_topic_session_message(bot, msg, state, ext).await;
                    }

                    Ok(())
                },
            ),
        );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state, ext])
        .default_handler(|_| async {})
        .error_handler(Arc::new(|err| async move {
            error!("Telegram dispatcher error: {err:?}");
        }))
        .build()
}
