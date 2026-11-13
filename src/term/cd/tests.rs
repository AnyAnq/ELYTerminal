use alacritty_terminal::{
    event::VoidListener,
    term::{Config, test::TermSize},
    vte::ansi::Processor,
};

use super::*;
use crate::term::{marks::Click, prompts::Prompts};

fn parsed(bytes: &[u8]) -> Term<VoidListener> {
    let mut term = Term::new(Config::default(), &TermSize::new(40, 6), VoidListener);
    let mut parser: Processor = Processor::new();
    parser.advance(&mut term, Prompts::default().pass(bytes));
    term
}

#[test]
fn directory_commands_keep_paths_literal_and_change_cmd_s_drive() {
    assert_eq!(
        command(Path::new(r"C:\my files\[a]'b"), Quote::PowerShell).unwrap(),
        "Set-Location -LiteralPath 'C:\\my files\\[a]''b' "
    );
    assert_eq!(
        command(Path::new("/tmp/a b;$x"), Quote::Posix).unwrap(),
        "cd -- /tmp/a\\ b\\;\\$x "
    );
    assert_eq!(
        command(Path::new(r"D:\my files"), Quote::Cmd).unwrap(),
        "cd /d \"D:\\my files\" "
    );
}

#[test]
fn a_wsl_share_uses_the_path_inside_the_distribution() {
    assert_eq!(
        command(
            Path::new(r"\\wsl.localhost\Ubuntu\home\me\my repo"),
            Quote::Posix
        )
        .unwrap(),
        "cd -- /home/me/my\\ repo "
    );
}

#[test]
fn unsafe_path_text_never_becomes_a_directory_command() {
    for quote in [Quote::Posix, Quote::PowerShell, Quote::Cmd] {
        assert!(command(Path::new("/tmp/a\nwhoami"), quote).is_err());
    }
    for path in [r"C:\%PATH%", r"C:\!SECRET!"] {
        assert!(command(Path::new(path), Quote::Cmd).is_err());
    }
    assert!(
        command(
            Path::new("\\\\wsl.localhost\\Ubuntu\\home\\me\\x\u{f00a}whoami"),
            Quote::Posix
        )
        .is_err()
    );
}

#[test]
fn only_an_empty_marked_shell_prompt_accepts_a_directory_command() {
    let reading = Shell::Reading(Click::Arrows);
    let prompt = b"out\r\n\x1b]133;A\x07$ \x1b]133;B\x07";
    assert!(ready(&parsed(prompt), reading));
    assert!(!ready(&parsed(prompt), Shell::Busy));
    assert!(!ready(&parsed(prompt), Shell::Prompting(Click::Arrows)));
    assert!(!ready(&parsed(b"$ "), reading));
    for input in [
        "echo pending",
        "echo pending\x1b[3G",
        "echo \\\r\n\x1b]133;P;k=s\x07> \x1b]133;B\x07",
        "\x1b[?1049h\x1b]133;A\x07$ \x1b]133;B\x07",
        "中文",
    ] {
        let mut bytes = prompt.to_vec();
        bytes.extend_from_slice(input.as_bytes());
        assert!(!ready(&parsed(&bytes), reading), "{input:?}");
    }
}

#[test]
fn multiline_and_wrapped_prompts_can_have_empty_input() {
    let reading = Shell::Reading(Click::Arrows);
    for prompt in [
        "\x1b]133;A\x07project\r\n$ \x1b]133;B\x07",
        "\x1b]133;A\x07########################################\r\n\x1b]133;B\x07",
        "\x1b]133;A\x07中文> \x1b]133;B\x07",
    ] {
        assert!(ready(&parsed(prompt.as_bytes()), reading), "{prompt:?}");
    }
}

#[test]
fn erased_input_is_empty_again() {
    let term = parsed(b"\x1b]133;A\x07$ \x1b]133;B\x07old text\x1b[3G\x1b[K");
    assert!(ready(&term, Shell::Reading(Click::Arrows)));
}
