"""
Modular Git Update Command Plugin for aiogram 3.

Description:
    Provides a standalone /update command that executes `git pull` asynchronously
    and returns formatted status, standard output, and error streams back to the admin.

Portability:
    To use this in any aiogram 3 project, copy this file into your project and either:
    1. Register the router manually:
       from plugins.update import router as update_router
       dp.include_router(update_router)

    2. Or use a dynamic plugin loader that calls `setup_plugin(dp, bot)`.

Environment Variables:
    ADMIN_ID: (Required) Telegram User ID of the administrator (e.g. 123456789).
    GITHUB_REPO: (Optional) Repository URL/remote to pull from. Defaults to standard `git pull`.
"""

import asyncio
import html
import logging
import os
from aiogram import Bot, Dispatcher, Router
from aiogram.filters import Command
from aiogram.types import Message

logger = logging.getLogger(__name__)

router = Router(name="update")


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


@router.message(Command("update"), is_admin)
async def update_cmd(message: Message):
    """Executes git pull and reports the outcome."""
    repo_url = os.environ.get("GITHUB_REPO")
    pull_cmd = f"git pull {repo_url}" if repo_url else "git pull"
    target_info = f" from {repo_url}" if repo_url else ""

    status_msg = await message.reply(f"⬇️ Pulling updates{target_info}...")
    logger.info("Executing update command: %s", pull_cmd)

    try:
        process = await asyncio.create_subprocess_shell(
            pull_cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        stdout, stderr = await process.communicate()

        out = stdout.decode("utf-8", errors="replace").strip()
        err = stderr.decode("utf-8", errors="replace").strip()

        msg = "<b>Git Pull Result:</b>\n"
        if out:
            msg += f"<code>{html.escape(out)}</code>\n"
        if err:
            msg += f"<i>Errors/Warnings:</i>\n<code>{html.escape(err)}</code>\n"

        await status_msg.edit_text(msg)
    except Exception as e:
        logger.exception("Failed to execute git pull")
        await status_msg.edit_text(f"❌ Failed to pull: <code>{html.escape(str(e))}</code>")


def setup_plugin(dp: Dispatcher, bot: Bot = None):
    """Entrypoint for dynamic plugin loaders."""
    dp.include_router(router)
