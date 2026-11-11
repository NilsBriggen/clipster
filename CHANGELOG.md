# Changelog

Each version's section is also its GitHub release description: the release workflow publishes it and refuses to release a version without one.

## 0.7.3 (2026-11-11)

Clipster moves out of the taskbar and into the system tray.

### Highlights

- **A tray icon:**
  - Left click puts Clipster away or brings him back.
  - Middle click opens the Desk.
  - Right click opens a menu: Hide/Show Clipster, Open my Desk, Be quiet for an hour, Settings…, Quit Clipster.
- **No taskbar button for the pet itself** when there's a tray. The Desk keeps its own button.
- **Putting him away:** he disappears from the screen and stays quiet. Reminders, timers, meeting heads-ups, Meta+C or anything you ask bring him back on their own. He stays put away after a restart.
- **Say "go to the tray" or "hide yourself"**, or pick "Hide in the tray" in his right-click menu.

### Install

- **Linux:** download `clipster_0.7.3_x86_64.AppImage`, make it executable (`chmod +x`) and replace your old AppImage with it. Or click **Install now** when he tells you about the update. Your settings, notes, to-dos and taught phrases are kept.
- **Windows:** run `clipster_0.7.3_x64_en-US.msi`, or click **Install now** in Clipster. It replaces the old version and keeps your settings. The installer still isn't code-signed, so SmartScreen may say "Windows protected your PC". Choose **More info → Run anyway**.

### Known limitations

- **GNOME** shows tray icons only with the AppIndicator extension. Without it, Clipster keeps his taskbar button.
- **Windows:** the tray icon is new and has only been build-checked, not tried by hand. If it ever doesn't appear, Clipster stays on screen but without a taskbar button.
- **On KDE**, the taskbar button goes away through Clipster's own KDE script. If his pointer tracking is off, the button stays.

## 0.7.2 (2026-11-10)

Fixes and game polish. Help buttons go where they say, typed answers count as clicks, he stops reading out his own commands, and the arcade gets difficulty levels you can actually find and a Desk Merge that doesn't drag on.

### Highlights

- **Help buttons fixed:** five of the topics under "what can you do" (Timers, Me and you, Agenda, People, Upkeep) opened the wrong thing. Every button now opens its own topic.
- **Typing instead of clicking:** with a balloon open, typing "yes", "no", "2", "the second one" or the start of a button's words presses that button. Before, he forgot the question and sometimes searched the web for your answer.
- **No more command text in his replies:**
  - "Yes, restart" and similar buttons say "Done" instead of showing the raw shell line.
  - "again" after a button repeats it exactly, not as a sentence.
  - "I didn't get that" replies never quote internal commands back.
- **A Difficulty menu:**
  - Chess, Clip Chain and Bug Hunt have a Difficulty menu in the window's menu bar.
  - The status bar says which level is on, and your choice is remembered.
  - Say "play chess on hard", "play snake on easy" or "chess as black" to open at a level.
- **Chess:** five strengths (Beginner, Easy, Medium, Hard, Expert). White, Black or two players is a separate choice.
- **Clip Chain:** a best score for each speed. Your old best counts as Normal.
- **Desk Merge:**
  - Bigger pieces (the Computer is now half the drawer wide), so the drawer fills up and games are about half as long.
  - Merges pop and shove the pile, round things roll into gaps, and quick merges in a row are a chain worth ×2, ×3…

### Install

- **Linux:** download `clipster_0.7.2_x86_64.AppImage`, make it executable (`chmod +x`) and replace your old AppImage with it. Or click **Install now** when he tells you about the update. Your settings, notes, to-dos and taught phrases are kept.
- **Windows:** run `clipster_0.7.2_x64_en-US.msi`, or click **Install now** in Clipster. It replaces the old version and keeps your settings. The installer still isn't code-signed, so SmartScreen may say "Windows protected your PC". Choose **More info → Run anyway**.

### Known limitations

- **Desk Merge's best scores** from before aren't comparable: the bigger pieces make games shorter.
- **Typed answers** only count while the balloon with the buttons is still open, and only when they clearly mean one button. Anything else is a new request.
- **Chess Expert** thinks for up to 4 seconds a move.

## 0.7.1 (2026-11-10)

