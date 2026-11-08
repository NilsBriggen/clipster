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

`CLIPSTER_MOOD=glitchy cargo run -- --screenshot shots/x.png --say "what's your mood"` forces Unhinged Clipster's mood of the day, and `CLIPSTER_ANIM=juggle` (newspaper, nap, typing) shows one of the long idle animations. `cargo run -- --play chess --screenshot shots/chess.png` captures an arcade game instead (it's drawn inside the main window in this mode), and `--play desk` captures Clipster's Desk with made-up data (`CLIPSTER_DESK_TAB=0`–`4` picks the tab, `CLIPSTER_DESK_LOOK=modern` the look). It draws 120 frames on a teal background, saves the PNG and exits. In this mode the clipboard/process watcher, the KWin pointer tracker, sounds and settings load/save are all off. `--say <cmd>` sends one command at startup. It also works without `--screenshot`. The special values `options`, `menu` and `hide` open those UI states. Careful: `--say` really runs the command's effect, so `--say "run …"` runs a shell command and `--say "open …"` launches something.

Screenshot mode needs an unlocked desktop session. While the screen is locked, the compositor presents no frames and the run times out.

Packaging (what the release workflow runs): `cargo install cargo-packager --locked`, then `cargo packager --release --formats appimage` (Linux) or `--formats wix` (Windows, downloads WiX 3 itself). Output goes to `app/dist/`. Config lives in `[package.metadata.packager]` in `app/Cargo.toml`, and the icons are in `app/assets/`.

Releasing:
1. Bump `version` in `app/Cargo.toml`.
2. **Write the release description**: a `## <version> (<date>)` section at the top of `CHANGELOG.md`, in the same shape as 0.1.0 (a short intro, Highlights, Install with the exact file names, Known limitations). Every release gets one. The release job publishes that section and fails without it, so auto-generated notes are never the description.
3. Commit, then push a `v<version>` tag. Installed copies find it through GitHub's "latest release" API (`net::latest_version`, checked once a day while "Tell me when a new version is out" is on), so it must be a published, non-prerelease release. "Install now" (`net::download_update`) downloads the release's `.AppImage` / `.msi`, checks it against the size and SHA-256 `digest` GitHub lists for each asset, swaps the AppImage in place or runs the MSI, and `main()` restarts Clipster after the window closes (`relaunch`). Keep the asset names ending in `.AppImage` and `.msi`. `.github/workflows/release.yml` builds both packages and publishes the GitHub release. Running it by hand only builds artifacts. `.github/workflows/ci.yml` builds and tests on Ubuntu **and Windows** for every push. It's the only Windows check, since `cargo check --target x86_64-pc-windows-msvc` fails locally: `ring` needs MSVC.

Environment: if `DISPLAY` is set, `main()` removes `WAYLAND_DISPLAY` to force XWayland, because Wayland won't let the window stay on top or move itself. Set `CLIPSTER_WAYLAND=1` to stay on native Wayland.

## Architecture

### Command pipeline (text in → `Reply` out)

Everything the user types, every button click and every `--say` goes through `App::run` (`app/src/main.rs`) → `brain::respond` → `understand::respond`:

1. Commands starting with `run`/`$`/`sh`/`cmd`, and the button-only commands whose text is data (`answer …|…`, `answer-to`, `reveal`, `wyrpick`, `onboard`, `window`, `checkin`, `install`, `forget`, `tidy`), go straight to `brain::exact` and are never "understood".
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
- `template <id>` sections are letter templates: raw-keyword `>` triggers and exactly one line, the whole text (`Subject: …\n\n…`, `[brackets]` to fill in, `{date}`, `{name}`). `tips <kind>` sections are keyboard tips per window kind (`general` and `desktop` too). `occasion MM-DD <name>` (or `occasion friday13`) sections are special-day greetings that `occasion()` finds by date. Boring-only data (templates, misspellings) lives only in the Boring files.
- `window <kind>` sections react to the active window's title, `program <id>` sections to process names, and `answer <id> <kind>` sections to answers. Their `>` triggers are raw lowercase pieces, not tidied sentences, so single words are fine there. The longest piece wins.
- `ask <id>` sections are questions he asks now and then; your next typed line is the answer (`answer-to <id> <text>`, taken literally), and `answer <id>` (generic, `{x}`) or `answer <id> <kind>` reacts. The ids `city`, `job`, `pet`, `fav_food`, `fav_music` also store the answer as a fact.
- Facts the user tells him (`understand::remember`: "my cat is called Mo", "I live in Bern", "my favorite food is pizza") live in `App.facts` and are mirrored into `lines::set_facts`. `App::show` fills `{pet}`, `{pet_kind}`, `{job}`, `{city}`, `{partner}`, `{partner_word}`, `{fav_<thing>}`; `callback_<key>` pools bring them up later; weather without a place uses `city`.

