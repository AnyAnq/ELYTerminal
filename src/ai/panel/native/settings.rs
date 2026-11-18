use std::path::Path;

use anyhow::{Result, ensure};
use gpui::{AppContext as _, AsyncApp};

use crate::{
    ai::credentials,
    settings::{Language, SettingsStore, window::SettingsWindow},
};

pub(super) async fn verify(root: &Path, cx: &mut AsyncApp) -> Result<()> {
    let mut model = cx.update(|cx| SettingsStore::get(cx).ai.models[0].clone());
    model.id = format!("credential-check-{:x}", getrandom::u64()?);
    ensure!(
        credentials::read(&model, cx).await?.is_empty(),
        "fixture credential already exists"
    );
    let checked: Result<()> = async {
        cx.update(|cx| credentials::write(&model, "ELYTerminal-test-fixture", cx))
            .await?;
        ensure!(
            credentials::read(&model, cx).await? == "ELYTerminal-test-fixture",
            "system credential did not round-trip"
        );
        Ok(())
    }
    .await;
    let cleanup = credentials::erase(&model, cx).await;
    checked?;
    cleanup?;
    ensure!(
        credentials::read(&model, cx).await?.is_empty(),
        "fixture credential was not removed"
    );
    let opened = std::time::Instant::now();
    cx.update(crate::settings::window::open_ai);
    if std::env::var_os("ELY_AI_BENCH").is_some() {
        eprintln!("AI settings open UI time: {:?}", opened.elapsed());
        ensure!(
            opened.elapsed() < std::time::Duration::from_millis(350),
            "opening AI settings blocks the terminal"
        );
    }
    let settings = cx
        .update(|cx| {
            cx.windows()
                .into_iter()
                .find_map(|handle| handle.downcast::<SettingsWindow>())
        })
        .ok_or_else(|| anyhow::anyhow!("AI settings window did not open"))?;
    if std::env::var_os("ELY_AI_BENCH").is_some() {
        super::settings_performance::measure(settings, cx).await?;
    }
    cx.update_window(settings.into(), |_, window, cx| -> Result<()> {
        window.draw(cx).clear(cx);
        window.render_to_image()?.save(root.join("settings.png"))?;
        Ok(())
    })??;
    cx.update(|cx| {
        SettingsStore::update(cx, |settings| {
            settings.language = Language::SimplifiedChinese
        })
    })?;
    cx.background_executor()
        .timer(std::time::Duration::from_millis(50))
        .await;
    cx.update_window(settings.into(), |_, window, cx| -> Result<()> {
        window.draw(cx).clear(cx);
        window
            .render_to_image()?
            .save(root.join("settings-zh.png"))?;
        window.remove_window();
        Ok(())
    })??;
    Ok(())
}
