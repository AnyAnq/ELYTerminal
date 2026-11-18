use std::{collections::HashMap, path::PathBuf};

use ely_gpui_component::forms::TextInput;
use futures::channel::oneshot;
use gpui::{
    App, AppContext as _, Context, Entity, EntityId, EventEmitter, Focusable, Subscription,
    WeakEntity, Window,
};
use serde_json::Value;

use super::{Active, Cancel, config::Model, protocol::Call, word};
use crate::{
    sessions::orb::Look,
    settings::SettingsStore,
    term::{TerminalEvent, TerminalView},
};

mod execute;
mod history;
mod message;
#[cfg(all(test, windows))]
mod native;
mod run;
mod text;
mod view;

#[derive(Clone)]
pub struct Target {
    pub terminal: WeakEntity<TerminalView>,
    pub id: EntityId,
    pub name: String,
    pub folder: PathBuf,
    pub shell: String,
    pub executable: bool,
}

pub enum Notice {
    Changed,
    Closed,
}

#[derive(Clone)]
enum Line {
    Message {
        user: bool,
        text: String,
        live: bool,
    },
    Tool {
        name: String,
        args: String,
        output: String,
        pending: bool,
        open: bool,
    },
    Note(String),
}

#[derive(Default)]
struct Conversation {
    messages: Vec<Value>,
    lines: Vec<Line>,
    draft: String,
    history: history::History,
    resume: Option<Continuation>,
}

struct Continuation {
    model: Model,
    folder: PathBuf,
}

struct Running {
    target: Target,
    model: Model,
    cancel: Cancel,
    count: usize,
    phase: String,
    approval: Option<(Call, oneshot::Sender<bool>)>,
}

pub struct Assistant {
    pub open: bool,
    target: Option<Target>,
    conversations: HashMap<EntityId, Conversation>,
    input: Entity<TextInput>,
    preview: String,
    preview_open: bool,
    running: Option<Running>,
    failed: bool,
    _terminal: Option<Subscription>,
    _settings: Subscription,
    _language: Subscription,
}

impl EventEmitter<Notice> for Assistant {}

impl Drop for Assistant {
    fn drop(&mut self) {
        if let Some(running) = &self.running {
            running.cancel.stop();
        }
    }
}

