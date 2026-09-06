import { Update } from '@grammyjs/types';
import { callApi } from './api';
import { router } from './router';

export async function startPolling() {
  let offset = 0;
  console.log('Bot is polling...');

  while (true) {
    try {
      const updates = await callApi<Update[]>('getUpdates', {
        offset,
        timeout: 30,
        allowed_updates: ['message', 'callback_query', 'my_chat_member']
      });

      for (const update of updates) {
        offset = update.update_id + 1;
        try {
          await router.handleUpdate(update);
        } catch (err) {
          console.error(`Error handling update ${update.update_id}:`, err);
        }
      }
    } catch (err) {
      console.error('Polling error:', err);
      // Wait a bit before retrying to avoid spamming on network error
      await new Promise(resolve => setTimeout(resolve, 3000));
    }
  }
}
