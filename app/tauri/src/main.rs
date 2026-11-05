#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::list_directions,
            commands::model_catalog,
            commands::project_info,
            commands::run_index,
            commands::run_bible,
            commands::run_drift,
            commands::var_audit,
            commands::barks,
            commands::episode_validate,
            commands::set_model,
        ])
        .run(tauri::generate_context!())
        .expect("помилка запуску Writer Studio");
}
