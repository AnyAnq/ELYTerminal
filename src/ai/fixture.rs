use super::config::Model;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::{Duration, Instant},
};

pub(super) fn local() -> (TcpListener, Model) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let model = Model {
        id: format!("fixture-{:x}", getrandom::u64().unwrap()),
        name: "Fixture".into(),
        url: format!("http://{}/v1", listener.local_addr().unwrap()),
        model: "fixture-model".into(),
        tools: false,
    };
    (listener, model)
}

pub(super) fn incoming(listener: &TcpListener) -> (TcpStream, Value, String) {
    let deadline = Instant::now() + Duration::from_secs(45);
    let (stream, _) = loop {
        match listener.accept() {
            Ok(found) => break found,
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(5))
            }
            Err(error) => panic!("mock service did not receive a request: {error}"),
        }
    };
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    let mut headers = String::new();
    let mut length = None;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = Some(value.trim().parse::<usize>().unwrap());
        }
        headers.push_str(&line);
    }
    let mut body = vec![0; length.unwrap()];
    reader.read_exact(&mut body).unwrap();
    (
        reader.into_inner(),
        serde_json::from_slice(&body).unwrap(),
        headers,
    )
}

pub(super) fn response(stream: &mut TcpStream, delta: Value, reason: &str) {
    let text = format!(
        "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
        json!({"choices":[{"index":0,"delta":delta}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":reason}]})
    );
    write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",text.len()).unwrap();
}
