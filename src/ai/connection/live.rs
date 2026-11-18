use std::sync::{Arc, Mutex};

use super::*;

#[test]
#[ignore = "contacts the user's configured model service"]
fn live_configured_model_tool_probe() {
    let path = dirs::config_dir()
        .unwrap()
        .join("ELYTerminal/settings.json");
    let settings: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let config: crate::ai::config::Settings =
        serde_json::from_value(settings["ai"].clone()).unwrap();
    let model = config.selected().unwrap().clone();
    model.validate().unwrap();
    let outcome = Arc::new(Mutex::new(None));
    let done = outcome.clone();
    gpui_platform::application().run(move |cx| {
        cx.spawn(async move |cx| {
            let checked: Result<()> = async {
                let key = credentials::read(&model, cx).await?;
                let redaction = key.clone();
                let name = model.model.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move { crate::ai::settings::probe(&model, &key) })
                    .await;
                match result {
                    Ok(true) => eprintln!(
                        "Configured model {name}: streamed chat and complete tool cycle passed"
                    ),
                    Ok(false) => anyhow::bail!("configured model did not complete a tool cycle"),
                    Err(error) => {
                        let message = format!("{error:#}");
                        anyhow::bail!(
                            "{}",
                            if redaction.is_empty() {
                                message
                            } else {
                                message.replace(&redaction, "<REDACTED>")
                            }
                        );
                    }
                }
                Ok(())
            }
            .await;
            *done.lock().unwrap() = Some(checked);
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
    outcome
        .lock()
        .unwrap()
        .take()
        .expect("model probe finished")
        .unwrap();
}
