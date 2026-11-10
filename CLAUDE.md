# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

**Clipster**: a Linux desktop pet in the style of the old Office paperclip assistant. It runs system-wide (not inside an app), is meant to be funnier and much more capable, and uses **no AI**: everything is rules, keyword tables and free keyless web APIs. The original request and build-check history are in `craft.json`.

Only `app/` is the program (a single Rust crate, eframe/egui). The other top-level files and folders (`craft.json`, `feed.json`, `build-activity.log`, `checks/`, `investigation.cany/`, `usage.json`, `pinned/`, `inbox/`, `reference/`) come from the tool that built the project. They are not part of the crate.

## Commands

Run all cargo commands from `app/`.

```bash
cargo build
cargo run                                   # the real pet: watcher, pointer tracker, saved settings
cargo test                                  # offline unit tests (brain, understand, fun, powers, net helpers)
cargo test routing                          # one test, by name substring
cargo test live -- --ignored --nocapture    # calls every real web service in net.rs and prints OK/ERR
```

**Screenshot mode** is how you check a change visually without a display session or touching the user's state:

```bash
cargo run -- --screenshot shots/joke.png --say "tell me a joke"
```

`CLIPSTER_MOOD=glitchy cargo run -- --screenshot shots/x.png --say "what's your mood"` forces Unhinged Clipster's mood of the day, and `CLIPSTER_ANIM=juggle` (newspaper, nap, typing, knit, plane, plant, yoyo, doodle, binoculars, or a mood like alarm, gift, carry, read, write, fix, coffee) shows one animation, and `CLIPSTER_ANIM_AT=<seconds>` freezes it at that moment. `cargo run -- --play chess --screenshot shots/chess.png` captures an arcade game instead (it's drawn inside the main window in this mode), `--play desk` captures Clipster's Desk with made-up data (`CLIPSTER_DESK_TAB` takes a number or any name `desk::tab` / `Desk::set_tab` know: the tabs `today`, `week`, `todo`, `agenda`, `clipboard`, `write`, `upkeep`, `you`, `settings`; pages such as `next7`, `focuslog`, `plan`, `planweek`, `list shopping`, `month`, `calendars`, `links`, `files`, `people`, `journal`, `checkup`, `rightnow`, `tidy`, `space`, `backup`, `startup`, `myday`, `comments`, `data`; and cards such as `wrapup`, `workday`, `habits`, `countdowns`, `kind video`, `todo #12`, `newlist`, `older`, `event`, `jot`, `person Anna Muster`. `CLIPSTER_DESK_LOOK=modern` picks the look, `CLIPSTER_DESK_SIZE=large` the larger size, `CLIPSTER_SAMPLE=crowded` or `empty` long lists or a new install instead of the normal sample data), and `--play welcome` captures the welcome window (`CLIPSTER_WELCOME_PAGE=1`–`15`), and `--say prank` the April Fools window. It draws 120 frames on a teal background, saves the PNG and exits. In this mode the clipboard/process watcher, the KWin pointer tracker, sounds and settings load/save are all off. `--say <cmd>` sends one command at startup. It also works without `--screenshot`. The special values `options`, `menu` and `hide` open those UI states (in screenshot mode `options` captures the Desk's Settings tab). Careful: `--say` really runs the command's effect, so `--say "run …"` runs a shell command and `--say "open …"` launches something.

Screenshot mode needs an unlocked desktop session. While the screen is locked, the compositor presents no frames and the run times out.

Debug builds keep line tables only and the dependencies no debug info (`[profile.dev]` in `app/Cargo.toml`); release builds are stripped. CI runs `cargo check` and `cargo test`. Never put a `target/` folder in `/tmp` or the scratchpad: on the owner's machine `/tmp` is RAM.

Packaging (what the release workflow runs): `cargo install cargo-packager --locked`, then `cargo packager --release --formats appimage` (Linux) or `--formats wix` (Windows, downloads WiX 3 itself). Output goes to `app/dist/`. Config lives in `[package.metadata.packager]` in `app/Cargo.toml`, and the icons are in `app/assets/`.

Releasing:
1. Bump `version` in `app/Cargo.toml`.
2. **Write the release description**: a `## <version> (<date>)` section at the top of `CHANGELOG.md`, in the same shape as 0.1.0 (a short intro, Highlights, Install with the exact file names, Known limitations). Every release gets one. The release job publishes that section and fails without it, so auto-generated notes are never the description.
3. Commit, then push a `v<version>` tag. Installed copies find it through GitHub's "latest release" API (`net::latest_version`, checked once a day while "Tell me when a new version is out" is on), so it must be a published, non-prerelease release. "Install now" (`net::download_update`) downloads the release's `.AppImage` / `.msi`, checks it against the size and SHA-256 `digest` GitHub lists for each asset, swaps the AppImage in place or runs the MSI, and `main()` restarts Clipster after the window closes (`relaunch`). Keep the asset names ending in `.AppImage` and `.msi`. `.github/workflows/release.yml` checks that the tag matches `version` in `app/Cargo.toml`, builds both packages and publishes the GitHub release. Running it by hand only builds artifacts. `.github/workflows/ci.yml` builds and tests on Ubuntu **and Windows** for every push. It's the only Windows check, since `cargo check --target x86_64-pc-windows-msvc` fails locally: `ring` needs MSVC.

