import { config } from './config';

const BASE_URL = `https://api.telegram.org/bot${config.BOT_TOKEN}`;

export async function callApi<T = any>(method: string, payload?: Record<string, any>): Promise<T> {
  const url = `${BASE_URL}/${method}`;
  
  const options: RequestInit = {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
  };

  if (payload) {
    options.body = JSON.stringify(payload);
  }

  // Handle Proxy if needed - Node 18+ native fetch doesn't natively support proxies well
  // In a real prod scenario, you'd use a proxy-agent. Here we ignore proxy for simplicity 
  // or use an undici dispatcher if needed.

  const response = await fetch(url, options);
  const data = await response.json();

  if (!data.ok) {
    throw new Error(`Telegram API Error: [${data.error_code}] ${data.description}`);
  }

  return data.result as T;
}
