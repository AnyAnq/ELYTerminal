use std::time::{Duration, Instant};

use anyhow::{Result, ensure};
use gpui::AsyncApp;
use serde_json::{Value, json};

use super::Target;
use crate::{
    ai::{Cancel, tools},
    term::TerminalView,
};

async fn ready(target: &Target, cx: &mut AsyncApp) -> Result<bool> {
    let state = target.terminal.read_with(cx, |terminal, _| {
        terminal
            .ai_ready()
            .then(|| (terminal.pid(), terminal.ai_prompt()))
    })?;
    let Some((pid, prompt)) = state else {
        return Ok(false);
    };
    let idle = cx
        .background_executor()
        .spawn(async move { TerminalView::ai_idle(pid) })
        .await?;
    if !idle {
        return Ok(false);
    }
    target.terminal.read_with(cx, |terminal, _| {
        terminal.ai_ready() && terminal.ai_prompt() == prompt
    })
}

pub(super) async fn execute(
    target: &Target,
    name: &str,
    args: &Value,
    cancel: &Cancel,
    cx: &mut AsyncApp,
) -> Result<Value> {
    cancel.check()?;
    match name {
        "read_terminal" => {
            let text = target.terminal.read_with(cx, |terminal, _| {
                terminal.tail(Some(args["lines"].as_u64().unwrap() as usize))
            })?;
            let (text, truncated) = tools::tail(&text, 48_000);
            Ok(json!({"text":text,"truncated":truncated}))
        }
        "run_command" => {
            ensure!(
                target.executable,
                "ai: autonomous execution is not supported for this terminal"
            );
            let start = Instant::now();
            loop {
                cancel.check()?;
                if ready(target, cx).await? {
                    break;
                }
                ensure!(
                    start.elapsed() < Duration::from_secs(120),
                    "ai: terminal is busy or requires manual input; take over before continuing"
                );
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
            }
            cancel.check()?;
            let prior = target.terminal.update(cx, |terminal, cx| {
                let prior = terminal.ai_prompt();
                terminal.ai_cancel_line(cx)?;
                Ok::<_, anyhow::Error>(prior)
            })??;
            let reset = Instant::now();
            loop {
                cancel.check()?;
                if target
                    .terminal
                    .read_with(cx, |terminal, _| terminal.ai_prompt() > prior)?
                    && ready(target, cx).await?
                {
                    break;
                }
                ensure!(
                    reset.elapsed() < Duration::from_secs(5),
                    "ai: the shell did not confirm cancellation of its input; command was not submitted"
                );
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
            }
            let id = target.terminal.update(cx, |terminal, cx| {
                terminal.ai_submit(args["command"].as_str().unwrap(), cx)
            })??;
            loop {
                cancel.check()?;
                let status = target
                    .terminal
                    .read_with(cx, |terminal, _| terminal.ai_status(id))??;
                if let Some(status) = status
                    && target.terminal.read_with(cx, |terminal, _| {
                        terminal.ai_prompt() > prior + 1 && terminal.ai_ready()
                    })?
                {
                    let text = target
                        .terminal
                        .read_with(cx, |terminal, _| terminal.tail(Some(1000)))?;
                    let (text, truncated) = tools::tail(&text, 48_000);
                    return Ok(
                        json!({"exit_code":status,"recent_terminal_output":text,"truncated":truncated}),
                    );
                }
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
            }
        }
        "read_file" | "edit_file" => {
            let (root, name, args, cancel) = (
                target.folder.clone(),
                name.to_string(),
                args.clone(),
                cancel.clone(),
            );
            cx.background_executor()
                .spawn(async move {
                    cancel.check()?;
                    tools::file(&root, &name, &args)
                })
                .await
        }
        _ => anyhow::bail!("ai: unknown tool"),
    }
}
