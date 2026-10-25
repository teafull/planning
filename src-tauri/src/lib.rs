use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder},
    Manager,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use chrono::{Local, Datelike};

mod db;
mod raft;
mod raft_cmd;

// 自定义错误类型，方便前端处理
#[derive(Debug, Error, Serialize)]
enum HttpError {
    #[error("请求失败: {0}")]
    RequestError(String),
    #[error("解析响应失败: {0}")]
    ParseError(String),
    #[error("网络错误: {0}")]
    NetworkError(String),
}

// 响应数据结构（示例）
#[derive(Debug, Deserialize, Serialize)]
struct ApiResponse {
    code: u32,
    holiday: serde_json::Value,
}

// GET 请求示例
#[tauri::command]
async fn http_get(url: &str) -> Result<ApiResponse, HttpError> {
    // 替换 {year} 为当前年份
    // 获取本地时区的当前年份
    let local_year = Local::now().year();

    // 获取 UTC 时区的当前年份（适合跨时区场景）
    // let utc_year = Utc::now().year();
    let final_url = url.replace("{year}", &format!("{}", local_year));

    // 创建 HTTP 客户端
    let client = reqwest::Client::new();
    
    // 发送 GET 请求
    let response: reqwest::Response = client
        .get(&final_url)
        .send()
        .await
        .map_err(|e| HttpError::NetworkError(e.to_string()))?;

    // 检查响应状态
    if !response.status().is_success() {
        return Err(HttpError::RequestError(format!(
            "请求失败，状态码: {}",
            response.status()
        )));
    }

    // 解析 JSON 响应
    let data: ApiResponse = response
        .json()
        .await
        .map_err(|e| HttpError::ParseError(e.to_string()))?;

    Ok(data)
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// 日志初始化: 默认安静, RUST_LOG=planning=debug,openraft=info 开启排障
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("planning=info,openraft=warn")
    });
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();
    tauri::Builder::default().setup(|app| {
        // 初始化 SQLite 存储(失败不阻塞窗口, 前端命令会报错)
        if let Err(e) = db::init(app.handle()) {
            tracing::error!("数据库初始化失败: {e}");
        }
        app.manage(db::ClusterState::default());
        // 按持久化配置自动重连集群(异步, 不阻塞窗口)
        let h = app.handle().clone();
        tauri::async_runtime::spawn(async move {
            db::auto_start_cluster(h).await;
        });

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
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            http_get,
            db::db_get_events,
            db::db_save_events,
            db::db_get_kv,
            db::db_set_kv,
            db::db_set_kv_multi,
            db::db_backup,
            db::db_get_data_dir,
            db::db_set_data_dir,
            db::db_get_node_id,
            db::db_get_cluster_config,
            db::db_set_cluster_config,
            db::cluster_create,
            db::cluster_join,
            db::cluster_status,
            db::cluster_leave,
            db::cluster_remove,
            db::cluster_set_offline,
            db::cluster_flush_pending
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
