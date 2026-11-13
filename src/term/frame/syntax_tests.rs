use alacritty_terminal::{
    event::VoidListener,
    term::{Config, Term, test::TermSize},
    vte::ansi::Processor,
};
use gpui::Hsla;

use super::{Frame, frame};
use crate::{
    term::{ink::Ink, prompts::Prompts},
    themes,
};

fn parsed(input: &str, columns: usize) -> Term<VoidListener> {
    let mut term = Term::new(Config::default(), &TermSize::new(columns, 6), VoidListener);
    let mut parser: Processor = Processor::new();
    parser.advance(&mut term, Prompts::default().pass(input.as_bytes()));
    term
}

fn colors(shown: &Frame, text: &str) -> Vec<Hsla> {
    let whole: String = shown.rows.iter().map(|row| row.text.as_str()).collect();
    let start = whole.find(text).expect("the text is on screen");
    shown
        .rows
        .iter()
        .flat_map(|row| {
            row.runs
                .iter()
                .flat_map(|(bytes, style)| std::iter::repeat_n(style.fg, *bytes))
        })
        .skip(start)
        .take(text.len())
        .collect()
}

#[test]
fn npm_subcommands_differ_from_packages_with_psreadline_s_colors() {
    for (line, package) in [
        (
            "\x1b[93mnpm\x1b[37m install\x1b[90m -g\x1b[37m opencode-ai",
            "opencode-ai",
        ),
        (
            "\x1b[93mnpm\x1b[37m install anywhere\x1b[90m -g",
            "anywhere",
        ),
    ] {
        let bytes = format!("\x1b]133;A\x07PS> \x1b]133;B\x07{line}");
        let term = parsed(&bytes, 80);
        let ink = Ink::new(themes::find("dracula").expect("the theme"), term.colors());
        let shown = frame(&term, &ink, &[], None);
        assert_eq!(colors(&shown, "npm"), vec![ink.ansi[11]; 3]);
        assert_eq!(colors(&shown, "install"), vec![ink.ansi[4]; 7]);
        assert_eq!(colors(&shown, package), vec![ink.ansi[7]; package.len()]);
        assert_eq!(colors(&shown, "-g"), vec![ink.ansi[8]; 2]);
    }
}

#[test]
fn the_subcommand_keeps_its_color_across_soft_wraps() {
    for columns in [12, 16, 80] {
        let term = parsed(
            "\x1b]133;A\x07PS> \x1b]133;B\x07\x1b[37mnpm install anywhere",
            columns,
        );
        let ink = Ink::new(themes::find("dracula").expect("the theme"), term.colors());
        let shown = frame(&term, &ink, &[], None);
        assert_eq!(colors(&shown, "install"), vec![ink.ansi[4]; 7]);
        assert_eq!(colors(&shown, "anywhere"), vec![ink.ansi[7]; 8]);
    }
}

#[test]
fn output_and_other_arguments_keep_the_shell_s_colors() {
    for bytes in [
        "\x1b[37mnpm install anywhere",
        "\x1b]133;A\x07PS> \x1b]133;B\x07echo npm install anywhere",
        "\x1b]133;A\x07PS> \x1b]133;B\x07npm --prefix . install anywhere",
        "\x1b]133;A\x07PS> \x1b]133;B\x07echo hello\x1b]133;C\x07\r\nnpm install anywhere",
        "\x1b[?1049h\x1b]133;A\x07PS> \x1b]133;B\x07npm install anywhere",
    ] {
        let term = parsed(bytes, 80);
        let ink = Ink::new(themes::find("dracula").expect("the theme"), term.colors());
        let shown = frame(&term, &ink, &[], None);
        assert_eq!(colors(&shown, "install"), vec![ink.fg; 7], "{bytes:?}");
    }
}

#[test]
fn predictions_and_explicit_syntax_colors_are_preserved() {
    for line in [
        "npm \x1b[97;2;3minstall anywhere",
        "npm \x1b[36minstall\x1b[37m anywhere",
        "npm \x1b[38;2;1;2;3minstall\x1b[37m anywhere",
    ] {
        let marked = parsed(&format!("\x1b]133;A\x07PS> \x1b]133;B\x07{line}"), 80);
        let plain = parsed(&format!("PS> {line}"), 80);
        let ink = Ink::new(themes::find("dracula").expect("the theme"), marked.colors());
        let shown = frame(&marked, &ink, &[], None);
        let original = frame(&plain, &ink, &[], None);
        assert_eq!(shown.rows, original.rows, "{line:?}");
    }
}

#[test]
fn the_selection_s_foreground_wins_over_the_subcommand() {
    use alacritty_terminal::{
        index::{Column, Line, Point, Side},
        selection::{Selection, SelectionType},
    };

    let mut term = parsed("\x1b]133;A\x07PS> \x1b]133;B\x07npm install anywhere", 80);
    let at = |column| Point::new(Line(0), Column(column));
    let mut selection = Selection::new(SelectionType::Simple, at(8), Side::Left);
    selection.update(at(14), Side::Right);
    term.selection = Some(selection);
    let mut ink = Ink::new(themes::find("dracula").expect("the theme"), term.colors());
    for selected in [None, Some(ink.ansi[1])] {
        ink.selection_text = selected;
        let shown = frame(&term, &ink, &[], None);
        assert_eq!(
            colors(&shown, "install"),
            vec![selected.unwrap_or(ink.fg); 7]
        );
    }
}

#[test]
fn completed_commands_and_the_next_prompt_each_keep_their_own_subcommand() {
    let term = parsed(
        "\x1b]133;A\x07PS> \x1b]133;B\x07\x1b[37mnpm install anywhere\x1b]133;C\x07\r\n\
         npm install output\r\n\x1b]133;D;0\x07\x1b]133;A\x07PS> \x1b]133;B\x07npm.cmd uninstall anywhere",
        80,
    );
    let ink = Ink::new(themes::find("dracula").expect("the theme"), term.colors());
    let shown = frame(&term, &ink, &[], None);
    assert_eq!(colors(&shown, "install anywhere")[..7], [ink.ansi[4]; 7]);
    assert_eq!(colors(&shown, "install output")[..7], [ink.ansi[7]; 7]);
    assert_eq!(colors(&shown, "uninstall"), vec![ink.ansi[4]; 9]);
}
