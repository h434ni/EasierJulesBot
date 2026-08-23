import logging
import asyncio
from aiogram import Router, F, Bot
from aiogram.types import Message, CallbackQuery, InlineKeyboardMarkup, InlineKeyboardButton
from database.sqlite_db import SQLiteDatabase
from jules.api import JulesAPIClient
from .topics import send_topic_setup_menu

router = Router()
logger = logging.getLogger(__name__)

setup_states = {}

# Simple in-memory storage for pagination tokens
session_pages = {}

@router.message(F.forum_topic_created)
async def topic_created(message: Message, bot: Bot, db: SQLiteDatabase):
    owner_id = await db.get_setting("owner_id")
    if str(message.from_user.id) != owner_id:
        return

    await db.create_topic(message.message_thread_id)
    setup_states[message.message_thread_id] = {"setup_type": "new", "auto_pr": False}
    await send_topic_setup_menu(bot, message.chat.id, message.message_thread_id, setup_type="new", auto_pr=False)

@router.callback_query(F.data.startswith("setup_type_"))
async def setup_type_cb(callback: CallbackQuery):
    topic_id = callback.message.message_thread_id
    setup_type = callback.data.split("_")[-1]

    state = setup_states.get(topic_id, {"setup_type": "new", "auto_pr": False})
    state["setup_type"] = setup_type
    setup_states[topic_id] = state

    await callback.message.delete()
    await send_topic_setup_menu(callback.bot, callback.message.chat.id, topic_id, setup_type=state["setup_type"], auto_pr=state["auto_pr"])
    await callback.answer()

@router.callback_query(F.data.startswith("toggle_pr_"))
async def toggle_pr_cb(callback: CallbackQuery):
    topic_id = callback.message.message_thread_id
    current_val = bool(int(callback.data.split("_")[-1]))
    new_val = not current_val

    state = setup_states.get(topic_id, {"setup_type": "new", "auto_pr": False})
    state["auto_pr"] = new_val
    setup_states[topic_id] = state

    await callback.message.delete()
    await send_topic_setup_menu(callback.bot, callback.message.chat.id, topic_id, setup_type=state["setup_type"], auto_pr=state["auto_pr"])
    await callback.answer()

@router.callback_query(F.data == "cancel_setup")
async def cancel_setup_cb(callback: CallbackQuery, bot: Bot, db: SQLiteDatabase):
    topic_id = callback.message.message_thread_id
    await db.delete_topic(topic_id)
    setup_states.pop(topic_id, None)

    await callback.answer("Deleting topic...")
    try:
        await bot.delete_forum_topic(chat_id=callback.message.chat.id, message_thread_id=topic_id)
    except Exception as e:
        logger.error(f"Failed to delete topic {topic_id}: {e}")
        await callback.message.edit_text("Failed to delete topic. Please delete it manually.")

@router.callback_query(F.data == "proceed_setup_new")
async def proceed_new_cb(callback: CallbackQuery, db: SQLiteDatabase):
    api_key = await db.get_setting("api_key")
    if not api_key:
        await callback.answer("API Key not found. Connect account first.", show_alert=True)
        return

    client = JulesAPIClient(api_key)
    try:
        sources_data = await client.list_sources(page_size=100)
        sources = sources_data.get("sources", [])

        if not sources:
            await callback.answer("No repositories found in your Jules account.", show_alert=True)
            return

        kb = []
        for s in sources:
            short_name = s.get('id', s.get('name', '')).replace('github-', '')
            kb.append([InlineKeyboardButton(text=short_name, callback_data=f"repo_{s['name']}")])

        kb.append([InlineKeyboardButton(text="🔄 Refresh", callback_data="proceed_setup_new")])
        kb.append([InlineKeyboardButton(text="Back", callback_data="back_to_setup")])

        await callback.message.edit_text("Select a repository:", reply_markup=InlineKeyboardMarkup(inline_keyboard=kb))
    except Exception as e:
        logger.error(f"Error fetching sources: {e}")
        await callback.answer("Error fetching repositories.", show_alert=True)

