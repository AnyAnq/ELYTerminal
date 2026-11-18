use super::*;

#[test]
fn command_receipts_match_identity_even_when_exit_codes_repeat() {
    let mut control = Control::default();
    control.command = Some((10, None));
    control.mark(9, Some(0));
    assert_eq!(control.command, Some((10, None)));
    control.mark(10, Some(0));
    assert_eq!(control.command, Some((10, Some(0))));
    control.command = Some((11, None));
    control.mark(11, Some(0));
    assert_eq!(control.command, Some((11, Some(0))));
    assert_eq!(
        super::super::marks::read(b"X;b;0"),
        Some(super::super::marks::Mark::Command(11, Some(0)))
    );
    assert!(super::super::marks::read(b"X;b;garbage").is_none());
    assert!(wrapped(Quote::Posix, "echo hello", 11).is_err());
    assert!(wrapped(Quote::Cmd, "echo one\necho two", 11).is_err());
}

#[cfg(windows)]
#[test]
fn powershell_and_cmd_report_native_failure_and_success_without_echo_receipts() {
    use std::process::Command;
    for shell in ["powershell.exe", "pwsh.exe", "cmd.exe"] {
        let cmd = shell == "cmd.exe";
        for (code, command) in if cmd {
            vec![(7, "cmd /d /c exit 7"), (0, "echo verified")]
        } else {
            vec![
                (7, "cmd /d /c exit 7"),
                (0, "Write-Output 'verified'"),
                (1, "cmd /d /c exit 7; Write-Error 'failure'"),
                (1, "throw 'failure'"),
                (0, "cmd /d /c exit 7; Write-Output 'recovered'"),
                (7, "cmd /d /c exit 7; cmd /d /c exit 7"),
            ]
        } {
            let wrapped = wrapped(
                if cmd { Quote::Cmd } else { Quote::PowerShell },
                command,
                31,
            )
            .unwrap();
            assert!(!wrapped.contains('\x1b'), "echo cannot forge a receipt");
            let mut process = Command::new(shell);
            if cmd {
                process.args(["/d", "/c"]);
            } else {
                process.args(["-NoProfile", "-NonInteractive", "-Command"]);
            }
            let script = if cmd {
                wrapped
            } else {
                format!(
                    "{}\n$global:LASTEXITCODE=7\n{wrapped}",
                    include_str!("../../../assets/shell-integration/pwsh/elyterminal.ps1")
                )
            };
            if cmd {
                use std::os::windows::process::CommandExt;
                process.raw_arg(script);
            } else {
                process.arg(script);
            }
            let output = process.output().unwrap();
            let text = String::from_utf8_lossy(&output.stdout);
            assert!(
                text.contains("\x1b]133;X;1f;start\x07"),
                "{shell}: {text:?}"
            );
            assert!(
                text.contains(&format!("\x1b]133;X;1f;{code}\x07")),
                "{shell}: {text:?}, {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
