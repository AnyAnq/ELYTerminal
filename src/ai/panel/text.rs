use std::ops::Range;

use super::Line;

const CHUNK: usize = 4096;

pub(super) fn rows(lines: &[Line]) -> Vec<(usize, Option<Range<usize>>)> {
    let mut rows = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if matches!(line, Line::Message { text, .. } if text.is_empty()) {
            continue;
        }
        if let Line::Message { text, .. } = line
            && text.len() > CHUNK
        {
            let mut start = 0;
            while start < text.len() {
                let (part, _) = crate::ai::tools::bounded(&text[start..], CHUNK);
                let end = part
                    .rfind('\n')
                    .filter(|at| *at >= CHUNK / 2)
                    .map_or(part.len(), |at| at + 1);
                rows.push((index, Some(start..start + end)));
                start += end;
            }
        } else {
            rows.push((index, None));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_rows_keep_the_complete_unicode_reply() {
        let text = "中文 🐾 terminal output\n".repeat(5000);
        let lines = vec![
            Line::Message {
                user: false,
                text: text.clone(),
                live: true,
            },
            Line::Note("after".into()),
        ];
        let rows = rows(&lines);
        assert!(rows.len() > 20);
        let restored: String = rows
            .iter()
            .filter_map(|(line, range)| {
                assert!(*line <= 1);
                range.as_ref().map(|range| {
                    assert!(range.len() <= CHUNK);
                    &text[range.clone()]
                })
            })
            .collect();
        assert_eq!(restored, text);
        assert_eq!(rows.last(), Some(&(1, None)));
    }

    #[test]
    fn tool_only_replies_have_no_empty_message_or_copy_button() {
        let lines = [
            Line::Message {
                user: false,
                text: String::new(),
                live: false,
            },
            Line::Note("tool result".into()),
        ];
        assert_eq!(rows(&lines), vec![(1, None)]);
    }
}
