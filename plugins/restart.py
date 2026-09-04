"""
Modular Restart Command Plugin for aiogram 3.

Description:
    Provides a standalone /restart command that sends SIGHUP to the current process,
    allowing process supervisors (such as systemd, Docker, or custom supervisors)
    to reload the bot cleanly. Also registers a startup hook to send a confirmation
    notification to the admin when the bot starts up.

Portability:
    To use this in any aiogram 3 project, copy this file into your project and either:
    1. Register the router manually:
       from plugins.restart import router as restart_router, register_startup_hook
       dp.include_router(restart_router)
       register_startup_hook(dp)

    2. Or use a dynamic plugin loader that calls `setup_plugin(dp, bot)`.

Environment Variables:
    ADMIN_ID: (Required) Telegram User ID of the administrator (e.g. 123456789).
"""

import logging
import os
import signal
from aiogram import Bot, Dispatcher, Router
from aiogram.filters import Command
from aiogram.types import Message

logger = logging.getLogger(__name__)

router = Router(name="restart")


def _get_admin_id() -> int | None:
    """Retrieve and validate the ADMIN_ID from the environment."""
    admin_id_str = os.environ.get("ADMIN_ID")
    if not admin_id_str:
        return None
    try:
        return int(admin_id_str)
    except ValueError:
        logger.error("ADMIN_ID '%s' is not a valid integer.", admin_id_str)
        return None


def is_admin(message: Message) -> bool:
    """Dynamic admin filter that resolves ADMIN_ID at request time."""
    admin_id = _get_admin_id()
    return bool(admin_id and message.from_user and message.from_user.id == admin_id)


@router.message(Command("restart"), is_admin)
async def restart_cmd(message: Message):
    """Handles the /restart command for authorized admins."""
    await message.reply("🔄 Restarting bot (SIGHUP)...")
    logger.info("Restart command issued by admin %s. Sending SIGHUP...", message.from_user.id)
    os.kill(os.getpid(), signal.SIGHUP)


def register_startup_hook(dp: Dispatcher):
    """Registers a startup notification hook that alerts the admin when the bot boots."""
    async def on_startup(bot: Bot):
        admin_id = _get_admin_id()
        if not admin_id:
            logger.debug("ADMIN_ID not set; skipping startup restart notification.")
            return

        try:
            await bot.send_message(chat_id=admin_id, text="🚀 Bot restarted successfully!")
            logger.info("Startup notification sent to admin %s.", admin_id)
        except Exception as e:
            logger.error("Failed to send startup message to admin %s: %s", admin_id, e)

    dp.startup.register(on_startup)


def setup_plugin(dp: Dispatcher, bot: Bot = None):
    """Entrypoint for dynamic plugin loaders."""
    dp.include_router(router)
    register_startup_hook(dp)
