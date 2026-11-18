use serde_json::Value;

pub fn approval(name: &str, args: &Value, automatic: bool) -> bool {
    if automatic {
        return false;
    }
    if args["requires_approval"].as_bool() == Some(true) {
        return true;
    }
    if name != "run_command" {
        return false;
    }
    let Some(command) = args["command"].as_str() else {
        return true;
    };
    let command = command.trim().to_ascii_lowercase();
    if command.contains([
        '\n', '\r', ';', '|', '&', '>', '<', '`', '$', '(', ')', '{', '}', '%', '!',
    ]) {
        return true;
    }
    let words: Vec<_> = command.split_whitespace().collect();
    let Some(&first) = words.first() else {
        return true;
    };
    let args = &words[1..];
    if words.iter().any(|word| {
        matches!(
            word.split('=').next().unwrap_or(word),
            "--global"
                | "-g"
                | "--system"
                | "--exec"
                | "-exec"
                | "--config"
                | "-c"
                | "--output"
                | "--out"
                | "--delete"
                | "--force"
                | "-f"
                | "--manifest-path"
                | "--target-dir"
                | "--ext-diff"
        )
    }) {
        return true;
    }
    let safe = match first {
        "pwd" | "get-location" | "get-childitem" | "dir" | "ls" | "get-command" | "where"
        | "where.exe" | "test-path" => true,
        // Git can start external helpers through repository configuration, even without options.
        "git" => false,
        "cargo" => {
            args.first().is_some_and(|arg| {
                matches!(
                    *arg,
                    "test" | "check" | "build" | "clippy" | "metadata" | "tree" | "version"
                )
            }) || args == ["fmt", "--check"]
        }
        "npm" => {
            args == ["test"]
                || args == ["run", "build"]
                || args == ["run", "test"]
                || args == ["--version"]
        }
        "dotnet" => args
            .first()
            .is_some_and(|arg| matches!(*arg, "test" | "build" | "--info" | "--version")),
        "node" | "python" | "python3" | "rustc" | "pwsh" | "powershell" => args == ["--version"],
        _ => false,
    };
    !safe
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn git_commands_require_approval_in_manual_mode() {
        for command in [
            "git grep --open-files-in-pager TODO",
            "git grep -O TODO",
            "git status",
            "git diff -- src",
        ] {
            let args = json!({"command": command, "reason": "Inspect the repository", "requires_approval": false});
            assert!(approval("run_command", &args, false), "{command}");
            assert!(!approval("run_command", &args, true), "{command}");
        }
    }

    #[test]
    fn high_impact_and_compound_commands_cannot_bypass_approval() {
        for command in [
            "Remove-Item -Recurse .",
            "git push",
            "git reset --hard",
            "npm install -g x",
            "cargo test; rm x",
            "python -c 'delete()'",
            "git -c core.pager=evil diff",
            "Invoke-Expression x",
        ] {
            assert!(
                approval(
                    "run_command",
                    &json!({"command":command,"requires_approval":false}),
                    false
                ),
                "{command}"
            );
        }
        for command in [
            "cargo test --locked",
            "npm run build",
            "Get-ChildItem .",
            "dotnet test",
        ] {
            assert!(
                !approval("run_command", &json!({"command":command}), false),
                "{command}"
            );
        }
        assert!(approval(
            "run_command",
            &json!({"command":"Get-ChildItem .","requires_approval":true}),
            false
        ));
        assert!(!approval(
            "run_command",
            &json!({"command":"Remove-Item must-remain.txt","requires_approval":true}),
            true
        ));
    }
}
