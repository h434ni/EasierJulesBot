import os
import signal
import asyncio
from aiogram import Dispatcher, Bot, Router
from aiogram.filters import Command
from aiogram.types import Message

def setup_plugin(dp: Dispatcher, bot: Bot):
    admin_id_str = os.environ.get("ADMIN_ID")
    if not admin_id_str:
        print("ADMIN_ID not found. admin features are disabled.")
        return
        
    try:
        admin_id = int(admin_id_str)
    except ValueError:
        print("ADMIN_ID is invalid. admin features are disabled.")
        return
        
    # Isolate admin commands to a specific router filtering by ADMIN_ID
    admin_router = Router()
    from aiogram import F
    admin_router.message.filter(F.from_user.id == admin_id)
    
    @admin_router.message(Command("restart"))
    async def restart_cmd(message: Message):
        await message.reply("🔄 Restarting bot (SIGHUP)...")
        # Send SIGHUP to the current process
        os.kill(os.getpid(), signal.SIGHUP)
        
    @admin_router.message(Command("stop"))
    async def stop_cmd(message: Message):
        await message.reply("Please use /restart instead, since the server automatically restarts the bot anyway.")
        
    @admin_router.message(Command("update"))
    async def update_cmd(message: Message):
        repo_url = os.environ.get("GITHUB_REPO", "https://github.com/h434ni/EasierJulesBot")
        status_msg = await message.reply(f"⬇️ Pulling from {repo_url}...")
        
        try:
            process = await asyncio.create_subprocess_shell(
                f"git pull {repo_url}",
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE
            )
            stdout, stderr = await process.communicate()
            
            out = stdout.decode('utf-8').strip()
            err = stderr.decode('utf-8').strip()
            
            msg = f"<b>Git Pull Result:</b>\n"
            if out:
                msg += f"<code>{out}</code>\n"
            if err:
                msg += f"<i>Errors/Warnings:</i>\n<code>{err}</code>"
                
            await status_msg.edit_text(msg)
        except Exception as e:
            await status_msg.edit_text(f"❌ Failed to pull: {e}")

    # Register startup hook for restart notification
    async def on_startup(bot: Bot):
        try:
            await bot.send_message(chat_id=admin_id, text="🚀 Bot restarted successfully!")
        except Exception as e:
            print(f"Failed to send startup message to admin {admin_id}: {e}")

    dp.startup.register(on_startup)
    dp.include_router(admin_router)
