import asyncio
import logging
import os
from aiogram import Bot, Dispatcher
from aiogram.client.default import DefaultBotProperties
from aiogram.enums import ParseMode
from aiogram.client.session.aiohttp import AiohttpSession

from config import BOT_TOKEN
from database.sqlite_db import SQLiteDatabase

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

bot_kwargs = {"token": BOT_TOKEN, "default": DefaultBotProperties(parse_mode=ParseMode.HTML)}
proxy_url = os.environ.get("PROXY")
if proxy_url:
    bot_kwargs["session"] = AiohttpSession(proxy=proxy_url)

bot = Bot(**bot_kwargs)
dp = Dispatcher()
db = SQLiteDatabase("bot.db")

from handlers.start import router as start_router
from handlers.group import router as group_router
from handlers.topic_lifecycle import router as topic_lifecycle_router
from handlers.session_messaging import router as session_messaging_router

from poller import poll_activities


async def main():
    await db.connect()

    # Dynamic Plugin Discovery
    import importlib
    if os.path.exists("plugins"):
        for filename in sorted(os.listdir("plugins")):
            if filename.endswith(".py") and not filename.startswith("__"):
                module_name = f"plugins.{filename[:-3]}"
                try:
                    module = importlib.import_module(module_name)
                    if hasattr(module, "setup_plugin"):
                        module.setup_plugin(dp, bot)
                        logger.info(f"Loaded plugin (setup_plugin): {module_name}")
                    elif hasattr(module, "router"):
                        dp.include_router(module.router)
                        logger.info(f"Loaded plugin (router): {module_name}")
                except Exception as e:
                    logger.error(f"Failed to load plugin {module_name}: {e}")

    try:
        asyncio.create_task(poll_activities(bot, db))
        dp.include_router(start_router)
        dp.include_router(group_router)
        dp.include_router(topic_lifecycle_router)
        dp.include_router(session_messaging_router)
        await dp.start_polling(bot, db=db)
    finally:
        await db.disconnect()

if __name__ == "__main__":
    asyncio.run(main())
