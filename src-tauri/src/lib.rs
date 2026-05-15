mod icon_extractor;
mod favicon_fetcher;

use icon_extractor::IconData;
use favicon_fetcher::FaviconData;
use image::ImageEncoder;
use tauri::Manager;
use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::image::Image;
use tauri::Emitter;

#[tauri::command]
fn extract_icons(path: String) -> Result<Vec<IconData>, String> {
    log::info!("[extract_icons] 开始提取图标, 路径: {}", path);
    match icon_extractor::extract_all_icons(&path) {
        Ok(icons) => {
            log::info!("[extract_icons] 提取成功, 找到 {} 个图标尺寸", icons.len());
            for icon in &icons {
                log::info!("[extract_icons]   尺寸: {}x{}", icon.width, icon.height);
            }
            Ok(icons)
        }
        Err(e) => {
            log::error!("[extract_icons] 提取失败: {}", e);
            Err(e)
        }
    }
}

#[tauri::command]
fn convert_icon(
    data_url: String,
    format: String,
    target_size: u32,
) -> Result<String, String> {
    log::info!("[convert_icon] 开始转换, 目标格式: {}, 目标尺寸: {}px", format, target_size);

    let base64_data = if data_url.starts_with("data:image/png;base64,") {
        data_url.strip_prefix("data:image/png;base64,").unwrap()
    } else {
        log::error!("[convert_icon] 无效的 data URL 格式");
        return Err("Invalid data URL format".to_string());
    };

    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| {
            log::error!("[convert_icon] Base64 解码失败: {}", e);
            format!("Base64 decode failed: {e}")
        })?;

    let img = image::load_from_memory(&decoded)
        .map_err(|e| {
            log::error!("[convert_icon] 图片加载失败: {}", e);
            format!("Image load failed: {e}")
        })?;

    log::info!("[convert_icon] 原始尺寸: {}x{}", img.width(), img.height());

    let resized = if target_size > 0 && target_size != img.width() {
        log::info!("[convert_icon] 缩放至: {}x{}", target_size, target_size);
        img.resize_exact(
            target_size,
            target_size,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };

    let output_bytes = match format.to_lowercase().as_str() {
        "png" => {
            let mut buf = std::io::Cursor::new(Vec::new());
            let encoder = image::codecs::png::PngEncoder::new(&mut buf);
            encoder
                .write_image(resized.as_bytes(), resized.width(), resized.height(), resized.color().into())
                .map_err(|e| format!("PNG encode failed: {e}"))?;
            buf.into_inner()
        }
        "jpg" | "jpeg" => {
            let mut buf = std::io::Cursor::new(Vec::new());
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 90);
            encoder
                .write_image(resized.as_bytes(), resized.width(), resized.height(), resized.color().into())
                .map_err(|e| format!("JPEG encode failed: {e}"))?;
            buf.into_inner()
        }
        "ico" => {
            let mut buf = std::io::Cursor::new(Vec::new());
            let encoder = image::codecs::ico::IcoEncoder::new(&mut buf);
            encoder
                .write_image(resized.as_bytes(), resized.width(), resized.height(), resized.color().into())
                .map_err(|e| format!("ICO encode failed: {e}"))?;
            buf.into_inner()
        }
        _ => return Err(format!("Unsupported format: {format}")),
    };

    let output_base64 = base64::engine::general_purpose::STANDARD.encode(&output_bytes);
    let mime = match format.to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    };

    log::info!("[convert_icon] 转换完成, 输出大小: {} bytes", output_bytes.len());
    Ok(format!("data:{mime};base64,{output_base64}"))
}

#[tauri::command]
fn save_icon_file(data_url: String, file_path: String) -> Result<(), String> {
    log::info!("[save_icon_file] 保存文件至: {}", file_path);

    let base64_data = if let Some(stripped) = data_url.strip_prefix("data:") {
        let comma_pos = stripped.find(',').ok_or("Invalid data URL: no comma found")?;
        &stripped[comma_pos + 1..]
    } else {
        log::error!("[save_icon_file] 无效的 data URL 格式");
        return Err("Invalid data URL format".to_string());
    };

    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| {
            log::error!("[save_icon_file] Base64 解码失败: {}", e);
            format!("Base64 decode failed: {e}")
        })?;

    std::fs::write(&file_path, &decoded)
        .map_err(|e| {
            log::error!("[save_icon_file] 写入文件失败: {}", e);
            format!("Failed to write file: {e}")
        })?;

    log::info!("[save_icon_file] 文件保存成功, 大小: {} bytes", decoded.len());
    Ok(())
}

