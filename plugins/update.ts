import { Router } from '../src/router';
import { Message } from '@grammyjs/types';
import { callApi } from '../src/api';
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

function getAdminId(): number | null {
  const adminIdStr = process.env.ADMIN_ID;
  if (!adminIdStr) return null;
  const id = parseInt(adminIdStr, 10);
  return isNaN(id) ? null : id;
}

function escapeHtml(unsafe: string) {
  return unsafe
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

export function setupPlugin(router: Router) {
  router.onCommand('update', async (msg: Message) => {
    const adminId = getAdminId();
    if (!adminId || msg.from?.id !== adminId) return;

    const repoUrl = process.env.GITHUB_REPO;
    const pullCmd = repoUrl ? `git pull ${repoUrl}` : "git pull";
    const targetInfo = repoUrl ? ` from ${repoUrl}` : "";

    const statusMsg = await callApi<any>('sendMessage', {
      chat_id: msg.chat.id,
      text: `⬇️ Pulling updates${targetInfo}...`
    });

    try {
      const { stdout, stderr } = await execAsync(pullCmd);
      const out = stdout.trim();
      const err = stderr.trim();

      let text = "<b>Git Pull Result:</b>\n";
      if (out) text += `<code>${escapeHtml(out)}</code>\n`;
      if (err) text += `<i>Errors/Warnings:</i>\n<code>${escapeHtml(err)}</code>\n`;

      await callApi('editMessageText', {
        chat_id: msg.chat.id,
        message_id: statusMsg.message_id,
        text,
        parse_mode: 'HTML'
      });
    } catch (e: any) {
      console.error("Failed to execute git pull", e);
      await callApi('editMessageText', {
        chat_id: msg.chat.id,
        message_id: statusMsg.message_id,
        text: `❌ Failed to pull: <code>${escapeHtml(String(e))}</code>`,
        parse_mode: 'HTML'
      });
    }
  });
}
