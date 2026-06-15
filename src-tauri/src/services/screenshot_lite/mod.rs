use once_cell::sync::OnceCell;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, WebviewWindow, WebviewWindowBuilder, PhysicalPosition};
use serde::{Serialize, Deserialize};

static SCREENSHOT_STATE: OnceCell<Mutex<ScreenshotState>> = OnceCell::new();

#[derive(Debug)]
struct ScreenshotState {
    current_mode: u8,
    screenshot_path: Option<String>,
    select_window_label: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScreenshotSelectData {
    pub mode: u8,
}

/// Initialize the screenshot lite module
pub fn init() {
    SCREENSHOT_STATE.get_or_init(|| {
        Mutex::new(ScreenshotState {
            current_mode: 0,
            screenshot_path: None,
            select_window_label: None,
        })
    });
}

/// Capture the entire screen and save to a temp file, then open selection window
pub fn start_screenshot_with_mode(app: &AppHandle, mode: u8) -> Result<(), String> {
    // 1. Capture screen using xcap
    let screenshot_image = capture_screen()
        .map_err(|e| format!("截屏失败: {}", e))?;

    // 2. Save to temp file
    let temp_dir = std::env::temp_dir().join("QuickClipboard_screenshots");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("创建临时目录失败: {}", e))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let screenshot_path = temp_dir.join(format!("screenshot_{}.png", timestamp));
    let screenshot_path_str = screenshot_path.to_string_lossy().to_string();

    screenshot_image.save(&screenshot_path)
        .map_err(|e| format!("保存截图失败: {}", e))?;

    // 3. Get screen info for window sizing
    let primary_monitor = app.primary_monitor()
        .map_err(|e| format!("获取主显示器失败: {}", e))?
        .ok_or("未找到主显示器")?;

    let screen_size = primary_monitor.size();
    let scale_factor = primary_monitor.scale_factor();

    // 4. Store state
    if let Some(state) = SCREENSHOT_STATE.get() {
        let mut s = state.lock().unwrap();
        s.current_mode = mode;
        s.screenshot_path = Some(screenshot_path_str.clone());
    }

    // 5. Create fullscreen selection window
    let window_label = format!("screenshot-select-{}", timestamp);

    if let Some(state) = SCREENSHOT_STATE.get() {
        let mut s = state.lock().unwrap();
        s.select_window_label = Some(window_label.clone());
    }

    let logical_w = screen_size.width as f64 / scale_factor;
    let logical_h = screen_size.height as f64 / scale_factor;

    let _window = WebviewWindowBuilder::new(
        app, &window_label,
        tauri::WebviewUrl::App("windows/screenshotSelect/screenshotSelect.html".into()),
    )
    .title("截图选区")
    .inner_size(logical_w, logical_h)
    .min_inner_size(1.0, 1.0)
    .resizable(false)
    .maximizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(true)
    .visible(true)
    .drag_and_drop(false)
    .build()
    .map_err(|e| format!("创建选区窗口失败: {}", e))?;

    // Position at screen origin
    let pos = primary_monitor.position();
    // Need to get the window we just created
    if let Some(window) = app.get_webview_window(&window_label) {
        window.set_position(PhysicalPosition::new(pos.x, pos.y))
            .map_err(|e| format!("设置窗口位置失败: {}", e))?;
    }

    Ok(())
}

/// Capture the screen image using xcap
fn capture_screen() -> Result<image::RgbaImage, String> {
    #[cfg(target_os = "windows")]
    {
        use xcap::Monitor;

        let monitors = Monitor::all()
            .map_err(|e| format!("获取显示器列表失败: {}", e))?;

        let primary = monitors.first()
            .ok_or("未找到显示器")?;

        let image = primary.capture_image()
            .map_err(|e| format!("截取屏幕失败: {}", e))?;

        Ok(image)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("当前平台不支持截屏".to_string())
    }
}

/// Get the screenshot image path (called by frontend)
#[tauri::command]
pub fn get_screenshot_image_path() -> Result<String, String> {
    if let Some(state) = SCREENSHOT_STATE.get() {
        let s = state.lock().unwrap();
        s.screenshot_path.clone()
            .ok_or("未找到截图文件路径".to_string())
    } else {
        Err("截图模块未初始化".to_string())
    }
}