@router.callback_query(F.data.startswith("repo_"))
async def repo_selected_cb(callback: CallbackQuery, db: SQLiteDatabase):
    repo_name = callback.data[5:]
    topic_id = callback.message.message_thread_id

    state = setup_states.get(topic_id, {})
    state["selected_repo"] = repo_name
    setup_states[topic_id] = state

    # Now fetch branches for the repo
    api_key = await db.get_setting("api_key")
    client = JulesAPIClient(api_key)

    try:
        source_data = await client.get_source(repo_name)
        branches = source_data.get("githubRepo", {}).get("branches", [])

        if not branches:
            await callback.answer("No branches found for this repository.", show_alert=True)
            return

        kb = []
        for b in branches:
            branch_name = b.get("displayName")
            kb.append([InlineKeyboardButton(text=branch_name, callback_data=f"branch_{branch_name}")])

        kb.append([InlineKeyboardButton(text="Back", callback_data="proceed_setup_new")])

        await callback.message.edit_text("Select a branch:", reply_markup=InlineKeyboardMarkup(inline_keyboard=kb))
    except Exception as e:
        logger.error(f"Error fetching branches: {e}")
        await callback.answer("Error fetching branches.", show_alert=True)

@router.callback_query(F.data.startswith("branch_"))
async def branch_selected_cb(callback: CallbackQuery, db: SQLiteDatabase):
    branch_name = callback.data[7:]
    topic_id = callback.message.message_thread_id

    state = setup_states.get(topic_id, {})
    state["selected_branch"] = branch_name
    setup_states[topic_id] = state

    repo_name = state.get("selected_repo", "Unknown")
    short_repo = repo_name.split("/")[-1].replace("github-", "")

    await callback.message.edit_text(
        f"Selected Repo: `{short_repo}`\nSelected Branch: `{branch_name}`\n\n"
        "Please send the initialization message (prompt) to start the session.",
        parse_mode="Markdown"
    )
    state["waiting_for_prompt"] = True
    setup_states[topic_id] = state

@router.message(F.text, lambda msg: setup_states.get(msg.message_thread_id, {}).get("waiting_for_prompt", False))
async def prompt_received(message: Message, bot: Bot, db: SQLiteDatabase):
    topic_id = message.message_thread_id
    state = setup_states.get(topic_id, {})

    api_key = await db.get_setting("api_key")
    client = JulesAPIClient(api_key)

    repo_name = state.get("selected_repo")
    branch_name = state.get("selected_branch")
    auto_pr = state.get("auto_pr", False)

    status_msg = await message.answer("Starting session...")

    try:
        session_data = await client.create_session(
            prompt=message.text,
            source=repo_name,
            branch=branch_name,
            auto_pr=auto_pr
        )

        session_id = session_data.get("name")
        await db.update_topic_session(topic_id, session_id)
        await db.update_topic_auto_pr(topic_id, auto_pr)

        short_repo = repo_name.split("/")[-1].replace("github-", "")
        pinned_msg = await bot.send_message(
            chat_id=message.chat.id,
            message_thread_id=topic_id,
            text=f"📌 **Session Active**\nSession ID: `{session_id}`\nRepo: `{short_repo}`\nBranch: `{branch_name}`\nState: `{session_data.get('state', 'QUEUED')}`",
            parse_mode="Markdown"
        )
        await bot.pin_chat_message(message.chat.id, pinned_msg.message_id)
        await db.update_topic_pinned_message(topic_id, pinned_msg.message_id)

        await status_msg.delete()
        setup_states.pop(topic_id, None)

    except Exception as e:
        logger.error(f"Error creating session: {e}")
        await status_msg.edit_text(f"Error creating session: {e}")

@router.callback_query(F.data == "proceed_setup_existing")
async def proceed_existing_cb(callback: CallbackQuery, db: SQLiteDatabase):
    topic_id = callback.message.message_thread_id
    session_pages[topic_id] = [None]  # List of page tokens
    await show_sessions_page(callback, db, 0)

@router.callback_query(F.data.startswith("page_"))
async def page_sessions_cb(callback: CallbackQuery, db: SQLiteDatabase):
    page_index = int(callback.data[5:])
    await show_sessions_page(callback, db, page_index)

