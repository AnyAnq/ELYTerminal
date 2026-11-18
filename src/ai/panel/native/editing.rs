use std::{path::Path, time::Duration};

use anyhow::{Result, ensure};
use gpui::{
    AppContext as _, AsyncApp, ClipboardItem, Focusable, Keystroke, MouseButton, MouseDownEvent,
    MouseUpEvent, PlatformInput, Window, point, px,
};

use super::{super::Line, Harness};

fn key(window: &mut Window, stroke: &str, cx: &mut gpui::App) {
    window.draw(cx).clear(cx);
    window.dispatch_keystroke(Keystroke::parse(stroke).unwrap(), cx);
}

pub(super) fn click(window: &mut Window, x: f32, y: f32, cx: &mut gpui::App) {
    window.draw(cx).clear(cx);
    let position = point(px(x), px(y));
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position,
            click_count: 1,
            ..Default::default()
        }),
        cx,
    );
    window.dispatch_event(
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position,
            click_count: 1,
            ..Default::default()
        }),
        cx,
    );
}

pub(super) async fn verify(
    window: gpui::WindowHandle<Harness>,
    project: &Path,
    cx: &mut AsyncApp,
) -> Result<()> {
    let (assistant, terminal) = window.read_with(cx, |harness, cx| {
        (
            harness.assistant.clone(),
            harness.terminal.read(cx).focus_handle(cx),
        )
    })?;
    let (input, pane, prompts, count) = assistant.read_with(cx, |assistant, _| {
        let conversation = assistant.conversation().unwrap();
        let prompts: Vec<_> = conversation
            .lines
            .iter()
            .filter_map(|line| match line {
                Line::Message {
                    user: true, text, ..
                } => Some(text.clone()),
                _ => None,
            })
            .collect();
        (
            assistant.input.clone(),
            assistant.target.as_ref().unwrap().id,
            prompts,
            conversation.messages.len(),
        )
    });
    ensure!(
        prompts.len() >= 2,
        "prompt history fixture has too few requests"
    );
    cx.update_window(window.into(), |_, window, cx| -> Result<()> {
        let draft = "  未发送草稿\nnext 🐾  ";
        input.update(cx, |input, cx| input.set_text(draft, cx));
        window.focus(&input.focus_handle(cx), cx);
        key(window, "alt-up", cx);
        ensure!(
            input.read(cx).text() == prompts[prompts.len() - 1],
            "Alt+Up did not recall the last input"
        );
        key(window, "x", cx);
        ensure!(
            input.read(cx).text() == format!("{}x", prompts.last().unwrap()),
            "recalled prompt could not be edited"
        );
        key(window, "alt-up", cx);
        ensure!(
            input.read(cx).text() == prompts[prompts.len() - 2],
            "older input was not recalled"
        );
        key(window, "alt-down", cx);
        ensure!(
            input.read(cx).text() == prompts[prompts.len() - 1],
            "newer input was not recalled"
        );
        key(window, "alt-down", cx);
        ensure!(
            input.read(cx).text() == draft,
            "history navigation lost the unsent draft"
        );
        let cursor = input.read(cx).cursor();
        key(window, "up", cx);
        ensure!(
            input.read(cx).text() == draft && input.read(cx).cursor() < cursor,
            "plain Up no longer navigates multiline input"
        );
        window.focus(&terminal, cx);
        key(window, "alt-up", cx);
        ensure!(
            input.read(cx).text() == draft,
            "history shortcut ran outside the AI input"
        );
        click(window, 610., 680., cx);
        ensure!(
            input.read(cx).text() == prompts[prompts.len() - 1],
            "previous-input button did not recall a prompt"
        );
        ensure!(
            input.focus_handle(cx).is_focused(window),
            "history button left focus outside the AI input"
        );
        assistant.update(cx, |assistant, cx| {
            let target = assistant.target.clone().unwrap();
            assistant.hide(cx);
            assistant.show(target, window, cx);
        });
        key(window, "alt-down", cx);
        ensure!(
            input.read(cx).text() == draft,
            "hiding the panel lost the history draft"
        );
        ensure!(
            assistant.read(cx).running.is_none()
                && assistant.read(cx).conversation().unwrap().messages.len() == count,
            "recalling a prompt submitted an AI request"
        );
        window.draw(cx).clear(cx);
        window
            .render_to_image()?
            .save(project.join("editing.png"))?;
        input.update(cx, |input, cx| input.set_text("", cx));
        Ok(())
    })??;

    let clipboard = cx.update(|cx| cx.read_from_clipboard());
    let result = async {
        let tests: Vec<_> = assistant.read_with(cx, |assistant, _| {
            assistant
                .conversation()
                .unwrap()
                .lines
                .iter()
                .enumerate()
                .map(|(index, line)| {
                    (
                        index,
                        match line {
                            Line::Message { text, .. } | Line::Note(text) => text.clone(),
                            Line::Tool {
                                name, args, output, ..
                            } => format!("{name}\n{args}\n{output}"),
                        },
                    )
                })
                .filter(|(_, text)| !text.is_empty())
                .collect::<Vec<_>>()
        });
        let expected = tests.last().unwrap().1.clone();
        cx.update_window(window.into(), |_, window, cx| {
            click(window, 946., 545., cx);
        })?;
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
        ensure!(
            cx.update(|cx| cx.read_from_clipboard())
                .and_then(|item| item.text())
                .as_deref()
                == Some(&expected),
            "message copy button did not copy the reply"
        );
        for (index, expected) in tests {
            cx.background_executor()
                .timer(Duration::from_millis(50))
                .await;
            assistant.update(cx, |assistant, cx| assistant.copy_line(pane, index, cx));
            cx.background_executor()
                .timer(Duration::from_millis(50))
                .await;
            let copied = cx
                .update(|cx| cx.read_from_clipboard())
                .and_then(|item| item.text());
            ensure!(
                copied.as_deref() == Some(&expected),
                "copy line {index}: expected {} bytes, read {:?} bytes",
                expected.len(),
                copied.as_ref().map(String::len)
            );
        }
        let command = assistant.read_with(cx, |assistant, _| {
            assistant
                .conversation()
                .unwrap()
                .lines
                .iter()
                .rev()
                .find_map(|line| {
                    if let Line::Tool { args, .. } = line {
                        let args: serde_json::Value = serde_json::from_str(args).unwrap();
                        args["command"].as_str().map(str::to_string)
                    } else {
                        None
                    }
                })
                .unwrap()
        });
        cx.update_window(window.into(), |_, window, cx| {
            click(window, 430., 490., cx);
            key(window, "ctrl-a", cx);
            key(window, "ctrl-c", cx);
        })?;
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
        ensure!(
            cx.update(|cx| cx.read_from_clipboard())
                .and_then(|item| item.text())
                .as_deref()
                == Some(&command),
            "selected tool text could not be copied with Ctrl+C"
        );
        let long = "中文 🐾\nall bytes must be copied\n".repeat(3000);
        let index = assistant.update(cx, |assistant, cx| {
            let lines = &mut assistant.conversations.get_mut(&pane).unwrap().lines;
            let index = lines.len();
            lines.push(Line::Message {
                user: false,
                text: long.clone(),
                live: false,
            });
            cx.notify();
            index
        });
        assistant.update(cx, |assistant, cx| assistant.copy_line(pane, index, cx));
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        let copied = loop {
            cx.background_executor()
                .timer(Duration::from_millis(50))
                .await;
            let copied = cx
                .update(|cx| cx.read_from_clipboard())
                .and_then(|item| item.text());
            if copied.is_some() || std::time::Instant::now() >= deadline {
                break copied;
            }
        };
        assistant.update(cx, |assistant, cx| {
            assistant.conversations.get_mut(&pane).unwrap().lines.pop();
            cx.notify();
        });
        ensure!(
            copied.as_deref() == Some(&long),
            "long reply copy: expected {} bytes, read {:?} bytes",
            long.len(),
            copied.as_ref().map(String::len)
        );
        cx.update_window(window.into(), |_, window, cx| -> Result<()> {
            window.draw(cx).clear(cx);
            window.render_to_image()?.save(project.join("copy.png"))?;
            Ok(())
        })??;
        Ok::<_, anyhow::Error>(())
    }
    .await;
    cx.update(|cx| cx.write_to_clipboard(clipboard.unwrap_or(ClipboardItem { entries: vec![] })));
    result?;
    Ok(())
}