A small fix release. The Desk's "What made up Everything else" card used to show one unhelpful line ("Other pages and apps"). Now it shows what the time actually went to, and lets you file it where it belongs.

### Highlights

- **Everything else, by name:** the card lists the eight biggest apps and sites of the day, each with its time.
  - **Apps** are named by the name at the end of the window title ("notes.txt — Kate" is Kate).
  - **In a browser,** the site before the browser's name ("… - Wikipedia — Mozilla Firefox" is Wikipedia).
  - Only a short name is kept, never the page title. Banking, mail and chat windows get no name.
- **Count as…** now works for these too: file Steam under Games or a work tool under Code, and it counts that way from then on. **Put back** undoes it.

### Install

- **Linux:** download `clipster_0.7.1_x86_64.AppImage`, make it executable (`chmod +x`) and replace your old AppImage with it. Or click **Install now** when he tells you about the update. Your settings, notes, to-dos and taught phrases are kept.
- **Windows:** run `clipster_0.7.1_x64_en-US.msi`, or click **Install now** in Clipster. It replaces the old version and keeps your settings. The installer still isn't code-signed, so SmartScreen may say "Windows protected your PC". Choose **More info → Run anyway**.

### Known limitations

- **Only from now on:** time from before this version stays in the "Other pages and apps" line.
- **Some apps have no name to give:** an app whose window title doesn't end in its name, a long title on its own, or a name in a non-Latin script stays under "Other pages and apps".
- **Short visits:** more than eight apps, or visits under 30 seconds, also stay in that line.

## 0.7.0 (2026-11-10)

The TimeTree release. Clipster can now read your TimeTree calendars, the shared calendar app many couples and families use, even though TimeTree has no export. He also understands dates the way people say them ("Friday in two weeks", "Friday evening", "the whole day"), keeps his own copy of every calendar file you import, and prints your agenda and your month as pages worth putting on the fridge.

### Highlights

- **TimeTree:**
  - Sign in once under Desk › Agenda › Calendars… (or say "connect my TimeTree"), then pick which TimeTree calendars to show.
  - They're read every 15 minutes, like any other calendar, with heads-ups, the agenda, Today and the briefing.
  - **It only reads:** nothing in TimeTree is ever changed. Your password isn't stored; only the session is, in a file only you can read.
- **Dates in plain words,** for to-dos, reminders and moving reminders alike:
  - "Friday in two weeks", "in two weeks on Friday", "the Friday after next", "Friday next week"
  - "Friday evening", "Friday morning", "Friday at noon", "Friday at 7 in the evening", "at 3 on Friday"
  - "Friday all day", "on Friday the whole day" (a date with no time)
  - "end of the month", "end of next month", "this weekend", "next weekend", "next week", "in a month"
  - "add call Anna Friday" is now a to-do, and the to-do keeps your capitals and words like "two".
  - **Reminders find the day anywhere in the sentence:**
    - "remind me Friday evening to call Anna" sets a reminder instead of saving a note.
    - "remind me Friday at 3" rings on Friday, not today.
    - The Desk's Remind me… preview always matches what it sets.
- **Imported calendar files keep working:**
  - An .ics file you import is copied into Clipster's own folder, so you can delete the download.
  - Invitations (a few events) go into one "Imported" calendar; an updated invitation replaces the old one.
  - A whole calendar becomes its own row.
  - Files you linked before are copied in on the next start (from the last copy he read, if the file is already gone).
- **Prints that look good:**
  - The seven-day agenda is a proper page: a block per day, times with their end, your calendar colours, places, all-day bars, reminders, to-dos, birthdays and holidays, on one A4 page.
  - In Month view, Print… now prints the month as a grid on a landscape page.
  - Lists, to-dos and the journal get the cleaner look too.

### Install

- **Linux:** download `clipster_0.7.0_x86_64.AppImage`, make it executable (`chmod +x`) and replace your old AppImage with it. Or click **Install now** when he tells you about the update. Your settings, notes, to-dos and taught phrases are kept.
- **Windows:** run `clipster_0.7.0_x64_en-US.msi`, or click **Install now** in Clipster. It replaces the old version and keeps your settings. The installer still isn't code-signed, so SmartScreen may say "Windows protected your PC". Choose **More info → Run anyway**.

### Known limitations

