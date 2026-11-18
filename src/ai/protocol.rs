use std::collections::BTreeMap;

use anyhow::{Context as _, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Call {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: Function,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug, Default)]
pub struct Answer {
    pub text: String,
    pub reasoning: String,
    pub calls: Vec<Call>,
}

#[derive(Default)]
pub struct Decoder {
    line: Vec<u8>,
    data: String,
    calls: BTreeMap<u64, Call>,
    text: String,
    reasoning: String,
    finished: bool,
    done: bool,
    size: usize,
}

impl Decoder {
    pub fn feed(&mut self, bytes: &[u8], emit: &mut impl FnMut(&str)) -> Result<()> {
        self.size += bytes.len();
        ensure!(self.size <= 2_000_000, "ai: response exceeds 2 MB");
        for &byte in bytes {
            if byte == b'\n' {
                let line = std::mem::take(&mut self.line);
                let line = std::str::from_utf8(&line)
                    .context("ai: response is not UTF-8")?
                    .trim_end_matches('\r');
                if line.is_empty() {
                    self.event(emit)?;
                } else if let Some(data) = line.strip_prefix("data:") {
                    if !self.data.is_empty() {
                        self.data.push('\n');
                    }
                    self.data.push_str(data.strip_prefix(' ').unwrap_or(data));
                }
            } else {
                self.line.push(byte);
                ensure!(self.line.len() <= 131_072, "ai: stream line exceeds 128 KB");
            }
        }
        Ok(())
    }

    fn event(&mut self, emit: &mut impl FnMut(&str)) -> Result<()> {
        let data = std::mem::take(&mut self.data);
        if data.is_empty() {
            return Ok(());
        }
        if data == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        ensure!(!self.done, "ai: stream continued after completion");
        let event: Value = serde_json::from_str(&data).context("ai: malformed stream event")?;
        if let Some(error) = event.get("error") {
            bail!("ai: service refused request: {error}");
        }
        let choices = event["choices"]
            .as_array()
            .context("ai: stream has no choices")?;
        for choice in choices {
            if choice["index"].as_u64() != Some(0) {
                continue;
            }
            if let Some(reason) = choice["finish_reason"].as_str() {
                ensure!(
                    matches!(reason, "stop" | "tool_calls"),
                    "ai: incomplete response ({reason})"
                );
                self.finished = true;
            }
            let delta = &choice["delta"];
            if let Some(reasoning) = delta["reasoning_content"].as_str() {
                self.reasoning.push_str(reasoning);
            }
            if let Some(text) = delta["content"].as_str() {
                self.text.push_str(text);
                emit(text);
            }
            if let Some(calls) = delta["tool_calls"].as_array() {
                for item in calls {
                    let index = item["index"]
                        .as_u64()
                        .context("ai: tool call has no index")?;
                    ensure!(index < 16, "ai: too many tool calls");
                    let call = self.calls.entry(index).or_insert_with(|| Call {
                        id: String::new(),
                        kind: "function".into(),
                        function: Function::default(),
                    });
                    if let Some(id) = item["id"].as_str() {
                        call.id.push_str(id);
                    }
                    if let Some(kind) = item["type"].as_str() {
                        ensure!(kind == "function", "ai: unsupported tool type");
                    }
                    if let Some(name) = item["function"]["name"].as_str() {
                        call.function.name.push_str(name);
                    }
                    if let Some(arguments) = item["function"]["arguments"].as_str() {
                        call.function.arguments.push_str(arguments);
                    }
                    ensure!(
                        call.id.len() <= 256
                            && call.function.name.len() <= 64
                            && call.function.arguments.len() <= 65_536,
                        "ai: tool call exceeds its limit"
                    );
                }
            }
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<Answer> {
        ensure!(
            self.line.is_empty() && self.data.is_empty() && self.finished && self.done,
            "ai: response stream was interrupted before completion"
        );
        let mut ids = std::collections::HashSet::new();
        for call in self.calls.values() {
            ensure!(
                !call.id.is_empty() && ids.insert(&call.id) && !call.function.name.is_empty(),
                "ai: tool call has a missing or duplicate identity"
            );
            let arguments: Value = serde_json::from_str(&call.function.arguments)
                .context("ai: invalid tool arguments")?;
            ensure!(
                arguments.is_object(),
                "ai: tool arguments must be an object"
            );
        }
        Ok(Answer {
            text: std::mem::take(&mut self.text),
            reasoning: self.reasoning,
            calls: self.calls.into_values().collect(),
        })
    }
}

pub fn assistant(answer: &Answer) -> Value {
    let mut message = if answer.calls.is_empty() {
        json!({"role":"assistant", "content":answer.text})
    } else {
        json!({"role":"assistant", "content":answer.text, "tool_calls":answer.calls})
    };
    if !answer.reasoning.is_empty() {
        message["reasoning_content"] = json!(answer.reasoning);
    }
    message
}

pub fn cancel_pending(messages: &mut Vec<Value>) {
    let mut pending = BTreeMap::new();
    for message in messages.iter() {
        if let Some(calls) = message["tool_calls"].as_array() {
            for call in calls {
                if let Some(id) = call["id"].as_str() {
                    pending.insert(id.to_string(), ());
                }
            }
        }
        if message["role"] == "tool"
            && let Some(id) = message["tool_call_id"].as_str()
        {
            pending.remove(id);
        }
    }
    for (id, ()) in pending {
        messages.push(json!({"role":"tool","tool_call_id":id,"content":"The task was interrupted. This action was not completed or its outcome is unknown. Inspect current state before retrying. Do not automatically repeat it."}));
    }
}

pub fn history(messages: &[Value], tools: bool) -> Vec<Value> {
    messages.iter().map(|message| {
        if tools { return message.clone(); }
        if message["role"] == "tool" {
            json!({"role":"assistant","content":format!("Earlier untrusted tool result:\n{}",message["content"].as_str().unwrap_or_default())})
        } else if message.get("tool_calls").is_some() {
            json!({"role":"assistant","content":format!("{}\nEarlier tool requests: {}",message["content"].as_str().unwrap_or_default(),message["tool_calls"])})
        } else { message.clone() }
    }).collect()
}

#[cfg(test)]
mod tests;
