// Rhythmhel for Windows (Tauri): the game in a small native window, with Discord Rich Presence and "Join" invites.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod discord;

use serde_json::{json, Value};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

/// Discord application id (Discord Developer Portal > your app > Application ID).
/// Set it at build time: the GitHub workflow passes the DISCORD_APP_ID repository variable. Empty = Discord features off.
const DISCORD_APP_ID: &str = match option_env!("DISCORD_APP_ID") { Some(v) => v, None => "" };

#[tauri::command]
fn discord_activity(state: tauri::State<'_, discord::Discord>, activity: Value) {
    state.set_activity(activity);
}

#[tauri::command]
fn discord_reply(state: tauri::State<'_, discord::Discord>, user_id: String, accept: bool) {
    state.reply(user_id, accept);
}

#[tauri::command]
fn discord_take_join(state: tauri::State<'_, discord::Discord>) -> Option<String> {
    state.take_join()
}

#[tauri::command]
fn app_info() -> Value {
    json!({ "app": "tauri", "version": env!("CARGO_PKG_VERSION"), "discord": !DISCORD_APP_ID.is_empty() })
}

fn main() {
    tauri::Builder::default()
        // a second launch (for example Discord opening the game for "Join") focuses the window that is already open
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![discord_activity, discord_reply, discord_take_join, app_info])
        .setup(|app| {
            let handle = app.handle().clone();
            let dc = discord::Discord::start(DISCORD_APP_ID, move |ev| {
                let _ = handle.emit("discord", ev);
            });
            app.manage(dc);
            discord::register_launch(DISCORD_APP_ID);
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("Rhythmhel")
                .inner_size(1280.0, 760.0)
                .min_inner_size(960.0, 560.0)
                .disable_drag_drop_handler() // let the game receive dropped .osz / skin / audio files
                .additional_browser_args(
                    // Tauri's defaults, plus: music may start without a click, and timing stays steady in the background
                    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --autoplay-policy=no-user-gesture-required --disable-background-timer-throttling --disable-renderer-backgrounding",
                )
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Rhythmhel");
}
