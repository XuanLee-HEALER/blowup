//! Auto-link film titles in wiki markdown to library detail pages.
//!
//! Scans knowledge-base entries for 《film title》 patterns (Chinese
//! book-name marks) and wraps them in markdown links pointing to the
//! library via `#film:{tmdb_id}`. The frontend wiki renderer handles
//! these custom anchors and navigates to the film detail page.
//!
//! Safety: replacements only happen in "safe" regions of the markdown
//! — fenced code blocks, inline code, and existing links are never
//! touched. The original markdown formatting is preserved exactly
//! because we only insert `[` and `](#film:N)` around matched text.

use crate::entries::service as entry_svc;
use crate::infra::events::{DomainEvent, EventBus};
use sqlx::SqlitePool;
use std::ops::Range;

// ── Public API ─────────────────────────────────────────────────────

/// Scan every non-empty wiki entry and inject `[《title》](#film:{tmdb_id})`
/// links wherever `《title》` appears in a safe region.
///
/// `titles` should contain the film's Chinese title and optionally the
/// original title, pre-filtered for length ≥ 2 and deduplicated.
///
/// Publishes `DomainEvent::EntriesChanged` once if any entry was modified.
pub async fn link_film_mentions(
    pool: &SqlitePool,
    events: &EventBus,
    tmdb_id: u64,
    titles: &[&str],
) {
    if titles.is_empty() {
        return;
    }

    let entries = match load_wiki_entries(pool).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "wiki_linker: failed to load entries");
            return;
        }
    };

    let mut any_changed = false;
    for (id, wiki) in entries {
        let (new_wiki, changed) = inject_links(&wiki, titles, tmdb_id);
        if changed {
            if let Err(e) = entry_svc::update_entry_wiki(pool, id, &new_wiki).await {
                tracing::warn!(entry_id = id, error = %e, "wiki_linker: failed to update entry");
            } else {
                tracing::info!(entry_id = id, tmdb_id, "wiki_linker: injected film links");
                any_changed = true;
            }
        }
    }

    if any_changed {
        events.publish(DomainEvent::EntriesChanged);
    }
}

/// Pure function: inject `[《title》](#film:{tmdb_id})` links into
/// markdown text within safe regions. Returns `(new_text, changed)`.
pub fn inject_links(wiki: &str, titles: &[&str], tmdb_id: u64) -> (String, bool) {
    if wiki.is_empty() || titles.is_empty() {
        return (wiki.to_string(), false);
    }

    let mut result = wiki.to_string();
    let mut changed = false;

    // Process titles longest-first to avoid partial-match issues
    // (e.g., "花样年华" before "花样" if both were in the list).
    let mut sorted_titles: Vec<&str> = titles.to_vec();
    sorted_titles.sort_by_key(|t| std::cmp::Reverse(t.len()));

    for title in sorted_titles {
        if title.chars().count() < 2 {
            continue;
        }
        let target = format!("\u{300a}{title}\u{300b}"); // 《title》
        let replacement = format!("[\u{300a}{title}\u{300b}](#film:{tmdb_id})");

        // Collect match positions (on current result string).
        // We must re-scan each time because previous replacements shift offsets.
        loop {
            let regions = find_unsafe_regions(&result);
            let Some(pos) = find_next_safe_match(&result, &target, &regions) else {
                break;
            };
            result = format!(
                "{}{}{}",
                &result[..pos],
                replacement,
                &result[pos + target.len()..]
            );
            changed = true;
        }
    }

    (result, changed)
}

// ── Unsafe region detection ────────────────────────────────────────

