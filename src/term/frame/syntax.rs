use alacritty_terminal::{
    term::cell::{Cell, Flags},
    vte::ansi::{Color, NamedColor},
};

use crate::term::prompts::{self, Part};

#[derive(Default)]
enum Stage {
    #[default]
    Off,
    Command(String),
    BeforeSubcommand,
    Subcommand,
}

/// PSReadLine gives npm's subcommand and package names the same default color.
#[derive(Default)]
pub(super) struct Syntax {
    part: Option<Part>,
    stage: Stage,
}

impl Syntax {
    pub fn subcommand(&mut self, cell: &Cell) -> bool {
        let part = prompts::part(cell);
        if part != self.part {
            self.stage = match (self.part, part) {
                (Some(Part::Prompt(_)), Some(Part::Input(_))) => Stage::Command(String::new()),
                _ => Stage::Off,
            };
            self.part = part;
        }
        if !matches!(part, Some(Part::Input(_))) {
            return false;
        }
        let ch = cell.c;
        let subcommand = match &mut self.stage {
            Stage::Off => false,
            Stage::Command(word) => {
                if ch.is_whitespace() {
                    if !word.is_empty() {
                        self.stage = if ["npm", "npm.cmd", "npm.exe"]
                            .iter()
                            .any(|name| word.eq_ignore_ascii_case(name))
                        {
                            Stage::BeforeSubcommand
                        } else {
                            Stage::Off
                        };
                    }
                } else if word.len() < 7 && (ch.is_ascii_alphanumeric() || ch == '.') {
                    word.push(ch);
                } else {
                    self.stage = Stage::Off;
                }
                false
            }
            Stage::BeforeSubcommand => {
                if ch.is_ascii_alphabetic() {
                    self.stage = Stage::Subcommand;
                    true
                } else {
                    // shortcut: global options keep shell colors; parse when needed.
                    if !ch.is_whitespace() {
                        self.stage = Stage::Off;
                    }
                    false
                }
            }
            Stage::Subcommand => {
                if ch.is_ascii_alphanumeric() || ch == '-' {
                    true
                } else {
                    self.stage = Stage::Off;
                    false
                }
            }
        };
        subcommand
            && !cell
                .flags
                .intersects(Flags::DIM | Flags::ITALIC | Flags::HIDDEN | Flags::INVERSE)
            && matches!(
                cell.fg,
                Color::Named(NamedColor::White | NamedColor::Foreground)
            )
    }
}
