/// Website favicon fetcher using multiple strategies.
/// Tries HTML parsing, direct /favicon.ico, and Google's favicon service.

use base64::Engine;
use image::ImageEncoder;
use regex::Regex;
use serde::Serialize;
use std::io::Cursor;
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct FaviconData {
    pub size: u32,
    pub width: u32,
    pub height: u32,
    /// Base64-encoded PNG data URL
    pub data_url: String,
    /// Where the favicon was found
    pub source: String,
}

/// Normalize user input into a proper URL.
fn normalize_url(input: &str) -> Result<Url, String> {
    let trimmed = input.trim();
    // If no scheme, prepend https://
    let with_scheme = if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        format!("https://{}", trimmed)
    } else {
        trimmed.to_string()
    };
    Url::parse(&with_scheme).map_err(|e| format!("无效的网址: {e}"))
}

/// Fetch favicons from a website using multiple strategies.
pub async fn fetch_favicons(url_input: &str) -> Result<Vec<FaviconData>, String> {
    let base_url = normalize_url(url_input)?;
    log::info!("[favicon] 目标网址: {}", base_url);

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let mut results: Vec<FaviconData> = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    // Strategy 1: Parse HTML homepage for <link rel="icon"> tags
    log::info!("[favicon] 策略1: 解析网页 HTML 查找图标链接...");
    if let Some(icons) = fetch_from_html(&client, &base_url).await {
        for icon_url in icons {
            if seen_urls.insert(icon_url.clone()) {
                log::info!("[favicon] 尝试下载: {}", icon_url);
                if let Some(fav) = download_and_convert(&client, &icon_url, "HTML <link>").await {
                    results.push(fav);
                }
            }
        }
    }

    // Strategy 2: Try /favicon.ico
    if let Ok(favicon_url) = base_url.join("/favicon.ico") {
        let url_str = favicon_url.to_string();
        if seen_urls.insert(url_str.clone()) {
            log::info!("[favicon] 策略2: 尝试 {}", url_str);
            if let Some(fav) = download_and_convert(&client, &url_str, "/favicon.ico").await {
                results.push(fav);
            }
        }
    }

    // Strategy 3: Try /favicon.png
    if let Ok(favicon_url) = base_url.join("/favicon.png") {
        let url_str = favicon_url.to_string();
        if seen_urls.insert(url_str.clone()) {
            log::info!("[favicon] 策略3: 尝试 {}", url_str);
            if let Some(fav) = download_and_convert(&client, &url_str, "/favicon.png").await {
                results.push(fav);
            }
        }
    }

    // Strategy 4: Google favicon service (multiple sizes)
    if let Some(domain) = base_url.domain() {
        let sizes = [16u32, 32, 64, 128, 256];
        for &sz in &sizes {
            let google_url = format!(
                "https://www.google.com/s2/favicons?domain={}&sz={}",
                domain, sz
            );
            if seen_urls.insert(google_url.clone()) {
                log::info!("[favicon] 策略4: 尝试 Google 服务 ({})", google_url);
                if let Some(fav) = download_and_convert(&client, &google_url, "Google Favicon").await {
                    results.push(fav);
                }
            }
        }
    }

    if results.is_empty() {
        Err(format!("未找到 {} 的图标", base_url))
    } else {
        // Deduplicate by size
        let mut unique: Vec<FaviconData> = Vec::new();
        let mut seen_sizes = std::collections::HashSet::new();
        for fav in results {
            if seen_sizes.insert(fav.size) {
                unique.push(fav);
            }
        }
        unique.sort_by_key(|f| f.size);
        log::info!("[favicon] 共获取 {} 个图标", unique.len());
        Ok(unique)
    }
}

/// Parse HTML to find favicon <link> tags. Returns absolute URLs.
async fn fetch_from_html(
    client: &reqwest::Client,
    base_url: &Url,
) -> Option<Vec<String>> {
    let resp = client.get(base_url.clone()).send().await.ok()?;
    let html = resp.text().await.ok()?;

    let mut icon_urls = Vec::new();

    // Regex to match <link ... rel="icon" ... href="...">
    // Match patterns like:
    // <link rel="icon" href="/favicon.ico">
    // <link rel="shortcut icon" href="https://.../icon.png">
    // <link rel='icon' type='image/png' href='...'>
    let re = Regex::new(
        r#"<link[^>]*rel=["'](?:shortcut )?icon["'][^>]*href=["']([^"']+)["']"#
    ).ok()?;

    // Also try reversed order: href before rel
    let re2 = Regex::new(
        r#"<link[^>]*href=["']([^"']+)["'][^>]*rel=["'](?:shortcut )?icon["']"#
    ).ok()?;

    for cap in re.captures_iter(&html) {
        if let Some(href) = cap.get(1) {
            if let Ok(abs_url) = base_url.join(href.as_str()) {
                icon_urls.push(abs_url.to_string());
            }
        }
    }

    if icon_urls.is_empty() {
        for cap in re2.captures_iter(&html) {
            if let Some(href) = cap.get(1) {
                if let Ok(abs_url) = base_url.join(href.as_str()) {
                    icon_urls.push(abs_url.to_string());
                }
            }
        }
    }

    if icon_urls.is_empty() {
        None
    } else {
        Some(icon_urls)
    }
}

/// Download an image from a URL, detect format, convert to PNG base64.
async fn download_and_convert(
    client: &reqwest::Client,
    url: &str,
    source: &str,
) -> Option<FaviconData> {
    let resp = client.get(url).send().await.ok()?;
    let bytes = resp.bytes().await.ok()?;

    if bytes.is_empty() {
        return None;
    }

    // Try to load the image (handles ICO, PNG, JPEG, GIF, BMP, etc.)
    let img = image::load_from_memory(&bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();

    let mut png_buf = Cursor::new(Vec::new());
    let encoder = image::codecs::png::PngEncoder::new(&mut png_buf);
    encoder
        .write_image(&rgba, w, h, image::ExtendedColorType::Rgba8)
        .ok()?;

    let png_bytes = png_buf.into_inner();
    let data_url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&png_bytes)
    );

    Some(FaviconData {
        size: w.max(h),
        width: w,
        height: h,
        data_url,
        source: source.to_string(),
    })
}