/// Identify byte ranges in `text` where replacements must NOT happen:
/// fenced code blocks, inline code, existing markdown links, images.
fn find_unsafe_regions(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut regions: Vec<Range<usize>> = Vec::new();
    let mut i = 0;

    while i < len {
        // Fenced code block: ``` ... ```
        if i + 2 < len && &bytes[i..i + 3] == b"```" {
            let start = i;
            i += 3;
            // Skip optional language tag on the same line
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            // Find closing ```
            loop {
                if i >= len {
                    regions.push(start..len);
                    break;
                }
                if i + 2 < len && &bytes[i..i + 3] == b"```" {
                    i += 3;
                    regions.push(start..i);
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Inline code: ` ... `
        if bytes[i] == b'`' {
            let start = i;
            i += 1;
            while i < len && bytes[i] != b'`' {
                i += 1;
            }
            if i < len {
                i += 1; // skip closing `
            }
            regions.push(start..i);
            continue;
        }

        // Image: ![alt](url)
        if i + 1 < len && bytes[i] == b'!' && bytes[i + 1] == b'[' {
            let start = i;
            i += 2;
            // Skip to closing ]
            while i < len && bytes[i] != b']' {
                i += 1;
            }
            if i < len {
                i += 1; // skip ]
            }
            // Check for (url)
            if i < len && bytes[i] == b'(' {
                i += 1;
                while i < len && bytes[i] != b')' {
                    i += 1;
                }
                if i < len {
                    i += 1; // skip )
                }
            }
            regions.push(start..i);
            continue;
        }

        // Existing link: [text](url) — mark the ENTIRE [...](...)
        if bytes[i] == b'[' {
            let start = i;
            i += 1;
            // Skip to closing ]
            let mut depth = 1;
            while i < len && depth > 0 {
                if bytes[i] == b'[' {
                    depth += 1;
                } else if bytes[i] == b']' {
                    depth -= 1;
                }
                i += 1;
            }
            // Check for (url) immediately after ]
            if i < len && bytes[i] == b'(' {
                i += 1;
                let mut paren_depth = 1;
                while i < len && paren_depth > 0 {
                    if bytes[i] == b'(' {
                        paren_depth += 1;
                    } else if bytes[i] == b')' {
                        paren_depth -= 1;
                    }
                    i += 1;
                }
                regions.push(start..i);
                continue;
            }
            // Not a link — just a bare [ ], don't mark unsafe.
            // But reset i to after the ].
            continue;
        }

        i += 1;
    }

    regions
}

/// Find the first occurrence of `target` in `text` that is NOT covered
/// by any unsafe region. Returns the byte offset or None.
fn find_next_safe_match(
    text: &str,
    target: &str,
    unsafe_regions: &[Range<usize>],
) -> Option<usize> {
    let mut search_from = 0;
    while let Some(pos) = text[search_from..].find(target) {
        let abs_pos = search_from + pos;
        let match_range = abs_pos..abs_pos + target.len();
        if !is_covered(&match_range, unsafe_regions) {
            return Some(abs_pos);
        }
        // Move past this match and try again
        search_from = abs_pos + target.len();
    }
    None
}

/// Check whether `range` overlaps with any region in `regions`.
fn is_covered(range: &Range<usize>, regions: &[Range<usize>]) -> bool {
    regions
        .iter()
        .any(|r| range.start < r.end && range.end > r.start)
}

// ── DB helpers ─────────────────────────────────────────────────────

/// Load all entries that have non-empty wiki content.
async fn load_wiki_entries(pool: &SqlitePool) -> Result<Vec<(i64, String)>, String> {
    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, wiki FROM entries WHERE wiki != '' AND wiki IS NOT NULL")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(rows)
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn inject(wiki: &str, titles: &[&str], tmdb_id: u64) -> (String, bool) {
        inject_links(wiki, titles, tmdb_id)
    }

    #[test]
    fn basic_replacement() {
        let (result, changed) = inject(
            "王家卫在《花样年华》中使用了大量的慢镜头",
            &["花样年华"],
            12345,
        );
        assert!(changed);
        assert_eq!(
            result,
            "王家卫在[《花样年华》](#film:12345)中使用了大量的慢镜头"
        );
    }

    #[test]
    fn already_linked_not_doubled() {
        let wiki = "他在[《花样年华》](#film:12345)中的表演";
        let (result, changed) = inject(wiki, &["花样年华"], 12345);
        assert!(!changed);
        assert_eq!(result, wiki);
    }

    #[test]
    fn fenced_code_not_replaced() {
        let wiki = "正文\n```\n《花样年华》\n```\n尾部";
        let (result, changed) = inject(wiki, &["花样年华"], 12345);
        assert!(!changed);
        assert_eq!(result, wiki);
    }

    #[test]
    fn inline_code_not_replaced() {
        let wiki = "示例 `《花样年华》` 中的代码";
        let (result, changed) = inject(wiki, &["花样年华"], 12345);
        assert!(!changed);
        assert_eq!(result, wiki);
    }

    #[test]
    fn multiple_occurrences_all_replaced() {
        let wiki = "《花样年华》和《花样年华》都是经典";
        let (result, changed) = inject(wiki, &["花样年华"], 12345);
        assert!(changed);
        assert_eq!(
            result,
            "[《花样年华》](#film:12345)和[《花样年华》](#film:12345)都是经典"
        );
    }

    #[test]
    fn multiple_titles() {
        let wiki = "《花样年华》和《重庆森林》都是王家卫的作品";
        let (result, changed) = inject(wiki, &["花样年华", "重庆森林"], 12345);
        assert!(changed);
        // Both should be linked with same tmdb_id (this is per-film call)
        assert!(result.contains("[《花样年华》](#film:12345)"));
        assert!(result.contains("[《重庆森林》](#film:12345)"));
    }

    #[test]
    fn short_title_skipped() {
        let wiki = "《春》是一部短片";
        let (result, changed) = inject(wiki, &["春"], 99);
        assert!(!changed);
        assert_eq!(result, wiki);
    }

    #[test]
    fn no_book_marks_not_replaced() {
        let wiki = "花样年华是一部好电影";
        let (result, changed) = inject(wiki, &["花样年华"], 12345);
        assert!(!changed);
        assert_eq!(result, wiki);
    }

    #[test]
    fn empty_wiki() {
        let (result, changed) = inject("", &["花样年华"], 12345);
        assert!(!changed);
        assert_eq!(result, "");
    }

    #[test]
    fn empty_titles() {
        let wiki = "《花样年华》是经典";
        let (result, changed) = inject(wiki, &[], 12345);
        assert!(!changed);
        assert_eq!(result, wiki);
    }

    #[test]
    fn image_not_replaced() {
        let wiki = "![《花样年华》海报](poster.jpg)\n正文中《花样年华》";
        let (result, changed) = inject(wiki, &["花样年华"], 12345);
        assert!(changed);
        // Image alt text should NOT be replaced; the text after should be.
        assert!(result.contains("![《花样年华》海报](poster.jpg)"));
        assert!(result.contains("正文中[《花样年华》](#film:12345)"));
    }

    #[test]
    fn mixed_safe_and_unsafe() {
        let wiki = "前文《花样年华》中间`《花样年华》`后文《花样年华》";
        let (result, changed) = inject(wiki, &["花样年华"], 12345);
        assert!(changed);
        assert_eq!(
            result,
            "前文[《花样年华》](#film:12345)中间`《花样年华》`后文[《花样年华》](#film:12345)"
        );
    }

    #[test]
    fn existing_different_link_preserved() {
        let wiki = "[《花样年华》](https://example.com) 是经典";
        let (result, changed) = inject(wiki, &["花样年华"], 12345);
        assert!(!changed);
        assert_eq!(result, wiki);
    }

    #[test]
    fn unsafe_regions_fenced_code() {
        let text = "before\n```\ncode\n```\nafter";
        let regions = find_unsafe_regions(text);
        assert!(!regions.is_empty());
        let code_region = &regions[0];
        assert!(text[code_region.clone()].contains("code"));
    }

    #[test]
    fn unsafe_regions_inline_code() {
        let text = "before `code` after";
        let regions = find_unsafe_regions(text);
        assert_eq!(regions.len(), 1);
        assert_eq!(&text[regions[0].clone()], "`code`");
    }

    #[test]
    fn unsafe_regions_link() {
        let text = "before [text](url) after";
        let regions = find_unsafe_regions(text);
        assert_eq!(regions.len(), 1);
        assert_eq!(&text[regions[0].clone()], "[text](url)");
    }

    #[test]
    fn unsafe_regions_bare_brackets_not_marked() {
        let text = "array[0] is fine";
        let regions = find_unsafe_regions(text);
        // [0] is not followed by (...), so it should NOT be marked unsafe
        assert!(regions.is_empty());
    }
}