Environment: if `DISPLAY` is set, `main()` removes `WAYLAND_DISPLAY` to force XWayland, because Wayland won't let the window stay on top or move itself. Set `CLIPSTER_WAYLAND=1` to stay on native Wayland.

## Architecture

### Command pipeline (text in → `Reply` out)

Everything the user types, every button click and every `--say` goes through `App::run` (`app/src/main.rs`) → `brain::respond` → `understand::respond`:

1. Commands starting with `run`/`$`/`sh`/`cmd`, and the button-only commands whose text is data (`answer …|…`, `answer-to`, `reveal`, `wyrpick`, `window`, `checkin`, `install`, `forget`, `tidy`, and the Desk's `pref`, `clip`, `forgetfact`, `startup`), go straight to `brain::exact` and are never "understood".
2. `understand::clean` normalises the free text: removes filler and politeness, turns number words into digits, etc.
3. `understand::intent` tries, **in order**:
   - the crisis check (self-harm phrases get a sincere reply and a helpline button; keep it first)
   - `teach` ("when I say X, say Y")
   - the user's own taught phrases (`lines::topic(s, true)`)
   - easter eggs
   - timers → notes → powers (the groups with side effects)
   - **built-in data topics** (`lines::topic(s, false)`)
   - extras → maths → verbs → text_tools → chance → chat → system → questions

   Most groups rewrite the sentence into a strict command and hand it to `brain::exact`.
4. If nothing matches, `brain::exact` is tried on the raw text and on the cleaned text.
5. If that fails too, `fix_typos` (edit distance against `VOCAB`) retries with a "(I'm guessing you meant …)" prefix.
6. Last comes `fallback`. Questions go to the internet (`ask …`). Anything else gets up to two keyword-scored guess buttons from `GUESSES`.

`brain::exact` is the strict grammar ("timer 5m tea", "5 km to miles"). `understand.rs` is the natural-language layer on top of it. To add a feature, add the strict command to `exact` (or a helper in `powers.rs` or `fun.rs`), then add the plain-English phrasings in the right `understand.rs` intent group. Order matters: an earlier group can swallow your phrase.

### Response database (`app/src/lines.rs`, `app/data/*.txt`)

Almost everything Clipster *says* is plain text in `app/data/humor.txt`, `chat.txt` and `reactions.txt`. They're compiled in with `include_str!`, and the user's own `~/.config/clipster/lines.txt` is merged on top at runtime. The format is documented at the top of `lines.rs`:
- `## name` starts a section; `chat <id>` sections are topics and `program <id>` sections are reactions to programs starting
- `>` trigger phrases, `=` exact phrases
- `@` mood, `+ Label = command` buttons, `% ±n` friendship change
- `!` command to run (user file only)
- anything else is a reply line

Rules that are easy to miss:
- Sections with the same name merge.
- Triggers go through `understand::clean` when they load. A trigger that tidies down to a single word (for example "can you swim" → "swim") is refused, because it would match almost any sentence.
- Among topics, exact phrases win, then the longest matching trigger.
- Code asks for sections by name: `lines::pick` / `reply` / `all`. Placeholders such as `{name}`, `{pct}`, `{label}` and `{choice}` are filled by whoever asks. `{name}` is filled in `App::show`.
- `.grumpy` / `.clingy` variants of a section are picked automatically when the friendship level is ≤ −40 or ≥ 40.
- Structured pools use ` | ` between fields: riddles, wyr, emoji_quiz, achievements, misspellings (`wrong | right`, read by `writing::proofread`).
- `template <id>` sections are letter templates: raw-keyword `>` triggers and exactly one line, the whole text (`Subject: …\n\n…`, `[brackets]` to fill in, `{date}`, `{name}`). `tips <kind>` sections are keyboard tips per window kind (`general` and `desktop` too). `occasion MM-DD <name>` (or `occasion friday13`) sections are special-day greetings that `occasion()` finds by date. `welcome <page>` sections are the welcome window's page texts (one line each, plain, so Boring-only) and `welcome_say <page>` what he says under his picture there (both voices). Boring-only data (templates, misspellings, welcome pages) lives only in the Boring files.
- `window <kind>` sections react to the active window's title, `site <kind> <id>` sections to one website or app in it (their own lines and cooldown; the site's piece also decides the kind for time tracking), `program <id>` sections to process names, and `answer <id> <kind>` sections to answers. Their `>` triggers are raw lowercase pieces, not tidied sentences, so single words are fine there (site pieces need 4+ characters). The longest piece wins. Window and site lines may quote the page title with `{title}` (never in banking, markets, passwords, email, inbox or chat sections).
- `ask <id>` sections are questions he asks now and then; your next typed line is the answer (`answer-to <id> <text>`, taken literally), and `answer <id>` (generic, `{x}`) or `answer <id> <kind>` reacts. The ids `city`, `job`, `pet`, `fav_food`, `fav_music` also store the answer as a fact.
- Facts the user tells him (`understand::remember`: "my cat is called Mo", "I live in Bern", "my favorite food is pizza") live in `App.facts` and are mirrored into `lines::set_facts`. `App::show` fills `{pet}`, `{pet_kind}`, `{job}`, `{city}`, `{partner}`, `{partner_word}`, `{fav_<thing>}`; `callback_<key>` pools bring them up later; weather without a place uses `city`.

