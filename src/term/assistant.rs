//! An assistant's lease and command receipts on one terminal.

use std::time::Instant;

use anyhow::{Context as _, Result, ensure};
use base64::Engine;
use gpui::{Context, Keystroke};

use super::{TerminalEvent, TerminalView, click::Shell, drop::Quote};
use crate::ai::Cancel;

#[derive(Default)]
pub(super) struct Control {
    lease: Option<Cancel>,
    pub writing: bool,
    command: Option<(u64, Option<i32>)>,
    pub prompt: u64,
    pub own_prompt: bool,
    pub takeover: Option<Takeover>,
}

pub(super) struct Takeover {
    prompt: u64,
    submitted: bool,
    bytes: Vec<u8>,
}

impl Drop for Control {
    fn drop(&mut self) {
        if let Some(lease) = &self.lease {
            lease.stop();
        }
    }
}

impl Control {
    pub fn mark(&mut self, id: u64, status: Option<i32>) {
        if let Some((current, code)) = &mut self.command
            && *current == id
        {
            *code = status;
            log::info!("ai: command {id:x} receipt {status:?}");
        }
    }
}

impl TerminalView {
    pub fn ai_bind(&mut self, cancel: Cancel) {
        self.agent.lease = Some(cancel);
    }

    pub fn ai_supported(&self) -> bool {
        cfg!(windows) && matches!(self.pty.quote, Quote::PowerShell | Quote::Cmd)
    }

    pub fn ai_ready(&self) -> bool {
        matches!(self.shell, Shell::Reading(_))
            && self.agent.own_prompt
            && self.agent.takeover.is_none()
            && !self.herdr
            && self.marked.is_none()
            && self.pasting.is_none()
            && !self
                .pty
                .term
                .lock()
                .mode()
                .contains(alacritty_terminal::term::TermMode::ALT_SCREEN)
            && self.records.shown().is_none_or(|(_, record)| {
                !matches!(record.state, super::State::Working | super::State::Blocked)
            })
    }

    pub(crate) fn ai_idle(pid: u32) -> Result<bool> {
        #[cfg(windows)]
        {
            let processes =
                super::procs::processes().context("ai: terminal processes could not read")?;
            ensure!(
                processes.iter().any(|(id, ..)| *id == pid),
                "ai: target shell process exited"
            );
            Ok(!processes.iter().any(|(_, parent, _)| *parent == pid))
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            anyhow::bail!("ai: autonomous execution is not supported on this platform")
        }
    }

    pub fn ai_prompt(&self) -> u64 {
        self.agent.prompt
    }

    pub fn ai_status(&self, id: u64) -> Result<Option<i32>> {
        let (current, status) = self
            .agent
            .command
            .context("ai: command no longer belongs to this task")?;
        ensure!(current == id, "ai: command identity changed");
        Ok(status)
    }

    pub fn ai_cancel_line(&mut self, cx: &mut Context<Self>) -> Result<()> {
        ensure!(
            self.ai_ready(),
            "ai: terminal is not at an available shell prompt"
        );
        self.ai_key("ctrl-c", cx)
    }

    fn ai_key(&mut self, key: &str, cx: &mut Context<Self>) -> Result<()> {
        let key = Keystroke::parse(key).map_err(|_| anyhow::anyhow!("ai: invalid control key"))?;
        self.agent.writing = true;
        let sent = self.press(&key, cx);
        self.agent.writing = false;
        ensure!(sent, "ai: terminal could not send control key");
        Ok(())
    }

    pub fn ai_stop(&mut self, cx: &mut Context<Self>) {
        if self.agent.command.is_some() && !self.ai_ready() && self.agent.takeover.is_none() {
            self.agent.takeover = Some(Takeover {
                prompt: self.agent.prompt,
                submitted: false,
                bytes: Vec::new(),
            });
        }
        if let Some(lease) = self.agent.lease.take() {
            lease.stop();
        }
        if self
            .agent
            .command
            .take()
            .is_some_and(|(_, status)| status.is_none())
            && let Err(error) = self.ai_key("ctrl-c", cx)
        {
            log::error!("{error:#}");
        }
    }

    pub(super) fn ai_human(&mut self, cx: &mut Context<Self>) {
        if !self.agent.writing
            && self
                .agent
                .lease
                .as_ref()
                .is_some_and(|lease| !lease.stopped())
        {
            self.ai_stop(cx);
            log::info!("ai: terminal {} taken over by its user", self.pty.pid);
            cx.emit(TerminalEvent::Taken);
        }
    }

    pub(super) fn ai_buffer(&mut self, bytes: &[u8]) -> bool {
        if !self.agent.writing
            && let Some(takeover) = &mut self.agent.takeover
        {
            takeover.submitted |= self.pty.quote == Quote::Cmd && bytes.contains(&b'\r');
            takeover.bytes.extend_from_slice(bytes);
            return true;
        }
        false
    }

