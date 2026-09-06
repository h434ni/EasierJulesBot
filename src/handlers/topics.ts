import { InlineKeyboardMarkup } from '@grammyjs/types';
import { callApi } from '../api';

export function getTopicSetupKeyboard(autoPr: boolean = false): InlineKeyboardMarkup {
  const btnAutoPr = { text: `Auto PR: ${autoPr ? 'ON' : 'OFF'}`, callback_data: `toggle_pr_${autoPr ? '1' : '0'}` };
  const btnProceed = { text: "Proceed ➔", callback_data: "proceed_setup_new" };
  const btnCancel = { text: "Cancel / Delete Topic", callback_data: "cancel_setup" };

  return {
    inline_keyboard: [
      [btnAutoPr],
      [btnProceed],
      [btnCancel]
    ]
  };
}

export async function sendTopicSetupMenu(chatId: number, messageThreadId: number, autoPr: boolean = false) {
  await callApi('sendMessage', {
    chat_id: chatId,
    message_thread_id: messageThreadId,
    text: "Please configure this topic:",
    reply_markup: getTopicSetupKeyboard(autoPr)
  });
}
