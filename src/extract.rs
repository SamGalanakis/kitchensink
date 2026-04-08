use anyhow::{Context, Result};
use bytes::Bytes;
use scraper::Html;

pub fn extract_text(content_type: &str, bytes: &Bytes) -> Result<String> {
    if content_type.contains("pdf") {
        let text = pdf_extract::extract_text_from_mem(bytes).context("extract text from pdf")?;
        return Ok(clean_text(&text));
    }

    let decoded = String::from_utf8_lossy(bytes);
    if content_type.contains("html") {
        return Ok(extract_text_from_html(&decoded));
    }

    Ok(clean_text(&decoded))
}

pub fn extract_text_from_html(html: &str) -> String {
    let fragment = Html::parse_document(html);
    let text = fragment.root_element().text().collect::<Vec<_>>().join(" ");
    clean_text(&text)
}

pub fn make_summary(text: &str) -> Option<String> {
    let compact = clean_text(text);
    if compact.is_empty() {
        return None;
    }
    let summary = compact
        .split_terminator(['.', '!', '?'])
        .map(str::trim)
        .filter(|sentence| !sentence.is_empty())
        .take(2)
        .collect::<Vec<_>>()
        .join(". ");
    if summary.is_empty() {
        Some(compact.chars().take(280).collect())
    } else if summary.ends_with('.') {
        Some(summary)
    } else {
        Some(format!("{summary}."))
    }
}

pub fn clean_text(input: &str) -> String {
    input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}
