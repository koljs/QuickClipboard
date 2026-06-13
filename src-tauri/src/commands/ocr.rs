// OCR 识别命令

use crate::services::get_settings;

// OCR识别结果结构
#[derive(Debug, serde::Serialize)]
pub struct OcrWord {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, serde::Serialize)]
pub struct OcrLine {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub words: Vec<OcrWord>,
    pub word_gaps: Vec<f32>,
}

#[derive(Debug, serde::Serialize)]
pub struct OcrResult {
    pub text: String,
    pub lines: Vec<OcrLine>,
}

// OCR识别图片字节数组
#[tauri::command]
pub async fn recognize_image_ocr(image_data: Vec<u8>) -> Result<OcrResult, String> {
    tokio::task::spawn_blocking(move || {
        use qcocr::recognize_from_bytes;
        
        let result = recognize_from_bytes(&image_data, None)
            .map_err(|e| format!("OCR识别失败: {}", e))?;
        
        convert_ocr_result(result)
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))?
}

// OCR识别图片文件
#[tauri::command]
pub async fn recognize_file_ocr(file_path: String, language: Option<String>) -> Result<OcrResult, String> {
    tokio::task::spawn_blocking(move || {
        use qcocr::recognize_from_file;
        
        let lang = language.as_deref();
        let result = recognize_from_file(&file_path, lang)
            .map_err(|e| format!("OCR识别失败: {}", e))?;
        
        convert_ocr_result(result)
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))?
}

/// 截图翻译：对图片执行OCR识别 + AI翻译，然后创建翻译覆盖窗口
/// 前端截图完成后调用此命令
#[tauri::command]
pub async fn screenshot_translate_ocr(
    app: tauri::AppHandle,
    image_path: String,
    physical_x: Option<i32>,
    physical_y: Option<i32>,
    image_physical_width: Option<u32>,
    image_physical_height: Option<u32>,
) -> Result<(), String> {
    let settings = get_settings();
    
    if !settings.ai_translation_enabled {
        return Err("AI翻译功能未启用，请在设置中开启".to_string());
    }
    if settings.ai_api_key.is_empty() {
        return Err("AI API Key 未设置，请在设置中配置".to_string());
    }

    // 1. OCR识别
    let ocr_result = {
        let path = image_path.clone();
        tokio::task::spawn_blocking(move || {
            use qcocr::recognize_from_file;
            let result = recognize_from_file(&path, None)
                .map_err(|e| format!("OCR识别失败: {}", e))?;
            convert_ocr_result(result)
        })
        .await
        .map_err(|e| format!("OCR任务执行失败: {}", e))??
    };

    if ocr_result.lines.is_empty() {
        return Err("未识别到任何文字".to_string());
    }

    // 2. AI翻译
    let lines: Vec<String> = ocr_result.lines.iter().map(|l| l.text.clone()).collect();
    let target_language = language_code_to_name(&settings.ai_target_language);
    
    let translations = {
        let lines_clone = lines.clone();
        let lang = target_language.clone();
        tokio::task::spawn_blocking(move || {
            crate::services::ai::translate_ocr_lines(&lines_clone, &lang)
        })
        .await
        .map_err(|e| format!("翻译任务执行失败: {}", e))??
    };

    // 3. 构建OCR行信息
    let ocr_lines: Vec<crate::windows::translate_overlay_window::OcrLineInfo> = ocr_result
        .lines
        .iter()
        .map(|line| crate::windows::translate_overlay_window::OcrLineInfo {
            text: line.text.clone(),
            x: line.x,
            y: line.y,
            width: line.width,
            height: line.height,
        })
        .collect();

    // 4. 创建翻译覆盖窗口
    crate::windows::translate_overlay_window::create_translate_overlay(
        app,
        image_path,
        ocr_lines,
        translations,
        target_language,
        physical_x,
        physical_y,
        image_physical_width,
        image_physical_height,
    )
    .await
}

/// 翻译OCR行（供前端直接调用）
#[tauri::command]
pub async fn translate_ocr_lines_cmd(
    lines: Vec<String>,
    target_language: String,
) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || {
        let lang_name = language_code_to_name(&target_language);
        crate::services::ai::translate_ocr_lines(&lines, &lang_name)
    })
    .await
    .map_err(|e| format!("翻译任务执行失败: {}", e))?
}

/// 测试AI翻译API连接
#[tauri::command]
pub async fn test_ai_translation() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        crate::services::ai::test_api_connection()
    })
    .await
    .map_err(|e| format!("测试任务执行失败: {}", e))?
}

// 转换OCR结果为返回格式
fn convert_ocr_result(result: qcocr::OcrRecognitionResult) -> Result<OcrResult, String> {
    let lines = result.lines.iter().map(|line| {
        let words = line.words.iter().map(|word| OcrWord {
            text: word.text.clone(),
            x: word.bounds.x,
            y: word.bounds.y,
            width: word.bounds.width,
            height: word.bounds.height,
        }).collect();
        
        let word_gaps = line.compute_word_gaps();
        
        OcrLine {
            text: line.text.clone(),
            x: line.bounds.x,
            y: line.bounds.y,
            width: line.bounds.width,
            height: line.bounds.height,
            words,
            word_gaps,
        }
    }).collect();
    
    Ok(OcrResult {
        text: result.text,
        lines,
    })
}

/// 将语言代码转换为语言名称（用于翻译prompt）
fn language_code_to_name(code: &str) -> String {
    match code {
        "auto" => "中文".to_string(),
        "zh-CN" | "zh-TW" => "中文".to_string(),
        "en" => "英语".to_string(),
        "ja" => "日语".to_string(),
        "ko" => "韩语".to_string(),
        "fr" => "法语".to_string(),
        "de" => "德语".to_string(),
        "es" => "西班牙语".to_string(),
        "ru" => "俄语".to_string(),
        _ => code.to_string(),
    }
}
