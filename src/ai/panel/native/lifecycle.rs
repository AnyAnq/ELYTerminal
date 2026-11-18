use std::{
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{Result, ensure};
use gpui::{AppContext as _, AsyncApp, EntityInputHandler, Focusable, Keystroke, WindowHandle};

use super::{Harness, until};

pub(super) async fn prepare_draft(
    window: WindowHandle<Harness>,
    cmd: bool,
    cx: &mut AsyncApp,
) -> Result<()> {
    let terminal = window.read_with(cx, |harness, _| harness.terminal.clone())?;
    cx.update_window(window.into(), |_, window, cx| {
        terminal.update(cx, |terminal, cx| {
            terminal.replace_text_in_range(
                None,
                if cmd {
                    "echo executed>draft-was-executed & "
                } else {
                    "Set-Content draft-was-executed yes; ("
                },
                window,
                cx,
            );
        });
        if !cmd {
            window.focus(&terminal.focus_handle(cx), cx);
            window.draw(cx).clear(cx);
            window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx);
        }
    })?;
    if !cmd {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let continued = terminal.read_with(cx, |terminal, _| {
                terminal
                    .tail(Some(3))
                    .lines()
                    .any(|line| line.starts_with(">>"))
            });
            if continued {
                break;
            }
            ensure!(
                Instant::now() < deadline,
                "PowerShell did not show the continuation prompt"
            );
            cx.background_executor()
                .timer(Duration::from_millis(25))
                .await;
        }
        ensure!(
            terminal.read_with(cx, |terminal, _| terminal.ai_ready()),
            "PowerShell's continuation prompt was marked busy"
        );
    }
    Ok(())
}

pub(super) async fn verify_close(
    window: WindowHandle<Harness>,
    project: &Path,
    cx: &mut AsyncApp,
) -> Result<()> {
    let expected = project.join("nested").canonicalize()?;
    window.update(cx, |harness, _, cx| {
        harness.assistant.update(cx, |assistant, cx| -> Result<()> {
            let target = assistant.target.as_ref().unwrap();
            ensure!(
                target.folder.canonicalize()? == expected,
                "assistant did not keep the new prompt's directory"
            );
            assistant
                .input
                .update(cx, |input, cx| input.set_text("Exercise pane close.", cx));
            assistant.start(false, cx);
            Ok(())
        })
    })??;
    until(window, cx, |assistant| {
        assistant
            .running
            .as_ref()
            .is_some_and(|run| run.approval.is_some())
    })
    .await?;
    window.update(cx, |harness, _, cx| {
        let pane = harness.terminal.entity_id();
        let background = cx.new(|_| ());
        harness.assistant.update(cx, |assistant, cx| -> Result<()> {
            let cancel = assistant.running.as_ref().unwrap().cancel.clone();
            assistant
                .conversations
                .entry(background.entity_id())
                .or_default()
                .draft = "Draft in another pane".into();
            assistant.pane_closed(background.entity_id(), cx);
            ensure!(
                !cancel.stopped()
                    && assistant.conversations.contains_key(&pane)
                    && !assistant
                        .conversations
                        .contains_key(&background.entity_id()),
                "closing a background pane should discard only its conversation"
            );
            assistant.pane_closed(pane, cx);
            ensure!(
                cancel.stopped() && assistant.running.is_none(),
                "pane closure did not cancel the task"
            );
            ensure!(
                assistant.target.is_none()
                    && !assistant.open
                    && assistant.conversations.is_empty()
                    && assistant.input.read(cx).text().is_empty()
                    && assistant.preview.is_empty(),
                "pane closure retained conversation data"
            );
            Ok(())
        })
    })??;
    Ok(())
}
