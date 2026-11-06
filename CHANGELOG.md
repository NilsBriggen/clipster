# Changelog

Each version's section is also its GitHub release description: the release workflow publishes it and refuses to release a version without one.

## 0.2.0 (2026-11-06)

The arcade release. Every game was reworked, Clip Glide is actually playable now, and there's a new game, Byte Size. Clipster also got much better at plain conversation, and from now on he tells you when a new version is out.

### Highlights

- **A properly working arcade:**
  - Games run at the same speed on every screen. Before, they could run about four times too fast on high-refresh monitors.
  - The game window gets the keyboard when it opens, so your keys no longer go to Clipster.
  - Esc or P pauses, and a game pauses by itself when you switch away.
  - Each game shows your best score and celebrates a new record, and Clipster comments on big moments.
  - Everything uses a classic 98-style font.
- **Clip Glide:** the clip hovers until your first flap, then glides instead of dropping like a stone. It starts gentle and speeds up slowly, and the hitbox matches the drawn wire. The backdrop is a full office.
- **Paper Jam:** smooth key-holding (holding Space no longer drops piece after piece), a short delay before a piece locks, a hold slot (C), turning both ways (X and Z), brighter paper and line-clear animations.
- **Bug Hunt:** a clean board with bigger tiles, bug and time counters, openings that ripple outwards, and a best time for each difficulty.
- **Desk Merge:** steady physics (stacks no longer sink into each other), merges that pop one after another, drawn office supplies instead of labels, and keyboard aiming.
- **Chess:** drag and drop, a move list, Undo, Flip board, playing as Black, draws by threefold repetition, and a Clipster who thinks within a time limit instead of freezing.
- **Hangman:** an animated paperclip, green and red letter keys, and win streaks.
- **New game, Byte Size:** slide files around a 4×4 disk. Two files of the same size merge into one twice as big. Build a 2 KB file to win, then keep going until the disk is full. Arrow keys, WASD or drag.
- **Better conversation:**
  - Everyday sentences get his own answer instead of an encyclopedia lookup. That covers "I'm tired of meetings", "do you like pizza", "my cat is sleeping on my keyboard" and over a hundred small-talk topics, from food and weather to pets and printers.
  - Real questions ("who painted the Mona Lisa?") still get looked up, and he no longer stops at "may refer to" pages. He offers the likely meanings instead.
- **Update notice:** once a day Clipster checks GitHub for a newer version and tells you once, with a download button. Ask him to "check for updates" any time, or switch it off in Options.

### Fixes

- **Linux:** "Start Clipster when I log in" now works for the AppImage. It used to point at a temporary folder that disappears when Clipster closes. If you turned it on in 0.1.0, starting 0.2.0 once fixes the entry.
- Talking about a game ("I lost at chess") no longer opens the arcade.

### Install

- **Linux:** download `clipster_0.2.0_x86_64.AppImage`, make it executable (`chmod +x`) and run it. To update, replace your old AppImage with this one. Your settings, scores and taught phrases are kept.
- **Windows:** run `clipster_0.2.0_x64_en-US.msi`. It replaces 0.1.0 and keeps your settings. The installer still isn't code-signed, so SmartScreen may say "Windows protected your PC". Choose **More info → Run anyway**.

### Known limitations

- **Updating from 0.1.0 is manual:** 0.1.0 can't check for updates, so download this version yourself. From 0.2.0 on, Clipster tells you.
- **Feel needs real players:** the games' controls and difficulty are tuned with automated tests, not long play sessions. If something feels off, please [report it](https://github.com/NilsBriggen/clipster/issues).
- The Windows and other-desktop limits from 0.1.0 still apply. Wi-Fi and Bluetooth open Settings, and there's no CPU temperature on Windows. Outside KDE Plasma, the global shortcut and mouse-following are off.

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
