import logging
from aiogram import Router, F, Bot
from aiogram.types import Message, ChatMemberUpdated, InlineKeyboardMarkup, InlineKeyboardButton, CallbackQuery
from aiogram.filters import ChatMemberUpdatedFilter, IS_MEMBER, IS_NOT_MEMBER, ADMINISTRATOR
from database.sqlite_db import SQLiteDatabase

router = Router()
logger = logging.getLogger(__name__)

def get_admin_check_keyboard(bot_member):
    is_admin = bot_member.status == "administrator"
    can_manage_topics = getattr(bot_member, 'can_manage_topics', False)
    can_pin_messages = getattr(bot_member, 'can_pin_messages', False)

    if is_admin and can_manage_topics and can_pin_messages:
        text = "Bot is Admin ✅"
        cb_data = "admin_ok"
    elif is_admin:
        text = "Permissions missing ⚠️"
        cb_data = "admin_missing_perms"
    else:
        text = "Bot is not Admin 🚫"
        cb_data = "admin_not_admin"

    return InlineKeyboardMarkup(inline_keyboard=[
        [InlineKeyboardButton(text=text, callback_data=cb_data)]
    ])

@router.my_chat_member(ChatMemberUpdatedFilter(member_status_changed=IS_NOT_MEMBER >> IS_MEMBER))
async def bot_added_to_group(event: ChatMemberUpdated, bot: Bot, db: SQLiteDatabase):
    if event.chat.type not in ["group", "supergroup"]:
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
        await bot.send_message(event.chat.id, "I am already setup in another group. Leaving...")
        await bot.leave_chat(event.chat.id)
        return

    # Check if forum
    if not event.chat.is_forum:
        await bot.send_message(
            event.chat.id,
            "⚠️ This group is not a Forum (Topics are not enabled).\n"
            "Please go to Group Settings -> Enable Topics, then grant me Admin rights."
        )

    await db.set_setting("group_id", str(event.chat.id))

    bot_member = await bot.get_chat_member(event.chat.id, bot.id)
    await bot.send_message(
        event.chat.id,
        "Hello! I am ready to manage Jules API tasks here.",
        reply_markup=get_admin_check_keyboard(bot_member)
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
        if bot_member.status == "administrator":
            await callback.message.edit_reply_markup(reply_markup=get_admin_check_keyboard(bot_member))
            await callback.answer("Status updated!")
        else:
            await callback.answer("Please make the bot an Admin.", show_alert=True)
    elif callback.data == "admin_missing_perms":
        bot_member = await bot.get_chat_member(callback.message.chat.id, bot.id)
        can_manage_topics = getattr(bot_member, 'can_manage_topics', False)
        can_pin_messages = getattr(bot_member, 'can_pin_messages', False)

        if can_manage_topics and can_pin_messages:
            await callback.message.edit_reply_markup(reply_markup=get_admin_check_keyboard(bot_member))
            await callback.answer("Permissions updated!")
        else:
            missing = []
            if not can_manage_topics: missing.append("Manage Topics")
            if not can_pin_messages: missing.append("Pin Messages")
            await callback.answer(f"Missing permissions:\n" + "\n".join(missing), show_alert=True)
