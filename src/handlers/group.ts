import { Update, ChatMember, InlineKeyboardMarkup, ChatMemberUpdated } from '@grammyjs/types';
import { router } from '../router';
import { DB } from '../db';
import { callApi } from '../api';

function getAdminCheckKeyboard(botMember: ChatMember, isForum: boolean): InlineKeyboardMarkup | undefined {
  const isAdmin = botMember.status === "administrator";
  const canManageTopics = isAdmin && (botMember as any).can_manage_topics === true;
  const canPinMessages = isAdmin && (botMember as any).can_pin_messages === true;

  const keyboard: any[][] = [];

  if (!(isAdmin && canManageTopics && canPinMessages)) {
    if (isAdmin) {
      keyboard.push([{ text: "Permissions missing ⚠️", callback_data: "admin_missing_perms" }]);
    } else {
      keyboard.push([{ text: "Bot is not Admin 🚫", callback_data: "admin_not_admin" }]);
    }
  }

  if (!isForum) {
    keyboard.push([{ text: "Topics not enabled 🚫", callback_data: "forum_not_enabled" }]);
  }

  if (keyboard.length === 0) {
    return undefined;
  }

  return { inline_keyboard: keyboard };
}

// Handler for my_chat_member updates
router.on((u: Update) => !!u.my_chat_member, async (update: Update) => {
  const event = update.my_chat_member!;
  
  // IS_NOT_MEMBER >> IS_MEMBER
  const wasNotMember = ['left', 'kicked', 'restricted'].includes(event.old_chat_member.status) || 
                       (event.old_chat_member.status === 'member' && (event.old_chat_member as any).is_member === false);
  const isMember = event.new_chat_member.status === 'member' || event.new_chat_member.status === 'administrator';

  if (wasNotMember && isMember) {
    if (!['group', 'supergroup'].includes(event.chat.type)) return;

    if (event.chat.type !== 'supergroup') {
      await callApi('sendMessage', { chat_id: event.chat.id, text: "this group is not a supergroup. make it supergroup by enabling topics then add me again" });
      await callApi('leaveChat', { chat_id: event.chat.id });
      return;
    }

    const ownerId = DB.getSetting("owner_id");
    const readyForGroup = DB.getSetting("ready_for_group");

    if (String(event.from.id) !== ownerId) {
      await callApi('sendMessage', { chat_id: event.chat.id, text: "I can only be added by my owner. Leaving..." });
      await callApi('leaveChat', { chat_id: event.chat.id });
      return;
    }

    if (readyForGroup !== "true") {
      await callApi('sendMessage', { chat_id: event.chat.id, text: "Please use 'Setup Group' in private chat before adding me. Leaving..." });
      await callApi('leaveChat', { chat_id: event.chat.id });
      return;
    }

    const existingGroup = DB.getSetting("group_id");
    if (existingGroup && existingGroup !== String(event.chat.id)) {
      try {
        await callApi('sendMessage', { chat_id: existingGroup, text: "I am being moved to another group by my owner. Goodbye!" });
        await callApi('leaveChat', { chat_id: existingGroup });
      } catch (e) {
        console.error(`Failed to leave old group ${existingGroup}:`, e);
      }
    }

    DB.setSetting("group_id", String(event.chat.id));
    DB.setSetting("ready_for_group", "false");

    const isForum = (event.chat as any).is_forum || false;
    const botMemberResponse = await callApi<{ status: string }>('getChatMember', { chat_id: event.chat.id, user_id: event.new_chat_member.user.id });
    
    await callApi('sendMessage', {
      chat_id: event.chat.id,
      text: "Hello! I am ready to manage Jules API tasks here.",
      reply_markup: getAdminCheckKeyboard(botMemberResponse as ChatMember, isForum)
    });
  }
});

router.onCallback('admin_', async (cb, state) => {
  if (!cb.message) return;
  const botInfo = await callApi<any>('getMe');
  const botId = botInfo.id;

  if (cb.data === "admin_ok") {
    await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "Bot is admin with all required permissions!", show_alert: true });
  } else if (cb.data === "admin_not_admin") {
    const botMember = await callApi<ChatMember>('getChatMember', { chat_id: cb.message.chat.id, user_id: botId });
    const isForum = (cb.message.chat as any).is_forum || false;

    if (botMember.status === "administrator") {
      try {
        await callApi('editMessageReplyMarkup', {
          chat_id: cb.message.chat.id,
          message_id: cb.message.message_id,
          reply_markup: getAdminCheckKeyboard(botMember, isForum)
        });
      } catch (e) {}
      await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "Status updated!" });
    } else {
      await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "Please make the bot an Admin.", show_alert: true });
    }
  } else if (cb.data === "admin_missing_perms") {
    const botMember = await callApi<ChatMember>('getChatMember', { chat_id: cb.message.chat.id, user_id: botId });
    const canManageTopics = (botMember as any).can_manage_topics || false;
    const canPinMessages = (botMember as any).can_pin_messages || false;
    const isForum = (cb.message.chat as any).is_forum || false;

    if (canManageTopics && canPinMessages) {
      try {
        await callApi('editMessageReplyMarkup', {
          chat_id: cb.message.chat.id,
          message_id: cb.message.message_id,
          reply_markup: getAdminCheckKeyboard(botMember, isForum)
        });
      } catch(e) {}
      await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "Permissions updated!" });
    } else {
      const missing = [];
      if (!canManageTopics) missing.push("Manage Topics");
      if (!canPinMessages) missing.push("Pin Messages");
      await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: `Missing permissions:\n${missing.join('\n')}`, show_alert: true });
    }
  }
});

router.onCallback('forum_not_enabled', async (cb, state) => {
  if (!cb.message) return;
  const botInfo = await callApi<any>('getMe');
  const chat = await callApi<any>('getChat', { chat_id: cb.message.chat.id });
  const isForum = chat.is_forum || false;

  if (isForum) {
    const botMember = await callApi<ChatMember>('getChatMember', { chat_id: cb.message.chat.id, user_id: botInfo.id });
    try {
      await callApi('editMessageReplyMarkup', {
        chat_id: cb.message.chat.id,
        message_id: cb.message.message_id,
        reply_markup: getAdminCheckKeyboard(botMember, isForum)
      });
    } catch(e) {}
    await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "Topics are now enabled!", show_alert: true });
  } else {
    await callApi('answerCallbackQuery', { callback_query_id: cb.id, text: "Please go to Group Settings -> Enable Topics.", show_alert: true });
  }
});
