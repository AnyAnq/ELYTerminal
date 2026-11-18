use serde_json::{Value, json};

use crate::ai::fixture;

fn call(stream: &mut std::net::TcpStream, id: &str, name: &str, args: Value) {
    fixture::response(
        stream,
        json!({"tool_calls":[{"index":0,"id":id,"type":"function","function":{"name":name,"arguments":args.to_string()}}]}),
        "tool_calls",
    );
}

pub(super) fn serve(listener: std::net::TcpListener, cmd: bool) {
    for step in 0..44 {
        let (mut stream, request, _) = fixture::incoming(&listener);
        let last = request["messages"].as_array().unwrap().last().unwrap();
        match step {
            0 => call(
                &mut stream,
                "reproduce",
                "run_command",
                json!({"command":"cargo test --offline","reason":"Reproduce the failing test"}),
            ),
            1 => {
                let result: Value =
                    serde_json::from_str(last["content"].as_str().unwrap()).unwrap();
                assert!(result["exit_code"].as_i64().unwrap() != 0, "{result}");
                call(
                    &mut stream,
                    "read",
                    "read_file",
                    json!({"path":"src/lib.rs"}),
                );
            }
            2 => call(
                &mut stream,
                "edit",
                "edit_file",
                json!({"path":"src/lib.rs","old":"false","new":"true"}),
            ),
            3 => call(
                &mut stream,
                "verify",
                "run_command",
                json!({"command":"cargo test --offline","reason":"Verify the repair","requires_approval":false}),
            ),
            4 => {
                let result: Value =
                    serde_json::from_str(last["content"].as_str().unwrap()).unwrap();
                assert_eq!(result["exit_code"], 0, "{result}");
                fixture::response(
                    &mut stream,
                    json!({"content":"Fixed and verified by cargo test."}),
                    "stop",
                );
            }
            5 => call(
                &mut stream,
                "interrupt",
                "run_command",
                json!({"command":if cmd { "echo AI_SLEEP_STARTED & ping -n 30 127.0.0.1 >nul" } else { "Write-Output 'AI_SLEEP_STARTED'; Start-Sleep -Seconds 30" },"reason":"Exercise manual takeover","requires_approval":true}),
            ),
            6..=35 => {
                if step == 6 {
                    assert!(
                        last["content"]
                            .as_str()
                            .unwrap()
                            .contains("Exercise the call limit"),
                        "a stopped task made another request"
                    );
                }
                call(
                    &mut stream,
                    &format!("limit{step}"),
                    "read_terminal",
                    json!({"lines":1}),
                );
            }
            36 => fixture::response(
                &mut stream,
                json!({"content":"Continued after 30 calls."}),
                "stop",
            ),
            37 => call(
                &mut stream,
                "decline",
                "run_command",
                json!({"command":if cmd { "del must-remain.txt" } else { "Remove-Item must-remain.txt" },"reason":"Exercise declined approval"}),
            ),
            38 => {
                assert!(last["content"].as_str().unwrap().contains("declined"));
                fixture::response(
                    &mut stream,
                    json!({"content":"The declined action was skipped."}),
                    "stop",
                );
            }
            39 => call(
                &mut stream,
                "auto-first",
                "run_command",
                json!({"command":if cmd { "copy must-remain.txt auto-first.txt" } else { "Copy-Item must-remain.txt auto-first.txt" },"reason":"Exercise automatic approval"}),
            ),
            40 => {
                let result: Value =
                    serde_json::from_str(last["content"].as_str().unwrap()).unwrap();
                assert_eq!(result["exit_code"], 0);
                assert!(
                    request["messages"][0]["content"]
                        .as_str()
                        .unwrap()
                        .contains("automatic approval")
                );
                call(
                    &mut stream,
                    "auto-second",
                    "run_command",
                    json!({"command":if cmd { "copy must-remain.txt auto-second.txt && cd nested" } else { "Copy-Item must-remain.txt auto-second.txt; Set-Location nested" },"reason":"Exercise the next automatically approved action and change directory"}),
                );
            }
            41 => {
                let result: Value =
                    serde_json::from_str(last["content"].as_str().unwrap()).unwrap();
                assert_eq!(result["exit_code"], 0);
                fixture::response(
                    &mut stream,
                    json!({"content":"Automatic approval verified."}),
                    "stop",
                );
            }
            42 => call(
                &mut stream,
                "read-new-folder",
                "read_file",
                json!({"path":"next-folder.txt"}),
            ),
            43 => {
                let result: Value =
                    serde_json::from_str(last["content"].as_str().unwrap()).unwrap();
                assert_eq!(result["text"], "next task folder", "{result}");
                call(
                    &mut stream,
                    "close-while-waiting",
                    "run_command",
                    json!({"command":"echo fixture","reason":"Wait for confirmation","requires_approval":true}),
                );
            }
            _ => unreachable!(),
        }
    }
}
