use super::*;

#[test]
fn command_without_approval_flag_uses_the_app_policy() {
    for (command, approval) in [
        ("tldr npm", true),
        ("Remove-Item must-remain.txt", true),
        ("Get-ChildItem .", false),
    ] {
        let args = json!({"command":command,"reason":"Inspect the task"});
        validate("run_command", &args).unwrap();
        assert_eq!(
            crate::ai::policy::approval("run_command", &args, false),
            approval
        );
        assert!(!crate::ai::policy::approval("run_command", &args, true));
    }
    for invalid in [json!("false"), json!(null), json!(0)] {
        assert!(
            validate(
                "run_command",
                &json!({
                    "command":"git status","reason":"check","requires_approval":invalid
                })
            )
            .is_err()
        );
    }
    assert!(validate("run_command", &json!({"reason":"check"})).is_err());
    assert!(validate("run_command", &json!({"command":"git status"})).is_err());
}

#[test]
fn edits_are_exact_and_cannot_leave_the_project() {
    let root = std::env::temp_dir().join(format!("ely-ai-files-{}", getrandom::u64().unwrap()));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("a.txt"), "wrong\nwrong\n").unwrap();
    assert!(
        file(
            &root,
            "edit_file",
            &json!({"path":"a.txt","old":"wrong","new":"right"})
        )
        .is_err()
    );
    file(
        &root,
        "edit_file",
        &json!({"path":"a.txt","old":"wrong\nwrong","new":"right"}),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).unwrap(),
        "right\n"
    );
    assert!(project_path(&root, "../").is_err());
    std::fs::write(root.join(".env"), "fixture").unwrap();
    assert!(project_path(&root, ".env").is_err());
    std::fs::create_dir(root.join(".GiT")).unwrap();
    std::fs::write(root.join(".GiT/config"), "fixture").unwrap();
    assert!(project_path(&root, ".GiT/config").is_err());
    assert!(
        validate(
            "run_command",
            &json!({"command":"git status","reason":"check","requires_approval":false,"extra":true})
        )
        .is_err()
    );
    assert!(validate("read_terminal", &json!({"lines":1001})).is_err());
    std::fs::remove_file(root.join("a.txt")).unwrap();
    std::fs::remove_file(root.join(".env")).unwrap();
    std::fs::remove_file(root.join(".GiT/config")).unwrap();
    std::fs::remove_dir(root.join(".GiT")).unwrap();
    std::fs::remove_dir(&root).unwrap();
}

#[test]
fn bounded_text_never_splits_a_unicode_character() {
    assert_eq!(bounded("中文", 4), ("中", true));
    assert_eq!(bounded("ok", 20), ("ok", false));
    assert_eq!(tail("中文", 4), ("文", true));
    assert_eq!(tail("old中文new", 8), ("文new", true));
    assert_eq!(tail("ok", 20), ("ok", false));
}
