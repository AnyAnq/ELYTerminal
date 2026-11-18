use std::{
    io::{Read, Write},
    thread,
    time::{Duration, Instant},
};

use serde_json::json;

use super::*;

use crate::ai::fixture::{incoming, local, response};

#[test]
fn curl_stdin_preserves_headers_unicode_json_and_tool_round_trips() {
    let (listener, model) = local();
    let server = thread::spawn(move || {
        let (mut stream, body, headers) = incoming(&listener);
        assert!(headers.contains("POST /v1/chat/completions HTTP/1.1"));
        assert!(headers.contains("Authorization: Bearer fixture-key"));
        assert_eq!(body["messages"][0]["content"], "中文\n\"quoted\"\\path");
        response(&mut stream, json!({"content":"收到"}), "stop");
    });
    let mut shown = String::new();
    let request = request(
        &model,
        &[json!({"role":"user","content":"中文\n\"quoted\"\\path"})],
        &[],
    );
    let answer = complete(
        &model,
        "fixture-key",
        &request,
        &Cancel::default(),
        |text| shown.push_str(text),
    )
    .unwrap();
    assert_eq!(shown, "收到");
    assert_eq!(answer.text, shown);
    server.join().unwrap();
}

#[test]
fn capability_probe_requires_an_actual_function_call_and_result_reply() {
    let (listener, model) = local();
    let server = thread::spawn(move || {
        let (mut stream, _, _) = incoming(&listener);
        response(&mut stream, json!({"content":"OK"}), "stop");
        let (mut stream, request, _) = incoming(&listener);
        assert_eq!(request["tool_choice"], "auto");
        response(
            &mut stream,
            json!({"tool_calls":[{"index":0,"id":"probe-1","type":"function","function":{"name":"connection_probe","arguments":"{}"}}]}),
            "tool_calls",
        );
        let (mut stream, request, _) = incoming(&listener);
        assert_eq!(request["messages"][2]["role"], "tool");
        assert_eq!(request["messages"][2]["tool_call_id"], "probe-1");
        response(&mut stream, json!({"content":"Verified"}), "stop");
    });
    assert!(crate::ai::settings::probe(&model, "").unwrap());
    server.join().unwrap();
}

#[test]
fn thinking_model_probe_accepts_auto_tools_and_returns_reasoning_history() {
    let (listener, model) = local();
    let server = thread::spawn(move || {
        let (mut stream, _, _) = incoming(&listener);
        response(&mut stream, json!({"content":"OK"}), "stop");
        let (mut stream, request, _) = incoming(&listener);
        if request["tool_choice"] != "auto" {
            let error =
                json!({"error":{"message":"forced tool_choice is not supported in thinking mode"}})
                    .to_string();
            write!(stream, "HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{error}", error.len()).unwrap();
            return;
        }
        response(
            &mut stream,
            json!({"reasoning_content":"fixture reasoning token","tool_calls":[{"index":0,"id":"thinking-probe","type":"function","function":{"name":"connection_probe","arguments":"{}"}}]}),
            "tool_calls",
        );
        let (mut stream, request, _) = incoming(&listener);
        assert_eq!(
            request["messages"][1]["reasoning_content"],
            "fixture reasoning token"
        );
        assert_eq!(request["tools"][0]["function"]["name"], "connection_probe");
        response(&mut stream, json!({"content":"OK"}), "stop");
    });
    assert!(
        crate::ai::settings::probe(&model, "").unwrap(),
        "thinking model was incorrectly downgraded to chat only"
    );
    server.join().unwrap();
}

#[test]
fn stop_terminates_a_silent_model_connection() {
    let (listener, model) = local();
    let (ready, heard) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _, _) = incoming(&listener);
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        stream.flush().unwrap();
        ready.send(()).unwrap();
        let mut byte = [0];
        match stream.read(&mut byte) {
            Ok(0) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
                ) => {}
            other => panic!("cancelled connection stayed open: {other:?}"),
        }
    });
    let cancel = Cancel::default();
    let running = cancel.clone();
    let client = thread::spawn(move || {
        complete(
            &model,
            "",
            &request(&model, &[json!({"role":"user","content":"Wait"})], &[]),
            &running,
            |_| {},
        )
    });
    heard.recv_timeout(Duration::from_secs(5)).unwrap();
    let since = Instant::now();
    cancel.stop();
    assert!(client.join().unwrap().is_err());
    assert!(since.elapsed() < Duration::from_secs(3));
    server.join().unwrap();
}
