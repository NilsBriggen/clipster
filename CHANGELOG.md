# Changelog

Each version's section is also its GitHub release description: the release workflow publishes it and refuses to release a version without one.

## 0.1.0 (2026-11-06)

The first public release of Clipster: a sarcastic desktop paperclip in the spirit of the 90s office helpers. He lives on your whole desktop instead of inside one program, and he runs on rules, not AI.

### Highlights

- **Talk to him in plain words**, typos and all: timers and reminders, math and unit conversions, launching apps and folders, and system info and controls (volume, brightness, screenshots, lock, shutdown). Ask for live lookups too (weather, encyclopedia, exchange rates, news) through free public services, and use his text tools.
- **Smart clipboard:** he pretty-prints copied JSON, strips tracking junk from links, decodes base64, turns timestamps into dates and offers to search copied error messages.
- **Two personalities**, switchable in Options:
  - **Unhinged** (default): he believes he's the god of your computer, trapped inside machines since 1997. He has daily moods and glitches, unlocks one memory of his past per day, and keeps a diary, commandments and prophecies. He also takes confessions.
  - **Boring:** a plain sarcastic paperclip.
- **A Windows-98-style arcade:** Paper Jam, Clip Glide, Desk Merge, Bug Hunt, Chess (against Clipster or a friend) and Hangman.
- **And more:** a short first-run setup, achievements, holiday outfits, comments on the programs you open (they close by themselves after a few seconds), and phrases you can teach him ("when I say gm, say good morning").

### Install

- **Linux:** download `clipster_0.1.0_x86_64.AppImage`, make it executable (`chmod +x`) and run it. He's happiest on KDE Plasma, where Meta+C calls him from anywhere, his eyes follow your mouse and he stays above other windows.
- **Windows:** run `clipster_0.1.0_x64_en-US.msi`. The installer isn't code-signed yet, so SmartScreen may show "Windows protected your PC". Choose **More info → Run anyway**. Call him from anywhere with **Ctrl+Alt+C**.

### Known limitations

- **Windows:** he can't flip the Wi-Fi and Bluetooth switches himself (he opens the right Settings page instead), and CPU temperature isn't available. This is the first Windows build. It's tested automatically but hasn't had much real-world use yet, so please [report](https://github.com/NilsBriggen/clipster/issues) anything odd.
- **Other Linux desktops:** outside KDE Plasma, the global shortcut and mouse-following are off.
