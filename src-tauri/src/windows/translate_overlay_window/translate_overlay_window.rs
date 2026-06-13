use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::collections::HashMap;
use once_cell::sync::OnceCell;
use tauri::{AppHandle, Manager, WebviewWindow, WebviewWindowBuilder, PhysicalPosition};

static TRANSLATE_OVERLAY_COUNTER: AtomicUsize = AtomicUsize::new(0);
static TRANSLATE_OVERLAY_DATA_MAP: OnceCell<Mutex<HashMap<String, TranslateOverlayData>>> = OnceCell::new();

#[derive(Clone, Debug)]
struct TranslateOverlayData {
    image_path: String,
    ocr_lines: Vec<OcrLineInfo>,
    translations: Vec<String>,
    target_language: String,
    physical_x: Option<i32>,
    physical_y: Option<i32>,
    image_width: u32,
    image_height: u32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OcrLineInfo {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub fn init_translate_overlay_window() {
    TRANSLATE_OVERLAY_COUNTER.store(0, Ordering::SeqCst);
    TRANSLATE_OVERLAY_DATA_MAP.get_or_init(|| Mutex::new(HashMap::new()));
}

/// Create a translate overlay window at the specified screen position
/// This is called after OCR + translation is complete
#[tauri::command]
pub async fn create_translate_overlay(
    app: AppHandle,
    image_path: String,
    ocr_lines: Vec<OcrLineInfo>,
    translations: Vec<String>,
    target_language: String,
    physical_x: Option<i32>,
    physical_y: Option<i32>,
    image_physical_width: Option<u32>,
    image_physical_height: Option<u32>,
) -> Result<(), String> {
    let scale_factor = crate::utils::screen::ScreenUtils::get_scale_factor_at_point(
        &app,
        physical_x.unwrap_or(0),
        physical_y.unwrap_or(0),
    );

    let logical_w = image_physical_width.map(|w| (w as f64 / scale_factor).round() as u32).unwrap_or(400);
    let logical_h = image_physical_height.map(|h| (h as f64 / scale_factor).round() as u32).unwrap_or(300);

    let window_label = format!("translate-overlay-{}", TRANSLATE_OVERLAY_COUNTER.fetch_add(1, Ordering::SeqCst));

    // Store data
    if let Some(data_map) = TRANSLATE_OVERLAY_DATA_MAP.get() {
        data_map.lock().unwrap().insert(
            window_label.clone(),
            TranslateOverlayData {
                image_path,
                ocr_lines,
                translations,
                target_language,
                physical_x,
                physical_y,
                image_width: logical_w,
                image_height: logical_h,
            },
        );
    }

    let padding = (5.0 * scale_factor).round() as i32;
    let control_bar_height = 36;

    let window = WebviewWindowBuilder::new(
        &app, &window_label,
        tauri::WebviewUrl::App("windows/translateOverlay/translateOverlay.html".into()),
    )
    .title("截图翻译")
    .inner_size(logical_w as f64 + 10.0, logical_h as f64 + 10.0 + control_bar_height as f64)
    .min_inner_size(1.0, 1.0)
    .resizable(false)
    .maximizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(true)
    .visible(false)
    .drag_and_drop(false)
    .build()
    .map_err(|e| format!("创建翻译覆盖窗口失败: {}", e))?;

    if let (Some(px), Some(py)) = (physical_x, physical_y) {
        window.set_position(PhysicalPosition::new(px - padding, py - padding))
            .map_err(|e| format!("设置窗口位置失败: {}", e))?;
    }

    window.show().map_err(|e| format!("显示翻译窗口失败: {}", e))?;
    Ok(())
}

/// Get translate overlay data (called by the frontend)
#[tauri::command]
pub fn get_translate_overlay_data(window: WebviewWindow) -> Result<serde_json::Value, String> {
    if let Some(data_map) = TRANSLATE_OVERLAY_DATA_MAP.get() {
        let map = data_map.lock().unwrap();
        if let Some(data) = map.get(window.label()) {
            return Ok(json!({
                "image_path": data.image_path,
                "ocr_lines": data.ocr_lines,
                "translations": data.translations,
                "target_language": data.target_language,
                "physical_x": data.physical_x,
                "physical_y": data.physical_y,
                "image_width": data.image_width,
                "image_height": data.image_height,
            }));
        }
    }
    Err("未找到翻译覆盖层数据".to_string())
}

/// Close translate overlay window
#[tauri::command]
pub async fn close_translate_overlay(window: WebviewWindow) -> Result<(), String> {
    if let Some(data_map) = TRANSLATE_OVERLAY_DATA_MAP.get() {
        data_map.lock().unwrap().remove(window.label());
    }
    window.close().map_err(|e| format!("关闭翻译窗口失败: {}", e))
}
