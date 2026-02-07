use tauri::{
  menu::{Menu, MenuItem},
  tray::{MouseButton, MouseButtonState, TrayIconBuilder},
  Manager,
};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default().setup(|app| {
        let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
        let about_i = MenuItem::with_id(app, "about", "关于", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&about_i, &  quit_i])?;

        // 监听窗口关闭事件，隐藏到托盘
        let window = app.get_webview_window("main").unwrap();
        let app_handle = app.handle().clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 阻止窗口关闭，改为隐藏到托盘
                api.prevent_close();
                // 使用应用句柄重新获取窗口并隐藏
                if let Some(w) = app_handle.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
        });

        let _ = TrayIconBuilder::new()
            .icon(app.default_window_icon().unwrap().clone())
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(move |_app, event| match event.id.as_ref() {

                "quit" => {
                    std::process::exit(0);
                }
                "about" => {
                    // TODO: Show about dialog
                }
                _ => {
                    println!("Unknown menu item:");
                }

            })
            .on_tray_icon_event(|tray, event| {
                if let tauri::tray::TrayIconEvent::Click {
                    id: _,
                    position: _,
                    rect: _,
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                } = event {
                    // 切换主窗口的显示状态
                    if let Some(window) = tray.app_handle().get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
            })
            .build(app)?;
        Ok(())
    })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
