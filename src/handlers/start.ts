import { Message, CallbackQuery, InlineKeyboardMarkup } from '@grammyjs/types';
import { router, FSMContext } from '../router';
import { DB } from '../db';
import { callApi } from '../api';

const STATES = {
  WAITING_FOR_API_KEY: 'waiting_for_api_key'
};

async function getStartKeyboard(isConnected: boolean, groupId: string | null): Promise<InlineKeyboardMarkup> {
  const connectBtnText = isConnected ? "Account Connected ✅" : "Connect Account";
  const groupBtnText = groupId ? "Group Connected ✅" : "Setup Group";

  const groupRow = [{ text: groupBtnText, callback_data: "setup_group" }];
  
  if (groupId) {
    let cleanId = groupId;
    if (cleanId.startsWith("-100")) {
      cleanId = cleanId.substring(4);
    }
    const groupUrl = `https://t.me/c/${cleanId}/1`;
    groupRow.push({ text: "Open Group", url: groupUrl } as any);
  }

  const keyboard: any[][] = [
    [{ text: connectBtnText, callback_data: "connect_account" }],
    groupRow,
  ];

  if (groupId) {
    keyboard.push([{ text: "New Task", callback_data: "new_task" }]);
  }

  return { inline_keyboard: keyboard };
}

function getCancelConnectKeyboard(): InlineKeyboardMarkup {
  return {
    inline_keyboard: [
      [{ text: "Back", callback_data: "cancel_connect" }]
    ]
  };
}

router.onCommand('start', async (msg: Message, state: FSMContext) => {
  if (msg.chat.type !== 'private') return;

  const ownerId = DB.getSetting('owner_id');
  const userIdStr = String(msg.from?.id);

  if (!ownerId) {
    DB.setSetting('owner_id', userIdStr);
  } else if (userIdStr !== ownerId) {
    await callApi('sendMessage', { chat_id: msg.chat.id, text: 'Sorry, this bot is already claimed by another user.' });
    return;
  }

  const apiKey = DB.getSetting('api_key');
  const isConnected = !!apiKey;
  const groupId = DB.getSetting('group_id');

  const keyboard = await getStartKeyboard(isConnected, groupId);

  await callApi('sendMessage', {
    chat_id: msg.chat.id,
    text: "Welcome to the Jules API Telegram Bot!\n\nUse the buttons below to configure the bot.",
    reply_markup: keyboard
  });
});

router.onCallback('connect_account', async (cb: CallbackQuery, state: FSMContext) => {
  if (!cb.message) return;
  
  await callApi('editMessageText', {
    chat_id: cb.message.chat.id,
    message_id: cb.message.message_id,
    text: "Please send me your Jules API Key.",
    reply_markup: getCancelConnectKeyboard()
  });

  state.setState(STATES.WAITING_FOR_API_KEY);
  state.updateData({ prompt_msg_id: cb.message.message_id });
  await callApi('answerCallbackQuery', { callback_query_id: cb.id });
});

router.onCallback('cancel_connect', async (cb: CallbackQuery, state: FSMContext) => {
  if (!cb.message) return;
  
  state.clear();
  const apiKey = DB.getSetting('api_key');
  const isConnected = !!apiKey;
  const groupId = DB.getSetting('group_id');

  const keyboard = await getStartKeyboard(isConnected, groupId);

  await callApi('editMessageText', {
    chat_id: cb.message.chat.id,
    message_id: cb.message.message_id,
    text: "Welcome to the Jules API Telegram Bot!\n\nUse the buttons below to configure the bot.",
    reply_markup: keyboard
  });
  await callApi('answerCallbackQuery', { callback_query_id: cb.id });
});

router.onState(STATES.WAITING_FOR_API_KEY, async (msg: Message, state: FSMContext) => {
  if (msg.chat.type !== 'private' || !msg.text) return;

  const data = state.getData();
  const promptMsgId = data.prompt_msg_id;

  if (promptMsgId) {
    try {
      await callApi('editMessageReplyMarkup', {
        chat_id: msg.chat.id,
        message_id: promptMsgId,
        reply_markup: null
      });
    } catch (e) {
      // Ignore if not modified
    }
  }

  DB.setSetting('api_key', msg.text.trim());
  const groupId = DB.getSetting('group_id');
  const keyboard = await getStartKeyboard(true, groupId);

  await callApi('sendMessage', {
    chat_id: msg.chat.id,
    text: "API Key saved! You can now proceed to setup a group.",
    reply_markup: keyboard
  });

  state.clear();
});

router.onCallback('setup_group', async (cb: CallbackQuery, state: FSMContext) => {
  if (!cb.message) return;

  DB.setSetting('ready_for_group', 'true');

  await callApi('editMessageText', {
    chat_id: cb.message.chat.id,
    message_id: cb.message.message_id,
    text: "Bot is now ready to be added to a group!\n\n(If you are moving me to a new group, I will try to leave the old one automatically. If I fail, please remove me manually.)\n\nPlease add me to a single group chat, and make sure the group has **Topics (Forum mode) enabled**.",
    parse_mode: "Markdown",
    reply_markup: getCancelConnectKeyboard()
  });
  await callApi('answerCallbackQuery', { callback_query_id: cb.id });
});

router.onCallback('new_task', async (cb: CallbackQuery, state: FSMContext) => {
  if (!cb.message) return;

  const groupId = DB.getSetting('group_id');
  if (!groupId) {
    await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "Please add me to a group first!", show_alert: true });
    return;
  }

  try {
    const topic = await callApi<any>('createForumTopic', { chat_id: groupId, name: "New Task" });
    await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: `Created new topic: ${topic.name}` });

    DB.createTopic(topic.message_thread_id);

    const { sendTopicSetupMenu } = await import('./topics.js');
    await sendTopicSetupMenu(Number(groupId), topic.message_thread_id, false);

    let cleanId = groupId;
    if (cleanId.startsWith("-100")) {
      cleanId = cleanId.substring(4);
    }
    const topicUrl = `https://t.me/c/${cleanId}/${topic.message_thread_id}`;

    const kb = {
      inline_keyboard: [
        [{ text: "Go to Task ➔", url: topicUrl }],
        [{ text: "Back", callback_data: "cancel_connect" }]
      ]
    };

    await callApi('editMessageText', {
      chat_id: cb.message.chat.id,
      message_id: cb.message.message_id,
      text: "new task created. go to task to continue",
      reply_markup: kb
    });
  } catch (e) {
    console.error("Failed to create topic:", e);
    await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "Failed to create topic. Make sure I have permissions to manage topics.", show_alert: true });
  }
});
