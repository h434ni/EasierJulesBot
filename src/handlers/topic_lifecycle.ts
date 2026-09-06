import { Update } from '@grammyjs/types';
import { router } from '../router';
import { DB } from '../db';
import { callApi } from '../api';
import { JulesAPIClient } from '../jules';
import { sendTopicSetupMenu, getTopicSetupKeyboard } from './topics';

const setupStates: Record<number, any> = {};

router.on((u: Update) => !!u.message?.forum_topic_created, async (update: Update) => {
  const message = update.message!;
  const ownerId = DB.getSetting("owner_id");

  if (String(message.from?.id) !== ownerId) return;

  const topicId = message.message_thread_id;
  if (!topicId) return;

  DB.createTopic(topicId);
  setupStates[topicId] = { auto_pr: false };
  await sendTopicSetupMenu(message.chat.id, topicId, false);
});

router.onCallback('toggle_pr_', async (cb, state) => {
  if (!cb.message) return;
  const topicId = cb.message.message_thread_id;
  if (!topicId) return;

  const currentVal = Boolean(parseInt(cb.data!.split("_").pop() || "0"));
  const newVal = !currentVal;

  const topicState = setupStates[topicId] || { auto_pr: false };
  topicState.auto_pr = newVal;
  setupStates[topicId] = topicState;

  await callApi('editMessageReplyMarkup', {
    chat_id: cb.message.chat.id,
    message_id: cb.message.message_id,
    reply_markup: getTopicSetupKeyboard(topicState.auto_pr)
  });
  await callApi('answerCallbackQuery', { callback_query_id: cb.id });
});

router.onCallback('cancel_setup', async (cb, state) => {
  if (!cb.message || !cb.message.message_thread_id) return;
  const topicId = cb.message.message_thread_id;

  DB.deleteTopic(topicId);
  delete setupStates[topicId];

  await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "Deleting topic..." });
  try {
    await callApi('deleteForumTopic', { chat_id: cb.message.chat.id, message_thread_id: topicId });
  } catch (e) {
    console.error(`Failed to delete topic ${topicId}:`, e);
    await callApi('editMessageText', {
      chat_id: cb.message.chat.id,
      message_id: cb.message.message_id,
      text: "Failed to delete topic. Please delete it manually."
    });
  }
});

router.onCallback('proceed_setup_new', async (cb, state) => {
  if (!cb.message) return;
  const apiKey = DB.getSetting("api_key");

  if (!apiKey) {
    await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "API Key not found. Connect account first.", show_alert: true });
    return;
  }

  const client = new JulesAPIClient(apiKey);
  try {
    const sourcesData = await client.listSources(100);
    const sources = sourcesData.sources || [];

    if (sources.length === 0) {
      await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "No repositories found in your Jules account.", show_alert: true });
      return;
    }

    const kb: any[][] = [];
    for (const s of sources) {
      const name = s.id || s.name || '';
      const shortName = name.replace('github-', '');
      kb.push([{ text: shortName, callback_data: `repo_${name}` }]);
    }

    kb.push([{ text: "🔄 Refresh", callback_data: "proceed_setup_new" }]);
    kb.push([{ text: "Back", callback_data: "back_to_setup" }]);

    await callApi('editMessageText', {
      chat_id: cb.message.chat.id,
      message_id: cb.message.message_id,
      text: "Select a repository:",
      reply_markup: { inline_keyboard: kb }
    });
  } catch (e) {
    console.error(`Error fetching sources:`, e);
    await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "Error fetching repositories.", show_alert: true });
  }
});

router.onCallback('repo_', async (cb, state) => {
  if (!cb.message || !cb.message.message_thread_id) return;
  const repoName = cb.data!.substring(5);
  const topicId = cb.message.message_thread_id;

  const topicState = setupStates[topicId] || {};
  topicState.selected_repo = repoName;
  setupStates[topicId] = topicState;

  const apiKey = DB.getSetting("api_key");
  if (!apiKey) return;

  const client = new JulesAPIClient(apiKey);
  try {
    const sourceData = await client.getSource(repoName);
    const branches = sourceData.githubRepo?.branches || [];

    if (branches.length === 0) {
      await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "No branches found for this repository.", show_alert: true });
      return;
    }

    const kb: any[][] = [];
    for (const b of branches) {
      const branchName = b.displayName;
      kb.push([{ text: branchName, callback_data: `branch_${branchName}` }]);
    }
    kb.push([{ text: "Back", callback_data: "proceed_setup_new" }]);

    await callApi('editMessageText', {
      chat_id: cb.message.chat.id,
      message_id: cb.message.message_id,
      text: "Select a branch:",
      reply_markup: { inline_keyboard: kb }
    });
  } catch (e) {
    console.error(`Error fetching branches:`, e);
    await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "Error fetching branches.", show_alert: true });
  }
});

