use ely_gpui_component::{
    buttons::{Button, ButtonVariant, IconButton},
    chat::{CodeBlock, MessageList, PromptInput},
    forms::{Choice, Select, Switch},
    primitives::{IconName, Tooltip},
    theme::{ActiveTheme, ControlSize, Radius, TextSize},
    typography::SelectableText,
};
use gpui::{
    AnyElement, Context, Focusable, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div,
};

use super::Assistant;
use crate::{ai::word, settings::SettingsStore};

pub(super) fn tool_name(name: &str, cx: &gpui::App) -> gpui::SharedString {
    word(
        cx,
        match name {
            "run_command" => "et.ai.run_command",
            "read_terminal" => "et.ai.read_terminal",
            "read_file" => "et.ai.read_file",
            "edit_file" => "et.ai.edit_file",
            _ => "et.ai.working",
        },
    )
}

impl Assistant {
    fn approval(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let (call, _) = self.running.as_ref()?.approval.as_ref()?;
        let args: serde_json::Value = serde_json::from_str(&call.function.arguments).ok()?;
        let command = args["command"].as_str().unwrap_or("").to_string();
        let reason = args["reason"].as_str().unwrap_or("").to_string();
        let yes = cx.entity();
        let no = yes.clone();
        Some(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .border_1()
                .border_color(cx.theme().colors.warning)
                .rounded(cx.theme().radius(Radius::Md))
                .child(word(cx, "et.ai.approval"))
                .child(CodeBlock::new("ai-approved-command", command))
                .child(SelectableText::new("ai-approval-reason", reason))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new("ai-approve", word(cx, "et.ai.approve"))
                                .variant(ButtonVariant::Primary)
                                .on_click(move |_, _, cx| {
                                    yes.update(cx, |assistant, cx| assistant.decide(true, cx))
                                }),
                        )
                        .child(
                            Button::new("ai-decline", word(cx, "et.ai.decline")).on_click(
                                move |_, _, cx| {
                                    no.update(cx, |assistant, cx| assistant.decide(false, cx))
                                },
                            ),
                        ),
                )
                .into_any_element(),
        )
    }
}

