use crate::services::get_settings;
use serde::{Deserialize, Serialize};

// ── OpenAI Chat Completions 请求/响应结构 ──

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ApiError {
    error: Option<ApiErrorDetail>,
}

#[derive(Deserialize)]
struct ApiErrorDetail {
    message: String,
}

// ── OCR 行翻译的 JSON 响应结构 ──

#[derive(Deserialize)]
struct OcrTranslationResponse {
    translations: Vec<String>,
}

// ── 内部通用调用函数 ──

fn call_chat_api(system_prompt: &str, user_content: &str) -> Result<String, String> {
    let settings = get_settings();

    if settings.ai_api_key.is_empty() {
        return Err("AI API Key 未设置，请在设置中配置".to_string());
    }

    let base_url = if settings.ai_base_url.is_empty() {
        "https://api.siliconflow.cn/v1".to_string()
    } else {
        settings
            .ai_base_url
            .trim()
            .trim_end_matches('/')
            .to_string()
    };

    let model = if settings.ai_model.is_empty() {
        "Qwen/Qwen2-7B-Instruct".to_string()
    } else {
        settings.ai_model.clone()
    };

    let url = format!("{}/chat/completions", base_url);

    let request = ChatRequest {
        model: model.clone(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_content.to_string(),
            },
        ],
        temperature: 0.3,
        max_tokens: 4096,
    };

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", settings.ai_api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .map_err(|e| format!("网络请求失败：{}", e))?;

    let status = response.status();
    let body = response
        .text()
        .map_err(|e| format!("读取响应失败：{}", e))?;

    if !status.is_success() {
        // 尝试解析 API 返回的错误信息
        if let Ok(api_err) = serde_json::from_str::<ApiError>(&body) {
            if let Some(detail) = api_err.error {
                return Err(format!("API 错误（{}）：{}", status, detail.message));
            }
        }
        return Err(format!("API 请求失败（HTTP {}）：{}", status, body));
    }

    let chat_resp: ChatResponse = serde_json::from_str(&body)
        .map_err(|e| format!("解析 API 响应失败：{}，响应内容：{}", e, body))?;

    let content = chat_resp
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "API 未返回任何结果".to_string())?;

    Ok(content.trim().to_string())
}

// ── 公开接口 ──

/// 翻译文本
pub fn translate_text(text: &str, target_language: &str) -> Result<String, String> {
    let settings = get_settings();

    let prompt_template = if settings.ai_translation_prompt.is_empty() {
        "请将以下文本翻译成{target_language}，严格保持原文的所有格式、换行符、段落结构和空白字符，只返回翻译结果，不要添加任何解释或修改格式：".to_string()
    } else {
        settings.ai_translation_prompt.clone()
    };

    let system_prompt = prompt_template.replace("{target_language}", target_language);

    call_chat_api(&system_prompt, text)
}

/// 翻译 OCR 识别的文本行（保持行数一致）
pub fn translate_ocr_lines(lines: &[String], target_language: &str) -> Result<Vec<String>, String> {
    if lines.is_empty() {
        return Ok(vec![]);
    }

    let line_count = lines.len();
    let system_prompt = format!(
        "请将以下文本逐行翻译为{}。严格按JSON格式返回：{{\"translations\":[\"第一行翻译\",\"第二行翻译\",...]}}。注意：翻译结果行数必须与原文行数完全一致（{}行），不要合并或拆分任何行。只返回JSON，不要添加任何其他内容。",
        target_language, line_count
    );

    let user_content = lines.join("\n");

    let raw = call_chat_api(&system_prompt, &user_content)?;

    // 尝试从响应中提取 JSON
    let json_str = extract_json_object(&raw);

    // 尝试解析为结构化 JSON
    if let Ok(parsed) = serde_json::from_str::<OcrTranslationResponse>(&json_str) {
        if parsed.translations.len() == line_count {
            return Ok(parsed.translations);
        }
        // 行数不匹配，尝试截断或补齐
        return Ok(align_translations(parsed.translations, lines));
    }

    // JSON 解析失败，回退到按换行分割
    let fallback: Vec<String> = raw
        .lines()
        .map(|l| l.to_string())
        .filter(|l| !l.trim().is_empty())
        .collect();

    Ok(align_translations(fallback, lines))
}

/// 测试 API 连接
pub fn test_api_connection() -> Result<String, String> {
    call_chat_api("你是一个翻译助手。", "Hello")
}

// ── 辅助函数 ──

/// 从可能包含 markdown 代码块的文本中提取 JSON 对象字符串
fn extract_json_object(text: &str) -> String {
    let trimmed = text.trim();

    // 如果直接以 { 开头，尝试直接返回
    if trimmed.starts_with('{') {
        return trimmed.to_string();
    }

    // 尝试从 markdown 代码块中提取
    if let Some(content) = extract_from_code_block(trimmed) {
        return content;
    }

    // 尝试找到第一个 { 和最后一个 }
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                return trimmed[start..=end].to_string();
            }
        }
    }

    trimmed.to_string()
}

fn extract_from_code_block(text: &str) -> Option<String> {
    // 匹配 ```json ... ``` 或 ``` ... ```
    let patterns = ["```json", "```"];
    for pattern in patterns {
        if let Some(start_idx) = text.find(pattern) {
            let content_start = start_idx + pattern.len();
            if let Some(end_idx) = text[content_start..].find("```") {
                let content = text[content_start..content_start + end_idx].trim();
                if content.starts_with('{') {
                    return Some(content.to_string());
                }
            }
        }
    }
    None
}

/// 将翻译结果与原文行数对齐：不足则用原文补齐，超出则截断
fn align_translations(translations: Vec<String>, original: &[String]) -> Vec<String> {
    let mut result = translations;

    // 不足：用原文补齐
    while result.len() < original.len() {
        let idx = result.len();
        result.push(original[idx].clone());
    }

    // 超出：截断
    result.truncate(original.len());

    result
}