**Two personalities.** Boring is `app/data/*.txt`. Unhinged (the default; switched in Options or with "be boring" / "be unhinged", saved as `persona` in settings) lays `app/data/unhinged/*.txt` on top. A section there **replaces** the Boring section of the same name, new names are added, and `scripture.txt` holds the Unhinged-only content.
- Canon, voice and the **hard limits** (for both voices, decided by the owner in a questionnaire) live in `app/data/LORE.md`: he believes he's the god of the computer, trapped since 1997. Read it before writing Unhinged lines.
- In Unhinged mode he wakes up in one of six moods per day (`mood_for` in `main.rs`, overridable with `CLIPSTER_MOOD=glitchy`). `name.<mood>` variants win over friendship variants, glitchy days corrupt some lines (`lines::glitch`, which never touches `{placeholders}` or `say_*` confirmations), and the look changes (halo at faith ≥ 80, beige/ember skins).
- Memory fragments unlock one per day of use (`memory_unlocked`).
- Hard-coded replies that differ by personality live in `say_*` pools. Use `lines::pick("say_…")` instead of writing a string in Rust.
- An Unhinged section that replaces a Boring one must keep its triggers, buttons, mood, `%` and the placeholders every Boring line has; `unhinged_overlay_keeps_structure` enforces it.
- An Unhinged `## +name` adds to the Boring pool instead of replacing it (his own jokes, puns and facts come on top of the plain ones). Data pools that are the same in both voices (riddles, wyr, emoji_quiz, hangman_words) live only in the Boring files.

**Sentence patterns** (`## pattern <id>` sections with `~ i am * | i feel *` lines) are the safety net for conversation.
- They're matched on `understand::tidy_words`, which keeps openers like "i want" and "can you" that `clean` drops.
- `*` captures the rest, and replies use `{x}`: the capture with pronouns flipped by `lines::reflect`, so "my boss" becomes "your boss".
- The most specific pattern wins. Captures longer than 6 words use the replies without `{x}`.
- Patterns run after every command group and topic, and before `questions()`, except for encyclopedia questions (`understand::knowledge`).
- When nothing matches, `fallback` sends only real lookups to the internet: knowledge questions, or a bare name or thing of up to three words. Everything else gets a line from `unknown_statement` / `unknown_question`, with web search as a button.
- `net::wiki` skips "may refer to:" disambiguation pages and offers the likeliest meanings as buttons instead.

After any data change run `cargo test lines`. It checks that everything parses and every button leads somewhere. It also checks that **every chat trigger reaches its own topic** and that a list of real commands isn't swallowed by a chat topic. Fix a failure by editing the trigger, not the test.

### `Reply` / `Effect` / `Act` (in `app/src/brain.rs`)

- Logic functions are **pure**: they return a `Reply { text, buttons, effect, mood, ask }` and never touch the GUI or the system themselves. That is why `understand::tests` can assert on `respond("…").effect` with no window.
- `Effect` is the side effect that `App::run` applies: timers, shell (on a background thread, output comes back as `Event::ShellDone`), open, net queries, clipboard, stopwatch, hat/skin, autostart, quit, macros (`Commands`), `Play` (arcade), `SetBirthday`, `Teach` / `Unlearn` (they write the user's lines file), `ListAchievements`, the productivity effects (`Todo`, `Focus`/`FocusStop`, `TimeReport`, `Goals`, `Wrapup`, `Review`, `Tidy`/`TidyUndo`, `CancelTimer`, `AppTips`, `OpenDesk(tab)`, `DeskLook`, `Pref`, `Clip`, `Note`, `TimerOp`, `RepeatAt`, `Habit`, `Countdown`, `Journal`, `FocusUntilNext`, `KindMove`, `ReviewCsv`, `ShowQr`, `SaveStarter`, `Relearn`), the upkeep ones (`UpkeepCheck`, `OpenSystem`, `Startup`, `Backup`, `UpkeepHide`, `Space`), `Undo`, `DeskNet` (a web or file answer that goes onto a Desk page), `Reveal`, `Pick` (the system's folder/file dialog), `Print`, `Data` (export, import, forget) and `Tour` (the welcome window). Effects that come from the Desk (settings, stars, clipboard) set `silent` in `App::run`: they answer in the Desk's status bar, not a balloon. Anything that writes a user file (to-dos, tidying, notes, taught lines) is an effect, so tests never touch real files. A new kind of side effect needs a new `Effect` variant and a match arm in `App::run`. Tests may check that `respond()` returns `Effect::Teach`, but must never call `lines::teach`, because it writes the real user file.
- `Reply.friend` is a friendship change. `App::run` applies it, clamps the level to −100..100 and pushes it to `lines::set_level`.
- `Act` is what a balloon button does. `Act::Run(cmd)` sends the command back through the whole pipeline. Destructive things (power off, empty trash) are a confirmation `Reply` whose "Yes" button is `run <shell cmd>`. Keep it that way.
- `Mood` maps onto `Anim` in `main.rs`. `Anim` also has idle-only animations that no `Mood` triggers.

