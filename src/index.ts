import fs from 'fs';
import path from 'path';
import { startPolling } from './poller';
import { startActivityPoller } from './activity_poller';
import { router } from './router';
import { EnvHttpProxyAgent, setGlobalDispatcher } from 'undici';

if (process.env.http_proxy || process.env.https_proxy || process.env.HTTP_PROXY || process.env.HTTPS_PROXY) {
  setGlobalDispatcher(new EnvHttpProxyAgent());
}

import { DB } from './db';
import { callApi } from './api';

async function bootstrap() {
  console.log('Starting bot...');

  router.use(async (update, next) => {
    let chat;
    if (update.message) chat = update.message.chat;
    else if (update.callback_query?.message) chat = update.callback_query.message.chat;
    else if (update.my_chat_member) chat = update.my_chat_member.chat;
    else if (update.edited_message) chat = update.edited_message.chat;

    if (chat && ['group', 'supergroup'].includes(chat.type)) {
      const groupId = DB.getSetting("group_id");
      const readyForGroup = DB.getSetting("ready_for_group");
      
      // If this chat is not the authorized group, AND we aren't currently waiting to be added to a group
      if (String(chat.id) !== groupId && readyForGroup !== "true") {
        try {
          await callApi('sendMessage', { chat_id: chat.id, text: "I am only authorized to operate in my officially configured group. Leaving..." });
          await callApi('leaveChat', { chat_id: chat.id });
        } catch (e) {}
        return; // Drop update completely
      }
    }
    await next();
  });

  // Load all handlers dynamically from the handlers directory
  const handlersPath = path.resolve(__dirname, 'handlers');
  if (fs.existsSync(handlersPath)) {
    const files = fs.readdirSync(handlersPath);
    for (const file of files) {
      if (file.endsWith('.ts') || file.endsWith('.js')) {
        await import(path.join(handlersPath, file));
        console.log(`Loaded handler: ${file}`);
      }
    }
  }

  // Same for plugins directory to replicate dynamic plugin loading
  const pluginsPath = path.resolve(process.cwd(), 'plugins');
  if (fs.existsSync(pluginsPath)) {
    const files = fs.readdirSync(pluginsPath);
    for (const file of files) {
      if ((file.endsWith('.ts') || file.endsWith('.js')) && !file.startsWith('_')) {
        try {
          const plugin = await import(path.join(pluginsPath, file));
          if (typeof plugin.setupPlugin === 'function') {
             plugin.setupPlugin(router);
          }
          console.log(`Loaded plugin: ${file}`);
        } catch (err) {
          console.error(`Failed to load plugin ${file}:`, err);
        }
      }
    }
  }

  startActivityPoller().catch(console.error);
  await startPolling();
}

bootstrap().catch(console.error);
