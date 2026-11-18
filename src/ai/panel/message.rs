use ely_gpui_component::{
    buttons::{Button, ButtonVariant, IconButton},
    chat::{MessageBubble, Role, StreamingMarkdown},
    primitives::IconName,
    theme::{ActiveTheme, ControlSize, TextSize},
    typography::SelectableText,
};
use gpui::{
    AnyElement, ClipboardItem, Context, EntityId, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div,
};

use super::{Assistant, Line, view::tool_name};
use crate::ai::word;

impl Line {
    fn clipboard_text(&self) -> String {
        match self {
            Self::Message { text, .. } | Self::Note(text) => text.clone(),
            Self::Tool {
                name, args, output, ..
            } => format!("{name}\n{args}\n{output}"),
        }
    }
}

impl Assistant {
    pub(super) fn copy_line(&self, pane: EntityId, index: usize, cx: &gpui::App) {
        if let Some(line) = self
            .conversations
            .get(&pane)
            .and_then(|c| c.lines.get(index))
        {
            cx.write_to_clipboard(ClipboardItem::new_string(line.clipboard_text()));
        }
    }

    fn copy_button(&self, index: usize, offset: usize, cx: &Context<Self>) -> IconButton {
        let pane = self.target.as_ref().unwrap().id;
        let this = cx.entity();
        IconButton::new(format!("ai-copy-{pane}-{index}-{offset}"), IconName::Copy)
            .size(ControlSize::Sm)
            .tooltip(word(cx, "et.ai.copy"))
            .on_click(move |_, _, cx| {
                this.update(cx, |assistant, cx| assistant.copy_line(pane, index, cx));
            })
    }

    pub(super) fn message(
        &self,
        index: usize,
        range: Option<std::ops::Range<usize>>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(line) = self.conversation().and_then(|c| c.lines.get(index)) else {
            return div().into_any_element();
        };
        let colors = cx.theme().colors.clone();
        let offset = range.as_ref().map_or(0, |range| range.start);
        let pane = self.target.as_ref().unwrap().id;
        let id = format!("ai-text-{pane}-{index}-{offset}");
        let copy = self.copy_button(index, offset, cx);
        match line {
            Line::Message { user, text, live } => {
                let body = if *user || *live || range.is_some() {
                    // shortcut: long replies use virtual text until Ely can virtualize Markdown blocks.
                    let text = range.map_or_else(|| text.clone(), |range| text[range].to_string());
                    SelectableText::new(id, text).into_any_element()
                } else {
                    StreamingMarkdown::new(id, text.clone(), *live).into_any_element()
                };
                div()
                    .px_3()
                    .py_2()
                    .child(
                        MessageBubble::new(if *user { Role::User } else { Role::Assistant })
                            .child(body),
                    )
                    .child(div().flex().justify_end().child(copy))
                    .into_any_element()
            }
            Line::Note(text) => div()
                .px_3()
                .py_2()
                .flex()
                .items_start()
                .gap_2()
                .text_size(cx.theme().text_size(TextSize::Sm))
                .text_color(colors.fg_muted)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(SelectableText::new(id, text.clone())),
                )
                .child(copy)
                .into_any_element(),
            Line::Tool {
                name,
                args,
                output,
                pending,
                open,
            } => {
                let result: serde_json::Value = serde_json::from_str(output).unwrap_or_default();
                let failed = result.get("error").is_some()
                    || result["exit_code"].as_i64().is_some_and(|code| code != 0);
                let title = format!(
                    "{} · {}",
                    tool_name(name, cx),
                    word(
                        cx,
                        if *pending {
                            "et.ai.running"
                        } else if output.is_empty() {
                            "et.ai.cancelled"
                        } else if failed {
                            "et.ai.failed"
                        } else {
                            "et.ai.finished"
                        }
                    )
                );
                let this = cx.entity();
                let args: serde_json::Value = serde_json::from_str(args).unwrap_or_default();
                let brief = args["command"]
                    .as_str()
                    .or_else(|| args["path"].as_str())
                    .unwrap_or(name)
                    .to_string();
                div()
                    .px_3()
                    .py_1()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .child(
                                        Button::new(("ai-tool", index), title)
                                            .variant(ButtonVariant::Ghost)
                                            .size(ControlSize::Sm)
                                            .on_click(move |_, _, cx| {
                                                this.update(cx, |assistant, cx| {
                                                    if let Some(Line::Tool { open, .. }) = assistant
                                                        .conversations
                                                        .get_mut(&pane)
                                                        .and_then(|c| c.lines.get_mut(index))
                                                    {
                                                        *open = !*open;
                                                    }
                                                    cx.notify();
                                                })
                                            }),
                                    )
                                    .child(copy),
                            )
                            .child(
                                div()
                                    .text_size(cx.theme().text_size(TextSize::Sm))
                                    .child(SelectableText::new(id.clone(), brief)),
                            )
                            .children(open.then(|| {
                                div()
                                    .id(("ai-tool-output", index))
                                    .max_h_64()
                                    .overflow_y_scroll()
                                    .text_size(cx.theme().text_size(TextSize::Sm))
                                    .text_color(colors.fg_muted)
                                    .child(SelectableText::new(
                                        format!("{id}-output"),
                                        output.clone(),
                                    ))
                            })),
                    )
                    .into_any_element()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_keeps_the_complete_message_and_tool_details() {
        let text = "中文 🐾\n```rust\nlet answer = 42;\n```\n".repeat(2000);
        let lines = [
            Line::Message {
                user: false,
                text: text.clone(),
                live: false,
            },
            Line::Note("错误 details\nnext line".into()),
            Line::Tool {
                name: "run_command".into(),
                args: "{\"command\":\"cargo test\"}".into(),
                output: text.clone(),
                pending: false,
                open: false,
            },
        ];
        assert!(super::super::text::rows(&lines).len() > 2);
        assert_eq!(lines[0].clipboard_text(), text);
        assert_eq!(lines[1].clipboard_text(), "错误 details\nnext line");
        assert_eq!(
            lines[2].clipboard_text(),
            format!("run_command\n{{\"command\":\"cargo test\"}}\n{text}")
        );
    }
}
