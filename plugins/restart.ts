import { config } from '../src/config';
import { callApi } from '../src/api';
import { Router } from '../src/router';
import { Message } from '@grammyjs/types';

function getAdminId(): number | null {
  const adminIdStr = process.env.ADMIN_ID;
  if (!adminIdStr) return null;
  const id = parseInt(adminIdStr, 10);
  return isNaN(id) ? null : id;
}

export function setupPlugin(router: Router) {
  // Command handler
  router.onCommand('restart', async (msg: Message) => {
    const adminId = getAdminId();
    if (!adminId || msg.from?.id !== adminId) return;

    await callApi('sendMessage', {
      chat_id: msg.chat.id,
      text: "🔄 Restarting bot (SIGHUP)..."
    });
    console.log(`Restart command issued by admin ${msg.from.id}. Sending SIGHUP in 2 seconds...`);
    setTimeout(() => {
      process.kill(process.pid, 'SIGHUP');
    }, 2000);
  });

  // Startup hook
  const adminId = getAdminId();
  if (adminId) {
    callApi('sendMessage', {
      chat_id: adminId,
      text: "🚀 Bot restarted successfully!"
    }).then(() => {
      console.log(`Startup notification sent to admin ${adminId}.`);
    }).catch(e => {
      console.error(`Failed to send startup message to admin ${adminId}:`, e);
    });
  }
}
