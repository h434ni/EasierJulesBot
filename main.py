import asyncio
import logging
import os
from aiogram import Bot, Dispatcher, F
from aiogram.types import Message, CallbackQuery, InlineKeyboardMarkup, InlineKeyboardButton
from aiogram.filters import CommandStart, Command
from aiogram.fsm.context import FSMContext
from aiogram.fsm.state import State, StatesGroup
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

class ConfigState(StatesGroup):
    waiting_for_api_key = State()

async def get_start_keyboard(bot: Bot, is_connected: bool = False, group_id: str = None):
    connect_btn_text = "Account Connected ✅" if is_connected else "Connect Account"
    group_btn_text = "Group Connected ✅" if group_id else "Setup Group"
    
    group_row = [InlineKeyboardButton(text=group_btn_text, callback_data="setup_group")]
    if group_id:
        try:
            group_url = await bot.export_chat_invite_link(group_id)
        except Exception as e:
            logger.error(f"Failed to export chat invite link: {e}")
            clean_id = str(group_id)
            if clean_id.startswith("-100"):
                clean_id = clean_id[4:]
            group_url = f"https://t.me/c/{clean_id}/1"
        group_row.append(InlineKeyboardButton(text="Open Group", url=group_url))

    keyboard = [
        [InlineKeyboardButton(text=connect_btn_text, callback_data="connect_account")],
        group_row,
    ]
    if group_id:
        keyboard.append([InlineKeyboardButton(text="New Task", callback_data="new_task")])
        
    return InlineKeyboardMarkup(inline_keyboard=keyboard)

@dp.message(CommandStart(), F.chat.type == "private")
async def start_cmd(message: Message):
    # Register user ID in db as the sole owner
    owner_id = await db.get_setting("owner_id")
    if not owner_id:
        await db.set_setting("owner_id", str(message.from_user.id))
    elif str(message.from_user.id) != owner_id:
        await message.reply("Sorry, this bot is already claimed by another user.")
        return

    api_key = await db.get_setting("api_key")
    is_connected = bool(api_key)
    group_id = await db.get_setting("group_id")

    await message.answer(
        "Welcome to the Jules API Telegram Bot!\n\n"
        "Use the buttons below to configure the bot.",
        reply_markup=await get_start_keyboard(bot, is_connected, group_id)
    )

def get_cancel_connect_keyboard():
    return InlineKeyboardMarkup(inline_keyboard=[
        [InlineKeyboardButton(text="Back", callback_data="cancel_connect")]
    ])

@dp.callback_query(F.data == "connect_account")
async def connect_account_cb(callback: CallbackQuery, state: FSMContext):
    await callback.message.edit_text("Please send me your Jules API Key.", reply_markup=get_cancel_connect_keyboard())
    await state.set_state(ConfigState.waiting_for_api_key)
    await state.update_data(prompt_msg_id=callback.message.message_id)
    await callback.answer()

@dp.callback_query(F.data == "cancel_connect")
async def cancel_connect_cb(callback: CallbackQuery, state: FSMContext):
    await state.clear()
    api_key = await db.get_setting("api_key")
    is_connected = bool(api_key)
    group_id = await db.get_setting("group_id")

    await callback.message.edit_text(
        "Welcome to the Jules API Telegram Bot!\n\n"
        "Use the buttons below to configure the bot.",
        reply_markup=await get_start_keyboard(bot, is_connected, group_id)
    )
    await callback.answer()

@dp.message(ConfigState.waiting_for_api_key, F.chat.type == "private")
async def process_api_key(message: Message, state: FSMContext):
    data = await state.get_data()
    prompt_msg_id = data.get("prompt_msg_id")
    if prompt_msg_id:
        try:
            await message.bot.edit_message_reply_markup(
                chat_id=message.chat.id,
                message_id=prompt_msg_id,
                reply_markup=None
            )
        except Exception:
            pass

    await db.set_setting("api_key", message.text.strip())
    group_id = await db.get_setting("group_id")
    await message.answer("API Key saved! You can now proceed to setup a group.", reply_markup=await get_start_keyboard(bot, is_connected=True, group_id=group_id))
    await state.clear()

@dp.callback_query(F.data == "setup_group")
async def setup_group_cb(callback: CallbackQuery):
    await db.set_setting("ready_for_group", "true")
    await callback.message.edit_text(
        "Bot is now ready to be added to a group!\n\n"
        "Please add me to a single group chat, and make sure the group has **Topics (Forum mode) enabled**.",
        reply_markup=get_cancel_connect_keyboard()
    )
    await callback.answer()

@dp.callback_query(F.data == "new_task")
async def new_task_cb(callback: CallbackQuery):
    group_id = await db.get_setting("group_id")
    if not group_id:
        await callback.answer("Please add me to a group first!", show_alert=True)
        return

    try:
        # Create a new topic in the group
        topic = await bot.create_forum_topic(chat_id=int(group_id), name="New Task")
        await callback.answer(f"Created new topic: {topic.name}", show_alert=False)

        # Insert into DB
        await db.create_topic(topic.message_thread_id)

        # We also need to send the setup message to that new topic, but that can be handled
        # by a function that sends the topic setup menu. We'll import it later or do it here.
        from handlers.topics import send_topic_setup_menu
        await send_topic_setup_menu(bot, int(group_id), topic.message_thread_id)

    except Exception as e:
        logger.error(f"Failed to create topic: {e}")
        await callback.answer("Failed to create topic. Make sure I have permissions to manage topics in the group.", show_alert=True)

from handlers.group import router as group_router
from handlers.topic_lifecycle import router as topic_lifecycle_router
from handlers.session_messaging import router as session_messaging_router

from poller import poll_activities
import asyncio

async def main():
    await db.connect()

    # We will register other handlers here

    try:
        asyncio.create_task(poll_activities(bot, db))
        dp.include_router(group_router)
        dp.include_router(topic_lifecycle_router)
        dp.include_router(session_messaging_router)
        await dp.start_polling(bot, db=db)
    finally:
        await db.disconnect()

if __name__ == "__main__":
    asyncio.run(main())