- **TimeTree** has no official API any more. Clipster reads it the way TimeTree's own website does, so it may stop working if TimeTree changes its website. It works with email-and-password accounts. Google, Apple or LINE sign-ins aren't supported yet.
- **TimeTree time zones:** a repeating TimeTree event made in another time zone repeats on your computer's clock. One-off events are always at the right time.
- **"next Friday"** means the coming Friday. "Friday next week" is the one after.
- **The month print** shows what the calendars are read for: about a month ahead.
- Everything listed for 0.6.0 still applies: Windows features not yet tried on a real PC, doses that aren't asked about again, slightly taller Beige Box rows, holidays and weather needing the internet.

## 0.6.1 (2026-11-10)

A small release with more to say. Many of the replies Clipster gives right after you do something had only two or three ways of saying it, so you heard the same line again and again. 68 of them now have more: 328 new lines in both personalities, each one still telling you exactly what happened. Nothing else changed.

### Highlights

- **More variety where you hear it most:**
  - ticking off, undoing, adding and removing habits, and streaks
  - pausing and resuming the clipboard, naming and unpinning pins
  - copying, renaming and deleting lists, and saving a list as a starter
  - moving a reminder, countdowns, the plan for the week, and nothing coming up today
  - pausing, resuming and the too-short gap before a meeting in focus mode
  - free time and full days on the agenda
  - backups starting by themselves, stopping and getting a file back
  - reading later, printing, QR codes, Space and the Monday tidy, exports and imports
  - updates, the weather's coat and umbrella, and closing your work apps
- **Unhinged** gets two or three new lines in each of them, Boring one or two plainer ones.

### Install

- **Linux:** download `clipster_0.6.1_x86_64.AppImage`, make it executable (`chmod +x`) and replace your old AppImage with it. Or click **Install now** when he tells you about the update. Your settings, notes, to-dos and taught phrases are kept.
- **Windows:** run `clipster_0.6.1_x64_en-US.msi`, or click **Install now** in Clipster. It replaces the old version and keeps your settings. The installer still isn't code-signed, so SmartScreen may say "Windows protected your PC". Choose **More info → Run anyway**.

### Known limitations

- Everything listed for 0.6.0 still applies: Windows features not yet tried on a real PC, doses that aren't asked about again, "take a break at 3 and 5" and "what's in my calendar today", slightly taller Beige Box rows, calendar time zones, and holidays and weather needing the internet.
- A few empty-page notes on the Desk (an empty clipboard, no pins, Right now on Upkeep) always show the same line. They didn't get new ones.

## 0.6.0 (2026-11-10)

The assistant-for-real release. Clipster now keeps track of the things a real assistant would: when your passport runs out, where you put the spare key, whether you took your pills, what you're still waiting on and from whom. He builds your morning from what's on your computer, without the internet. The Modern look is new: Beige Box, the 1997 tower he was compiled in. Over a hundred bugs were fixed after a full bug hunt and a review, and the Desk got tidier throughout.

### Highlights