router.onCallback('branch_', async (cb, state) => {
  if (!cb.message || !cb.message.message_thread_id) return;
  const branchName = cb.data!.substring(7);
  const topicId = cb.message.message_thread_id;

  const topicState = setupStates[topicId] || {};
  topicState.selected_branch = branchName;
  
  const repoName = topicState.selected_repo || "Unknown";
  const shortRepo = repoName.split("/").pop()?.replace("github-", "");

  await callApi('editMessageText', {
    chat_id: cb.message.chat.id,
    message_id: cb.message.message_id,
    text: `Selected Repo: \`${shortRepo}\`\nSelected Branch: \`${branchName}\`\n\nPlease send the initialization message (prompt) to start the session.`,
    parse_mode: "Markdown"
  });

  topicState.waiting_for_prompt = true;
  setupStates[topicId] = topicState;
});

// We handle prompt responses in a generic filter
router.on((u) => {
  if (!u.message || !u.message.text || !u.message.message_thread_id) return false;
  const state = setupStates[u.message.message_thread_id];
  return state && state.waiting_for_prompt === true;
}, async (update) => {
  const message = update.message!;
  const topicId = message.message_thread_id!;
  const topicState = setupStates[topicId] || {};

  const apiKey = DB.getSetting("api_key");
  if (!apiKey) return;
  const client = new JulesAPIClient(apiKey);

  const repoName = topicState.selected_repo;
  const branchName = topicState.selected_branch;
  const autoPr = topicState.auto_pr || false;

  const statusMsg = await callApi<any>('sendMessage', {
    chat_id: message.chat.id,
    message_thread_id: topicId,
    text: "Starting session..."
  });

  try {
    const sessionData = await client.createSession(message.text!, repoName, branchName, autoPr);
    const sessionId = sessionData.name;

    DB.updateTopicSession(topicId, sessionId);
    DB.updateTopicAutoPr(topicId, autoPr);

    const shortRepo = repoName.split("/").pop()?.replace("github-", "") || '';
    const cleanId = sessionId.split("/").pop() || '';
    const julesUrl = sessionData.url || `https://jules.google.com/session/${cleanId}`;

    const autoPrStr = autoPr ? "`ON`" : "`OFF`";
    const stateStr = sessionData.state || 'QUEUED';
    
    const emojiMap: Record<string, string> = {
      "QUEUED": "⏳", "PLANNING": "🧠", "AWAITING_PLAN_APPROVAL": "✋", 
      "AWAITING_USER_FEEDBACK": "💬", "IN_PROGRESS": "🔄", "PAUSED": "⏸️", 
      "FAILED": "❌", "COMPLETED": "✅"
    };
    const stateEmoji = emojiMap[stateStr] || "🔵";

    const richMd = `
# ${sessionId}

| **Status** | **Auto PR** | **Repo** | **Branch** | **State** |
|:---|:---|:---|:---|:---|
| \`Active\` | ${autoPrStr} | \`${shortRepo}\` | \`${branchName}\` | ${stateEmoji} \`${stateStr}\` |
`;
    
    // sendRichMessage is supported in recent Bot APIs. 
    // For raw Telegram API, sendRichMessage isn't broadly standard if not yet rolled out globally or if they don't have access.
    // Assuming they have it or it's similar:
    const pinnedMsg = await callApi<any>('sendRichMessage', {
      chat_id: message.chat.id,
      message_thread_id: topicId,
      rich_message: { markdown: richMd },
      reply_markup: {
        inline_keyboard: [
          [
            { text: "Open Session", url: julesUrl },
            { text: "No PR", callback_data: "no_pr_alert" }
          ]
        ]
      }
    });

    await callApi('pinChatMessage', { chat_id: message.chat.id, message_id: pinnedMsg.message_id });
    DB.updateTopicPinnedMessage(topicId, pinnedMsg.message_id);

    await callApi('deleteMessage', { chat_id: message.chat.id, message_id: statusMsg.message_id });
    delete setupStates[topicId];

  } catch (e) {
    console.error(`Error creating session:`, e);
    await callApi('editMessageText', {
      chat_id: message.chat.id,
      message_id: statusMsg.message_id,
      text: `Error creating session: ${e}`
    });
  }
});

router.onCallback('no_pr_alert', async (cb) => {
  await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "There is no PR yet.", show_alert: true });
});

router.onCallback('back_to_setup', async (cb) => {
  if (!cb.message || !cb.message.message_thread_id) return;
  const topicId = cb.message.message_thread_id;
  const topicState = setupStates[topicId] || { auto_pr: false };

  await callApi('editMessageText', {
    chat_id: cb.message.chat.id,
    message_id: cb.message.message_id,
    text: "Please configure this topic:",
    reply_markup: getTopicSetupKeyboard(topicState.auto_pr)
  });
});