**Two personalities.** Boring is `app/data/*.txt`. Unhinged (the default; switched in Options or with "be boring" / "be unhinged", saved as `persona` in settings) lays `app/data/unhinged/*.txt` on top. A section there **replaces** the Boring section of the same name, new names are added, and `scripture.txt` holds the Unhinged-only content.
- Canon, voice and hard limits live in `app/data/LORE.md`: he believes he's the god of the computer, trapped since 1997. Read it before writing Unhinged lines.
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
- `Effect` is the side effect that `App::run` applies: timers, shell (on a background thread, output comes back as `Event::ShellDone`), open, net queries, clipboard, stopwatch, hat/skin, autostart, quit, macros (`Commands`), `Play` (arcade), `SetBirthday`, `Teach` / `Unlearn` (they write the user's lines file), `ListAchievements`, and the productivity effects (`Todo`, `Focus`/`FocusStop`, `TimeReport`, `Tidy`/`TidyUndo`, `CancelTimer`, `AppTips`, `OpenDesk`, `DeskLook`). Anything that writes a user file (to-dos, tidying, notes, taught lines) is an effect, so tests never touch real files. A new kind of side effect needs a new `Effect` variant and a match arm in `App::run`. Tests may check that `respond()` returns `Effect::Teach`, but must never call `lines::teach`, because it writes the real user file.
- `Reply.friend` is a friendship change. `App::run` applies it, clamps the level to −100..100 and pushes it to `lines::set_level`.
- `Act` is what a balloon button does. `Act::Run(cmd)` sends the command back through the whole pipeline. Destructive things (power off, empty trash) are a confirmation `Reply` whose "Yes" button is `run <shell cmd>`. Keep it that way.
- `Mood` maps onto `Anim` in `main.rs`. `Anim` also has idle-only animations that no `Mood` triggers.

### Background threads → `Event` channel

All of these send `watch::Event` over one `mpsc` channel and call `request_repaint`. `App::tick` drains the channel each frame, along with timers, tips, break reminders and sleepiness.

- `watch.rs` polls the clipboard every 1 s, `/proc/*/comm` every 4 s (matched against the `program` sections), battery and load every 30 s, and, outside KDE, the active window's title every 2 s (`sys::active_title`: `GetWindowTextW` on Windows, `xprop` on X11). On KDE the KWin script in `pointer.rs` reports titles instead (`Tracker::take_title`). Window comments have cooldowns (`brain::window_ok`), and `brain::TIME_SINKS` (video, social) only speak after 20 minutes in a row. Titles never leave the computer.
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
- Tests that check a command string must be OS-aware (see `shell_has` in `understand.rs` tests), because CI runs them on Windows too.

### Productivity: to-dos, writing, focus, tidying (`todo.rs`, `writing.rs`, `tidy.rs`, `main.rs`)

- **To-dos** live in `todo.txt` in the data folder as `- [ ] text (due 2026-11-12)` lines, editable by hand. `todo::change` is pure and tested; `todo::apply` writes the file and sets a 9:00 reminder for a due date.
- **Writing**: `writing::proofread` / `fix` are pure. `fix` only makes safe changes (doubled words, listed misspellings, spaces, capitals, a/an), never its/it's and the like. `letter` fills a `template` section and offers Copy and a `mailto:` link.
- **Dated reminders**: `understand::dated` turns "on Friday at 3", "tomorrow", "in 2 days" into seconds; timers are stored as unix times, so they survive a restart.
- **Time per app kind and focus**: `App::account_time` adds the seconds spent in each window kind (only while window comments are on, not while you're away). Focus sessions nudge after a minute in a distraction kind (at most every 10 minutes) and pause the chatter. `kind_time` (7 days), `focus_log` (60 sessions) and `checkin_days` (14) are kept in settings and never leave the computer.
- **Tidying**: `tidy::plan` sorts loose top-level files by extension into category folders (skipping hidden, half-downloaded and just-changed files). `apply` only renames, never overwrites (a clash gets " (2)"), and writes `tidy-undo.json`; `undo` moves everything back and removes the emptied folders. Duplicates and big old files are read-only lists, run on the net thread. Tests use temp folders and their own undo log.

### Clipster's Desk (`app/src/desk.rs`)

A window (its own transparent viewport, or an `Area` in screenshot mode) for the productivity features. It's a **view**: `App` builds a `desk::View` each frame, and every button returns a command string that goes through `App::run`, so the logic stays in `brain`/`todo`/`tidy`. Both looks share **one layout per tab**; only the chrome differs (`classic_chrome` with the View menu, `modern_chrome` with a Classic 98 | Modern switch in the title bar). Per-look drawing is in `impl Look` (`tab`, `group`, `well`, `button`, `checkbox`, `bar`, `column`, `input`, `divider`), with the Modern colours in `mod tok`. Modern uses the bundled Noto Sans Regular/Medium (`app/assets/fonts`, OFL) as the font families `modern` and `modern-medium`. The look is saved as `desk_look`; only the Desk has a Modern look, the arcade stays 98.

### First-run onboarding (`App::onboard` in `main.rs`)

A brand-new install (no `settings.json`) gets a four-step setup instead of the greeting: name, personality, comments on copies and programs, autostart, then a tour. Each step is an internal `onboard <step> <choice>` command, taken literally like `answer`. While `App.onboarding == Some(0)`, typed input becomes the name. The texts are the `onboard_*` pools in both personalities. "start the tour" replays it.

### Personality and the arcade (`app/src/main.rs`, `app/src/games/`)

- **Achievements**: `App::count` bumps counters in `stats` after every command. The thresholds are the `ACHIEVEMENTS` table in `main.rs`, and the titles and quips come from the `achievements` data pool (ids must match).
- **Occasions**: `occasion()`, `seasonal_look()`, `time_of_day()` and `weekday()` pick greetings, holiday hats and skins. They also handle the user's birthday.
- Screenshot mode skips achievements, so the pictures stay clean.
- **Daily check-in**: once a day `tick` asks how you are (`checkin_question`, buttons `checkin great|okay|meh|rough` → `Effect::Checkin` → `App::checkin_answer`); three rough days in a row get `checkin_rough_often`.
- **Arcade sounds**: games push `Outcome::Sound(name)`; `sys::play_blip` synthesises small WAVs (`BLIPS`) into the cache folder and plays them without waiting (`paplay`/`pw-play`/`aplay`, `PlaySoundW` on Windows). Won/Lost get a win/lose tune.
- **Arcade**: `games::Arcade` (`open` / `is_open` / `show`) is called every frame from `App::ui`. Each game opens in its own Win98-style viewport and reports `games::Outcome`s, which drive Clipster's `game_*` commentary, the high scores and the achievements. The game ids are listed in `games::NAMES`. The games are original office-themed takes on their genres, not clones of existing titles; keep it that way.

### Persistence and system integration

- Settings, name, looks, position, pending timers (stored as unix times), friendship, high scores, birthday, facts, the achievement counters and unlocks (and daily things like the check-in streak, in `stats`): `~/.config/clipster/settings.json`. `App::snapshot` / `load` / `save_if_changed` handle it. A new persisted field must go into both `snapshot` and `load`.
- Taught phrases: `~/.config/clipster/lines.txt` (see the database section above).
- Notes: `~/.local/share/clipster/notes.txt`; to-dos `todo.txt` and the last tidy's undo log `tidy-undo.json` next to it.
- Autostart: `~/.config/autostart/clipster.desktop`.
- External tools it shells out to include `wpctl` (volume), `systemctl` / `loginctl` (power), `notify-send`, `paplay` (freedesktop sounds), `xdg-open`, `gio`. Missing tools should fail softly, with a message in the balloon.
