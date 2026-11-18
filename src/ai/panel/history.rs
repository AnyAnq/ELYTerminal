use ely_gpui_component::{buttons::IconButton, primitives::IconName, theme::ControlSize};
use gpui::{Context, Focusable, Window};

use super::{Assistant, Line};
use crate::ai::word;

#[derive(Default)]
pub(super) struct History {
    cursor: Option<usize>,
    draft: String,
}

impl History {
    fn previous(&self, lines: &[Line]) -> Option<usize> {
        let end = self.cursor.unwrap_or(lines.len());
        lines[..end]
            .iter()
            .rposition(|line| matches!(line, Line::Message { user: true, .. }))
    }

    fn step(&mut self, lines: &[Line], current: &str, older: bool) -> Option<String> {
        let index = if older {
            let index = self.previous(lines)?;
            if self.cursor.is_none() {
                self.draft = current.to_string();
            }
            index
        } else {
            let cursor = self.cursor?;
            let next = lines
                .iter()
                .enumerate()
                .skip(cursor + 1)
                .find_map(|(i, line)| {
                    matches!(line, Line::Message { user: true, .. }).then_some(i)
                });
            let Some(next) = next else {
                self.cursor = None;
                return Some(std::mem::take(&mut self.draft));
            };
            next
        };
        let Line::Message { text, .. } = &lines[index] else {
            unreachable!()
        };
        self.cursor = Some(index);
        Some(text.clone())
    }
}

impl Assistant {
    pub(super) fn recall(&mut self, older: bool, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.input.read(cx).text();
        let recalled = self
            .target
            .as_ref()
            .and_then(|target| self.conversations.get_mut(&target.id))
            .and_then(|conversation| {
                conversation
                    .history
                    .step(&conversation.lines, current, older)
            });
        if let Some(recalled) = recalled {
            self.input
                .update(cx, |input, cx| input.set_text(recalled, cx));
            window.focus(&self.input.focus_handle(cx), cx);
            cx.notify();
        }
    }

    pub(super) fn history_button(&self, older: bool, cx: &Context<Self>) -> IconButton {
        let enabled = self.conversation().is_some_and(|conversation| {
            if older {
                conversation.history.previous(&conversation.lines).is_some()
            } else {
                conversation.history.cursor.is_some()
            }
        });
        let this = cx.entity();
        IconButton::new(
            if older {
                "ai-history-previous"
            } else {
                "ai-history-next"
            },
            if older {
                IconName::ChevronUp
            } else {
                IconName::ChevronDown
            },
        )
        .size(ControlSize::Sm)
        .tooltip(word(
            cx,
            if older {
                "et.ai.previous_input"
            } else {
                "et.ai.next_input"
            },
        ))
        .disabled(!enabled)
        .on_click(move |_, window, cx| {
            this.update(cx, |assistant, cx| assistant.recall(older, window, cx));
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt(text: &str) -> Line {
        Line::Message {
            user: true,
            text: text.into(),
            live: false,
        }
    }

    #[test]
    fn recall_skips_replies_and_restores_the_unsent_draft() {
        let lines = vec![
            prompt("first"),
            Line::Note("reply".into()),
            prompt("中文\n🐾"),
        ];
        let mut history = History::default();
        assert_eq!(
            history.step(&lines, "  draft\n草稿  ", true).as_deref(),
            Some("中文\n🐾")
        );
        assert_eq!(
            history.step(&lines, "edited recall", true).as_deref(),
            Some("first")
        );
        assert_eq!(history.step(&lines, "first", true), None);
        assert_eq!(
            history.step(&lines, "first", false).as_deref(),
            Some("中文\n🐾")
        );
        assert_eq!(
            history.step(&lines, "中文\n🐾", false).as_deref(),
            Some("  draft\n草稿  ")
        );
        assert_eq!(history.step(&lines, "draft", false), None);
        assert!(history.cursor.is_none());
    }

    #[test]
    fn empty_and_reset_history_leave_the_input_alone() {
        let mut history = History::default();
        assert_eq!(history.step(&[], "draft", true), None);
        assert_eq!(history.step(&[], "draft", false), None);
        let lines = vec![prompt("last")];
        assert_eq!(history.step(&lines, "", true).as_deref(), Some("last"));
        history = History::default();
        assert_eq!(history.step(&lines, "new draft", false), None);
        assert_eq!(
            history.step(&lines, "new draft", true).as_deref(),
            Some("last")
        );
        assert_eq!(
            history.step(&lines, "edited", false).as_deref(),
            Some("new draft")
        );
    }
}
