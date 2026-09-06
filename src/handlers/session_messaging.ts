import { Update } from '@grammyjs/types';
import { router } from '../router';
import { DB } from '../db';
import { callApi } from '../api';
import { JulesAPIClient } from '../jules';

const pendingDeletions: Record<number, NodeJS.Timeout> = {};

router.on((u: Update) => !!u.message?.forum_topic_closed, async (update: Update) => {
  const message = update.message!;
  const topicId = message.message_thread_id;
  if (!topicId) return;

  const topicData = DB.getTopic(topicId);
  if (!topicData || !topicData.session_id) return;

  DB.updateTopicState(topicId, 'closed');

  const kb = {
    inline_keyboard: [
      [{ text: "Yes, archive/delete session", callback_data: "delete_session_yes" }],
      [{ text: "No, reopen topic", callback_data: "delete_session_no" }]
    ]
  };

  const msg = await callApi<any>('sendMessage', {
    chat_id: message.chat.id,
    message_thread_id: topicId,
    text: "You closed this topic. Do you want to archive/delete the associated Jules session as well?\n(If no response in 5 minutes, I will reopen the topic)",
    reply_markup: kb
  });

  const timer = setTimeout(async () => {
    delete pendingDeletions[topicId];
    try {
      await callApi('reopenForumTopic', { chat_id: message.chat.id, message_thread_id: topicId });
      DB.updateTopicState(topicId, 'open');
      await callApi('editMessageText', {
        chat_id: message.chat.id,
        message_id: msg.message_id,
        text: "Time's up! Reopened the topic."
      });
    } catch (e) {
      console.error(`Failed to reopen topic ${topicId}:`, e);
    }
  }, 300000); // 5 minutes

  pendingDeletions[topicId] = timer;
});

router.onCallback('delete_session_', async (cb, state) => {
  if (!cb.message || !cb.message.message_thread_id) return;
  const topicId = cb.message.message_thread_id;
  const choice = cb.data!.split("_").pop();

  if (pendingDeletions[topicId]) {
    clearTimeout(pendingDeletions[topicId]);
    delete pendingDeletions[topicId];
  }

  if (choice === "yes") {
    const topicData = DB.getTopic(topicId);
    const sessionId = topicData?.session_id;

    if (sessionId) {
      const apiKey = DB.getSetting("api_key");
      if (!apiKey) return;
      const client = new JulesAPIClient(apiKey);
      try {
        await client.deleteSession(sessionId);
        await callApi('editMessageText', {
          chat_id: cb.message.chat.id,
          message_id: cb.message.message_id,
          text: "Session has been deleted. The topic will remain closed."
        });
        DB.updateTopicState(topicId, 'deleted');
      } catch (e) {
        console.error(`Failed to delete session ${sessionId}:`, e);
        await callApi('editMessageText', {
          chat_id: cb.message.chat.id,
          message_id: cb.message.message_id,
          text: "Failed to delete session via API."
        });
      }
    }
  } else {
    try {
      await callApi('reopenForumTopic', { chat_id: cb.message.chat.id, message_thread_id: topicId });
      DB.updateTopicState(topicId, 'open');
      await callApi('editMessageText', {
        chat_id: cb.message.chat.id,
        message_id: cb.message.message_id,
        text: "Topic reopened."
      });
    } catch (e) {
      console.error(`Failed to reopen topic ${topicId}:`, e);
      await callApi('editMessageText', {
        chat_id: cb.message.chat.id,
        message_id: cb.message.message_id,
        text: "Failed to reopen topic."
      });
    }
  }
});

router.on((u: Update) => !!u.message?.forum_topic_reopened, async (update: Update) => {
  const message = update.message!;
  const topicId = message.message_thread_id;
  if (!topicId) return;

  if (pendingDeletions[topicId]) {
    clearTimeout(pendingDeletions[topicId]);
    delete pendingDeletions[topicId];
  }

  DB.updateTopicState(topicId, 'open');
});

// Generic text messages handler for sessions
router.on((u: Update) => {
  if (!u.message || !u.message.text || u.message.text.startsWith('/')) return false;
  return !!u.message.is_topic_message;
}, async (update: Update) => {
  const message = update.message!;
  const ownerId = DB.getSetting("owner_id");

  if (String(message.from?.id) !== ownerId) return;

  const topicId = message.message_thread_id;
  if (!topicId) return;

  const topicData = DB.getTopic(topicId);
  if (!topicData || !topicData.session_id || topicData.state !== 'open') return;

  const sessionId = topicData.session_id;
  const apiKey = DB.getSetting("api_key");
  if (!apiKey) return;
  const client = new JulesAPIClient(apiKey);

  try {
    await client.sendMessage(sessionId, message.text!);
  } catch (e) {
    console.error(`Failed to send message to session ${sessionId}:`, e);
    await callApi('sendMessage', {
      chat_id: message.chat.id,
      message_thread_id: topicId,
      text: "Failed to send message to Jules."
    });
  }
});