### Background threads → `Event` channel

All of these send `watch::Event` over one `mpsc` channel and call `request_repaint`. `App::tick` drains the channel each frame, along with timers, tips, break reminders and sleepiness.

- `watch.rs` polls the clipboard every 1 s, `/proc/*/comm` every 4 s (matched against the `program` sections), battery and load every 30 s, and, outside KDE, the active window's title every 2 s (`sys::active_title`: `GetWindowTextW` on Windows, one kept-open X connection through `x11rb` on X11; `running_programs` reads a Toolhelp snapshot on Windows). On KDE the KWin script in `pointer.rs` reports titles instead (`Tracker::take_title`). Window comments have cooldowns (`brain::window_ok`), and `brain::TIME_SINKS` (video, social) only speak after 20 minutes in a row. Titles never leave the computer.
- Frames: `App::ui` asks for 33 ms frames only while something moves (animations, text typing out, drags, the arcade, the pointer over him) and 100 ms otherwise (250 ms asleep). The pointer tracker calls `request_repaint` when the mouse moves or the shortcut is pressed, so the eyes and Meta+C stay instant.
- `net.rs` turns a `Query` into a `Reply` with blocking `ureq` calls (10 s timeout). `App::ask_net` runs each query on its own thread and tags it with `net_id`, so only the newest answer reaches the balloon. Every query must degrade through `offline()` when the network is down.
- Shell effects report back as `Event::ShellDone`.

### Window and drawing (`app/src/main.rs`)

- Clipster is painted entirely with egui `Painter` shapes in `draw_clipster` / `draw_hat`. There are no image assets.
- The window is transparent and undecorated. It resizes between `CLIP_W` (clip only) and `FULL_W` (with balloon) and flips the balloon left or up away from the screen edges. Its position is tracked as `anchor`.
- `app/src/pointer.rs` matters only on KDE Wayland. It writes a KWin script into the runtime dir, loads it over D-Bus, and the script reports the global pointer position (for eye tracking) and the Meta+C summon shortcut back on the bus name `org.clipster.Pointer`. The script is unloaded on `Drop`. Everywhere else `pointer::start()` returns `None` without an error.

### Linux and Windows (`app/src/sys.rs`)

All OS differences go through `sys.rs`. Linux code is never written inline next to a Windows branch somewhere else.
- `sys::command` (no console window flashes on Windows) and `sys::shell` (`sh -c` on Linux, PowerShell on Windows, which is also what `run …` uses).
- Battery, CPU load, memory, uptime and running programs, with Win32 calls hand-declared in `sys::win` (no extra crates).
- Sounds, notifications, autostart, close and memory per program, OS name.

Feature commands pick their command string with `cfg!(windows)` in `brain.rs` / `powers.rs` / `understand.rs`. When adding a command that shells out, write both versions, or a graceful Windows reply like the Wi-Fi switch (it opens Settings) or the CPU temperature.

Other platform notes:
- `pointer.rs` has a KWin/D-Bus half (Linux) and a `GetCursorPos`/`GetAsyncKeyState` half (Windows, summon with Ctrl+Alt+C). Replies say "Meta+C", and `App::show` swaps in `sys::HOTKEY`.
- `zbus` is a Linux-only dependency.
- **Every feature works on both systems.** Where one really can't (Windows has no public Do Not Disturb switch for apps), the closest honest alternative is offered and said out loud. The upkeep functions in `sys.rs` (free space, trash, temporary files, pending updates, battery health, start-up apps, drives, Do Not Disturb) each have a Linux and a Windows body, and everything that parses command output is a plain function on `&str` that both CI runners test. The same goes for the newer ones: `reveal` (FileManager1 over D-Bus, else the folder; `explorer /select,`), `pick`/`can_pick` (zenity or kdialog; the Windows Forms dialogs through PowerShell), `top_programs` (`/proc` sampled twice, `parse_proc_stat`; `Get-Process` JSON, `parse_top_json`), `trash_file` (`gio trash`; the Recycle Bin through VisualBasic's `DeleteFile`) and `locale_region` (`LANG`; `GetUserDefaultLocaleName`).
- Tests that check a command string must be OS-aware (see `shell_has` in `understand.rs` tests), because CI runs them on Windows too.

