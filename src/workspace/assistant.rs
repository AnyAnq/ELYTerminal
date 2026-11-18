use ely_gpui_component::theme::{ActiveTheme, ContainerSize};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, rems};

use super::Workspace;
use crate::ai::Target;

impl Workspace {
    pub fn toggle_ai(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.assistant.read(cx).open {
            self.assistant
                .update(cx, |assistant, cx| assistant.hide(cx));
            return;
        }
        let Some(session) = self.sessions.get(self.active) else {
            return;
        };
        let pane = session.pane();
        let view = pane.terminal.read(cx);
        let target = Target {
            terminal: pane.terminal.downgrade(),
            id: pane.id(),
            name: session.named().to_string(),
            folder: pane.folder.clone(),
            shell: if pane.program.shell.is_empty() {
                if view.ai_supported() {
                    "PowerShell / cmd".into()
                } else {
                    "Shell".into()
                }
            } else {
                pane.program.shell.clone()
            },
            executable: view.ai_supported()
                && pane.reports
                && !pane.program.written
                && pane.program.args.is_empty(),
        };
        window.activate_window();
        self.assistant
            .update(cx, |assistant, cx| assistant.show(target, window, cx));
        cx.notify();
    }

    pub(super) fn ai_overlay(&self, window: &Window, cx: &Context<Self>) -> Option<AnyElement> {
        if !self.assistant.read(cx).open {
            return None;
        }
        let room = (window.viewport_size().width - rems(2.).to_pixels(window.rem_size()))
            .max(gpui::px(0.));
        let width = cx
            .theme()
            .container_width(ContainerSize::Sm)
            .to_pixels(window.rem_size())
            .min(room);
        Some(
            div()
                .absolute()
                .top_12()
                .bottom_4()
                .right_4()
                .w(width)
                .child(self.assistant.clone())
                .into_any_element(),
        )
    }

    pub fn ai_look(&self, cx: &gpui::App) -> Option<crate::sessions::orb::Look> {
        self.assistant.read(cx).look()
    }
}
