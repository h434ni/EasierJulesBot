use std::sync::Arc;
use teloxide::types::Chat;
use tracing::warn;
use crate::state::AppState;
use crate::telegram_ext::TelegramExtClient;

pub async fn is_authorized_chat(
    chat: &Chat,
    state: &Arc<AppState>,
    ext: &TelegramExtClient,
) -> bool {
    if chat.is_private() {
        return true;
    }

    let chat_id = chat.id.0;
    let group_id = state.db.get_setting("group_id").ok().flatten();
    let ready_for_group = state.db.get_setting("ready_for_group").ok().flatten();

    let chat_id_str = chat_id.to_string();
    let is_configured_group = group_id.as_deref() == Some(&chat_id_str);
    let is_ready = ready_for_group.as_deref() == Some("true");

    if !is_configured_group && !is_ready {
        warn!("Dropping update from unauthorized group {chat_id}; leaving chat");
        let msg_payload = serde_json::json!({
            "chat_id": chat_id,
            "text": "I am only authorized to operate in my officially configured group. Leaving..."
        });
        let _ = ext.call_api::<serde_json::Value>("sendMessage", Some(&msg_payload)).await;
        let _ = ext.leave_chat(chat_id).await;
        return false;
    }

    true
}
