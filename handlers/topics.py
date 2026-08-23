from aiogram import Bot
from aiogram.types import InlineKeyboardMarkup, InlineKeyboardButton

async def send_topic_setup_menu(bot: Bot, chat_id: int, message_thread_id: int, setup_type: str = "new", auto_pr: bool = False):
    btn_new = InlineKeyboardButton(text=f"{'✅ ' if setup_type == 'new' else ''}New session", callback_data="setup_type_new")
    btn_existing = InlineKeyboardButton(text=f"{'✅ ' if setup_type == 'existing' else ''}Attach existing", callback_data="setup_type_existing")

    btn_auto_pr = InlineKeyboardButton(text=f"Auto PR: {'ON' if auto_pr else 'OFF'}", callback_data=f"toggle_pr_{int(auto_pr)}")
    btn_proceed = InlineKeyboardButton(text="Proceed ➔", callback_data=f"proceed_setup_{setup_type}")
    btn_cancel = InlineKeyboardButton(text="Cancel / Delete Topic", callback_data="cancel_setup")

    keyboard = InlineKeyboardMarkup(inline_keyboard=[
        [btn_new, btn_existing],
        [btn_auto_pr],
        [btn_proceed],
        [btn_cancel]
    ])

    await bot.send_message(
        chat_id=chat_id,
        message_thread_id=message_thread_id,
        text="Please configure this topic:",
        reply_markup=keyboard
    )
