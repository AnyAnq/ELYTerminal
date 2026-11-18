use anyhow::{Context as _, Result};
use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    forms::{Choice, Input, Select, TextInput},
    theme::ControlSize,
};
use gpui::{
    AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div,
};

use super::{Cancel, config::Model, connection, credentials, word};
use crate::settings::SettingsStore;

pub struct Editor {
    selected: Option<String>,
    name: Entity<TextInput>,
    url: Entity<TextInput>,
    model: Entity<TextInput>,
    key: Entity<TextInput>,
    busy: bool,
    notice: String,
}

impl Editor {
    fn waiting(&mut self, busy: bool, cx: &mut Context<Self>) {
        self.busy = busy;
        for field in [&self.name, &self.url, &self.model, &self.key] {
            field.update(cx, |field, cx| field.set_disabled(busy, cx));
        }
        cx.notify();
    }
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| TextInput::new(window, cx));
        let url = cx.new(|cx| TextInput::new(window, cx));
        let model = cx.new(|cx| TextInput::new(window, cx));
        let key = cx.new(|cx| TextInput::new(window, cx).masked());
        let mut editor = Self {
            selected: None,
            name,
            url,
            model,
            key,
            busy: false,
            notice: String::new(),
        };
        if let Some(model) = SettingsStore::get(cx).ai.selected().cloned() {
            editor.load(Some(model), cx);
        }
        editor
    }

    fn load(&mut self, model: Option<Model>, cx: &mut Context<Self>) {
        self.selected = model.as_ref().map(|model| model.id.clone());
        self.name.update(cx, |field, cx| {
            field.set_text(model.as_ref().map_or("", |model| &model.name), cx)
        });
        self.url.update(cx, |field, cx| {
            field.set_text(model.as_ref().map_or("", |model| &model.url), cx)
        });
        self.model.update(cx, |field, cx| {
            field.set_text(model.as_ref().map_or("", |model| &model.model), cx)
        });
        self.key.update(cx, |field, cx| field.set_text("", cx));
        self.notice.clear();
        cx.notify();
    }

    fn draft(&self, cx: &Context<Self>) -> Result<Model> {
        let id = match &self.selected {
            Some(id) => id.clone(),
            None => format!(
                "{:016x}",
                getrandom::u64().context("ai: model id could not be generated")?
            ),
        };
        let mut draft = Model {
            id,
            name: self.name.read(cx).text().trim().into(),
            url: self.url.read(cx).text().trim().into(),
            model: self.model.read(cx).text().trim().into(),
            tools: false,
        };
        if let Some(saved) = SettingsStore::get(cx)
            .ai
            .models
            .iter()
            .find(|model| model.id == draft.id)
            && saved.url == draft.url
            && saved.model == draft.model
            && self.key.read(cx).text().is_empty()
        {
            draft.tools = saved.tools;
        }
        draft.validate()?;
        credentials::check(self.key.read(cx).text())?;
        Ok(draft)
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let draft = match self.draft(cx) {
            Ok(draft) => draft,
            Err(error) => {
                self.notice = format!("{error:#}");
                return cx.notify();
            }
        };
        let key = self.key.read(cx).text().to_string();
        self.waiting(true, cx);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result: Result<()> = async {
                let previous = if key.is_empty() {
                    None
                } else {
                    Some(credentials::read(&draft, cx).await?)
                };
                if !key.is_empty() {
                    cx.update(|cx| credentials::write(&draft, &key, cx)).await?;
                }
                let saved = cx.update(|cx| {
                    SettingsStore::update(cx, |settings| {
                        if let Some(model) = settings
                            .ai
                            .models
                            .iter_mut()
                            .find(|model| model.id == draft.id)
                        {
                            *model = draft.clone();
                        } else {
                            settings.ai.models.push(draft.clone());
                        }
                        settings.ai.selected = Some(draft.id.clone());
                    })
                });
                if let Err(error) = saved {
                    if let Some(previous) = previous {
                        cx.update(|cx| credentials::write(&draft, &previous, cx))
                            .await
                            .context("ai: credential rollback failed")?;
                    }
                    return Err(error);
                }
                Ok(())
            }
            .await;
            this.update(cx, |editor, cx| {
                editor.waiting(false, cx);
                match result {
                    Ok(()) => {
                        editor.load(Some(draft), cx);
                        editor.notice = word(cx, "et.ai.saved").to_string();
                        log::info!("ai: model configuration saved");
                    }
                    Err(error) => editor.notice = format!("{error:#}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn test(&mut self, cx: &mut Context<Self>) {
        let Some(model) = self
            .selected
            .as_ref()
            .and_then(|id| {
                SettingsStore::get(cx)
                    .ai
                    .models
                    .iter()
                    .find(|model| &model.id == id)
            })
            .cloned()
        else {
            return;
        };
        if !self.draft(cx).is_ok_and(|draft| draft == model) || !self.key.read(cx).text().is_empty()
        {
            self.notice = word(cx, "et.ai.save_first").to_string();
            return cx.notify();
        }
        self.waiting(true, cx);
        self.notice = word(cx, "et.ai.testing").to_string();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result: Result<bool> = async {
                let key = credentials::read(&model, cx).await?;
                let test_model = model.clone();
                cx.background_executor()
                    .spawn(async move { probe(&test_model, &key) })
                    .await
            }
            .await;
            this.update(cx, |editor, cx| {
                editor.waiting(false, cx);
                match result {
                    Ok(tools) => {
                        let saved = SettingsStore::update(cx, |settings| {
                            if let Some(current) = settings.ai.models.iter_mut().find(|each| {
                                each.id == model.id
                                    && each.url == model.url
                                    && each.model == model.model
                            }) {
                                current.tools = tools;
                            }
                        });
                        editor.notice = match saved {
                            Ok(()) => word(
                                cx,
                                if tools {
                                    "et.ai.tools_ready"
                                } else {
                                    "et.ai.chat_only"
                                },
                            )
                            .to_string(),
                            Err(error) => format!("{error:#}"),
                        };
                    }
                    Err(error) => editor.notice = format!("{error:#}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn remove(&mut self, cx: &mut Context<Self>) {
        let Some(model) = self
            .selected
            .as_ref()
            .and_then(|id| {
                SettingsStore::get(cx)
                    .ai
                    .models
                    .iter()
                    .find(|model| &model.id == id)
            })
            .cloned()
        else {
            return;
        };
        self.waiting(true, cx);
        cx.spawn(async move |this, cx| {
            let result: Result<()> = async {
                let previous = credentials::read(&model, cx).await?;
                credentials::erase(&model, cx).await?;
                let saved = cx.update(|cx| {
                    SettingsStore::update(cx, |settings| {
                        settings.ai.models.retain(|each| each.id != model.id);
                        if settings.ai.selected.as_ref() == Some(&model.id) {
                            settings.ai.selected =
                                settings.ai.models.first().map(|each| each.id.clone());
                        }
                    })
                });
                if saved.is_err() && !previous.is_empty() {
                    cx.update(|cx| credentials::write(&model, &previous, cx))
                        .await
                        .context("ai: deleted key could not be restored")?;
                }
                saved
            }
            .await;
            this.update(cx, |editor, cx| {
                editor.waiting(false, cx);
                match result {
                    Ok(()) => editor.load(None, cx),
                    Err(error) => editor.notice = format!("{error:#}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn clear_key(&mut self, cx: &mut Context<Self>) {
        let Some(model) = self
            .selected
            .as_ref()
            .and_then(|id| {
                SettingsStore::get(cx)
                    .ai
                    .models
                    .iter()
                    .find(|model| &model.id == id)
            })
            .cloned()
        else {
            return;
        };
        self.waiting(true, cx);
        cx.spawn(async move |this, cx| {
            let result = credentials::erase(&model, cx).await;
            this.update(cx, |editor, cx| {
                editor.waiting(false, cx);
                editor.key.update(cx, |field, cx| field.set_text("", cx));
                editor.notice = match result {
                    Ok(()) => word(cx, "et.ai.key_cleared").to_string(),
                    Err(error) => format!("{error:#}"),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

pub(super) fn probe(model: &Model, key: &str) -> Result<bool> {
    use serde_json::json;
    let cancel = Cancel::default();
    let mut messages = vec![json!({"role":"user","content":"Reply with OK."})];
    connection::complete(
        model,
        key,
        &connection::request(model, &messages, &[]),
        &cancel,
        |_| {},
    )?;
    let tools = vec![
        json!({"type":"function","function":{"name":"connection_probe","description":"A harmless local connection test. Call it with an empty object.","parameters":{"type":"object","properties":{},"additionalProperties":false}}}),
    ];
    messages = vec![
        json!({"role":"user","content":"Call connection_probe exactly once. After receiving its result, reply with OK."}),
    ];
    let request = connection::request(model, &messages, &tools);
    let answer = connection::complete(model, key, &request, &cancel, |_| {})
        .context("ai: chat connected, but the tool-call test failed")?;
    if answer.calls.len() != 1
        || answer.calls[0].function.name != "connection_probe"
        || serde_json::from_str::<serde_json::Value>(&answer.calls[0].function.arguments)?
            != json!({})
    {
        return Ok(false);
    }
    messages.push(super::protocol::assistant(&answer));
    messages.push(json!({"role":"tool","tool_call_id":answer.calls[0].id,"content":"OK"}));
    let follow = connection::complete(
        model,
        key,
        &connection::request(model, &messages, &tools),
        &cancel,
        |_| {},
    )?;
    Ok(follow.calls.is_empty() && !follow.text.is_empty())
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = &SettingsStore::get(cx).ai;
        let this = cx.entity();
        let pick = this.clone();
        let mut select = Select::new(
            "ai-config-model",
            settings
                .models
                .iter()
                .map(|model| Choice::new(model.id.clone(), model.name.clone())),
        )
        .label(word(cx, "et.ai.models"))
        .placeholder(word(cx, "et.ai.models"))
        .disabled(self.busy || settings.models.is_empty())
        .on_change(move |id, _, cx| {
            let model = SettingsStore::get(cx)
                .ai
                .models
                .iter()
                .find(|model| model.id == id.as_ref())
                .cloned();
            pick.update(cx, |editor, cx| editor.load(model, cx));
        });
        if let Some(id) = &self.selected {
            select = select.selected(id.clone());
        }
        let mut body = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(word(cx, "et.ai.settings_hint"))
            .child(select);
        for (label, field) in [
            ("et.ai.name", &self.name),
            ("et.ai.url", &self.url),
            ("et.ai.model", &self.model),
            ("et.ai.key", &self.key),
        ] {
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(word(cx, label))
                    .child(Input::new(field)),
            );
        }
        let add = this.clone();
        let save = this.clone();
        let test = this.clone();
        let remove = this.clone();
        let clear = this;
        body.child(word(cx, "et.ai.key_hint"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("ai-config-new", word(cx, "et.ai.new_model"))
                            .disabled(self.busy)
                            .on_click(move |_, _, cx| {
                                add.update(cx, |editor, cx| editor.load(None, cx))
                            }),
                    )
                    .child(
                        Button::new("ai-config-save", word(cx, "et.ai.save"))
                            .variant(ButtonVariant::Primary)
                            .disabled(self.busy)
                            .on_click(move |_, _, cx| {
                                save.update(cx, |editor, cx| editor.save(cx))
                            }),
                    )
                    .child(
                        Button::new("ai-config-test", word(cx, "et.ai.test"))
                            .disabled(self.busy || self.selected.is_none())
                            .on_click(move |_, _, cx| {
                                test.update(cx, |editor, cx| editor.test(cx))
                            }),
                    )
                    .child(
                        Button::new("ai-config-key-clear", word(cx, "et.ai.clear_key"))
                            .size(ControlSize::Sm)
                            .disabled(self.busy || self.selected.is_none())
                            .on_click(move |_, _, cx| {
                                clear.update(cx, |editor, cx| editor.clear_key(cx))
                            }),
                    )
                    .child(
                        Button::new("ai-config-remove", word(cx, "et.ai.delete_model"))
                            .disabled(self.busy || self.selected.is_none())
                            .on_click(move |_, _, cx| {
                                remove.update(cx, |editor, cx| editor.remove(cx))
                            }),
                    ),
            )
            .child(self.notice.clone())
    }
}
