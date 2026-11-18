use std::time::{Duration, Instant};

use anyhow::{Result, ensure};
use gpui::{AppContext as _, AsyncApp};

use super::{super::Line, Harness};

pub(super) async fn measure(window: gpui::WindowHandle<Harness>, cx: &mut AsyncApp) -> Result<()> {
    let mut polls = Vec::new();
    for _ in 0..40 {
        polls.push(window.read_with(cx, |harness, cx| {
            let start = Instant::now();
            std::hint::black_box(harness.terminal.read(cx).ai_ready());
            start.elapsed()
        })?);
        cx.background_executor()
            .timer(Duration::from_millis(1))
            .await;
    }
    polls.sort();
    eprintln!(
        "AI readiness UI time: p50={:?}, p95={:?}, max={:?}",
        polls[20], polls[38], polls[39]
    );
    let text = "### Diagnostic output\n\nA measured streaming reply with `code`, plain text and repeated terminal observations.\n\n".repeat(600);
    window.update(cx, |harness, _, cx| {
        harness.assistant.update(cx, |assistant, cx| {
            let conversation = assistant
                .conversations
                .get_mut(&assistant.target.as_ref().unwrap().id)
                .unwrap();
            conversation.lines.push(Line::Message {
                user: false,
                text,
                live: true,
            });
            cx.notify();
        });
    })?;
    let mut frames = Vec::new();
    for _ in 0..20 {
        frames.push(cx.update_window(window.into(), |_, window, cx| {
            let start = Instant::now();
            window.draw(cx).clear(cx);
            start.elapsed()
        })?);
        cx.background_executor()
            .timer(Duration::from_millis(16))
            .await;
    }
    frames.sort();
    eprintln!(
        "AI reply frame UI time: p50={:?}, p95={:?}, max={:?}",
        frames[10], frames[18], frames[19]
    );
    window.update(cx, |harness, _, cx| {
        harness.assistant.update(cx, |assistant, cx| {
            assistant
                .conversations
                .get_mut(&assistant.target.as_ref().unwrap().id)
                .unwrap()
                .lines
                .pop();
            cx.notify();
        });
    })?;
    ensure!(
        polls[38] < Duration::from_millis(16),
        "AI readiness blocks a 60 Hz frame"
    );
    ensure!(
        frames[18] < Duration::from_millis(32),
        "AI streaming stalls terminal frames"
    );
    Ok(())
}