impl Render for Assistant {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors.clone();
        let selected = &SettingsStore::get(cx).ai;
        let mut choices: Vec<_> = selected
            .models
            .iter()
            .map(|model| Choice::new(model.id.clone(), model.name.clone()))
            .collect();
        let pinned = self
            .running
            .as_ref()
            .map(|running| running.model.clone())
            .or_else(|| {
                self.conversation()
                    .and_then(|conversation| conversation.resume.as_ref())
                    .map(|resume| resume.model.clone())
            });
        let current = pinned
            .as_ref()
            .map(|model| model.id.clone())
            .or_else(|| selected.selected.clone());
        if let Some(model) = &pinned
            && !selected.models.iter().any(|each| each.id == model.id)
        {
            choices.push(Choice::new(model.id.clone(), model.name.clone()));
        }
        let choose = cx.entity();
        let mut select = Select::new("ai-model-select", choices)
            .label(word(cx, "et.ai.models"))
            .placeholder(word(cx, "et.ai.models"))
            .size(ControlSize::Sm)
            .disabled(pinned.is_some() || selected.models.is_empty())
            .on_change(move |id, _, cx| {
                if let Err(error) = SettingsStore::update(cx, |settings| {
                    settings.ai.selected = Some(id.to_string())
                }) {
                    log::error!("ai: model selection did not save: {error:#}");
                    choose.update(cx, |assistant, cx| {
                        assistant.note(format!("ai: model selection did not save: {error:#}"), cx)
                    });
                }
            });
        if let Some(current) = current {
            select = select.selected(current);
        }
        let can_execute = self.target.as_ref().is_some_and(|target| target.executable)
            && pinned
                .as_ref()
                .or_else(|| selected.selected())
                .is_some_and(|model| model.tools);
        let label = (!can_execute).then(|| {
            word(
                cx,
                if pinned
                    .as_ref()
                    .or_else(|| selected.selected())
                    .is_some_and(|model| !model.tools)
                {
                    "et.ai.chat_unverified"
                } else if self
                    .target
                    .as_ref()
                    .is_some_and(|target| !target.executable)
                {
                    "et.ai.chat_terminal"
                } else {
                    "et.ai.chat_mode"
                },
            )
        });
        let close = cx.entity();
        let automatic = close.clone();
        let auto_approve = div()
            .id("ai-auto-approve-help")
            .tooltip(Tooltip::text(word(cx, "et.ai.auto_approve_hint")))
            .child(
                Switch::new("ai-auto-approve", selected.auto_approve)
                    .label(word(cx, "et.ai.auto_approve"))
                    .on_change(move |on, _, cx| {
                        if let Err(error) = SettingsStore::update(cx, |settings| {
                            settings.ai.auto_approve = on;
                        }) {
                            log::error!("ai: automatic approval did not save: {error:#}");
                            automatic.update(cx, |assistant, cx| {
                                assistant.note(
                                    format!("ai: automatic approval did not save: {error:#}"),
                                    cx,
                                );
                            });
                        } else {
                            log::info!("ai: automatic approval is {on}");
                        }
                    }),
            );
        let reset = close.clone();
        let preview = close.clone();
        let send = close.clone();
        let mut composer = PromptInput::new("ai-prompt", &self.input, move |_, cx| {
            send.update(cx, |assistant, cx| assistant.start(false, cx))
        })
        .tool(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .min_w_0()
                .gap_1()
                .child(select)
                .child(auto_approve)
                .child(self.history_button(true, cx))
                .child(self.history_button(false, cx))
                .children(label.map(|label| {
                    div()
                        .text_size(cx.theme().text_size(TextSize::Xs))
                        .text_color(colors.fg_muted)
                        .child(label)
                })),
        );
        if self.running.is_some() {
            let stop = cx.entity();
            composer =
                composer.busy(move |_, cx| stop.update(cx, |assistant, cx| assistant.stop(cx)));
        }
        let title = self.target.as_ref().map_or(String::new(), |target| {
            format!("{} · {}", target.name, target.shell)
        });
        let rows = self.conversation().map_or_else(Vec::new, |conversation| {
            super::text::rows(&conversation.lines)
        });
        let count = rows.len();
        let owner = cx.entity();
        let messages = MessageList::new(
            (
                "ai-messages",
                self.target
                    .as_ref()
                    .map_or(0, |target| target.id.as_u64() as usize),
            ),
            count,
            move |index, _, cx| {
                let (line, range) = rows[index].clone();
                owner.update(cx, |assistant, cx| assistant.message(line, range, cx))
            },
        );
        let phase = self.running.as_ref().map(|running| {
            format!(
                "{} · {}/{}",
                running.phase,
                running.count,
                crate::ai::CALL_LIMIT
            )
        });
        let continued = self.running.is_none()
            && self
                .conversation()
                .is_some_and(|conversation| conversation.resume.is_some());
        let resume = cx.entity();
        div()
            .id("ai-panel")
            .occlude()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .bg(colors.surface)
            .text_color(colors.fg)
            .border_1()
            .border_color(colors.border)
            .rounded(cx.theme().radius(Radius::Lg))
            .capture_key_down(
                cx.listener(|assistant, event: &gpui::KeyDownEvent, window, cx| {
                    let key = &event.keystroke;
                    if assistant.input.focus_handle(cx).is_focused(window)
                        && key.modifiers.alt
                        && !key.modifiers.control
                        && !key.modifiers.platform
                        && !key.modifiers.shift
                        && matches!(key.key.as_str(), "up" | "down")
                    {
                        assistant.recall(key.key == "up", window, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .on_key_down(cx.listener(|assistant, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    assistant.hide(cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(word(cx, "et.ai.title")),
                    )
                    .child(
                        IconButton::new("ai-settings", IconName::Settings)
                            .tooltip(word(cx, "et.ai.models"))
                            .on_click(|_, _, cx| crate::settings::window::open_ai(cx)),
                    )
                    .child(
                        IconButton::new("ai-new-chat", IconName::Plus)
                            .tooltip(word(cx, "et.ai.new_chat"))
                            .disabled(self.running.is_some())
                            .on_click(move |_, _, cx| {
                                reset.update(cx, |assistant, cx| assistant.reset(cx))
                            }),
                    )
                    .child(
                        IconButton::new("ai-close", IconName::X)
                            .tooltip(word(cx, "et.ai.close"))
                            .on_click(move |_, _, cx| {
                                close.update(cx, |assistant, cx| assistant.hide(cx))
                            }),
                    ),
            )
            .child(
                div()
                    .text_size(cx.theme().text_size(TextSize::Xs))
                    .text_color(colors.fg_muted)
                    .truncate()
                    .child(title),
            )
            .children(phase.map(|phase| {
                div()
                    .text_size(cx.theme().text_size(TextSize::Sm))
                    .text_color(colors.info)
                    .child(phase)
            }))
            .child(div().flex_1().min_h_0().child(messages))
            .children((count == 0).then(|| {
                div()
                    .px_2()
                    .text_size(cx.theme().text_size(TextSize::Sm))
                    .text_color(colors.fg_muted)
                    .child(word(cx, "et.ai.empty"))
            }))
            .children(self.approval(cx))
            .children(continued.then(|| {
                Button::new("ai-continue", word(cx, "et.ai.continue")).on_click(move |_, _, cx| {
                    resume.update(cx, |assistant, cx| assistant.start(true, cx))
                })
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("ai-context-preview", word(cx, "et.ai.preview"))
                            .variant(ButtonVariant::Ghost)
                            .size(ControlSize::Sm)
                            .on_click(move |_, _, cx| {
                                preview.update(cx, |assistant, cx| {
                                    if let Some(target) = &assistant.target {
                                        assistant.preview = target
                                            .terminal
                                            .read_with(cx, |terminal, _| terminal.ai_context())
                                            .unwrap_or_else(|error| {
                                                format!("ai: context could not read: {error}")
                                            });
                                    }
                                    assistant.preview_open = !assistant.preview_open;
                                    cx.notify();
                                })
                            }),
                    )
                    .children(self.preview_open.then(|| {
                        let this = cx.entity();
                        IconButton::new("ai-copy-context", IconName::Copy)
                            .size(ControlSize::Sm)
                            .tooltip(word(cx, "et.ai.copy"))
                            .on_click(move |_, _, cx| {
                                this.update(cx, |assistant, cx| {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        assistant.preview.clone(),
                                    ));
                                });
                            })
                    })),
            )
            .children(self.preview_open.then(|| {
                div()
                    .id("ai-preview")
                    .max_h_40()
                    .overflow_y_scroll()
                    .text_size(cx.theme().text_size(TextSize::Xs))
                    .text_color(colors.fg_muted)
                    .child(SelectableText::new("ai-context-text", self.preview.clone()))
            }))
            .child(composer)
            .child(
                div()
                    .text_size(cx.theme().text_size(TextSize::Xs))
                    .text_color(colors.fg_muted)
                    .child(word(cx, "et.ai.input_hint")),
            )
    }
}
