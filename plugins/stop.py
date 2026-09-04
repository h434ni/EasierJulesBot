"""
Modular Stop Command Plugin for aiogram 3.

Description:
    Provides a standalone /stop command informing the administrator that the bot
    process is monitored by a supervisor and advising using /restart instead.

Portability:
    To use this in any aiogram 3 project, copy this file into your project and either:
    1. Register the router manually:
       from plugins.stop import router as stop_router
       dp.include_router(stop_router)

    2. Or use a dynamic plugin loader that calls `setup_plugin(dp, bot)`.

Environment Variables:
    ADMIN_ID: (Required) Telegram User ID of the administrator (e.g. 123456789).
"""

import logging
import os
from aiogram import Bot, Dispatcher, Router
from aiogram.filters import Command
from aiogram.types import Message

logger = logging.getLogger(__name__)

router = Router(name="stop")


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


@router.message(Command("stop"), is_admin)
async def stop_cmd(message: Message):
    """Handles the /stop command."""
    await message.reply("Please use /restart instead, since the server automatically restarts the bot anyway.")


def setup_plugin(dp: Dispatcher, bot: Bot = None):
    """Entrypoint for dynamic plugin loaders."""
    dp.include_router(router)
