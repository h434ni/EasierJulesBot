import logging
import asyncio
from aiogram import Router, F, Bot
from aiogram.types import Message, CallbackQuery, InlineKeyboardMarkup, InlineKeyboardButton
from database.sqlite_db import SQLiteDatabase
from jules.api import JulesAPIClient

router = Router()
logger = logging.getLogger(__name__)

# Dictionary to keep track of pending deletions tasks
pending_deletions = {}

@router.message(F.forum_topic_closed)
async def topic_closed(message: Message, db: SQLiteDatabase):
    topic_id = message.message_thread_id
    topic_data = await db.get_topic(topic_id)

    if not topic_data or not topic_data.get("session_id"):
        return

    await db.update_topic_state(topic_id, 'closed')

    kb = InlineKeyboardMarkup(inline_keyboard=[
        [InlineKeyboardButton(text="Yes, archive/delete session", callback_data="delete_session_yes")],
        [InlineKeyboardButton(text="No, reopen topic", callback_data="delete_session_no")]
    ])

    msg = await message.answer(
        "You closed this topic. Do you want to archive/delete the associated Jules session as well?\n"
        "(If no response in 5 minutes, I will reopen the topic)",
        reply_markup=kb
    )

    # Schedule deletion task
    task = asyncio.create_task(delayed_reopen(message.chat.id, topic_id, msg.message_id, message.bot, db))
    pending_deletions[topic_id] = task

async def delayed_reopen(chat_id: int, topic_id: int, message_id: int, bot: Bot, db: SQLiteDatabase):
    await asyncio.sleep(300) # 5 minutes

    # If task completes without being cancelled, reopen topic
    if topic_id in pending_deletions:
        del pending_deletions[topic_id]

    try:
        await bot.reopen_forum_topic(chat_id, topic_id)
        await db.update_topic_state(topic_id, 'open')
        await bot.edit_message_text("Time's up! Reopened the topic.", chat_id=chat_id, message_id=message_id)
    except Exception as e:
        logger.error(f"Failed to reopen topic {topic_id}: {e}")

@router.callback_query(F.data.startswith("delete_session_"))
async def delete_session_cb(callback: CallbackQuery, bot: Bot, db: SQLiteDatabase):
    topic_id = callback.message.message_thread_id
    choice = callback.data.split("_")[-1]

    if topic_id in pending_deletions:
        pending_deletions[topic_id].cancel()
        del pending_deletions[topic_id]

    if choice == "yes":
        topic_data = await db.get_topic(topic_id)
        session_id = topic_data.get("session_id")

        if session_id:
            api_key = await db.get_setting("api_key")
            client = JulesAPIClient(api_key)
            try:
                await client.delete_session(session_id)
                await callback.message.edit_text("Session has been deleted. The topic will remain closed.")
                # We update state to deleted so we don't poll it anymore
                await db.update_topic_state(topic_id, 'deleted')
            except Exception as e:
                logger.error(f"Failed to delete session {session_id}: {e}")
                await callback.message.edit_text("Failed to delete session via API.")
    else:
        try:
            await bot.reopen_forum_topic(callback.message.chat.id, topic_id)
            await db.update_topic_state(topic_id, 'open')
            await callback.message.edit_text("Topic reopened.")
        except Exception as e:
            logger.error(f"Failed to reopen topic {topic_id}: {e}")
            await callback.message.edit_text("Failed to reopen topic.")

@router.message(F.forum_topic_reopened)
async def topic_reopened(message: Message, db: SQLiteDatabase):
    topic_id = message.message_thread_id

    if topic_id in pending_deletions:
        pending_deletions[topic_id].cancel()
        del pending_deletions[topic_id]

    await db.update_topic_state(topic_id, 'open')

@router.message(~F.text.startswith("/") & F.text)
async def handle_user_message(message: Message, db: SQLiteDatabase):
    # Only handle messages in topics
    if not message.is_topic_message:
        return

    # Ignore messages not from owner for security (as requested)
    owner_id = await db.get_setting("owner_id")
    if str(message.from_user.id) != owner_id:
        return

    topic_id = message.message_thread_id
    topic_data = await db.get_topic(topic_id)

    # If no session or closed, ignore
    if not topic_data or not topic_data.get("session_id") or topic_data.get("state") != 'open':
        return

    session_id = topic_data["session_id"]
    api_key = await db.get_setting("api_key")
    client = JulesAPIClient(api_key)

    try:
        await client.send_message(session_id, message.text)
        # React to message to show it was sent
        # await message.react(...) aiogram 3 supports reactions, but a simple checkmark reply works too.
        # But to be unobtrusive, maybe do nothing.
    except Exception as e:
        logger.error(f"Failed to send message to session {session_id}: {e}")
        await message.reply("Failed to send message to Jules.")