- **A morning briefing that works offline:** "good morning", "brief me" or "what's my day like" lists your meetings and free time, reminders, your top 3, what's due or overdue, who to chase, birthdays this week, countdowns and habits left (plus the weather when it's on). "what's on this weekend" works too. The weekday-morning offer is now the briefing, and it skips public holidays.
- **Renewals and expiry dates:** "my passport expires on 3 March 2027", "the car insurance renews on 1 March every year", "netflix renews on the 15th every month for 12.99", "what's expiring soon", "renewed the car insurance". ID papers get four months' notice, everything else a month, with a heads-up a week before and on the day.
- **Where things are:** "my passport is in the top drawer", "where is my passport", "what's in my blue bag". When he doesn't know, he looks in your notes and then your files. He never keeps codes or passwords.
- **Water and medicine:** "8 glasses of water a day" and "+1 water" count up to a daily target. "remind me to take my pills every day at 8 and 20" rings with **Taken**, **In 15 min** and **Skip today**, and "did I take my pills" answers from the log.
- **Appointments from plain sentences:** "dentist on Tuesday at 10" offers a reminder an hour before, one the evening before, or a to-do. He always asks first.
- **Repeats that mean what you say:** every 3 days, every other week, every 2 months, every year on a date, the 15th of every month, every workday (skipping public holidays). To-dos can repeat yearly or every few days and keep their rhythm when you tick them off late.
- **Follow-ups and people:** "what's due this week", "anything overdue", "what am I waiting for", "follow up with Ben on Friday". "Ben sent the numbers" ticks off what you were waiting for. You can also ask "when is Anna's birthday", "what should I get Ben", "tell me about Anna", and "write a thank-you note to Anna" (her name and email filled in).
- **Thoughtful nudges:** ten days before a birthday, with the present ideas you saved; an evening word before a habit streak breaks; "how did it go?" after a meeting, with notes and a follow-up to-do. Reminders that rang while you were away are listed under the next one. Each nudge can be switched off in Settings.
- **Beige Box:** the Modern look is now a putty-coloured 1997 tower: keycap buttons, tabs with a Caps-Lock light, label-maker titles, a green power light and an LCD status strip, in Atkinson Hyperlegible with plain zeros.
- **A tidier Desk:** one main button per row with the rest under **…**, boxes sized to what they hold, second-level pages as segments, a readable Next 7 days, warning colours that read on the grey, risky buttons set apart, and empty pages that tell you what to do. You & me has a new **Papers & things** page for renewals and where things are. Today shows counting habits with **+** and doses as 1/2. The welcome window has live previews, and the balloon's Options and Close are now two small icons in the corner.
- **Over a hundred fixes,** found by a bug hunt and a review. Among them:
  - "every 3 days" and "every 2 months" repeated every few minutes.
  - "tomorrow evening" rang in the morning, and "quiet after 1 am" meant all day.
  - Importing from Clipster's own data folder emptied your files.
  - The journal forgot hand-written lines and anything older than a year.
  - Files saved in Notepad had a broken first line.
  - One broken calendar event blanked every calendar.
  - A tidy with unticked files moved them anyway.
  - Esc in a text box closed the whole Desk.
- **Faster and lighter:** the phrase database is read once instead of on every reload, settings are no longer written to disk every 2 seconds, window titles come straight from X instead of two programs every 2 seconds, and Windows lists programs without starting `tasklist`. The program itself is about a fifth smaller.

### Install

- **Linux:** download `clipster_0.6.0_x86_64.AppImage`, make it executable (`chmod +x`) and replace your old AppImage with it. Or click **Install now** when he tells you about the update. Your settings, notes, to-dos and taught phrases are kept.
- **Windows:** run `clipster_0.6.0_x64_en-US.msi`, or click **Install now** in Clipster. It replaces the old version and keeps your settings. The installer still isn't code-signed, so SmartScreen may say "Windows protected your PC". Choose **More info → Run anyway**.

### Known limitations

