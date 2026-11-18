use super::*;

fn event(value: Value) -> String {
    format!("data: {value}\r\n\r\n")
}

#[test]
fn fragmented_streams_reassemble_unicode_and_multiple_tool_calls() {
    let mut stream = event(json!({"choices":[{"index":0,"delta":{"content":"检查中"}}]}));
    stream += &event(json!({"choices":[{"index":0,"delta":{"reasoning_content":"opaque "}}]}));
    stream += &event(json!({"choices":[{"index":0,"delta":{"reasoning_content":"reasoning"}}]}));
    stream += &event(
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"one","type":"function","function":{"name":"read_terminal","arguments":"{\"lines\":"}}]}}]}),
    );
    stream += &event(
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"20}"}},{"index":1,"id":"two","function":{"name":"read_file","arguments":"{\"path\":\"a\"}"}}]},"finish_reason":"tool_calls"}]}),
    );
    stream += "data: [DONE]\r\n\r\n";
    let mut decoder = Decoder::default();
    let mut text = String::new();
    for byte in stream.as_bytes() {
        decoder
            .feed(&[*byte], &mut |part| text.push_str(part))
            .unwrap();
    }
    let answer = decoder.finish().unwrap();
    assert_eq!(text, "检查中");
    assert_eq!(assistant(&answer)["reasoning_content"], "opaque reasoning");
    assert_eq!(answer.calls.len(), 2);
    assert_eq!(answer.calls[0].function.arguments, "{\"lines\":20}");
    assert_eq!(assistant(&answer)["tool_calls"][1]["id"], "two");
}

#[test]
fn truncated_invalid_and_oversized_tools_never_become_executable() {
    let mut decoder = Decoder::default();
    decoder
        .feed(
            event(json!({"choices":[{"index":0,"delta":{"content":"partial"}}]})).as_bytes(),
            &mut |_| {},
        )
        .unwrap();
    assert!(decoder.finish().is_err());
    let mut decoder = Decoder::default();
    assert!(
        decoder
            .feed(
                event(json!({"choices":[{"index":0,"delta":{},"finish_reason":"length"}]}))
                    .as_bytes(),
                &mut |_| {}
            )
            .is_err()
    );
    let mut decoder = Decoder::default();
    let mut stream = event(
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"bad","function":{"name":"run_command","arguments":"{bad"}}]},"finish_reason":"tool_calls"}]}),
    );
    stream += "data: [DONE]\n\n";
    decoder.feed(stream.as_bytes(), &mut |_| {}).unwrap();
    assert!(decoder.finish().is_err());
}

#[test]
fn interruption_closes_pending_calls_and_chat_only_models_receive_plain_history() {
    let mut messages = vec![
        json!({"role":"assistant","content":"Checking","tool_calls":[{"id":"one"},{"id":"two"}]}),
        json!({"role":"tool","tool_call_id":"one","content":"OK"}),
    ];
    cancel_pending(&mut messages);
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[2]["tool_call_id"], "two");
    cancel_pending(&mut messages);
    assert_eq!(messages.len(), 3);
    let chat = history(&messages, false);
    assert!(
        chat.iter()
            .all(|message| message["role"] == "assistant" && message.get("tool_calls").is_none())
    );
    assert_eq!(history(&messages, true), messages);
}
