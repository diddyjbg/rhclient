# Rhythmhel for Windows (Tauri)

A small native window for the game. It uses the WebView2 engine that Windows 10 and 11 already have, so the installer is
a few MB (the Electron version was about 80 MB) and it uses noticeably less memory. It always loads the newest game from
the server, so you never need to rebuild it when the game updates.

What it adds:
- **Discord status (Rich Presence):** what you are doing (menu, song select, playing a song with a time bar, results,
  editing, multiplayer), the mode icon, and the party size in multiplayer.
- **Discord "Join" invites:** while you are in a multiplayer room, friends can press **Join** on your profile or use **Ask to Join**,
  and you can invite people with the **+** button in any Discord chat. Pressing Join opens Rhythmhel (even if it is closed)
  and drops them straight into your room. When someone asks to join you, the game asks whether to let them in.
- F11 fullscreen, links open in your normal browser, files can be dropped onto the window, only one copy runs at a time,
  and a "Try again" screen when you are offline.

## 1. Create the Discord application (once, about 5 minutes)
1. Go to https://discord.com/developers/applications, press **New Application**, name it **Rhythmhel**.
2. Copy the **Application ID** on the General Information page.
3. Open **Rich Presence > Art Assets** and upload these images (at least 512 x 512 for the big one), with exactly these names:
   - `logo`: the Rhythmhel logo (the big picture on your status)
   - `mania`: a small piano/keys icon (shown while playing Mania charts)
   - `circle`: a small circle icon (shown while playing Circle mode)
   New art can take a few minutes to show up in Discord.

## 2. Build the .exe (no installs needed)
1. Create a GitHub repository and upload everything in this folder, including the hidden `.github` folder.
2. In the repository: **Settings > Secrets and variables > Actions > Variables > New repository variable**:
   name `DISCORD_APP_ID`, value: the Application ID from step 1.
3. Open the **Actions** tab, pick **Build Windows app**, press **Run workflow**. The first build takes about 10 minutes.
4. Download **Rhythmhel-Windows** under Artifacts. It contains `Rhythmhel_1.0.0_x64-setup.exe` (installer, no admin rights needed)
   and `rhythmhel.exe` (runs without installing).

To build on your own PC instead: install Rust (https://rustup.rs) and Node.js 20, then in this folder run
`npm install` and `set DISCORD_APP_ID=your id` and `npx tauri build`.

## Notes
- **The Discord overlay** (Shift+Tab) is drawn by Discord, not by the game. It works with games Discord can hook into;
  web-based windows (this app, Electron apps, browsers) are often not supported. To try it, open Discord
  **Settings > Registered Games**, add Rhythmhel if it is not listed, and turn its overlay on. The status and Join
  invites work either way.
- **Memory:** the app itself (the Rust part) uses a few MB. The game runs in WebView2, which shares its engine with Edge
  but still needs memory for the page, the song audio and graphics; expect roughly 150 to 250 MB while playing,
  less than the Electron version. Nothing extra stays running when you close the window.
- The app is not code-signed, so Windows SmartScreen may say "Windows protected your PC" the first time: "More info", then "Run anyway".
- If your site moves, change the addresses in `dist/index.html` and `src-tauri/capabilities/game.json`.
