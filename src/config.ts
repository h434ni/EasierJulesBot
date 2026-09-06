import dotenv from 'dotenv';
dotenv.config();

export const config = {
  BOT_TOKEN: process.env.BOT_TOKEN || '',
  PROXY: process.env.PROXY || '',
};

if (!config.BOT_TOKEN) {
  throw new Error("BOT_TOKEN is required in .env");
}
