use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use crate::bot::handlers::topic_setup::get_state_emoji;
use crate::bot::keyboards::get_status_card_keyboard;
use crate::jules::models::Activity;
use crate::jules::JulesApiClient;
use crate::state::AppState;
use crate::telegram_ext::TelegramExtClient;

pub async fn start_activity_poller(state: Arc<AppState>, ext: TelegramExtClient) {
    info!("Starting activity poller background loop...");

    loop {
        if let Err(e) = run_poller_cycle(&state, &ext).await {
            error!("Activity poller cycle error: {e}");
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn run_poller_cycle(
    state: &Arc<AppState>,
    ext: &TelegramExtClient,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let api_key = match state.db.get_setting("api_key")? {
        Some(k) if !k.trim().is_empty() => k,
        _ => {
            tokio::time::sleep(Duration::from_secs(10)).await;
            return Ok(());
        }
    };

    let group_id_str = match state.db.get_setting("group_id")? {
        Some(g) if !g.trim().is_empty() => g,
        _ => {
            tokio::time::sleep(Duration::from_secs(10)).await;
            return Ok(());
        }
    };

    let group_id = match group_id_str.parse::<i64>() {
        Ok(id) => id,
        Err(_) => return Ok(()),
    };

    let client = JulesApiClient::new(state.http_client.clone(), api_key);
    let active_topics = state.db.get_all_active_topics()?;

    for topic in active_topics {
        let session_id = match topic.session_id {
            Some(ref s) => s.clone(),
            None => continue,
        };
        let topic_id = topic.topic_id;
        let pinned_msg_id = topic.pinned_message_id;

        // 1. Fetch and update session status card
        if let Ok(session_data) = client.get_session(&session_id).await {
            let session_state = session_data
                .state
                .as_deref()
                .unwrap_or("UNKNOWN")
                .to_string();

            if let Some(msg_id) = pinned_msg_id {
                let title = session_data
                    .title
                    .as_deref()
                    .unwrap_or(&session_id);

                let clean_id = session_id
                    .split('/')
                    .last()
                    .unwrap_or(&session_id);

                let jules_url = session_data
                    .url
                    .unwrap_or_else(|| format!("https://jules.google.com/session/{clean_id}"));

                let auto_pr_str = if session_data.automation_mode.as_deref() == Some("AUTO_CREATE_PR") {
                    "`ON`"
                } else {
                    "`OFF`"
                };

                let state_emoji = get_state_emoji(&session_state);

                let mut pr_url = None;
                for out in &session_data.outputs {
                    if let Some(ref pr) = out.pull_request {
                        if let Some(ref url) = pr.url {
                            pr_url = Some(url.as_str());
                            break;
                        }
                    }
                }

                let rich_md = format!(
                    "\n# {title}\n\n| **Status** | **Auto PR** | **State** |\n|:---|:---|:---|\n| `Active` | {auto_pr_str} | {state_emoji} `{session_state}` |\n"
                );

                let kb = get_status_card_keyboard(&jules_url, pr_url);

                if let Err(e) = ext
                    .edit_rich_message(group_id, msg_id, &rich_md, Some(&kb))
                    .await
                {
                    let err_str = e.to_string();
                    if !err_str.contains("message is not modified") {
                        warn!("Failed to edit pinned message in topic {topic_id}: {err_str}");
                    }
                }
            }
        }

        // 2. Fetch new activities
        let last_time = state.last_seen_times.get(&session_id).map(|t| t.clone());

        match client.list_activities(&session_id, Some(20), None).await {
            Ok(act_res) => {
                let mut activities = act_res.activities;
                if !activities.is_empty() {
                    activities.sort_by(|a, b| {
                        a.create_time
                            .as_deref()
                            .unwrap_or_default()
                            .cmp(b.create_time.as_deref().unwrap_or_default())
                    });

                    for act in activities {
                        let act_time = match act.create_time {
                            Some(ref t) => t.clone(),
                            None => continue,
                        };

                        if let Some(ref last) = last_time {
                            if &act_time <= last {
                                continue;
                            }
                        }

                        if process_activity(ext, group_id, topic_id, &act).await {
                            state.last_seen_times.insert(session_id.clone(), act_time);
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Error fetching activities for {session_id}: {e}");
            }
        }
    }

    Ok(())
}

async fn process_activity(
    ext: &TelegramExtClient,
    chat_id: i64,
    topic_id: i32,
    act: &Activity,
) -> bool {
    // If it's a user message, skip as in the original code
    if act.user_messaged.is_some() {
        return true;
    }

    let mut is_important = false;
    let mut text = format!("🔹 {}", act.description.as_deref().unwrap_or(""));

    if act.session_completed.is_some() {
        is_important = true;
        text = "✅ **Session Completed!**".to_string();
    } else if let Some(ref failed) = act.session_failed {
        is_important = true;
        let reason = failed.reason.as_deref().unwrap_or("Unknown error");
        text = format!("❌ **Session Failed**\nReason: {reason}");
    } else if let Some(ref agent_msg) = act.agent_messaged {
        is_important = true;
        let msg = agent_msg.agent_message.as_deref().unwrap_or("");
        text = format!("🤖 **Jules:**\n{msg}");
    } else if let Some(ref plan_gen) = act.plan_generated {
        is_important = true;
        text = "📋 **Plan Generated**\n".to_string();
        if let Some(ref plan) = plan_gen.plan {
            for (idx, step) in plan.steps.iter().enumerate() {
                let title = step.title.as_deref().unwrap_or("");
                text.push_str(&format!("{}. {}\n", idx + 1, title));
            }
        }
    } else if act.plan_approved.is_some() {
        text = "✅ Plan Approved".to_string();
    } else if let Some(ref prog) = act.progress_updated {
        let title = prog.title.as_deref().unwrap_or("");
        let desc = prog.description.as_deref().unwrap_or("");
        text = format!("⏳ **Progress:** {title}\n_{desc}_");
    }

    // Process attached artifacts
    for art in &act.artifacts {
        if let Some(ref cs) = art.change_set {
            if let Some(ref patch_obj) = cs.git_patch {
                let mut patch = patch_obj
                    .unidiff_patch
                    .as_deref()
                    .unwrap_or("")
                    .to_string();

                if !patch.is_empty() {
                    let commit_msg = patch_obj
                        .suggested_commit_message
                        .as_deref()
                        .unwrap_or("Code changes generated");

                    if patch.len() > 3000 {
                        patch.truncate(3000);
                        patch.push_str("\n... (diff truncated)");
                    }

                    text.push_str(&format!(
                        "\n\n📝 **Code Changes Ready**\n_{commit_msg}_\n<details>\n<summary>View Changes</summary>\n\n```diff\n{patch}\n```\n</details>"
                    ));
                }
            }
        } else if let Some(ref bash) = art.bash_output {
            let cmd = bash.command.as_deref().unwrap_or("");
            let code = bash.exit_code.unwrap_or(0);
            text.push_str(&format!(
                "\n\n💻 **Command Executed:** `{cmd}`\nExit Code: {code}"
            ));
        }
    }

    match ext
        .send_rich_message(chat_id, Some(topic_id), &text, None, !is_important)
        .await
    {
        Ok(_) => true,
        Err(e) => {
            warn!("Failed to send activity message to topic {topic_id}: {e}");
            false
        }
    }
}
