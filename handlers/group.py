import logging
from aiogram import Router, F, Bot
from aiogram.types import Message, ChatMemberUpdated, InlineKeyboardMarkup, InlineKeyboardButton, CallbackQuery
from aiogram.filters import ChatMemberUpdatedFilter, IS_MEMBER, IS_NOT_MEMBER, ADMINISTRATOR
from database.sqlite_db import SQLiteDatabase

router = Router()
logger = logging.getLogger(__name__)

def get_admin_check_keyboard(bot_member, is_forum: bool = True):
    is_admin = bot_member.status == "administrator"
    can_manage_topics = getattr(bot_member, 'can_manage_topics', False)
    can_pin_messages = getattr(bot_member, 'can_pin_messages', False)

    keyboard = []

    if not (is_admin and can_manage_topics and can_pin_messages):
        if is_admin:
            text = "Permissions missing ⚠️"
            cb_data = "admin_missing_perms"
        else:
            text = "Bot is not Admin 🚫"
            cb_data = "admin_not_admin"
        keyboard.append([InlineKeyboardButton(text=text, callback_data=cb_data)])

    if not is_forum:
        keyboard.append([InlineKeyboardButton(text="Topics not enabled 🚫", callback_data="forum_not_enabled")])

    if not keyboard:
        return None

    return InlineKeyboardMarkup(inline_keyboard=keyboard)

@router.my_chat_member(ChatMemberUpdatedFilter(member_status_changed=IS_NOT_MEMBER >> IS_MEMBER))
async def bot_added_to_group(event: ChatMemberUpdated, bot: Bot, db: SQLiteDatabase):
    if event.chat.type not in ["group", "supergroup"]:
        return

    if event.chat.type != "supergroup":
        await bot.send_message(event.chat.id, "this group is not a supergroup. make it supergroup by enabling topics then add me again")
        await bot.leave_chat(event.chat.id)
        return

    owner_id = await db.get_setting("owner_id")
    ready_for_group = await db.get_setting("ready_for_group")

    if str(event.from_user.id) != owner_id:
        await bot.send_message(event.chat.id, "I can only be added by my owner. Leaving...")
        await bot.leave_chat(event.chat.id)
        return

    if ready_for_group != "true":
        await bot.send_message(event.chat.id, "Please use 'Setup Group' in private chat before adding me. Leaving...")
        await bot.leave_chat(event.chat.id)
        return

    existing_group = await db.get_setting("group_id")
    if existing_group and existing_group != str(event.chat.id):
        try:
            await bot.send_message(existing_group, "I am being moved to another group by my owner. Goodbye!")
            await bot.leave_chat(existing_group)
        except Exception as e:
            logger.error(f"Failed to leave old group {existing_group}: {e}")

    await db.set_setting("group_id", str(event.chat.id))
    await db.set_setting("ready_for_group", "false")
    
    is_forum = getattr(event.chat, 'is_forum', False)

    bot_member = await bot.get_chat_member(event.chat.id, bot.id)
    await bot.send_message(
        event.chat.id,
        "Hello! I am ready to manage Jules API tasks here.",
        reply_markup=get_admin_check_keyboard(bot_member, is_forum=is_forum)
    )

@router.my_chat_member(ChatMemberUpdatedFilter(member_status_changed=IS_MEMBER >> ADMINISTRATOR))
async def bot_promoted(event: ChatMemberUpdated, bot: Bot):
    # Whenever bot status changes to admin, we might want to update some pinned message if any,
    # but for now we just rely on the manual check button.
    pass

@router.callback_query(F.data.startswith("admin_"))
async def admin_check_cb(callback: CallbackQuery, bot: Bot):
    if callback.data == "admin_ok":
        await callback.answer("Bot is admin with all required permissions!", show_alert=True)
    elif callback.data == "admin_not_admin":
        # Check current status
        bot_member = await bot.get_chat_member(callback.message.chat.id, bot.id)
        is_forum = getattr(callback.message.chat, 'is_forum', False)
        if bot_member.status == "administrator":
            await callback.message.edit_reply_markup(reply_markup=get_admin_check_keyboard(bot_member, is_forum=is_forum))
            await callback.answer("Status updated!")
        else:
            await callback.answer("Please make the bot an Admin.", show_alert=True)
    elif callback.data == "admin_missing_perms":
        bot_member = await bot.get_chat_member(callback.message.chat.id, bot.id)
        can_manage_topics = getattr(bot_member, 'can_manage_topics', False)
        can_pin_messages = getattr(bot_member, 'can_pin_messages', False)
        is_forum = getattr(callback.message.chat, 'is_forum', False)

        if can_manage_topics and can_pin_messages:
            await callback.message.edit_reply_markup(reply_markup=get_admin_check_keyboard(bot_member, is_forum=is_forum))
            await callback.answer("Permissions updated!")
        else:
            missing = []
            if not can_manage_topics: missing.append("Manage Topics")
            if not can_pin_messages: missing.append("Pin Messages")
            await callback.answer(f"Missing permissions:\n" + "\n".join(missing), show_alert=True)

@router.callback_query(F.data == "forum_not_enabled")
async def forum_not_enabled_cb(callback: CallbackQuery, bot: Bot):
    chat = await bot.get_chat(callback.message.chat.id)
    is_forum = getattr(chat, 'is_forum', False)
    if is_forum:
        bot_member = await bot.get_chat_member(callback.message.chat.id, bot.id)
        await callback.message.edit_reply_markup(reply_markup=get_admin_check_keyboard(bot_member, is_forum=is_forum))
        await callback.answer("Topics are now enabled!", show_alert=True)
    else:
        await callback.answer("Please go to Group Settings -> Enable Topics.", show_alert=True)
