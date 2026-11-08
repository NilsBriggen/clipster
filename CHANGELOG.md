# Changelog

Each version's section is also its GitHub release description: the release workflow publishes it and refuses to release a version without one.

## 0.3.0 (2026-11-08)

The Desk release. Clipster grew from a chat balloon into an office helper with a window of his own: Clipster's Desk keeps your day, your to-dos, your clipboard and the upkeep of your computer in one place. He also learned to proofread, write letters, run focus sessions, tidy your downloads and back up your files, and he welcomes new users with a proper Windows 98-style setup.

### Highlights

- **Clipster's Desk** ("open my desk", Meta+Shift+C, or right-click him), with eight tabs:
  - **Today:** time per kind of app, your focus sessions, your daily goal and limits, and today's top 3
  - **Week:** the last seven days, with a week in review you can copy
  - **To-do:** Today, Upcoming and Later, a star for the top 3, priorities, repeating items, double-click to rename
  - **Reminders** and notes
  - **Clipboard:** your last copies, plus pinned snippets that are kept
  - **Upkeep:** a computer checkup, tidying and backups
  - **You & me:** what he knows about you (forget any of it), your taught phrases, and every achievement with its progress
  - **Settings:** everything that used to be squeezed into the speech balloon

  It comes in the Windows 98 look, or a flat modern one for the faint of heart.
- **Your day:**
  - Reminders on other days: "remind me on Friday at 3 to call Anna"
  - A real to-do list: "todo send the report by friday", "todo water the plants every monday"
  - Focus sessions that nudge you back from videos and silence notifications
  - A daily focus goal and limits for time sinks: "limit video to 30 minutes a day"
  - "plan my day", "wrap up my day" and "my week in review"
- **Writing help:**
  - "proofread this" finds doubled words, common misspellings, a/an, its/it's and long sentences, and fixes the safe ones.
  - Twelve letter templates (complaint, cover letter, out of office…) to fill in, copy or open as an email.
- **Looking after the computer:**
  - A checkup for free space, the trash, temporary files, system updates, battery health and apps that start with the computer, each with a fix (he asks before deleting anything).
  - "tidy my downloads" sorts loose files into folders, and "undo the tidy" puts them back.
  - Backups to a USB drive or folder copy only what changed and never delete anything.
- **He knows you better:**
  - He notices what you're working in from the active window's title ("It looks like you're writing a letter…"). Titles never leave your computer.
  - He remembers what you tell him ("my cat is called Mo", "I live in Bern"), asks you things now and then, and brings them up later.
  - Once a day he asks how you're doing.
- **More alive:** four new idle animations, arcade sounds, tips for the app you're in, about 25 special days, help sorted by topic, and thousands of new lines in both voices, including some 80 office-life topics.
- **One-click updates:** from this version on, "Install now" downloads the next release, checks it, installs it and restarts Clipster.
- **Lighter:** he draws at 10 frames a second while nothing moves, so he uses much less CPU when idle.
- **A new welcome:** a Windows 98-style setup and tour window, with Try it buttons that run real examples. If you've used Clipster before, he offers the tour once after the update. "Show me around" opens it any time.

### Fixes

- "forget it" and "clear my notes" no longer delete your notes without asking.
- Linux: emptying the trash works on desktops without gvfs (it falls back to ktrash).
- Windows: "Clear temp files" only removes files older than a week. It used to empty the whole temp folder, including a downloaded update.
- Windows: the summon keys no longer fire on AltGr+C (typing "ć").
- The battery reading no longer picks up a wireless mouse.
- Text fields have their padding back.

### Install

- **Linux:** download `clipster_0.3.0_x86_64.AppImage`, make it executable (`chmod +x`) and replace your old AppImage with it. Your settings, scores, notes and taught phrases are kept.
- **Windows:** run `clipster_0.3.0_x64_en-US.msi`. It replaces the old version and keeps your settings. The installer still isn't code-signed, so SmartScreen may say "Windows protected your PC". Choose **More info → Run anyway**.

### Known limitations

- **Updating from 0.2.0 is still manual:** 0.2.0's update notice only links to the download page. From 0.3.0 on, "Install now" does it for you.
- **Do Not Disturb on Windows:** Windows keeps its Focus switch for its own apps. During focus sessions Clipster opens the Focus settings for you instead, and stays quiet himself.
- **The Windows checkup and backups are only tested by the build machine:** the Recycle Bin, the Windows Update count, battery health, start-up apps, drive detection and backups compile and pass their tests on Windows, but haven't yet been run by hand on a Windows PC. If something looks wrong, please [report it](https://github.com/NilsBriggen/clipster/issues).
- **Update counts:** they come from the package manager (Arch, Debian/Ubuntu, Fedora, plus Flatpak) or from Windows Update's last scan. AUR, Store and winget apps aren't counted.
- As before, outside KDE Plasma there are no global shortcuts on Linux (to call Clipster or open the Desk), and no mouse-following.

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
