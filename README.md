# EasierJules (Rust Edition) 🦀🤖

An idiomatic, high-performance Telegram bot written in Rust with [Teloxide](https://github.com/teloxide/teloxide) that interfaces with **Google Jules API** (`v1alpha`)—Google's autonomous coding assistant for GitHub repositories.

---

## 🌟 Features

- **Private Chat Setup & Security**:
  - `/start` claims bot ownership on first launch (rejects unauthorized users).
  - Interactive FSM dialogue to configure your Google Jules API Key securely.
  - One-click forum topic launcher (`New Task`) directly from private chat.
- **Group & Forum Mode Integration**:
  - Requires a Telegram **Supergroup** with **Topics (Forum mode)** enabled.
  - Interactive admin permission validation (`can_manage_topics`, `can_pin_messages`, `is_forum`).
  - Strict security guard: automatically drops updates and leaves unauthorized groups.
- **Interactive Task Lifecycle in Topics**:
  - Toggle **Auto PR** mode on or off.
  - Fetches GitHub repositories linked to your Google Jules account.
  - Branch selection picker for the chosen repository.
  - Launch task session by sending your task prompt into the topic.
- **Pinned Rich Status Cards**:
  - Pinned Markdown status table reflecting real-time task status:
    ```markdown
    # {Session Title}

    | **Status** | **Auto PR** | **State** |
    |:---|:---|:---|
    | `Active` | `ON` / `OFF` | 🧠 `PLANNING` |
    ```
  - Direct action buttons: `[ Open Session ]` and `[ View PR ]` (or `[ No PR ]`).
- **Live Activity Poller**:
  - Background worker polling every 5 seconds for status updates.
  - Truncates unified git diff patches (at 3,000 characters) wrapped in expandable details blocks.
  - Formats bash commands, execution exit codes, plans, and progress updates.
- **Topic Closure & Auto-Reopen Timer**:
  - When an owner closes a forum topic, prompts whether to delete/archive the Jules session.
  - 5-minute timeout: automatically reopens the topic if no response is received.
  - Asynchronously cancellable via Tokio `CancellationToken`.

---

## 📋 Prerequisites

- **Rust toolchain** (1.75+ recommended):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **Telegram Bot Token** from [@BotFather](https://t.me/BotFather).
- **Google Jules API Key** (accessible with `x-goog-api-key`).

---

## 🚀 Installation & Build

1. Navigate to the project directory:
   ```bash
   cd EasierJulesBot
   ```

2. Build the project:
   - **Debug build**:
     ```bash
     cargo build
     ```
   - **Optimized release build**:
     ```bash
     cargo build --release
     ```
   The compiled executable will be located at `target/release/easier_jules`.

---

## ⚙️ Configuration (`.env`)

Create a `.env` file in the root of `EasierJules-rs`:

```bash
# Telegram Bot Token from @BotFather (Required)
BOT_TOKEN=1234567890:ABCdefGHIjklMNOpqrsTUVwxyz

# Optional: HTTP/HTTPS or SOCKS5 Proxy
# PROXY=socks5://127.0.0.1:9050
# PROXY=http://127.0.0.1:8080

# Optional: SQLite Database Path (Defaults to "bot.db")
DB_PATH=bot.db
```

---

## ▶️ Running the Bot

### Option 1: Run with Cargo
```bash
cargo run --release
```

### Option 2: Run the Compiled Binary
```bash
./target/release/easier_jules
```

### Option 3: Run as a systemd Service (Production)

Create `/etc/systemd/system/easier-jules.service`:

```ini
[Unit]
Description=EasierJules Telegram Bot
After=network.target

[Service]
Type=simple
User=ubuntu
WorkingDirectory=/path/to/EasierJulesBot
ExecStart=/path/to/EasierJulesBot/target/release/easier_jules
Restart=always
RestartSec=5
EnvironmentFile=/path/to/EasierJulesBot/.env

[Install]
WantedBy=multi-user.target
```

Enable and start the service:
```bash
sudo systemctl daemon-reload
sudo systemctl enable easier-jules
sudo systemctl start easier-jules
sudo journalctl -u easier-jules -f
```

---

## 🧪 Running Tests

The test suite validates database upserts, Jules API Serde models, and inline keyboard generators:

```bash
cargo test
```

---

## 💡 How to Use the Bot

1. **Private Chat**:
   - Send `/start` to your bot in Telegram to claim ownership.
   - Click **Connect Account** and reply with your Google Jules API Key.
   - Click **Setup Group**.
2. **Add to Group**:
   - Create or open a Telegram Supergroup and ensure **Topics (Forum mode)** is turned on in Group Settings.
   - Add the bot as an Administrator with:
     - ✅ **Manage Topics**
     - ✅ **Pin Messages**
   - The bot will greet you and verify all permissions.
3. **Start a Task**:
   - Either click **New Task** in private chat or create a new topic directly in the forum group.
   - In the topic menu:
     - Toggle **Auto PR** (`ON` or `OFF`).
     - Click **Proceed ➔**.
     - Choose a GitHub repository.
     - Choose a branch.
     - Send your task description/prompt in the topic.
4. **Interact**:
   - Any message you send in the topic is forwarded to Jules.
   - Status updates, code patches, and command outputs will stream directly into the topic.