/// Get screenshot select data (called by frontend)
#[tauri::command]
pub fn get_screenshot_select_data(window: WebviewWindow) -> Result<ScreenshotSelectData, String> {
    let _ = window; // acknowledge parameter
    if let Some(state) = SCREENSHOT_STATE.get() {
        let s = state.lock().unwrap();
        Ok(ScreenshotSelectData {
            mode: s.current_mode,
        })
    } else {
        Err("截图模块未初始化".to_string())
    }
}

/// Handle screenshot selection complete (called by frontend after user selects area)
#[tauri::command]
pub async fn screenshot_selection_complete(
    app: AppHandle,
    mode: u8,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<(), String> {
    // Get the screenshot path
    let screenshot_path = if let Some(state) = SCREENSHOT_STATE.get() {
        let s = state.lock().unwrap();
        s.screenshot_path.clone()
            .ok_or("未找到截图文件路径".to_string())?
    } else {
        return Err("截图模块未初始化".to_string());
    };

    // Close the selection window first
    if let Some(state) = SCREENSHOT_STATE.get() {
        let s = state.lock().unwrap();
        if let Some(label) = &s.select_window_label {
            if let Some(win) = app.get_webview_window(label) {
                let _ = win.close();
            }
        }
    }

    // Small delay to let the selection window close and restore screen
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Crop the selected region from the full screenshot
    let mut img = image::open(&screenshot_path)
        .map_err(|e| format!("打开截图失败: {}", e))?;

    let crop_x = x.max(0) as u32;
    let crop_y = y.max(0) as u32;
    let crop_w = (width as u32).min(img.width().saturating_sub(crop_x));
    let crop_h = (height as u32).min(img.height().saturating_sub(crop_y));

    if crop_w == 0 || crop_h == 0 {
        return Err("选区大小为零".to_string());
    }

    let cropped = img.crop(crop_x, crop_y, crop_w, crop_h);

    // Save cropped image
    let temp_dir = std::env::temp_dir().join("QuickClipboard_screenshots");
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let cropped_path = temp_dir.join(format!("cropped_{}.png", timestamp));
    let cropped_path_str = cropped_path.to_string_lossy().to_string();

    cropped.save(&cropped_path)
        .map_err(|e| format!("保存裁剪截图失败: {}", e))?;

    // Handle based on mode
    match mode {
        0 => {
            // Normal screenshot - copy image to clipboard using existing utility
            crate::commands::copy_image_to_clipboard(cropped_path_str)?;
        }
        1 => {
            // Quick save - save to pictures directory
            let pictures_dir = dirs::picture_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            let save_path = pictures_dir.join(format!("screenshot_{}.png", timestamp));
            std::fs::copy(&cropped_path, &save_path)
                .map_err(|e| format!("保存截图到图片目录失败: {}", e))?;
        }
        2 => {
            // Quick pin - create pin image window
            crate::windows::pin_image_window::pin_image_from_file(
                app.clone(),
                cropped_path_str,
                Some(x),
                Some(y),
                Some(width as u32),
                Some(height as u32),
                None, // preview_mode
                Some(x), // image_physical_x
                Some(y), // image_physical_y
                Some(width as u32), // image_physical_width
                Some(height as u32), // image_physical_height
                None, // original_image_path
                None, // edit_data
            ).await?;
        }
        3 => {
            // Quick OCR - recognize text and copy to clipboard
            let path_for_ocr = cropped_path_str.clone();
            let ocr_result = tokio::task::spawn_blocking(move || {
                use qcocr::recognize_from_file;
                recognize_from_file(&path_for_ocr, None)
                    .map_err(|e| format!("OCR识别失败: {}", e))
            })
            .await
            .map_err(|e| format!("OCR任务执行失败: {}", e))??;

            use clipboard_rs::{Clipboard, ClipboardContext};
            let ctx = ClipboardContext::new()
                .map_err(|e| format!("创建剪贴板上下文失败: {}", e))?;
            ctx.set_text(ocr_result.text)
                .map_err(|e| format!("复制OCR文本失败: {}", e))?;
        }
        4 => {
            // Screenshot translate - OCR + translate + overlay
            crate::commands::ocr::screenshot_translate_ocr(
                app,
                cropped_path_str,
                Some(x),
                Some(y),
                Some(width as u32),
                Some(height as u32),
            ).await?;
        }
        _ => return Err(format!("不支持的截图模式: {}", mode)),
    }

    // Clean up temp files
    let _ = std::fs::remove_file(&screenshot_path);

    Ok(())
}
