use std::time::{Duration, Instant};

use anyhow::{Result, ensure};
use gpui::{
    AppContext as _, AsyncApp, MouseButton, MouseDownEvent, MouseUpEvent, PlatformInput, point, px,
};

use crate::settings::{SettingsStore, window::SettingsWindow};

pub(super) async fn measure(
    window: gpui::WindowHandle<SettingsWindow>,
    cx: &mut AsyncApp,
) -> Result<()> {
    let draw = cx.update_window(window.into(), |_, window, cx| {
        let start = Instant::now();
        window.draw(cx).clear(cx);
        start.elapsed()
    })?;
    let pick = cx.update_window(window.into(), |_, window, cx| {
        let start = Instant::now();
        let position = point(px(600.), px(145.));
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position,
                button: MouseButton::Left,
                click_count: 1,
                ..Default::default()
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                button: MouseButton::Left,
                click_count: 1,
                ..Default::default()
            }),
            cx,
        );
        window.draw(cx).clear(cx);
        window.dispatch_keystroke(gpui::Keystroke::parse("down").unwrap(), cx);
        window.draw(cx).clear(cx);
        window.dispatch_keystroke(gpui::Keystroke::parse("enter").unwrap(), cx);
        window.draw(cx).clear(cx);
        start.elapsed()
    })?;
    let mut saves = Vec::new();
    for index in 0..12 {
        let start = Instant::now();
        cx.update(|cx| {
            SettingsStore::update(cx, |settings| {
                settings.ai.selected = Some(settings.ai.models[index % 2].id.clone())
            })
        })?;
        saves.push(start.elapsed());
        cx.background_executor()
            .timer(Duration::from_millis(10))
            .await;
    }
    saves.sort();
    eprintln!(
        "AI settings initial draw={draw:?}; model dropdown and pick={pick:?}; selection/save p50={:?}, p95={:?}",
        saves[6], saves[11]
    );
    ensure!(
        draw < Duration::from_millis(100),
        "opening AI settings stalls rendering"
    );
    ensure!(
        pick < Duration::from_millis(150),
        "model dropdown stalls rendering"
    );
    ensure!(
        saves[11] < Duration::from_millis(50),
        "model selection blocks the terminal"
    );
    Ok(())
}
