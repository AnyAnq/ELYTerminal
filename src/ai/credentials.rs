use anyhow::{Context as _, Result, ensure};
use gpui::App;

use super::config::Model;

pub async fn read(model: &Model, cx: &mut gpui::AsyncApp) -> Result<String> {
    let address = model.credential();
    let read = cx.update(|cx| cx.read_credentials(&address));
    match read
        .await
        .context("ai: cannot read the system credential store")?
    {
        Some((_, bytes)) => String::from_utf8(bytes).context("ai: model key is not UTF-8"),
        None => Ok(String::new()),
    }
}

pub fn check(key: &str) -> Result<()> {
    ensure!(
        key.len() <= 2000 && !key.chars().any(char::is_control),
        "ai: API key is too long or contains control characters"
    );
    Ok(())
}

pub async fn erase(model: &Model, cx: &mut gpui::AsyncApp) -> Result<()> {
    if cx
        .update(|cx| cx.read_credentials(&model.credential()))
        .await
        .context("ai: cannot read the system credential store")?
        .is_some()
    {
        cx.update(|cx| cx.delete_credentials(&model.credential()))
            .await
            .context("ai: cannot clear the saved key")?;
    }
    Ok(())
}

pub fn write(model: &Model, key: &str, cx: &App) -> gpui::Task<Result<()>> {
    if key.is_empty() {
        cx.delete_credentials(&model.credential())
    } else {
        cx.write_credentials(&model.credential(), "api-key", key.as_bytes())
    }
}
