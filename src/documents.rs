use std::collections::BTreeSet;

use scraper::{Html, Selector};

use crate::extract::{clean_text, extract_text_from_html};

pub const RELATION_DOCUMENTS: &str = "documents";
pub const RELATION_REFERENCES: &str = "references";

const DOCUMENT_REFERENCE_TAGS: &[&str] = &[
    "hirsel-node-ref",
    "hirsel-node-field",
    "hirsel-node-list",
    "hirsel-doc-target",
    "hirsel-doc-link",
    "hirsel-doc-embed",
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DocumentReference {
    pub kind: String,
    pub node_id: String,
    pub relation: String,
}

pub fn graph_node_record_key(kind: &str, node_id: &str) -> String {
    format!("{kind}--{node_id}")
}

pub fn normalize_node_id(candidate: &str) -> Option<String> {
    let mut slug = String::new();
    let mut last_dash = false;

    for ch in candidate.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if matches!(ch, '-' | '_' | '/' | ':' | '.' | ' ') {
            Some('-')
        } else {
            None
        };

        let Some(mapped) = mapped else {
            continue;
        };

        if mapped == '-' {
            if slug.is_empty() || last_dash {
                continue;
            }
            last_dash = true;
            slug.push('-');
        } else {
            last_dash = false;
            slug.push(mapped);
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() { None } else { Some(slug) }
}

pub fn slugify_node_id(label: &str) -> String {
    normalize_node_id(label).unwrap_or_else(|| "node".to_string())
}

pub fn ensure_document_markup(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if looks_like_markup(trimmed) {
        trimmed.to_string()
    } else {
        plain_text_to_markup(trimmed)
    }
}

pub fn content_search_text(content: &str) -> String {
    if looks_like_markup(content) {
        extract_text_from_html(content)
    } else {
        clean_text(content)
    }
}

pub fn imported_document_markup(label: &str, text: &str) -> Option<String> {
    let body = paragraphize(text);
    if body.is_empty() {
        return None;
    }
    Some(format!(
        "<hirsel-card eyebrow=\"Imported document\" heading=\"{}\">{}</hirsel-card>",
        escape_html(label),
        body
    ))
}

pub fn imported_url_markup(label: &str, url: &str, text: &str) -> Option<String> {
    let body = paragraphize(text);
    if body.is_empty() {
        return Some(format!(
            "<hirsel-card eyebrow=\"Source URL\" heading=\"{}\"><p><a href=\"{}\">{}</a></p></hirsel-card>",
            escape_html(label),
            escape_html(url),
            escape_html(url)
        ));
    }

    Some(format!(
        "<hirsel-card eyebrow=\"Source URL\" heading=\"{}\"><p><a href=\"{}\">{}</a></p>{}</hirsel-card>",
        escape_html(label),
        escape_html(url),
        escape_html(url),
        body
    ))
}

pub fn extract_document_references(html: &str) -> Vec<DocumentReference> {
    if html.trim().is_empty() {
        return Vec::new();
    }

    let fragment = Html::parse_fragment(html);
    let mut refs = BTreeSet::new();
    for tag_name in DOCUMENT_REFERENCE_TAGS {
        let Ok(selector) = Selector::parse(tag_name) else {
            continue;
        };

        for node in fragment.select(&selector) {
            let relation = if *tag_name == "hirsel-doc-target" {
                RELATION_DOCUMENTS
            } else {
                RELATION_REFERENCES
            };
            let raw = node.value().attr("node").map(str::trim).unwrap_or("");
            let Some((kind, node_id)) = parse_node_reference(raw) else {
                continue;
            };
            refs.insert(DocumentReference {
                kind,
                node_id,
                relation: relation.to_string(),
            });
        }
    }
    refs.into_iter().collect()
}

pub fn parse_node_reference(value: &str) -> Option<(String, String)> {
    let trimmed = value.trim();
    let (kind, node_id) = trimmed.split_once(':')?;
    let kind = normalize_node_id(kind)?;
    let node_id = normalize_node_id(node_id)?;
    Some((kind, node_id))
}

pub fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn looks_like_markup(value: &str) -> bool {
    let trimmed = value.trim_start();
    trimmed.starts_with('<') && trimmed.contains('>')
}

fn plain_text_to_markup(text: &str) -> String {
    paragraphize(text)
}

fn paragraphize(text: &str) -> String {
    let compact = text
        .replace("\r\n", "\n")
        .split("\n\n")
        .map(str::trim)
        .filter(|chunk| !chunk.is_empty())
        .map(clean_text)
        .filter(|chunk| !chunk.is_empty())
        .collect::<Vec<_>>();

    if compact.is_empty() {
        return String::new();
    }

    compact
        .into_iter()
        .map(|paragraph| format!("<p>{}</p>", escape_html(&paragraph)))
        .collect::<Vec<_>>()
        .join("")
}