### Productivity: to-dos, writing, focus, goals, tidying, upkeep, backups

- `todo.rs`, `notes.rs`, `timers.rs`, `habits.rs`, `journal.rs`, `writing.rs`, `tidy.rs`, `plan.rs`, `upkeep.rs`, `backup.rs`, `calendar.rs`, `people.rs`, `find.rs`, `print.rs`, and the glue in `main.rs`.

- **To-dos** live in `todo.txt` in the data folder, editable by hand: `- [ ] ! call Anna (due 2026-11-12) (every monday) (top 2026-11-08)`, `- [x] buy milk (done 2026-11-08)`. `!` is important, `top` is that day's top 3 (at most three), `every` repeats (ticking one off keeps it as history and adds the next: day, week, month, weekday, a weekday name, `year`, or `days-3` / `weeks-2` / `months-2`, which count from the day it's ticked; a late tick keeps the rhythm). `Item::overdue` is the one overdue rule (waiting items aren't overdue). The chat lists `todos due today|tomorrow|week|overdue` and `todos waiting`; "Ben sent the numbers" ticks the one item waiting on Ben (`todos gotfrom`). `todo::change` is pure and tested; `todo::apply` writes the file and sets a 9:00 reminder for a due date. The Desk names items by line (`todo #3 done|star|prio|edit|remove …`; only that exact shape: `todo #2 remove the old shed` is a new to-do); the chat matches by text (`todoremove <text>`). A `(list shopping)` tag puts an item on a named list (`onlist shopping add eggs, milk`, `onlist shopping clear`), which the to-do list itself leaves out; the To-do tab shows a chip per list.
- **Calendar** (`calendar.rs`): the `calendars` setting holds private iCal links (`webcal://` becomes https) or .ics files, read on a thread at start and every 15 minutes (`Event::Calendar`), with the last good copy cached for offline. `parse` handles folded lines, all-day, UTC, RRULE (DAILY/WEEKLY with BYDAY/MONTHLY with BYDAY or BYMONTHDAY/YEARLY, INTERVAL, COUNT, UNTIL), EXDATE, RECURRENCE-ID moves and cancelled events; TZID times are read as local time. A heads-up comes 10 minutes before timed events (through quiet mode, like reminders), with a Join link found in the event. Chat: `calendar today|tomorrow|day <date>|next|free <unix>|add <link>|remove <n>|refresh`.
- **People** (`people.rs`): the `people` setting (name, birthday, email, keep-in-touch days, last contact). `person add|birthday|email|every|met|last|mail|forget <name>|<value>`, `people`, `people birthdays`, `contacts import [file]` (.vcf). "I called mum" only counts for known names (`people::set_names`). Once a day after 8:00: a birthday in 3 days or today, or an overdue keep-in-touch.
- **Find and read later** (`find.rs`): `findfile <words>` walks the home folder (every word in the name, the extension counts; exact names first, then newest), `recent` reads `recently-used.xbel` (Linux) or the Recent folder (Windows). `readlater [link]` (no link: the newest copied one) keeps the `reading` setting; copied links get a Read later button.
- **Start my workday**: the `workday` setting lists apps and links; `workday` opens each through `open`, shows the Today tab and offers focus; weekday mornings he offers it once (`workday_ask`).
- **Goals and limits**: `focus_goal` (minutes a day) and `budgets` (minutes a day per time-sink kind) in settings. `plan.rs` holds the pure summaries (today's focus, the end of the day, the week in review). A used-up limit gets one `budget_over` a day; reaching the goal a `goal_met` and a streak. "plan my day", "wrap up my day" (offered after 5 pm) and "my week in review" (offered Friday afternoon) open the Desk on the right page.
- **Writing**: `writing::proofread` / `fix` are pure. `fix` only makes safe changes (doubled words, listed misspellings, spaces, capitals, a/an), never its/it's and the like. `letter` fills a `template` section and offers Copy and a `mailto:` link.
- **Dated reminders**: `understand::dated` turns "on Friday at 3", "tomorrow evening", "in 2 days", "in 6 months" into seconds (it tries every "at", reads "17.30", and "tonight at 12" is after midnight); timers are stored as unix times, so they survive a restart. `brain::parse_dur` knows days and weeks and refuses months and years (those go through `dated`). Buttons that set a reminder later use `timerat <unix>|<label>`, never a sentence.
- **Appointments** (`understand::appointment`): "dentist on Tuesday at 10" (a time must be said) offers a reminder an hour before, one the evening before, or a to-do; it never sets anything by itself.
- **Time per app kind and focus**: `App::account_time` adds the seconds spent in each window kind (only while window comments are on, not while you're away). Focus sessions nudge after a minute in a distraction kind (at most every 10 minutes) and pause the chatter. `kind_time` (7 days), `focus_log` (60 sessions) and `checkin_days` (14) are kept in settings and never leave the computer.
- **Tidying**: `tidy::plan` sorts loose top-level files by extension into category folders (skipping hidden, half-downloaded and just-changed files). `apply` only renames, never overwrites (a clash gets " (2)"), and writes `tidy-undo.json`; `undo` moves everything back and removes the emptied folders. Duplicates and big old files are read-only lists, run on the net thread. Tests use temp folders and their own undo log.
- **Upkeep**: `upkeep::run` reads the system (slow: it asks the package manager) on a background thread and sends `Event::Upkeep`; `upkeep::judge` turns the readings into checks (a level and a fix command; `@tidy`, `@backup`, `@startup` are the Desk's own). Once a week he checks quietly and speaks only if something needs attention.
- **Backups**: `backup::plan` decides what to copy (missing, different size, or more than 2 s newer; a newer backup copy is a conflict and left alone; names made FAT/NTFS-safe; links skipped), `backup::run` writes `name.clipster-part`, sets the date and renames. Nothing is ever deleted. It runs on a thread with `Event::BackupProgress` / `BackupDone`. Tests use temp folders.
- **Papers & things** (You & me's fourth page, `desk papers`): the renewals with Renewed, a to-do or Forget, where things are, and an add row for each.
- **Where things are** (`things.rs`): `things` in settings (thing → place, date). "My passport is in the top drawer", "where is my passport", "what's in my blue bag" (`thing set|where|in|forget`, `Effect::Things`); with nothing kept it searches the notes, then the files. It refuses codes and passwords, and `understand::where_things` leaves people, pets and idioms alone.
- **Renewals** (`renewals.rs`): `renewals` in settings (what, date, every month/year, lead days, amount, note). "My passport expires on 3 March 2027", "netflix renews on the 15th every month for 12.99", "what's expiring soon", "renewed the car insurance" (`renewal add|renewed|date|remove`, `Effect::Renewal`). ID papers get 120 days' notice, the rest 30; a daily heads-up on the lead day, a week before and on the day; something running out within two weeks is offered as a to-do.
- **The briefing** (`plan::briefing`, `Effect::Briefing`): "good morning" builds the day from what's on the computer (no network): meetings and free time, reminders, the top 3, due and overdue, chases, birthdays, countdowns, habits left, renewals, plus the cached weather; `briefing web` is the old online one (quote, on this day). The weekday-morning workday offer is the briefing now (not on public holidays).
- **More nudges**: ten days before a birthday with the saved present ideas (`gift_lead`), "how did it go?" after a meeting with heads-ups on (`after_meeting`; notes or a follow-up to-do, three a day at most).
- **More to-do tags**: `(at 14:00)`, `(link …)`, `(note …)`, `(waiting Ben)` (a follow-up date comes with it; Today shows a chase row on that day), `(steps id)` (its steps are a hidden `steps-<id>` list), `(week 2026-46)` (the week's three). Renaming takes a date or repeat like a new to-do ("… by friday", "… by none"). `done.txt` keeps what "Clear finished" takes away, so the week still counts it.
- **Reminders** (`timers.rs`): `Timer { id, at, label, every, rule }`; `timer #id cancel|snooze <dur>|at <when>|rename <text>`; rules like "weekday 08:00" or "m15 09:00" (`next_fire`) for "every weekday at 8", and the counted ones `parse_interval` makes: `n3/2026-11-09 09:00` (every 3 days from that day; weeks are n7, n14), `M2/2026-11-09 09:00` (every 2 months), `y03-03 09:00` (yearly; 29 February rings on the 28th). A ` -h` suffix skips public holidays ("every workday" gets it; `timers::set_holidays` is fed from the `holidays` cache). Everything due rings together in one balloon (`last_rung` is a list, Snooze takes all), and reminders that rang with nobody there (`unseen_rings`, cleared by any click or command) are listed under the next one. "Cancel all my reminders" asks first with the count (`Effect::CancelAsk`, then `cancel all confirmed`).
- **Notes** (`notes.rs`): `2026-11-09 * | text` (date, pin), `note #n edit|pin|remove|copy`.
- **Habits, countdowns, journal**: `habits.rs` (a few daily ticks with streaks; "I did my walk" only for known habits, like people; a habit can count towards a daily `target` ("8 glasses of water a day", `habit count`) or have dose `times` ("pills at 8 and 20"): each time is a timer labelled `habit:<Name>@HH:MM` that rings with Taken / In 15 min / Skip, `taken` logs the doses, and "did I take my pills" (`habit status`) answers from the log in plain, neutral words; an evening nudge (`habit_nudge`) comes before a streak of 3+ breaks), `countdowns` in settings (one cheer on the day, gone the next), `journal.rs` (`journal.txt`, a line and a mood a day; the check-in writes its mood there; `journal set` replaces a day's line).
- **Time per site**: `site_time` (7 days, section ids only, never titles) next to `kind_time`; `kind move <site> <kind>` files a site under another kind (`kind_moves`). `day_totals` keeps eight weeks of daily totals (work, distractions, other, focus) for "vs last week", the four-week bars and `plan::patterns` (three weeks needed).
- **Plans**: `plan::coming_up` (Today's strip), `week_ahead` (Next 7 days, with a busy bar from the working hours), `suggest_top3`, `free_slots`, `compare`, `limits_kept`, `focus_by_label`, `week_csv`, `day_story`. All pure and tested.
- **Calendar extras**: each calendar's own name (`X-WR-CALNAME`), your name, colour, Show and Heads-up per calendar (`calendar_meta`; hidden calendars stay in `events_all`, out of `events`), `calendar freeday <date>`, `calendar export timer|todo <n>` (`to_ics`, read back by `parse` in a test), `calendar notes <key>` (a notes file in Documents/Clipster notes). Public holidays (`holidays` setting, a country code) come from date.nager.at once a year (cached) and are added to the Desk's events.
- **Settings that shape the day**: `work_days`/`work_hours` (the workday offer, the wrap-up, the review, free time), `quiet_after`/`quiet_weekend`/`quiet_meetings` and `pref quietfor` (all in `App::quiet()`), `headsup`, `break_every`, `alarm_sound`, `muted` ("Not about this" on program and window comments), `desk_morning`, and the clock (`brain::clock`, `clock12`, `week_sunday`: use `brain::clock(t)` for times shown to people, never `format("%H:%M")`).
- **Upkeep extras**: `upkeep::now` (Right now: `sys::top_programs`, every 3 s while that page is open) and `upkeep::space` (Space: folder sizes and `find::biggest`; `space trash` asks first, then `sys::trash_file`). `upkeep_hide` hides checks, `space_log` keeps weekly free space. Tidy: `tidy::set_targets` (a category to your own folder), `tidy <which> skip 2,5 confirmed`, `tidy_auto` (Mondays, week-old files, `plan_older`); an empty plan is never applied (it would wipe the undo log). Backups: `backup_skip` (`backup::skipped`), `backup_auto` (plugged/evening), `backup stop` (an `AtomicBool` checked between files), `backup_history`, `backup find`/`restore` (`original_of`, `restore` beside the original as "(from backup)").
- **Your data**: `data export` copies settings.json, todo.txt, done.txt, notes.txt, journal.txt and your lines.txt into a dated folder in Documents; `data import <folder>` asks, keeps `.bak` copies, then reloads (`App::load`, `lines::reload`).
- **Do Not Disturb**: `sys::dnd_on()` returns a guard kept in `Focus`; dropping it restores the setting (KDE's inhibition goes away by itself if Clipster crashes; the GNOME/Cinnamon/XFCE value is also saved as `dnd_restore` and put back on the next start).

### Clipster's Desk (`app/src/desk/`)

A 720×560 window (900×660 when `desk_size` is large; its own transparent viewport, or an `Area` in screenshot mode) with nine tabs, one file each in `app/src/desk/`: `today.rs` (Coming up, time per app and per site, focus, the top 3 and habits, the wrap-up), `week.rs` (Last 7 days, Next 7 days, Focus), `todos.rs` (filters, details, steps, Plan my day and the week, named lists), `agenda.rs` (days or a month, free time, event and reminder cards, notes, Calendars), `clipboard.rs` (Copies, Saved links, Files), `write.rs`, `upkeep.rs` (Checkup, Right now, Tidy, Space, Backup), `you.rs` (You, People with a card each, Journal), `settings.rs` (five pages). `mod.rs` has the window, chrome, keys, the status bar with Undo, the find box (`find.rs`), the helpers (`rows` for scrolling lists, `popup` for drop-down menus, `remove_btn`, `line` for text cut with a hover, `browse` for Browse…), and `set_tab`/`tab` which open tabs, pages and cards by name (`desk <name>`). `look.rs` draws per look; `sample.rs` makes the screenshot data. Cards over a page are `Card` variants drawn by their tab (a ✕ or Esc closes them); a confirmation from a click is a `Confirm` card. It's a **view**: `App` builds a `desk::View` each frame, and every button returns a command string that goes through `App::run`, so the logic stays in `brain`/`todo`/`tidy`. Both looks share **one layout per tab**; only the chrome differs (`classic_chrome` with the View menu, `modern_chrome` with a Classic 98 | Modern switch in the title bar). Per-look drawing is in `impl Look` (`tab`, `group`, `well`, `button`, `checkbox`, `bar`, `column`, `input`, `divider`), with the Modern colours in `mod tok`. Modern is "Beige Box" (the 1997 tower: putty case, keycap buttons, label-maker titles, a green power LED, an LCD status strip). It uses the bundled Atkinson Hyperlegible Regular/Bold (`app/assets/fonts`, OFL) as the font families `modern` and `modern-medium`, each with a digits-only Noto Sans cut in front, so numbers have a plain zero. The look is saved as `desk_look`; only the Desk has a Modern look, the arcade stays 98. Settings changes come back as `pref <key> <value>`; `Look::dialog` draws its message cards (the wrap-up, "forget everything?"). Long lists scroll by whole rows (`Desk::first_row`). Meta+Shift+C (Ctrl+Alt+Shift+C on Windows) opens it from anywhere.

### The welcome window (`app/src/welcome.rs`)

A brand-new install (no `settings.json`) gets a Windows-98-style window instead of the greeting: five setup pages (name, personality, what he may comment on, starting up), applied together on Finish (`App::welcome_done`), and ten tour pages with Try it buttons that run real examples. Like the Desk it's a view: `App` passes a closure that draws Clipster, and it returns commands plus how it ended. Picking a personality applies at once (`pref persona …`) so the later pages speak in that voice. "show me around", "start the tour" and the right-click menu open the tour (`Effect::Tour`); someone who used Clipster before (settings without `welcome_seen`) gets one "want the tour?" offer.

### Personality and the arcade (`app/src/main.rs`, `app/src/games/`)

- **Achievements**: `App::count` bumps counters in `stats` after every command. The thresholds are the `ACHIEVEMENTS` table in `main.rs`, and the titles and quips come from the `achievements` data pool (ids must match).
- **Occasions**: `occasion()`, `seasonal_look()`, `time_of_day()` and `weekday()` pick greetings, holiday hats and skins. They also handle the user's birthday.
- Screenshot mode skips achievements, so the pictures stay clean.
- **Daily check-in**: once a day `tick` asks how you are (`checkin_question`, buttons `checkin great|okay|meh|rough` → `Effect::Checkin` → `App::checkin_answer`); three rough days in a row get `checkin_rough_often`.
- **Arcade sounds**: games push `Outcome::Sound(name)`; `sys::play_blip` synthesises small WAVs (`BLIPS`) into the cache folder and plays them without waiting (`paplay`/`pw-play`/`aplay`, `PlaySoundW` on Windows). Won/Lost get a win/lose tune.
- **Animations**: `Anim` in `main.rs` (body motion in `draw_clipster`), with the props of the newer ones in `props.rs`. Moods `alarm`, `gift`, `carry`, `read`, `write`, `fix`, `coffee` have their own animations; idle tricks include knit, plane, plant, yoyo, doodle and binoculars.
- **April Fools**: on 1 April, once a year, `show_prank` opens an obviously silly 98 message box from the `prank_dialog` pool (`Title | Message | Button`).
- **Arcade**: `games::Arcade` (`open` / `is_open` / `show`) is called every frame from `App::ui`. Each game opens in its own Win98-style viewport and reports `games::Outcome`s, which drive Clipster's `game_*` commentary, the high scores and the achievements. The game ids are listed in `games::NAMES`. The games are original office-themed takes on their genres, not clones of existing titles; keep it that way.

### Persistence and system integration

- Settings, name, looks, position, pending timers (stored as unix times), friendship, high scores, birthday, facts, the achievement counters and unlocks (and daily things like the check-in streak, in `stats`): `~/.config/clipster/settings.json`. `App::snapshot` / `load` / `save_if_changed` handle it. A new persisted field must go into both `snapshot` and `load`.
- Taught phrases: `~/.config/clipster/lines.txt` (see the database section above).
- Notes: `~/.local/share/clipster/notes.txt`; to-dos `todo.txt` and the last tidy's undo log `tidy-undo.json` next to it.
- In `settings.json` too: `calendars`, `calendar_meta`, `holidays`, `people`, `reading`, `reading_titles`, `workday`, `workday_ask`, `work_days`, `work_hours`, `focus_goal`, `budgets`, `dnd`, `wrapup`, `upkeep_day`, `upkeep_hide`, `space_log`, `pins` (pinned clipboard snippets; the history itself is never saved), `pin_labels`, `habits`, `countdowns`, `site_time`, `kind_moves`, `day_totals`, `weather`, `clock12`, `week_sunday`, `quiet_after`, `quiet_weekend`, `quiet_meetings`, `headsup`, `break_every`, `alarm_sound`, `muted`, `desk_morning`, `desk_size`, `tidy_targets`, `tidy_auto`, `backup_sources`, `backup_target`, `backup_last`, `backup_result`, `backup_skip`, `backup_auto`, `backup_history`, `dnd_restore`, `welcome_seen`, `things`, `renewals`, `gift_lead`, `habit_nudge`, `after_meeting`, `quiet_until` (unix seconds). Habits also keep `target`, `counts`, `times` and `taken`. Settings are written within 2 s after a command (`settings_dirty`) and otherwise at most once a minute (the time per app changes every second).
- Data files next to `todo.txt`: `done.txt` (finished to-dos cleared from the list), `journal.txt`, `notes.txt`.
- Autostart: `~/.config/autostart/clipster.desktop`.
- External tools it shells out to include `wpctl` (volume), `systemctl` / `loginctl` (power), `notify-send`, `paplay` (freedesktop sounds), `xdg-open`, `gio`. Missing tools should fail softly, with a message in the balloon.