impl Assistant {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .multi_line(2, 6)
                .placeholder(word(cx, "et.ai.placeholder"))
        });
        Self {
            open: false,
            target: None,
            conversations: HashMap::new(),
            input,
            preview: String::new(),
            preview_open: false,
            running: None,
            failed: false,
            _terminal: None,
            _settings: cx.observe_global::<SettingsStore>(|assistant, cx| {
                if SettingsStore::get(cx).ai.auto_approve {
                    assistant.decide(true, cx);
                }
                cx.notify();
            }),
            _language: cx.observe_global::<ely_gpui_component::i18n::I18n>(|assistant, cx| {
                assistant.input.update(cx, |input, cx| {
                    input.set_placeholder(word(cx, "et.ai.placeholder"), cx)
                });
                cx.notify();
            }),
        }
    }

    pub fn show(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(previous) = &self.target {
            self.conversations.entry(previous.id).or_default().draft =
                self.input.read(cx).text().into();
        }
        let target = if self.running.is_some() {
            self.target.clone().expect("running assistant has a target")
        } else {
            target
        };
        let conversation = self.conversations.entry(target.id).or_default();
        self.input.update(cx, |field, cx| {
            field.set_text(conversation.draft.clone(), cx);
            field.set_placeholder(word(cx, "et.ai.placeholder"), cx);
        });
        self.preview = target
            .terminal
            .read_with(cx, |terminal, _| terminal.ai_context())
            .unwrap_or_else(|error| format!("ai: terminal context could not read: {error}"));
        self._terminal = target.terminal.upgrade().map(|terminal| {
            cx.subscribe(&terminal, |this, _, event, cx| {
                if matches!(event, TerminalEvent::Taken | TerminalEvent::Exited(_)) {
                    this.stop(cx);
                }
                if let TerminalEvent::Folder(folder) = event
                    && let Some(target) = &mut this.target
                {
                    target.folder.clone_from(folder);
                }
            })
        });
        self.target = Some(target);
        self.open = true;
        window.focus(&self.input.focus_handle(cx), cx);
        cx.notify();
    }

    pub fn hide(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        cx.emit(Notice::Closed);
        cx.notify();
    }

    pub fn pane_closed(&mut self, id: EntityId, cx: &mut Context<Self>) {
        if self.conversations.remove(&id).is_some() {
            log::debug!("ai: conversation cleared for closed pane {id}");
        }
        if self.running.as_ref().is_some_and(|run| run.target.id == id) {
            self.stop(cx);
        }
        if self.target.as_ref().is_some_and(|target| target.id == id) {
            self.target = None;
            self._terminal = None;
            self.preview = String::new();
            self.preview_open = false;
            self.failed = false;
            self.input.update(cx, |input, cx| input.set_text("", cx));
            if self.open {
                self.hide(cx);
            } else {
                cx.emit(Notice::Changed);
                cx.notify();
            }
        }
    }

    pub fn look(&self) -> Option<Look> {
        if let Some(running) = &self.running {
            Some(if running.approval.is_some() {
                Look::Waiting
            } else {
                Look::Working(None)
            })
        } else if self.failed {
            Some(Look::Error)
        } else {
            None
        }
    }

    fn conversation(&self) -> Option<&Conversation> {
        self.target
            .as_ref()
            .and_then(|target| self.conversations.get(&target.id))
    }

    fn note(&mut self, note: String, cx: &mut Context<Self>) {
        if let Some(target) = &self.target {
            self.conversations
                .entry(target.id)
                .or_default()
                .lines
                .push(Line::Note(note));
        }
        cx.notify();
    }

    fn current(&self, cancel: &Cancel) -> bool {
        self.running
            .as_ref()
            .is_some_and(|running| running.cancel.same(cancel))
    }

    fn release(cancel: &Cancel, cx: &mut App) {
        if cx
            .global::<Active>()
            .0
            .as_ref()
            .is_some_and(|active| active.same(cancel))
        {
            cx.global_mut::<Active>().0 = None;
        }
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        let Some(running) = self.running.take() else {
            return;
        };
        running.cancel.stop();
        if let Err(error) = running
            .target
            .terminal
            .update(cx, |terminal, cx| terminal.ai_stop(cx))
        {
            log::debug!("ai: stopped task lost its terminal: {error}");
        }
        Self::release(&running.cancel, cx);
        if let Some(conversation) = self.conversations.get_mut(&running.target.id) {
            super::protocol::cancel_pending(&mut conversation.messages);
            conversation.resume = None;
            for line in &mut conversation.lines {
                match line {
                    Line::Message { live, .. } => *live = false,
                    Line::Tool { pending, .. } => *pending = false,
                    Line::Note(_) => {}
                }
            }
            conversation
                .lines
                .push(Line::Note(word(cx, "et.ai.stopped").to_string()));
        }
        log::info!("ai: task stopped on {}", running.target.id);
        cx.emit(Notice::Changed);
        cx.notify();
    }

    fn decide(&mut self, allowed: bool, cx: &mut Context<Self>) {
        if let Some(running) = &mut self.running
            && let Some((_, answer)) = running.approval.take()
        {
            answer.send(allowed).ok();
            running.phase = word(cx, "et.ai.working").to_string();
            cx.emit(Notice::Changed);
            cx.notify();
        }
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        if self.running.is_some() {
            return;
        }
        if let Some(target) = &self.target {
            self.conversations
                .insert(target.id, Conversation::default());
        }
        self.failed = false;
        self.input.update(cx, |input, cx| input.set_text("", cx));
        cx.emit(Notice::Changed);
        cx.notify();
    }
}
