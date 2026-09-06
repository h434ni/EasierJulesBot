import { Router } from '../src/router';
import { Message } from '@grammyjs/types';
import { callApi } from '../src/api';

function getAdminId(): number | null {
  const adminIdStr = process.env.ADMIN_ID;
  if (!adminIdStr) return null;
  const id = parseInt(adminIdStr, 10);
  return isNaN(id) ? null : id;
}

export function setupPlugin(router: Router) {
  router.onCommand('stop', async (msg: Message) => {
    const adminId = getAdminId();
    if (!adminId || msg.from?.id !== adminId) return;

    console.log(`Stop command issued by admin ${msg.from.id}. Sending SIGINT in 2 seconds...`);
    setTimeout(() => {
      process.kill(process.pid, 'SIGINT');
    }, 2000);

    await callApi('sendMessage', {
      chat_id: msg.chat.id,
      text: "Please use /restart instead, since the server automatically restarts the bot anyway."
    });
  });
}
