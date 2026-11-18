use anyhow::{Result, ensure};
use gpui::{
    AppContext as _, AsyncApp, Focusable, MouseButton, MouseDownEvent, MouseUpEvent, PlatformInput,
    point, px,
};

use super::Harness;

pub(super) fn verify(window: gpui::WindowHandle<Harness>, cx: &mut AsyncApp) -> Result<()> {
    let (input, terminal) = window.read_with(cx, |harness, cx| {
        (
            harness.assistant.read(cx).input.clone(),
            harness.terminal.read(cx).focus_handle(cx),
        )
    })?;
    cx.update_window(window.into(), |_, window, cx| -> Result<()> {
        window.draw(cx).clear(cx);
        window.focus(&terminal, cx);
        let position = point(px(600.), px(635.));
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
        ensure!(
            input.read(cx).focus_handle(cx).is_focused(window),
            "clicking the AI composer left keyboard focus on the terminal"
        );
        window.draw(cx).clear(cx);
        window.dispatch_keystroke(gpui::Keystroke::parse("a").unwrap(), cx);
        ensure!(
            input.read(cx).text() == "a",
            "typing after an AI composer click did not reach the input"
        );
        input.update(cx, |input, cx| input.set_text("", cx));
        Ok(())
    })??;
    Ok(())
}