    // cmd reports its prompts but has no command-start marker.
    pub(super) fn cmd_submitted(&mut self) {
        if self.pty.quote == Quote::Cmd {
            if let Some(takeover) = &mut self.agent.takeover {
                takeover.submitted = true;
            } else {
                self.shell = Shell::Busy;
            }
        }
    }

    pub(super) fn ai_resume_input(&mut self, cx: &mut Context<Self>) {
        if self.agent.own_prompt
            && self
                .agent
                .takeover
                .as_ref()
                .is_some_and(|takeover| self.agent.prompt > takeover.prompt)
            && let Some(takeover) = self.agent.takeover.take()
        {
            log::info!(
                "ai: terminal {} resumes {} buffered input bytes",
                self.pty.pid,
                takeover.bytes.len()
            );
            if !takeover.bytes.is_empty() {
                self.input(takeover.bytes, cx);
                if takeover.submitted {
                    self.shell = Shell::Busy;
                }
            }
        }
    }

    pub fn ai_submit(&mut self, command: &str, cx: &mut Context<Self>) -> Result<u64> {
        ensure!(
            self.ai_ready() && self.ai_supported(),
            "ai: terminal is not ready for autonomous execution"
        );
        self.agent
            .lease
            .as_ref()
            .context("ai: terminal has no task")?
            .check()?;
        let id = getrandom::u64().context("ai: command identity could not be generated")?;
        let command = wrapped(self.pty.quote, command, id)?;
        self.agent.command = Some((id, None));
        self.agent.writing = true;
        self.shell = Shell::Busy;
        self.typed(&command, cx);
        self.agent.writing = false;
        self.ran = Some(Instant::now());
        log::info!("ai: terminal {} command {id:x} submitted", self.pty.pid);
        Ok(id)
    }

    pub fn ai_context(&self) -> String {
        use alacritty_terminal::{
            grid::Dimensions,
            index::{Column, Line},
            term::cell::Flags,
        };
        let term = self.pty.term.lock();
        let grid = term.grid();
        let offset = grid.display_offset() as i32;
        let mut rows = Vec::new();
        for line in -offset..term.screen_lines() as i32 - offset {
            let mut row = String::new();
            for column in 0..term.columns() {
                let cell = &grid[Line(line)][Column(column)];
                if !cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    row.push(cell.c);
                    row.extend(cell.zerowidth().into_iter().flatten());
                }
            }
            rows.push(row.trim_end().to_string());
        }
        drop(term);
        format!(
            "Visible terminal:\n{}\n\nRecent output (200 lines):\n{}\n\nSelection:\n{}",
            rows.join("\n"),
            self.tail(Some(200)),
            self.selected().unwrap_or_default()
        )
    }
}

pub(crate) fn wrapped(quote: Quote, command: &str, id: u64) -> Result<String> {
    ensure!(
        !command.contains('\0') && command.len() <= 8192,
        "ai: command exceeds its limit"
    );
    let start = format!("[Console]::Write([char]27+']133;X;{id:x};start'+[char]7)");
    match quote {
        Quote::PowerShell => {
            let encoded = base64::engine::general_purpose::STANDARD
                .encode(format!("{command}\n$__ely_ai_ok=$?"));
            let source = base64::engine::general_purpose::STANDARD.encode(command);
            Ok(format!(
                "{start}; $__ely_ai_code=0; $__ely_ai_ok=$false; $__ely_ai_before=Get-Variable LASTEXITCODE -Scope Global -ValueOnly -ErrorAction Ignore; $__ely_ai_shell=ElyCommandIsPowerShell ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{source}'))); try {{ . ([scriptblock]::Create([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{encoded}')))); if (!$__ely_ai_ok) {{ $__ely_ai_exit=Get-Variable LASTEXITCODE -Scope Global -ValueOnly -ErrorAction Ignore; $__ely_ai_code=if (!$__ely_ai_shell -and $null -ne $__ely_ai_exit -and -not [object]::ReferenceEquals($__ely_ai_exit,$__ely_ai_before) -and $__ely_ai_exit -ne 0) {{$__ely_ai_exit}} else {{1}} }} }} catch {{ $__ely_ai_code=1; Write-Error $_ }} finally {{ [Console]::Write([char]27+']133;X;{id:x};'+$__ely_ai_code+[char]7) }}"
            ))
        }
        Quote::Cmd => {
            ensure!(
                !command.contains(['\r', '\n']),
                "ai: cmd requires a single command line"
            );
            let command = format!(
                "powershell.exe -NoLogo -NoProfile -NonInteractive -Command \"{start}\" && ({command}) & call powershell.exe -NoLogo -NoProfile -NonInteractive -Command \"& {{ param($code) [Console]::Write([char]27+']133;X;{id:x};'+$code+[char]7) }}\" ^%errorlevel^%"
            );
            ensure!(
                command.len() < 8000,
                "ai: wrapped cmd command exceeds 8000 bytes"
            );
            Ok(command)
        }
        Quote::Posix => {
            anyhow::bail!("ai: autonomous execution for this shell is not yet supported")
        }
    }
}

#[cfg(test)]
mod tests;