async def show_sessions_page(callback: CallbackQuery, db: SQLiteDatabase, page_index: int):
    topic_id = callback.message.message_thread_id
    api_key = await db.get_setting("api_key")
    if not api_key:
        await callback.answer("API Key not found.", show_alert=True)
        return

    tokens = session_pages.get(topic_id, [None])
    page_token = tokens[page_index] if page_index < len(tokens) else None

    client = JulesAPIClient(api_key)
    try:
        sessions_data = await client.list_sessions(page_size=10, page_token=page_token)
        sessions = sessions_data.get("sessions", [])
        next_token = sessions_data.get("nextPageToken")

        if not sessions:
            if page_index == 0:
                await callback.answer("No sessions found.", show_alert=True)
            else:
                await callback.answer("No more sessions.", show_alert=True)
            return

        if next_token and len(tokens) == page_index + 1:
            tokens.append(next_token)

        session_pages[topic_id] = tokens

        kb = []
        for s in sessions:
            title = s.get("title", s.get("id", "Unknown"))
            state = s.get("state", "")
            kb.append([InlineKeyboardButton(text=f"[{state}] {title}", callback_data=f"select_sess_{s['name']}")])

        nav_row = []
        if page_index > 0:
            nav_row.append(InlineKeyboardButton(text="⬅️ Prev", callback_data=f"page_{page_index-1}"))
        if next_token:
            nav_row.append(InlineKeyboardButton(text="Next ➡️", callback_data=f"page_{page_index+1}"))

        if nav_row:
            kb.append(nav_row)

        kb.append([InlineKeyboardButton(text="Back", callback_data="back_to_setup")])

        await callback.message.edit_text("Select an existing session:", reply_markup=InlineKeyboardMarkup(inline_keyboard=kb))
    except Exception as e:
        logger.error(f"Error fetching sessions: {e}")
        await callback.answer("Error fetching sessions.", show_alert=True)

@router.callback_query(F.data.startswith("select_sess_"))
async def select_session_cb(callback: CallbackQuery, bot: Bot, db: SQLiteDatabase):
    session_id = callback.data[12:]
    topic_id = callback.message.message_thread_id

    existing_topic = await db.get_topic_by_session(session_id)
    if existing_topic and existing_topic["topic_id"] != topic_id:
        await callback.answer("This session is already attached to another topic.", show_alert=True)
        return

    state = setup_states.get(topic_id, {"setup_type": "existing", "auto_pr": False})
    state["selected_session"] = session_id
    setup_states[topic_id] = state

    await callback.answer("Session selected!")

    btn_new = InlineKeyboardButton(text=f"New session", callback_data="setup_type_new")
    btn_existing = InlineKeyboardButton(text=f"✅ Attach existing", callback_data="setup_type_existing")
    btn_auto_pr = InlineKeyboardButton(text=f"Auto PR: {'ON' if state.get('auto_pr') else 'OFF'}", callback_data=f"toggle_pr_{int(state.get('auto_pr', False))}")
    btn_proceed = InlineKeyboardButton(text="Proceed ➔", callback_data=f"attach_{session_id}")
    btn_cancel = InlineKeyboardButton(text="Cancel / Delete Topic", callback_data="cancel_setup")

    keyboard = InlineKeyboardMarkup(inline_keyboard=[
        [btn_new, btn_existing],
        [btn_auto_pr],
        [btn_proceed],
        [btn_cancel]
    ])

    await callback.message.edit_text(
        f"Selected Session: `{session_id}`\n\nPlease proceed.",
        reply_markup=keyboard,
        parse_mode="Markdown"
    )

@router.callback_query(F.data == "back_to_setup")
async def back_to_setup_cb(callback: CallbackQuery):
    topic_id = callback.message.message_thread_id
    state = setup_states.get(topic_id, {"setup_type": "existing", "auto_pr": False})
    await send_topic_setup_menu(callback.bot, callback.message.chat.id, topic_id, setup_type=state["setup_type"], auto_pr=state.get("auto_pr", False))
    await callback.message.delete()

@router.callback_query(F.data.startswith("attach_"))
async def attach_session_cb(callback: CallbackQuery, bot: Bot, db: SQLiteDatabase):
    session_id = callback.data[7:]
    topic_id = callback.message.message_thread_id

    existing_topic = await db.get_topic_by_session(session_id)
    if existing_topic and existing_topic["topic_id"] != topic_id:
        await callback.answer("This session is already attached to another topic.", show_alert=True)
        return

    await db.update_topic_session(topic_id, session_id)

    api_key = await db.get_setting("api_key")
    client = JulesAPIClient(api_key)
    session_data = await client.get_session(session_id)

    pinned_msg = await bot.send_message(
        chat_id=callback.message.chat.id,
        message_thread_id=topic_id,
        text=f"📌 **Session Attached**\nSession ID: `{session_id}`\nState: `{session_data.get('state', 'UNKNOWN')}`",
        parse_mode="Markdown"
    )
    await bot.pin_chat_message(callback.message.chat.id, pinned_msg.message_id)
    await db.update_topic_pinned_message(topic_id, pinned_msg.message_id)

    await callback.message.delete()
    setup_states.pop(topic_id, None)
