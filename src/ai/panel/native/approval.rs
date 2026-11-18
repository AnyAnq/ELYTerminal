use std::path::Path;

use anyhow::{Result, ensure};
use gpui::{AppContext as _, AsyncApp};

use super::{Harness, editing::click, until};
use crate::settings::SettingsStore;

pub(super) async fn verify(
    window: gpui::WindowHandle<Harness>,
    project: &Path,
    cx: &mut AsyncApp,
) -> Result<()> {
    ensure!(
        !cx.update(|cx| SettingsStore::get(cx).ai.auto_approve),
        "automatic approval must default off"
    );
    window.update(cx, |harness, _, cx| {
        harness.assistant.update(cx, |assistant, cx| {
            assistant.input.update(cx, |input, cx| {
                input.set_text("Exercise automatic approval.", cx)
            });
            assistant.start(false, cx);
        });
    })?;
    until(window, cx, |assistant| {
        assistant
            .running
            .as_ref()
            .is_some_and(|run| run.approval.is_some())
    })
    .await?;
    cx.update_window(window.into(), |_, window, cx| -> Result<()> {
        click(window, 500., 680., cx);
        ensure!(
            SettingsStore::get(cx).ai.auto_approve,
            "composer auto-allow switch did not turn on"
        );
        Ok(())
    })??;
    until(window, cx, |assistant| assistant.running.is_none()).await?;
    window.read_with(cx, |harness, cx| -> Result<()> {
        ensure!(
            !harness.assistant.read(cx).failed,
            "automatically approved task failed"
        );
        ensure!(
            project.join("auto-first.txt").is_file(),
            "pending approval did not resume"
        );
        ensure!(
            project.join("auto-second.txt").is_file(),
            "later action asked for approval"
        );
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(project.join("settings.json"))?)?;
        ensure!(
            saved["ai"]["auto_approve"] == true,
            "automatic approval was not saved"
        );
        Ok(())
    })??;
    cx.update_window(window.into(), |_, window, cx| -> Result<()> {
        window.draw(cx).clear(cx);
        window
            .render_to_image()?
            .save(project.join("auto-allow-on.png"))?;
        click(window, 500., 680., cx);
        ensure!(
            !SettingsStore::get(cx).ai.auto_approve,
            "composer auto-allow switch did not turn off"
        );
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(project.join("settings.json"))?)?;
        ensure!(
            saved["ai"]["auto_approve"] == false,
            "disabled auto-allow switch was not saved"
        );
        Ok(())
    })??;
    cx.background_executor()
        .timer(std::time::Duration::from_millis(250))
        .await;
    cx.update_window(window.into(), |_, window, cx| -> Result<()> {
        window.draw(cx).clear(cx);
        window
            .render_to_image()?
            .save(project.join("auto-allow-off.png"))?;
        Ok(())
    })??;
    Ok(())
}
