use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{Chat, ChatKind, ChatMemberKind, ChatMemberUpdated, PublicChatKind};
use crate::bot::keyboards::*;
use crate::error::BotError;
use crate::state::AppState;
use crate::telegram_ext::TelegramExtClient;

pub async fn handle_my_chat_member(
    bot: Bot,
    event: ChatMemberUpdated,
    state: Arc<AppState>,
    ext: TelegramExtClient,
) -> Result<(), BotError> {
    let was_not_member = match event.old_chat_member.kind {
        ChatMemberKind::Left | ChatMemberKind::Banned(_) | ChatMemberKind::Restricted(_) => true,
        _ => false,
    };

    let is_member = match event.new_chat_member.kind {
        ChatMemberKind::Member | ChatMemberKind::Administrator(_) => true,
        _ => false,
    };

    if !(was_not_member && is_member) {
        return Ok(());
    }

    let chat_id = event.chat.id.0;

    if !event.chat.is_supergroup() {
        let _ = bot
            .send_message(
                event.chat.id,
                "this group is not a supergroup. make it supergroup by enabling topics then add me again",
            )
            .await;
        let _ = ext.leave_chat(chat_id).await;
        return Ok(());
    }

    let owner_id = state.db.get_setting("owner_id")?;
    let adder_id = event.from.id.0.to_string();

    if owner_id.as_deref() != Some(&adder_id) {
        let _ = bot
            .send_message(
                event.chat.id,
                "I can only be added by my owner. Leaving...",
            )
            .await;
        let _ = ext.leave_chat(chat_id).await;
        return Ok(());
    }

    let ready_for_group = state.db.get_setting("ready_for_group")?;
    if ready_for_group.as_deref() != Some("true") {
        let _ = bot
            .send_message(
                event.chat.id,
                "Please use 'Setup Group' in private chat before adding me. Leaving...",
            )
            .await;
        let _ = ext.leave_chat(chat_id).await;
        return Ok(());
    }

    let chat_id_str = chat_id.to_string();
    let existing_group = state.db.get_setting("group_id")?;

    if let Some(ref old_gid) = existing_group {
        if old_gid != &chat_id_str {
            if let Ok(old_id) = old_gid.parse::<i64>() {
                let _ = bot
                    .send_message(
                        ChatId(old_id),
                        "I am being moved to another group by my owner. Goodbye!",
                    )
                    .await;
                let _ = ext.leave_chat(old_id).await;
            }
        }
    }

    state.db.set_setting("group_id", &chat_id_str)?;
    state.db.set_setting("ready_for_group", "false")?;

    // Check forum and permissions
    let (is_forum_chat, is_admin, can_manage_topics, can_pin) = check_chat_status(&bot, chat_id).await;

    let mut send = bot.send_message(
        event.chat.id,
        "Hello! I am ready to manage Jules API tasks here.",
    );

    if let Some(kb) = get_admin_check_keyboard(is_admin, can_manage_topics, can_pin, is_forum_chat) {
        send = send.reply_markup(kb);
    }

    send.await?;
    Ok(())
}

fn check_is_forum(chat: &Chat) -> bool {
    if let ChatKind::Public(ref p) = chat.kind {
        if let PublicChatKind::Supergroup(ref s) = p.kind {
            return s.is_forum;
        }
    }
    false
}

async fn check_chat_status(bot: &Bot, chat_id: i64) -> (bool, bool, bool, bool) {
    let me = match bot.get_me().await {
        Ok(m) => m,
        Err(_) => return (false, false, false, false),
    };

    let chat = match bot.get_chat(ChatId(chat_id)).await {
        Ok(c) => c,
        Err(_) => return (false, false, false, false),
    };

    let is_forum = check_is_forum(&chat);

    let member = match bot.get_chat_member(ChatId(chat_id), me.id).await {
        Ok(m) => m,
        Err(_) => return (is_forum, false, false, false),
    };

    match member.kind {
        ChatMemberKind::Administrator(admin) => (
            is_forum,
            true,
            admin.can_manage_topics,
            admin.can_pin_messages,
        ),
        _ => (is_forum, false, false, false),
    }
}

pub async fn handle_group_callbacks(
    bot: Bot,
    q: CallbackQuery,
    _state: Arc<AppState>,
    _ext: TelegramExtClient,
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
    let message_id = msg.id();

    if data == "admin_ok" {
        bot.answer_callback_query(q.id)
            .text("Bot is admin with all required permissions!")
            .show_alert(true)
            .await?;
    } else if data == "admin_not_admin" {
        let (is_forum, is_admin, can_manage_topics, can_pin) =
            check_chat_status(&bot, chat_id).await;

        if is_admin {
            let kb = get_admin_check_keyboard(is_admin, can_manage_topics, can_pin, is_forum);
            let _ = bot
                .edit_message_reply_markup(msg.chat().id, message_id)
                .reply_markup(kb.unwrap_or_default())
                .await;
            bot.answer_callback_query(q.id)
                .text("Status updated!")
                .await?;
        } else {
            bot.answer_callback_query(q.id)
                .text("Please make the bot an Admin.")
                .show_alert(true)
                .await?;
        }
    } else if data == "admin_missing_perms" {
        let (is_forum, is_admin, can_manage_topics, can_pin) =
            check_chat_status(&bot, chat_id).await;

        if can_manage_topics && can_pin {
            let kb = get_admin_check_keyboard(is_admin, can_manage_topics, can_pin, is_forum);
            let _ = bot
                .edit_message_reply_markup(msg.chat().id, message_id)
                .reply_markup(kb.unwrap_or_default())
                .await;
            bot.answer_callback_query(q.id)
                .text("Permissions updated!")
                .await?;
        } else {
            let mut missing = Vec::new();
            if !can_manage_topics {
                missing.push("Manage Topics");
            }
            if !can_pin {
                missing.push("Pin Messages");
            }
            bot.answer_callback_query(q.id)
                .text(format!("Missing permissions:\n{}", missing.join("\n")))
                .show_alert(true)
                .await?;
        }
    } else if data == "forum_not_enabled" {
        let (is_forum, is_admin, can_manage_topics, can_pin) =
            check_chat_status(&bot, chat_id).await;

        if is_forum {
            let kb = get_admin_check_keyboard(is_admin, can_manage_topics, can_pin, is_forum);
            let _ = bot
                .edit_message_reply_markup(msg.chat().id, message_id)
                .reply_markup(kb.unwrap_or_default())
                .await;
            bot.answer_callback_query(q.id)
                .text("Topics are now enabled!")
                .show_alert(true)
                .await?;
        } else {
            bot.answer_callback_query(q.id)
                .text("Please go to Group Settings -> Enable Topics.")
                .show_alert(true)
                .await?;
        }
    }

    Ok(())
}
