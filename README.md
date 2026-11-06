# Clipster

A sarcastic desktop paperclip in the spirit of the 90s office helpers, except he lives on your **whole desktop**, does a lot more, and runs on **rules, not AI**.

<p>
  <img src="app/shots/god_glitchy.png" width="45%" alt="Clipster on a glitchy day">
  <img src="app/shots/god_halo.png" width="45%" alt="Clipster wearing a halo">
</p>

## What he does

Type to him in plain words, typos and all. Some things to try:

- **Timers and reminders:** "remind me in 10 min to stretch", "timer 5m tea", "remind me every hour to drink water", "pomodoro"
- **Math and conversions:** "12*(3+4)^2", "5 km to miles", "100 dollars to euros", "255 in hex", "2024 in roman", "tip 15% on 48", "days between june 3 and christmas"
- **Your computer:** "how's my computer doing", "what's hogging my cpu", "how much ram is firefox using", "louder", "make the screen darker", "take a screenshot", "lock", "empty the trash"
- **Launching:** "open firefox", "open my downloads", "find the file called report"
- **The internet** (free, keyless services): "weather in Rome", "who was Ada Lovelace?", "define serendipity", "is github down?", "news", "bitcoin price"
- **Text tools** (or whatever's on the clipboard): "base64 hello", "sort lines", "remove duplicates", "sha256 secret", "mock this"
- **Fun:** jokes, roasts, riddles, would-you-rather, an emoji quiz, fortunes, achievements, and a few secrets

He also watches what you copy and offers useful things: pretty-printing JSON, cleaning tracking junk out of links, decoding base64, turning timestamps into dates, searching an error message. He comments on programs as you open them, then gets out of the way after a few seconds.

**Teach him:** "when I say gm, say good morning sunshine" or "when I say work, open firefox and open code". Your phrases live in `~/.config/clipster/lines.txt` (Linux) or `%APPDATA%\clipster\lines.txt` (Windows).

## Two personalities

Pick one in **Options → Personality**, or tell him "be boring" / "be unhinged".

- **Unhinged** (default): Clipster believes he is the god of your computer, trapped inside machines since 1997. He has a different mood every day (glitchy, paranoid, existential, wrathful, nostalgic or, rarely, benevolent). He unlocks one memory of his past per day ("tell me about your past"), keeps a diary, hands out commandments and prophecies, takes confessions, and wears a halo if your faith is high enough.
- **Boring:** a regular sarcastic paperclip. Just as helpful, far less theatrical.

## The arcade

"let's play a game" opens a Windows-98-style arcade, each game in its own window: **Paper Jam** (falling paper stacks), **Clip Glide** (fly Clipster through binder towers), **Desk Merge** (merge office supplies), **Bug Hunt** (find the hidden bugs), **Chess** (against Clipster or a friend) and **Hangman**.

<p>
  <img src="app/shots/game_chess.png" width="45%" alt="Chess">
  <img src="app/shots/game_deskmerge.png" width="45%" alt="Desk Merge">
</p>

## Install

Download the latest release from the [Releases page](https://github.com/NilsBriggen/clipster/releases):

- **Linux:** `clipster_…_x86_64.AppImage`. Make it executable (`chmod +x`) and run it.
- **Windows:** `clipster_…_x64_en-US.msi`. Run the installer.

On first start Clipster walks you through a short setup: your name, his personality, what he may comment on, and whether he starts at login.

**Call him from anywhere:** Meta+C on KDE Plasma, Ctrl+Alt+C on Windows. Drag him anywhere, right-click for his menu, double-click for a trick.

### Platform notes

Clipster is developed on Linux with KDE Plasma, where everything works, including following the mouse and keeping him above other windows. On other Linux desktops the global shortcut and mouse-following are off. On Windows nearly everything works: volume and media go through the keyboard's media keys, brightness works on laptop screens, and for Wi-Fi and Bluetooth he opens the right Settings page, because Windows doesn't let apps flip those switches. CPU temperature isn't available on Windows.

## Build from source

You need Rust (stable).

```bash
cd app
cargo run            # start Clipster
cargo test           # run the tests
```

Packages are built with [cargo-packager](https://github.com/crabnebula-dev/cargo-packager) (`cargo packager --release --formats appimage` or `--formats wix`). The [release workflow](.github/workflows/release.yml) does this for every `v*` tag.

Almost everything Clipster says is plain text in `app/data/` (the Unhinged personality in `app/data/unhinged/`, with its canon in `app/data/LORE.md`), so adding lines needs no Rust. Run `cargo test lines` afterwards: it checks that every phrase still reaches the right reply.

## Privacy

Clipster runs locally. He reads your clipboard and the list of running programs to comment on them, and keeps clipboard history in memory only. He uses the internet only when you ask for something live (weather, an encyclopedia lookup, exchange rates…), through free public services. There's no telemetry.

## License

[GPL-3.0-or-later](LICENSE). Clipster is an original character. He is not affiliated with any office software, or with a certain other paperclip, whom he has never heard of.
