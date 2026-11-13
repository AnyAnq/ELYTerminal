use std::path::Path;

use alacritty_terminal::{
    grid::Dimensions,
    index::{Column, Line},
    term::{Term, TermMode},
};
use ely_gpui_component::i18n::text;
use gpui::Context;

use super::{
    click::Shell,
    drop::{self, Quote},
    prompts::{self, Part},
    view::TerminalView,
};

/// A directory command, with the path literal for its shell.
fn command(folder: &Path, quote: Quote) -> anyhow::Result<String> {
    let path = folder.to_path_buf();
    #[cfg(any(windows, test))]
    let path = match (quote, crate::git::wsl::split(folder)) {
        (Quote::Posix, Some((_, inside))) => inside.into(),
        _ => path,
    };
    let path = drop::typed(&[path], quote)?;
    let cd = match quote {
        Quote::Posix => "cd -- ",
        Quote::PowerShell => "Set-Location -LiteralPath ",
        Quote::Cmd => "cd /d ",
    };
    Ok(format!("{cd}{path}"))
}

/// An empty input after a known prompt, outside full-screen programs.
fn ready<T>(term: &Term<T>, shell: Shell) -> bool {
    if !matches!(shell, Shell::Reading(_)) || term.mode().contains(TermMode::ALT_SCREEN) {
        return false;
    }
    let grid = term.grid();
    let cursor = grid.cursor.point;
    let before = if cursor.column.0 > 0 {
        (cursor.line, Column(cursor.column.0 - 1))
    } else if cursor.line.0 > term.topmost_line().0 {
        (Line(cursor.line.0 - 1), Column(term.columns() - 1))
    } else {
        return false;
    };
    if !prompts::prompted(&grid[before.0][before.1]) {
        return false;
    }
    let Some(first) = prompts::start(term, cursor.line.0) else {
        return false;
    };
    (first..=term.bottommost_line().0).all(|line| {
        (0..term.columns()).all(|column| {
            let cell = &grid[Line(line)][Column(column)];
            !matches!(prompts::part(cell), Some(Part::Input(_)))
                || (cell.c.is_whitespace() && cell.zerowidth().is_none_or(|marks| marks.is_empty()))
        })
    })
}

impl TerminalView {
    /// Runs a directory command only at an empty shell prompt.
    pub fn change_directory(
        &mut self,
        folder: &Path,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.herdr && self.marked.is_none() && ready(&self.pty.term.lock(), self.shell),
            "terminal {}: {}",
            self.pty.pid,
            text(cx, "et.files.change_directory_wait", &[])
        );
        anyhow::ensure!(
            folder.is_absolute(),
            "terminal {}: {} is not an absolute folder",
            self.pty.pid,
            folder.display()
        );
        let command = command(folder, self.pty.quote)?;
        log::info!(
            "terminal {}: changing directory to {}",
            self.pty.pid,
            folder.display()
        );
        self.shell = Shell::Busy;
        self.typed(&command, cx);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
