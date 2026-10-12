use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use url::Url;

pub fn clean_group_id(group_id: &str) -> &str {
    if let Some(stripped) = group_id.strip_prefix("-100") {
        stripped
    } else {
        group_id
    }
}

pub fn get_start_keyboard(is_connected: bool, group_id: Option<&str>) -> InlineKeyboardMarkup {
    let connect_text = if is_connected {
        "Account Connected ✅"
    } else {
        "Connect Account"
    };

    let group_text = if group_id.is_some() {
        "Group Connected ✅"
    } else {
        "Setup Group"
    };

    let mut rows = Vec::new();

    rows.push(vec![InlineKeyboardButton::callback(
        connect_text,
        "connect_account",
    )]);

    let mut group_row = vec![InlineKeyboardButton::callback(group_text, "setup_group")];
    if let Some(gid) = group_id {
        let clean = clean_group_id(gid);
        let group_url = format!("https://t.me/c/{clean}/1");
        if let Ok(url) = Url::parse(&group_url) {
            group_row.push(InlineKeyboardButton::url("Open Group", url));
        }
    }
    rows.push(group_row);

    if group_id.is_some() {
        rows.push(vec![InlineKeyboardButton::callback("New Task", "new_task")]);
    }

    InlineKeyboardMarkup::new(rows)
}

pub fn get_cancel_connect_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        "Back",
        "cancel_connect",
    )]])
}

pub fn get_admin_check_keyboard(
    is_admin: bool,
    can_manage_topics: bool,
    can_pin_messages: bool,
    is_forum: bool,
) -> Option<InlineKeyboardMarkup> {
    let mut rows = Vec::new();

    if !(is_admin && can_manage_topics && can_pin_messages) {
        if is_admin {
            rows.push(vec![InlineKeyboardButton::callback(
                "Permissions missing ⚠️",
                "admin_missing_perms",
            )]);
        } else {
            rows.push(vec![InlineKeyboardButton::callback(
                "Bot is not Admin 🚫",
                "admin_not_admin",
            )]);
        }
    }

    if !is_forum {
        rows.push(vec![InlineKeyboardButton::callback(
            "Topics not enabled 🚫",
            "forum_not_enabled",
        )]);
    }

    if rows.is_empty() {
        None
    } else {
        Some(InlineKeyboardMarkup::new(rows))
    }
}

pub fn get_topic_setup_keyboard(auto_pr: bool) -> InlineKeyboardMarkup {
    let pr_status = if auto_pr { "ON" } else { "OFF" };
    let pr_val = if auto_pr { "1" } else { "0" };

    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            format!("Auto PR: {pr_status}"),
            format!("toggle_pr_{pr_val}"),
        )],
        vec![InlineKeyboardButton::callback(
            "Proceed ➔",
            "proceed_setup_new",
        )],
        vec![InlineKeyboardButton::callback(
            "Cancel / Delete Topic",
            "cancel_setup",
        )],
    ])
}

pub fn get_repo_picker_keyboard(
    sources: &[(String, String)], // (short_name, full_name)
) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();

    for (short_name, full_name) in sources {
        rows.push(vec![InlineKeyboardButton::callback(
            short_name.clone(),
            format!("repo_{full_name}"),
        )]);
    }

    rows.push(vec![InlineKeyboardButton::callback(
        "🔄 Refresh",
        "proceed_setup_new",
    )]);
    rows.push(vec![InlineKeyboardButton::callback(
        "Back",
        "back_to_setup",
    )]);

    InlineKeyboardMarkup::new(rows)
}

pub fn get_branch_picker_keyboard(branches: &[String]) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();

    for b in branches {
        rows.push(vec![InlineKeyboardButton::callback(
            b.clone(),
            format!("branch_{b}"),
        )]);
    }

    rows.push(vec![InlineKeyboardButton::callback(
        "Back",
        "proceed_setup_new",
    )]);

    InlineKeyboardMarkup::new(rows)
}

pub fn get_new_task_created_keyboard(
    group_id: &str,
    thread_id: i32,
) -> InlineKeyboardMarkup {
    let clean = clean_group_id(group_id);
    let topic_url = format!("https://t.me/c/{clean}/{thread_id}");
    let mut rows = Vec::new();

    if let Ok(url) = Url::parse(&topic_url) {
        rows.push(vec![InlineKeyboardButton::url("Go to Task ➔", url)]);
    }
    rows.push(vec![InlineKeyboardButton::callback("Back", "cancel_connect")]);

    InlineKeyboardMarkup::new(rows)
}

pub fn get_topic_closed_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "Yes, archive/delete session",
            "delete_session_yes",
        )],
        vec![InlineKeyboardButton::callback(
            "No, reopen topic",
            "delete_session_no",
        )],
    ])
}

pub fn get_status_card_keyboard(
    jules_url: &str,
    pr_url: Option<&str>,
) -> serde_json::Value {
    let open_btn = serde_json::json!({
        "text": "Open Session",
        "url": jules_url
    });

    let pr_btn = if let Some(url) = pr_url {
        serde_json::json!({
            "text": "View PR",
            "url": url
        })
    } else {
        serde_json::json!({
            "text": "No PR",
            "callback_data": "no_pr_alert"
        })
    };

    serde_json::json!({
        "inline_keyboard": [
            [open_btn, pr_btn]
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_group_id() {
        assert_eq!(clean_group_id("-1001234567890"), "1234567890");
        assert_eq!(clean_group_id("-987654321"), "-987654321");
    }

    #[test]
    fn test_status_card_keyboard() {
        let kb_no_pr = get_status_card_keyboard("https://jules.google.com/session/123", None);
        let rows = kb_no_pr["inline_keyboard"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        let buttons = rows[0].as_array().unwrap();
        assert_eq!(buttons.len(), 2);
        assert_eq!(buttons[0]["text"], "Open Session");
        assert_eq!(buttons[1]["text"], "No PR");

        let kb_with_pr = get_status_card_keyboard(
            "https://jules.google.com/session/123",
            Some("https://github.com/pull/1"),
        );
        let rows_pr = kb_with_pr["inline_keyboard"].as_array().unwrap();
        let buttons_pr = rows_pr[0].as_array().unwrap();
        assert_eq!(buttons_pr[1]["text"], "View PR");
        assert_eq!(buttons_pr[1]["url"], "https://github.com/pull/1");
    }
}

