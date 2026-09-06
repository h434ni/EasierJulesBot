import fs from 'fs';
import path from 'path';
import { startPolling } from './poller';
import { startActivityPoller } from './activity_poller';
import { router } from './router';
import { EnvHttpProxyAgent, setGlobalDispatcher } from 'undici';

if (process.env.http_proxy || process.env.https_proxy || process.env.HTTP_PROXY || process.env.HTTPS_PROXY) {
  setGlobalDispatcher(new EnvHttpProxyAgent());
}

async function bootstrap() {
  console.log('Starting bot...');

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
