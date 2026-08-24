from aiogram import Bot
from aiogram.types import InlineKeyboardMarkup, InlineKeyboardButton

def get_topic_setup_keyboard(auto_pr: bool = False):
    btn_auto_pr = InlineKeyboardButton(text=f"Auto PR: {'ON' if auto_pr else 'OFF'}", callback_data=f"toggle_pr_{int(auto_pr)}")
    btn_proceed = InlineKeyboardButton(text="Proceed ➔", callback_data="proceed_setup_new")
    btn_cancel = InlineKeyboardButton(text="Cancel / Delete Topic", callback_data="cancel_setup")

    return InlineKeyboardMarkup(inline_keyboard=[
        [btn_auto_pr],
        [btn_proceed],
        [btn_cancel]
    ])

async def send_topic_setup_menu(bot: Bot, chat_id: int, message_thread_id: int, auto_pr: bool = False):
    await bot.send_message(
        chat_id=chat_id,
        message_thread_id=message_thread_id,
        text="Please configure this topic:",
        reply_markup=get_topic_setup_keyboard(auto_pr)
    )
