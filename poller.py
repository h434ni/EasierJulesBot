import asyncio
import logging
from aiogram import Bot
from database.sqlite_db import SQLiteDatabase
from jules.api import JulesAPIClient

logger = logging.getLogger(__name__)

# To avoid repeating activities, we keep track of the last createTime we saw per session
last_seen_times = {}

async def poll_activities(bot: Bot, db: SQLiteDatabase):
    while True:
        try:
            api_key = await db.get_setting("api_key")
            if not api_key:
                await asyncio.sleep(10)
                continue

            client = JulesAPIClient(api_key)

            group_id = await db.get_setting("group_id")
            if not group_id:
                await asyncio.sleep(10)
                continue

            active_topics = await db.get_all_active_topics()

            for topic in active_topics:
                session_id = topic["session_id"]
                topic_id = topic["topic_id"]
                pinned_msg_id = topic.get("pinned_message_id")

                # We need to fetch session state to update pinned message and see if it's done
                try:
                    session_data = await client.get_session(session_id)
                    state = session_data.get("state", "UNKNOWN")

                    if pinned_msg_id:
                        try:
                            # We don't have the repo name directly from session response usually,
                            # but title has it or we can just show title.
                            title = session_data.get("title", session_id)
                            clean_id = session_id.split("/")[-1]
                            jules_url = session_data.get("url", f"https://jules.google.com/session/{clean_id}")
                            
                            auto_pr_str = "`ON`" if session_data.get("automationMode") == "AUTO_CREATE_PR" else "`OFF`"
                            state_emoji = {"QUEUED": "⏳", "PLANNING": "🧠", "AWAITING_PLAN_APPROVAL": "✋", "AWAITING_USER_FEEDBACK": "💬", "IN_PROGRESS": "🔄", "PAUSED": "⏸️", "FAILED": "❌", "COMPLETED": "✅"}.get(state, "🔵")
                            
                            outputs = session_data.get("outputs", [])
                            pr_url = None
                            for out in outputs:
                                if "pullRequest" in out and "url" in out["pullRequest"]:
                                    pr_url = out["pullRequest"]["url"]
                                    break
                            
                            from aiogram.types import InputRichMessage, InlineKeyboardMarkup, InlineKeyboardButton
                            
                            pr_button = InlineKeyboardButton(text="View PR", url=pr_url) if pr_url else InlineKeyboardButton(text="No PR", callback_data="no_pr_alert")
                            kb = InlineKeyboardMarkup(inline_keyboard=[
                                [
                                    InlineKeyboardButton(text="Open Session", url=jules_url),
                                    pr_button
                                ]
                            ])
                            
                            rich_md = f"""
# {title}

| **Status** | **Auto PR** | **State** |
|:---|:---|:---|
| `Active` | {auto_pr_str} | {state_emoji} `{state}` |
"""
                            # Ideally we only update if changed, but we can catch "message is not modified" exceptions
                            await bot.edit_message_text(
                                chat_id=group_id,
                                message_id=pinned_msg_id,
                                rich_message=InputRichMessage(markdown=rich_md),
                                reply_markup=kb
                            )
                        except Exception as edit_err:
                            if "message is not modified" not in str(edit_err).lower():
                                logger.debug(f"Failed to edit pinned message: {edit_err}")
                except Exception as e:
                    logger.error(f"Error fetching session {session_id}: {e}")

                # Fetch new activities
                last_time = last_seen_times.get(session_id)
                try:
                    activities_res = await client.list_activities(
                        session_id=session_id,
                        page_size=20
                    )

                    activities = activities_res.get("activities", [])
                    if activities:
                        # Activities are often sorted by createTime desc in response, so we need to reverse to process oldest first
                        # Let's sort them explicitly by createTime
                        activities.sort(key=lambda x: x.get("createTime", ""))

                        for act in activities:
                            act_time = act.get("createTime")
                            # If we already saw it (or it's exact same timestamp, skip).
                            # createTime filter in API might be inclusive, so we might get the exact same one again.
                            if last_time and act_time <= last_time:
                                continue

                            await process_activity(bot, int(group_id), topic_id, act)

                            # Update last seen
                            last_seen_times[session_id] = act_time

                except Exception as e:
                    logger.error(f"Error fetching activities for {session_id}: {e}")

        except Exception as e:
            logger.error(f"Polling loop error: {e}")

        await asyncio.sleep(5) # Poll every 5 seconds

async def process_activity(bot: Bot, chat_id: int, topic_id: int, activity: dict):
    # Determine if it's important (notify) or not (silent)
    is_important = False

    desc = activity.get("description", "")
    text = f"🔹 {desc}"

    if "sessionCompleted" in activity:
        is_important = True
        text = "✅ **Session Completed!**"
    elif "sessionFailed" in activity:
        is_important = True
        reason = activity["sessionFailed"].get("reason", "Unknown error")
        text = f"❌ **Session Failed**\nReason: {reason}"
    elif "agentMessaged" in activity:
        is_important = True
        msg = activity["agentMessaged"].get("agentMessage", "")
        text = f"🤖 **Jules:**\n{msg}"
    elif "planGenerated" in activity:
        is_important = True
        text = "📋 **Plan Generated**\n"
        steps = activity["planGenerated"].get("plan", {}).get("steps", [])
        for step in steps:
            text += f"{step.get('index', 0)+1}. {step.get('title', '')}\n"
    elif "planApproved" in activity:
        text = "✅ Plan Approved"
    elif "progressUpdated" in activity:
        prog = activity["progressUpdated"]
        title = prog.get("title", "")
        desc = prog.get("description", "")
        text = f"⏳ **Progress:** {title}\n_{desc}_"
    elif "userMessaged" in activity:
        # We probably already saw this in Telegram, but we can echo it or skip
        return

    # Check for artifacts (code changes, bash output, media)
    artifacts = activity.get("artifacts", [])
    if artifacts:
        for art in artifacts:
            if "changeSet" in art:
                patch = art["changeSet"].get("gitPatch", {}).get("unidiffPatch", "")
                if patch:
                    commit_msg = art["changeSet"].get("gitPatch", {}).get("suggestedCommitMessage", "Code changes generated")
                    if len(patch) > 3000:
                        patch = patch[:3000] + "\n... (diff truncated)"
                        
                    text += f"\n\n📝 **Code Changes Ready**\n_{commit_msg}_\n"
                    text += f"<details>\n<summary>View Changes</summary>\n\n```diff\n{patch}\n```\n</details>"
            elif "bashOutput" in art:
                cmd = art["bashOutput"].get("command", "")
                code = art["bashOutput"].get("exitCode", 0)
                text += f"\n\n💻 **Command Executed:** `{cmd}`\nExit Code: {code}"

    try:
        from aiogram.types import InputRichMessage
        await bot.send_rich_message(
            chat_id=chat_id,
            message_thread_id=topic_id,
            rich_message=InputRichMessage(markdown=text),
            disable_notification=not is_important
        )
    except Exception as e:
        logger.error(f"Failed to send activity message to {topic_id}: {e}")
