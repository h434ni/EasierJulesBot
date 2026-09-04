"""
Start / Onboarding Handlers for EasierJulesBot.

Handles:
- Private chat /start command (claiming owner, initial welcome menu)
- Jules API key configuration flow (FSM)
- Group setup notifications
- Quick task creation from private chat
"""

import logging
from aiogram import Bot, F, Router
from aiogram.filters import CommandStart
from aiogram.fsm.context import FSMContext
from aiogram.fsm.state import State, StatesGroup
from aiogram.types import CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup, Message
from database.sqlite_db import SQLiteDatabase
from handlers.topics import send_topic_setup_menu

logger = logging.getLogger(__name__)

router = Router(name="start")


class ConfigState(StatesGroup):
    waiting_for_api_key = State()


async def get_start_keyboard(bot: Bot, is_connected: bool = False, group_id: str = None) -> InlineKeyboardMarkup:
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


def get_cancel_connect_keyboard() -> InlineKeyboardMarkup:
    return InlineKeyboardMarkup(inline_keyboard=[
        [InlineKeyboardButton(text="Back", callback_data="cancel_connect")]
    ])


@router.message(CommandStart(), F.chat.type == "private")
async def start_cmd(message: Message, bot: Bot, db: SQLiteDatabase):
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


@router.callback_query(F.data == "connect_account")
async def connect_account_cb(callback: CallbackQuery, state: FSMContext):
    await callback.message.edit_text("Please send me your Jules API Key.", reply_markup=get_cancel_connect_keyboard())
    await state.set_state(ConfigState.waiting_for_api_key)
    await state.update_data(prompt_msg_id=callback.message.message_id)
    await callback.answer()


@router.callback_query(F.data == "cancel_connect")
async def cancel_connect_cb(callback: CallbackQuery, state: FSMContext, bot: Bot, db: SQLiteDatabase):
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


@router.message(ConfigState.waiting_for_api_key, F.chat.type == "private")
async def process_api_key(message: Message, state: FSMContext, bot: Bot, db: SQLiteDatabase):
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
    await message.answer(
        "API Key saved! You can now proceed to setup a group.",
        reply_markup=await get_start_keyboard(bot, is_connected=True, group_id=group_id)
    )
    await state.clear()


@router.callback_query(F.data == "setup_group")
async def setup_group_cb(callback: CallbackQuery, db: SQLiteDatabase):
    await db.set_setting("ready_for_group", "true")
    await callback.message.edit_text(
        "Bot is now ready to be added to a group!\n\n"
        "(If you are moving me to a new group, I will try to leave the old one automatically. If I fail, please remove me manually.)\n\n"
        "Please add me to a single group chat, and make sure the group has **Topics (Forum mode) enabled**.",
        reply_markup=get_cancel_connect_keyboard()
    )
    await callback.answer()


@router.callback_query(F.data == "new_task")
async def new_task_cb(callback: CallbackQuery, bot: Bot, db: SQLiteDatabase):
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

        await send_topic_setup_menu(bot, int(group_id), topic.message_thread_id)

        clean_id = str(group_id)
        if clean_id.startswith("-100"):
            clean_id = clean_id[4:]
        topic_url = f"https://t.me/c/{clean_id}/{topic.message_thread_id}"

        kb = InlineKeyboardMarkup(inline_keyboard=[
            [InlineKeyboardButton(text="Go to Task ➔", url=topic_url)],
            [InlineKeyboardButton(text="Back", callback_data="cancel_connect")]
        ])

        await callback.message.edit_text("new task created. go to task to continue", reply_markup=kb)

    except Exception as e:
        logger.error(f"Failed to create topic: {e}")
        await callback.answer("Failed to create topic. Make sure I have permissions to manage topics in the group.", show_alert=True)
