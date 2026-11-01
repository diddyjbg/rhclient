fn main() {
    // the commands the game page may call (each gets an allow-<name> permission, used in capabilities/game.json)
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&["discord_activity", "discord_reply", "discord_take_join", "app_info"]),
    ))
    .expect("failed to run tauri-build");
}
