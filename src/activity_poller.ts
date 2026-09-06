import { DB } from './db';
import { callApi } from './api';
import { JulesAPIClient } from './jules';

const lastSeenTimes: Record<string, string> = {};

export async function startActivityPoller() {
  console.log('Starting activity poller...');

  while (true) {
    try {
      const apiKey = DB.getSetting('api_key');
      if (!apiKey) {
        await new Promise(r => setTimeout(r, 10000));
        continue;
      }

      const client = new JulesAPIClient(apiKey);
      const groupId = DB.getSetting('group_id');
      if (!groupId) {
        await new Promise(r => setTimeout(r, 10000));
        continue;
      }

      const activeTopics = DB.getAllActiveTopics();

      for (const topic of activeTopics) {
        const sessionId = topic.session_id;
        const topicId = topic.topic_id;
        const pinnedMsgId = topic.pinned_message_id;

        // Fetch session state
        try {
          const sessionData = await client.getSession(sessionId);
          const state = sessionData.state || 'UNKNOWN';

          if (pinnedMsgId) {
            try {
              const title = sessionData.title || sessionId;
              const cleanId = sessionId.split('/').pop() || '';
              const julesUrl = sessionData.url || `https://jules.google.com/session/${cleanId}`;

              const autoPrStr = sessionData.automationMode === "AUTO_CREATE_PR" ? "`ON`" : "`OFF`";
              
              const emojiMap: Record<string, string> = {
                "QUEUED": "⏳", "PLANNING": "🧠", "AWAITING_PLAN_APPROVAL": "✋", 
                "AWAITING_USER_FEEDBACK": "💬", "IN_PROGRESS": "🔄", "PAUSED": "⏸️", 
                "FAILED": "❌", "COMPLETED": "✅"
              };
              const stateEmoji = emojiMap[state] || "🔵";

              const outputs = sessionData.outputs || [];
              let prUrl = null;
              for (const out of outputs) {
                if (out.pullRequest && out.pullRequest.url) {
                  prUrl = out.pullRequest.url;
                  break;
                }
              }

              const prButton = prUrl 
                ? { text: "View PR", url: prUrl }
                : { text: "No PR", callback_data: "no_pr_alert" };

              const kb = {
                inline_keyboard: [
                  [
                    { text: "Open Session", url: julesUrl },
                    prButton
                  ]
                ]
              };

              const richMd = `
# ${title}

| **Status** | **Auto PR** | **State** |
|:---|:---|:---|
| \`Active\` | ${autoPrStr} | ${stateEmoji} \`${state}\` |
`;

              await callApi('editMessageText', {
                chat_id: groupId,
                message_id: pinnedMsgId,
                rich_message: { markdown: richMd },
                reply_markup: kb
              });
            } catch (editErr: any) {
              if (!editErr.message.includes('message is not modified')) {
                console.error(`Failed to edit pinned message:`, editErr);
              }
            }
          }
        } catch (e) {
          console.error(`Error fetching session ${sessionId}:`, e);
        }

        // Fetch new activities
        const lastTime = lastSeenTimes[sessionId];
        try {
          const activitiesRes = await client.listActivities(sessionId, 20);
          let activities = activitiesRes.activities || [];

          if (activities.length > 0) {
            activities.sort((a: any, b: any) => (a.createTime || "").localeCompare(b.createTime || ""));

            for (const act of activities) {
              const actTime = act.createTime;
              if (lastTime && actTime <= lastTime) {
                continue;
              }

              await processActivity(groupId, topicId, act);
              lastSeenTimes[sessionId] = actTime;
            }
          }
        } catch (e) {
          console.error(`Error fetching activities for ${sessionId}:`, e);
        }
      }
    } catch (e) {
      console.error(`Activity poller loop error:`, e);
    }

    await new Promise(r => setTimeout(r, 5000));
  }
}

async function processActivity(chatId: string, topicId: number, activity: any) {
  let isImportant = false;
  let text = `🔹 ${activity.description || ""}`;

  if (activity.sessionCompleted) {
    isImportant = true;
    text = "✅ **Session Completed!**";
  } else if (activity.sessionFailed) {
    isImportant = true;
    const reason = activity.sessionFailed.reason || "Unknown error";
    text = `❌ **Session Failed**\nReason: ${reason}`;
  } else if (activity.agentMessaged) {
    isImportant = true;
    const msg = activity.agentMessaged.agentMessage || "";
    text = `🤖 **Jules:**\n${msg}`;
  } else if (activity.planGenerated) {
    isImportant = true;
    text = "📋 **Plan Generated**\n";
    const steps = activity.planGenerated.plan?.steps || [];
    steps.forEach((step: any, idx: number) => {
      text += `${idx + 1}. ${step.title || ''}\n`;
    });
  } else if (activity.planApproved) {
    text = "✅ Plan Approved";
  } else if (activity.progressUpdated) {
    const prog = activity.progressUpdated;
    const title = prog.title || "";
    const desc = prog.description || "";
    text = `⏳ **Progress:** ${title}\n_${desc}_`;
  } else if (activity.userMessaged) {
    return;
  }

  const artifacts = activity.artifacts || [];
  for (const art of artifacts) {
    if (art.changeSet) {
      let patch = art.changeSet.gitPatch?.unidiffPatch || "";
      if (patch) {
        const commitMsg = art.changeSet.gitPatch?.suggestedCommitMessage || "Code changes generated";
        if (patch.length > 3000) {
          patch = patch.substring(0, 3000) + "\n... (diff truncated)";
        }
        text += `\n\n📝 **Code Changes Ready**\n_${commitMsg}_\n`;
        text += `<details>\n<summary>View Changes</summary>\n\n\`\`\`diff\n${patch}\n\`\`\`\n</details>`;
      }
    } else if (art.bashOutput) {
      const cmd = art.bashOutput.command || "";
      const code = art.bashOutput.exitCode || 0;
      text += `\n\n💻 **Command Executed:** \`${cmd}\`\nExit Code: ${code}`;
    }
  }

  try {
    await callApi('sendRichMessage', {
      chat_id: chatId,
      message_thread_id: topicId,
      rich_message: { markdown: text },
      disable_notification: !isImportant
    });
  } catch (e) {
    console.error(`Failed to send activity message to ${topicId}:`, e);
  }
}