#[tauri::command]
async fn fetch_favicons(url: String) -> Result<Vec<FaviconData>, String> {
    favicon_fetcher::fetch_favicons(&url).await
}

fn restore_and_activate_window<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();

    #[cfg(windows)]
    force_activate_window(window);
}

#[cfg(windows)]
fn force_activate_window<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
    };

    let Ok(window_handle) = window.window_handle() else {
        return;
    };

    let RawWindowHandle::Win32(handle) = window_handle.as_raw() else {
        return;
    };

    let hwnd = HWND(handle.hwnd.get() as *mut core::ffi::c_void);

    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    log::info!("[run] 启动图标提取器...");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log::info!("[single_instance] 检测到重复启动，激活现有主窗口");
            if let Some(window) = app.get_webview_window("main") {
                restore_and_activate_window(&window);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
                log::info!("[setup] 日志插件已初始化 (debug 模式)");
            }

            // --- Load custom icon ---
            log::info!("[setup] 加载自定义图标...");
            let custom_icon = {
                let candidates = [
                    app.path().resource_dir()
                        .unwrap_or_else(|_| std::path::PathBuf::from("."))
                        .join("icons").join("to-ico.png"),
                    std::path::PathBuf::from("src-tauri/icons/to-ico.png"),
                    std::path::PathBuf::from("icons/to-ico.png"),
                ];

                let mut found = None;
                for p in &candidates {
                    log::info!("[setup] 尝试图标路径: {}", p.display());
                    if let Ok(img) = image::open(p) {
                        let rgba = img.to_rgba8();
                        let (w, h) = rgba.dimensions();
                        found = Some(Image::new_owned(rgba.into_raw(), w, h));
                        log::info!("[setup] 图标加载成功: {}x{}", w, h);
                        break;
                    }
                }
                found.unwrap_or_else(|| {
                    log::warn!("[setup] 未找到自定义图标, 使用默认图标");
                    app.default_window_icon().unwrap().clone()
                })
            };

            // --- Build tray menu (right-click) ---
            log::info!("[setup] 构建托盘菜单...");
            let quit_item = MenuItemBuilder::with_id("quit", "退出")
                .build(app)?;
            let show_item = MenuItemBuilder::with_id("show", "显示窗口")
                .build(app)?;
            let tray_menu = MenuBuilder::new(app)
                .item(&show_item)
                .separator()
                .item(&quit_item)
                .build()?;

            // --- Get main window ---
            let window = app.get_webview_window("main")
                .expect("main window not found");

            // --- Set window icon (taskbar + title bar) ---
            log::info!("[setup] 设置窗口图标...");
            window.set_icon(custom_icon.clone())?;

            // --- Drag and drop + close-to-hide handler ---
            let window_handler = window.clone();
            window.on_window_event(move |event| {
                match event {
                    tauri::WindowEvent::DragDrop(drop_event) => {
                        match drop_event {
                            tauri::DragDropEvent::Drop { paths, .. } => {
                                if let Some(path) = paths.first() {
                                    let path_str = path.to_string_lossy().to_string();
                                    log::info!("[window_event] 文件拖放: {}", path_str);
                                    let _ = window_handler.emit("file-dropped", path_str);
                                }
                            }
                            tauri::DragDropEvent::Enter { .. } => {}
                            tauri::DragDropEvent::Over { .. } => {}
                            tauri::DragDropEvent::Leave => {}
                            _ => {}
                        }
                    }
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        log::info!("[window_event] 关闭请求 → 隐藏窗口");
                        api.prevent_close();
                        let _ = window_handler.hide();
                    }
                    _ => {}
                }
            });

            // --- Build tray icon ---
            log::info!("[setup] 构建托盘图标...");
            let tray_window = window.clone();
            let _tray = TrayIconBuilder::new()
                .icon(custom_icon)
                .tooltip("图标提取器")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app_handle, event| {
                    log::info!("[tray_menu] 菜单事件: {:?}", event.id());
                    match event.id().as_ref() {
                        "quit" => {
                            log::info!("[tray_menu] 退出程序");
                            app_handle.exit(0);
                        }
                        "show" => {
                            log::info!("[tray_menu] 显示窗口");
                            let _ = tray_window.show();
                            let _ = tray_window.set_focus();
                        }
                        _ => {
                            log::warn!("[tray_menu] 未知菜单项: {:?}", event.id());
                        }
                    }
                })
                .on_tray_icon_event(move |tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        log::info!("[tray_icon] 左键单击 → 显示窗口");
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            log::info!("[setup] 初始化完成");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            extract_icons,
            convert_icon,
            save_icon_file,
            fetch_favicons,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
