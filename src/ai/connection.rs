use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context as _, Result, ensure};
use serde_json::{Value, json};

use super::{
    config::Model,
    credentials,
    protocol::{Answer, Decoder},
};

#[derive(Clone, Default)]
pub struct Cancel(Arc<Inner>);

#[derive(Default)]
struct Inner {
    stopped: AtomicBool,
    child: Mutex<Option<Child>>,
}

impl Cancel {
    pub fn stop(&self) {
        self.0.stopped.store(true, Ordering::SeqCst);
        if let Some(child) = self.0.child.lock().expect("ai process lock").as_mut()
            && let Err(error) = child.kill()
        {
            log::debug!("ai: cancelling model connection: {error}");
        }
    }

    pub fn stopped(&self) -> bool {
        self.0.stopped.load(Ordering::SeqCst)
    }

    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn check(&self) -> Result<()> {
        ensure!(!self.stopped(), "ai: task cancelled");
        Ok(())
    }
}

fn quoted(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\r', "\\r")
            .replace('\n', "\\n")
            .replace('\t', "\\t")
    )
}

fn config(model: &Model, key: &str, request: &Value) -> Result<String> {
    model.validate()?;
    credentials::check(key)?;
    let mut config = format!(
        "url = {}\nheader = \"Content-Type: application/json\"\ndata-binary = {}\n",
        quoted(&model.endpoint()),
        quoted(&serde_json::to_string(request)?)
    );
    if !key.is_empty() {
        config.push_str(&format!(
            "header = {}\n",
            quoted(&format!("Authorization: Bearer {key}"))
        ));
    }
    Ok(config)
}

pub fn request(model: &Model, messages: &[Value], tools: &[Value]) -> Value {
    let mut request = json!({"model":model.model, "messages":messages, "stream":true});
    if !tools.is_empty() {
        request["tools"] = json!(tools);
        request["tool_choice"] = json!("auto");
    }
    request
}

pub fn complete(
    model: &Model,
    key: &str,
    request: &Value,
    cancel: &Cancel,
    mut emit: impl FnMut(&str),
) -> Result<Answer> {
    cancel.check()?;
    let config = config(model, key, request)?;
    let mut command = Command::new("curl");
    command
        .args([
            "--disable",
            "--config",
            "-",
            "--silent",
            "--show-error",
            "--no-buffer",
            "--fail-with-body",
            "--proto",
            "=http,https",
            "--connect-timeout",
            "10",
            "--max-time",
            "180",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().context("ai: could not start curl")?;
    let mut stdin = child
        .stdin
        .take()
        .context("ai: model request has no input pipe")?;
    let mut stdout = child
        .stdout
        .take()
        .context("ai: model request has no output pipe")?;
    {
        let mut process = cancel.0.child.lock().expect("ai process lock");
        if cancel.stopped() {
            child.kill().ok();
            child.wait().ok();
            anyhow::bail!("ai: task cancelled");
        }
        *process = Some(child);
    }
    let streamed = (|| {
        stdin
            .write_all(config.as_bytes())
            .context("ai: could not write model request")?;
        drop(stdin);
        let mut decoder = Decoder::default();
        let mut buffer = [0; 8192];
        loop {
            cancel.check()?;
            let count = stdout
                .read(&mut buffer)
                .context("ai: model stream could not read")?;
            if count == 0 {
                break;
            }
            decoder.feed(&buffer[..count], &mut emit)?;
        }
        decoder.finish()
    })();
    if streamed.is_err() {
        cancel
            .0
            .child
            .lock()
            .expect("ai process lock")
            .as_mut()
            .map(|child| child.kill());
    }
    let child = cancel
        .0
        .child
        .lock()
        .expect("ai process lock")
        .take()
        .context("ai: model process disappeared")?;
    let output = child
        .wait_with_output()
        .context("ai: model connection did not close")?;
    cancel.check()?;
    if !output.status.success() {
        let failed = format!(
            "ai: model request failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return match streamed {
            Err(error) => Err(error.context(failed)),
            Ok(_) => Err(anyhow::anyhow!(failed)),
        };
    }
    streamed
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn secrets_and_prompts_are_escaped_inside_the_stdin_config() {
        let model = Model {
            id: "test".into(),
            name: "Test".into(),
            url: "http://localhost/v1".into(),
            model: "model".into(),
            tools: false,
        };
        let config = config(
            &model,
            "key\"\\value",
            &json!({"messages":[{"content":"hello\n\"world\""}]}),
        )
        .unwrap();
        assert_eq!(config.lines().count(), 4);
        assert!(config.contains("key\\\"\\\\value"));
        assert!(config.contains("hello\\\\n"));
        assert!(super::config(&model, "key\nheader", &json!({})).is_err());
    }
}

#[cfg(all(test, windows))]
mod live;
#[cfg(test)]
mod tests;
