//! ツールチップ用テキスト折り返し

pub const TOOLTIP_WRAP_LIMIT_BODY: usize = 42;
pub const TOOLTIP_WRAP_LIMIT_ICON_ROW: usize = 36;

pub fn wrap_tooltip_text(text: &str, limit: usize) -> Vec<String> {
    let mut output = Vec::new();
    for raw_line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let mut remaining = raw_line.to_string();
        while remaining.chars().count() > limit {
            let split_idx = preferred_split_index(&remaining, limit)
                .or_else(|| whitespace_split_index(&remaining, limit))
                .unwrap_or_else(|| char_to_byte_idx(&remaining, limit));
            let (head, tail) = remaining.split_at(split_idx);
            output.push(head.trim().to_string());
            remaining = tail
                .trim_start_matches(|ch: char| ch.is_whitespace() || [',', ';', '|'].contains(&ch))
                .to_string();
            if remaining.is_empty() {
                break;
            }
        }
        if !remaining.is_empty() {
            output.push(remaining);
        }
    }

    if output.is_empty() {
        output.push(String::new());
    }
    output
}

fn preferred_split_index(text: &str, limit: usize) -> Option<usize> {
    let limit_byte = char_to_byte_idx(text, limit);
    let mut best: Option<usize> = None;
    for pattern in [": ", ", ", " - ", " | ", "; "] {
        if let Some((idx, _)) = text[..limit_byte].rmatch_indices(pattern).next() {
            let split_at = if pattern == ": " { idx + 1 } else { idx };
            if split_at > 0 {
                best = best.max(Some(split_at));
            }
        }
    }
    best
}

fn whitespace_split_index(text: &str, limit: usize) -> Option<usize> {
    let limit_byte = char_to_byte_idx(text, limit);
    text[..limit_byte]
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| ch.is_whitespace().then_some(idx))
}

fn char_to_byte_idx(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len())
}