- **Windows, untested by hand:** the new program list, sounds and the Recycle Bin size check build and pass their tests on Windows, but haven't been tried on a real Windows PC yet. The same goes for 0.5.0's Browse…, Right now, Recycle Bin and printing. Please [report anything odd](https://github.com/NilsBriggen/clipster/issues).
- **Doses:** a dose you don't answer isn't asked about again. It shows as not taken.
- **Two phrasings still miss:** "take a break at 3 and 5" sets one reminder, and "what's in my calendar today" searches the web. "my agenda" and "what's on today" work.
- **Beige Box** rows are a little taller than before, so long lists show one or two rows fewer.
- **Calendar time zones:** events created in another time zone (TZID) are still read as local time. UTC times are converted correctly.
- **Holidays and weather** need the internet (date.nager.at and open-meteo.com). Without it the briefing, Today and the Agenda leave them out.
- As before, outside KDE Plasma there are no global shortcuts on Linux and no mouse-following.

## 0.5.0 (2026-11-09)

The Desk release. Clipster's Desk was rebuilt tab by tab: every page shows more, and almost everything on it can now be changed right there instead of asked for in the balloon. Habits, countdowns and a one-line journal are new, and about 60 bugs were fixed along the way, many of them on Windows.

### Highlights

- **A new Desk throughout:**
  - A status bar that says what happened, with **Undo** for anything you remove.
  - "Are you sure?" questions on the Desk itself, not in a balloon.
  - A find box (Ctrl+K) for anything on the Desk, or for a question to Clipster.
  - Two sizes, keys for everything (Ctrl+Tab, Ctrl+1–9, Ctrl+Z, Esc), and Classic 98 or Modern.
  - Browse… buttons, files you can drop on it, and Print… for lists and your agenda.
- **Today:** what's coming up (appointments, reminders, due to-dos, the weather), focus controls with pause and +10 min, your top 3 and habits, time per app and per website, countdowns, and a wrap-up that closes the day: a journal line, tomorrow's three, and your work apps closed if you like.
- **Week:** the last 7 days against the week before, the next 7 days with how busy each one is, your focus sessions, a short story of any day, and a spreadsheet of the week.
- **To-do:** filters (Today, Important, No date, Repeats, Waiting), search, a date menu, and a details card for each to-do: a time, a link, a note, a repeat, steps, or someone you're waiting on. Plan your day or your week, keep named lists with a chip each, start new lists from a saved one, and show a list as a QR code for your phone.
- **Agenda:** days or a whole month, free time between appointments (within your working hours), a card for each event and reminder, notes you can edit, a name, colour and heads-up setting for each calendar, public holidays for your country, and any reminder or to-do sent to your calendar app as a file.
- **Clipboard:** turn a copy into a to-do, a note or a link to read later, or change it (UPPER CASE, one line, sorted, base64…). Name your pins, pause the history, see your saved links with their titles, and find recent files and new downloads on the new Files page.
- **Upkeep:**
  - **Right now** shows what's using your computer.
  - **Space** shows what's taking room, and moves big files to the trash.
  - **Tidy** lets you choose the files, and can tidy Downloads every Monday.
  - **Backup** can skip names like `node_modules`, run on its own (when the drive is plugged in, or in the evening), stop halfway, show its history and get a single file back.
- **You & me:** edit what he knows about you, edit and try the phrases you taught him, a card for each person (birthday, email, phone, keeping in touch, notes, present ideas) and a journal page with a month view.
- **Settings:** five pages: General (personality, look, his outfit), My day (working days and hours, 12- or 24-hour clock, the first day of the week), Focus and goals, Comments (quiet times, what he keeps quiet about) and Your data (export, import, forget).
- **New things to say:**
  - "I did my walk" (habits), "count down to my holiday on 2 December", "journal: finished the slides"
  - "I'm waiting for Ben's numbers", "remind me every weekday at 8 to take my pills"
  - "focus until my next meeting", "pause my focus", "reset my packing list"
- **About 60 fixes,** found by two rounds of bug hunting and a review. Among them:
  - "every evening at 7" and "tonight at 9" rang in the morning.
  - "start reboot" opened something on Windows instead of asking first.
  - Reminders that mentioned a funeral or a hospital were silently not made.
  - "my phone is going to die" got the helpline message.
  - Renaming a person on the Desk didn't work, and editing a to-do's text lost its reminder.
  - Windows paths with ’ in them broke the trash and screenshots.

### Install

- **Linux:** download `clipster_0.5.0_x86_64.AppImage`, make it executable (`chmod +x`) and replace your old AppImage with it. Or click **Install now** when he tells you about the update. Your settings, notes, to-dos and taught phrases are kept.
- **Windows:** run `clipster_0.5.0_x64_en-US.msi`, or click **Install now** in Clipster. It replaces the old version and keeps your settings. The installer still isn't code-signed, so SmartScreen may say "Windows protected your PC". Choose **More info → Run anyway**.

### Known limitations

- **Windows, untested by hand:** the file and folder pickers (Browse…), Right now, moving files to the Recycle Bin and printing build and pass their tests on Windows, but haven't been tried on a real Windows PC yet. Please [report anything odd](https://github.com/NilsBriggen/clipster/issues).
- **Backup page and network drives:** while the Backup page is open it lists your drives every 10 seconds. An unreachable network drive can make the Desk stutter while it waits.
- **Holidays and weather** need the internet (date.nager.at and open-meteo.com). Without it, Today and the Agenda simply leave them out.
- **Calendar time zones:** events created in another time zone (TZID) are still read as local time, so they can be off by the difference. UTC times are converted correctly.
- **Saved links** take dropped files, not text dragged from a browser.
- As before, outside KDE Plasma there are no global shortcuts on Linux and no mouse-following.

## 0.4.0 (2026-11-09)

The assistant release. Clipster learned what a personal assistant needs to know: your appointments, the people in your life, your shopping list, where you saved that file and what you meant to read later. He also got a snake game, new animations, comments on 400 more programs and 400 websites, and more than twice as many lines as before (about 50,000 now), written to new limits you can read in `LORE.md`.

### Highlights

- **Your calendar:**
  - Paste your calendar's private iCal link (Google, Outlook, Nextcloud, Proton all have one) on the Desk's new **Agenda** tab, or say "add my calendar" and the link.
  - Ask "my agenda", "what's on Friday", "my next meeting" or "am I free on Friday at 3".
  - He gives you a heads-up 10 minutes before each appointment, with a **Join** button for Zoom, Meet, Teams, Jitsi and Webex calls.
  - The morning briefing lists your day, and the Agenda shows events, reminders and due to-dos together.
- **People:**
  - "Anna's birthday is on 3 March", "keep in touch with mum every 2 weeks", "I called mum", "when did I last talk to mum", "email Anna", "upcoming birthdays".
  - Import your contacts from a .vcf file.
  - He reminds you three days before a birthday, on the day, and when it's been too long.
  - You & me has a new People page.
- **Start my workday:** one click (or "start my workday") opens your work apps and links, shows your day and offers a focus session. On weekday mornings he offers it once.
- **Lists:**
  - "add eggs, milk and bread to my shopping list", "what's on my packing list", "clear my shopping list".
  - Each list gets its own chip on the To-do tab.
- **Find my files:** "find my CV", "where did I save the invoice", "recent files".
- **Read later:** copied links get a Read later button, and "save this link" keeps the last one. They're on the Clipboard tab's new Saved links page.
- **Clip Chain:** a snake game. Mini Clipster pulls a chain of coloured paperclips across the desk blotter. Easy, Normal and Hard.
- **New animations:**
  - He rings a desk bell before meetings, carries a box during backups, puts on reading glasses, scribbles on a clipboard, turns a wrench during upkeep, sips coffee in the morning and blows a party horn for birthdays.
  - When nobody's looking, he knits a paperclip chain, folds a paper plane, waters a desk plant, plays with a yo-yo, doodles, and spies through binoculars.
- **He comments on much more:**
  - 401 more programs (625 in all, many of them Windows apps), with Windows process names added for existing ones
  - 404 websites by name (YouTube, Reddit, GitHub, SBB, Digitec Galaxus, 20 Minuten…)
  - 23 new kinds of app and site (notes, design, projects, banking, news…), with shortcut tips
  - Comments may now mention the page or file you're on. Titles still never leave your computer.
- **About 28,000 new lines in both voices:**
  - everyday chatter
  - 260 new conversation topics and 82 sentence patterns, plus new ways to phrase 150 existing ones
  - more jokes, puns, true facts, riddles and would-you-rathers
  - more arcade commentary
  - more of his Unhinged lore: diary, prophecies, memories
  - About 300 older lines were rewritten, and an editor reviewed everything for facts, repeats and the limits.
- **New limits:** what he may and may not joke about was decided topic by topic and is written down in `app/data/LORE.md`: a bit sharper, still kind, never about illness, faith, real people or anything that could hurt.
- **April Fools:** on 1 April he has a little surprise. It's harmless.

### Install

- **Linux:** download `clipster_0.4.0_x86_64.AppImage`, make it executable (`chmod +x`) and replace your old AppImage with it. Or click **Install now** when he tells you about the update. Your settings, notes, to-dos and taught phrases are kept.
- **Windows:** run `clipster_0.4.0_x64_en-US.msi`, or click **Install now** in Clipster. It replaces the old version and keeps your settings. The installer still isn't code-signed, so SmartScreen may say "Windows protected your PC". Choose **More info → Run anyway**.

### Known limitations

- **Calendar time zones:** events are shown in your computer's time zone. Events created in another time zone (TZID) are read as local time, so they can be off by the difference. UTC times are converted correctly.
- **Windows, untested by hand:** recent files (from the Recent folder) and opening apps by name for Start my workday compile and pass their tests on Windows, but haven't been tried on a real Windows PC yet. Please [report anything odd](https://github.com/NilsBriggen/clipster/issues).
- **Website comments** depend on the words a site puts in its tab title. A few sites name themselves in a way he doesn't recognise; those just get the general comments.
- As before, outside KDE Plasma there are no global shortcuts on Linux and no mouse-following.

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
