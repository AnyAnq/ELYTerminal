use std::{
    borrow::Cow,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Result, ensure};
use ely_gpui_component::theme::ActiveTheme;
use gpui::{
    App, AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement, Render, Styled,
    Window, WindowBounds, WindowOptions, div, px, size,
};

mod approval;
mod editing;
mod focus;
mod lifecycle;
mod model;
mod performance;
mod settings;
mod settings_performance;

use super::{Assistant, Target};
use crate::{
    ai::{config, fixture},
    settings::{Settings, SettingsStore},
    term::{
        TerminalView,
        shell::{self, Start},
    },
};

struct Harness {
    terminal: Entity<TerminalView>,
    assistant: Entity<Assistant>,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::interface::dress(div(), cx)
            .size_full()
            .relative()
            .bg(cx.theme().colors.bg)
            .text_color(cx.theme().colors.fg)
            .child(self.terminal.clone())
            .child(
                div()
                    .absolute()
                    .top_4()
                    .bottom_4()
                    .right_4()
                    .w(gpui::rems(38.))
                    .max_w_full()
                    .child(self.assistant.clone()),
            )
    }
}

async fn until(
    window: gpui::WindowHandle<Harness>,
    cx: &mut gpui::AsyncApp,
    predicate: impl Fn(&Assistant) -> bool,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if window.read_with(cx, |harness, cx| predicate(harness.assistant.read(cx)))? {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "assistant did not reach the expected state"
        );
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
    }
}

fn bootstrap(root: &std::path::Path, model: config::Model, cx: &mut App) -> Result<()> {
    ely_gpui_component::init(cx)?;
    cx.text_system().add_fonts(vec![
        Cow::Borrowed(include_bytes!(
            "../../../assets/fonts/JetBrainsMono-Bold.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../../../assets/fonts/JetBrainsMono-Italic.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../../../assets/fonts/JetBrainsMono-BoldItalic.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../../../assets/fonts/JetBrainsMonoNerdFontMono-Regular.ttf"
        )),
    ])?;
    let settings = Settings {
        check_updates: false,
        ai: config::Settings {
            selected: Some(model.id.clone()),
            models: vec![
                model.clone(),
                config::Model {
                    id: format!("{}-alternate", model.id),
                    name: "Fixture alternative".into(),
                    ..model
                },
            ],
            ..Default::default()
        },
        ..Settings::default()
    };
    SettingsStore::fixture(settings, root.join("settings.json"), cx)?;
    crate::i18n::init(cx);
    crate::ai::init(cx);
    crate::interface::init(cx);
    crate::themes::apply(cx);
    cx.set_global(crate::term::zoom::Enlarged::default());
    cx.set_global(crate::updates::Updates::Unchecked);
    shell::install(&root.join("integration"))?;
    Ok(())
}

#[test]
#[ignore = "requires an interactive Windows desktop and GPU"]
fn native_assistant_repairs_a_project_and_manual_input_stops_it() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("ai-native-{:x}", getrandom::u64().unwrap()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir(root.join("nested")).unwrap();
    std::fs::write(root.join("nested/next-folder.txt"), "next task folder").unwrap();
    std::fs::write(root.join("must-remain.txt"), "fixture").unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='ai-verification-fixture'\nversion='0.0.0'\nedition='2024'\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn works() -> bool { false }\n#[test] fn verify() { assert!(works()); }\n",
    )
    .unwrap();
    let (listener, mut model) = fixture::local();
    model.tools = true;
    let shell = std::env::var("ELY_AI_TEST_SHELL").unwrap_or_else(|_| "powershell.exe".into());
    assert!(["powershell.exe", "pwsh.exe", "cmd.exe"].contains(&shell.as_str()));
    let cmd = shell == "cmd.exe";
    let server = std::thread::spawn(move || model::serve(listener, cmd));
    let outcome = Arc::new(Mutex::new(None));
    let done = outcome.clone();
    let project = root.clone();
    gpui_platform::application()
        .with_assets(ely_gpui_component::Assets)
        .run(move |cx| {
            let started = (|| -> Result<_> {
                bootstrap(&project, model, cx)?;
                let mut launch = shell::launch(
                    &shell,
                    &[],
                    Start::In(project.clone()),
                    &project.join("integration"),
                )?;
                if !cmd {
                    launch
                        .program
                        .as_mut()
                        .unwrap()
                        .1
                        .insert(0, "-NoProfile".into());
                }
                let (pty, events) = crate::term::spawn(
                    &launch,
                    (120, 32),
                    (8, 16),
                    crate::term::config(SettingsStore::get(cx)),
                )?;
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                            None,
                            size(px(1000.), px(760.)),
                            cx,
                        ))),
                        focus: false,
                        ..Default::default()
                    },
                    |window, cx| {
                        cx.new(|cx| {
                            let terminal = cx.new(|cx| TerminalView::new(pty, events, window, cx));
                            let assistant = cx.new(|cx| Assistant::new(window, cx));
                            Harness {
                                terminal,
                                assistant,
                            }
                        })
                    },
                )
            })();
            let window = match started {
                Ok(window) => window,
                Err(error) => {
                    *done.lock().unwrap() = Some(Err(error));
                    cx.quit();
                    return;
                }
            };
            cx.spawn(async move |cx| {
                let result: Result<()> = async {
                    let target = window.update(cx, |harness, _, _| Target {
                        terminal: harness.terminal.downgrade(),
                        id: harness.terminal.entity_id(),
                        name: "Isolated verification".into(),
                        folder: project.clone(),
                        shell: shell.clone(),
                        executable: true,
                    })?;
                    let ready = Instant::now() + Duration::from_secs(10);
                    loop {
                        if window
                            .update(cx, |harness, _, cx| harness.terminal.read(cx).ai_ready())?
                        {
                            break;
                        }
                        ensure!(Instant::now() < ready, "shell did not become ready");
                        cx.background_executor()
                            .timer(Duration::from_millis(50))
                            .await;
                    }
                    lifecycle::prepare_draft(window, cmd, cx).await?;
                    window.update(cx, |harness, window, cx| {
                        harness.assistant.update(cx, |assistant, cx| {
                            assistant.show(target, window, cx);
                        })
                    })?;
                    focus::verify(window, cx)?;
                    if std::env::var_os("ELY_AI_BENCH").is_some() {
                        performance::measure(window, cx).await?;
                    }
                    window.update(cx, |harness, _, cx| {
                        harness.assistant.update(cx, |assistant, cx| {
                            assistant.input.update(cx, |input, cx| {
                                input.set_text("Fix the failing test and verify the result.", cx)
                            });
                            assistant.start(false, cx);
                        })
                    })?;
                    let deadline = Instant::now() + Duration::from_secs(45);
                    loop {
                        let finished = window.update(cx, |harness, _, cx| {
                            harness.assistant.read(cx).running.is_none()
                        })?;
                        if finished {
                            break;
                        }
                        ensure!(Instant::now() < deadline, "native assistant did not finish");
                        cx.background_executor()
                            .timer(Duration::from_millis(100))
                            .await;
                    }
                    window.update(cx, |harness, _, cx| -> Result<()> {
                        ensure!(
                            !harness.assistant.read(cx).failed,
                            "assistant failed: {:?}",
                            harness
                                .assistant
                                .read(cx)
                                .conversation()
                                .map(|c| c.messages.clone())
                        );
                        ensure!(
                            std::fs::read_to_string(project.join("src/lib.rs"))?
                                .contains("{ true }"),
                            "project was not repaired"
                        );
                        ensure!(
                            !project.join("draft-was-executed").exists(),
                            "unfinished input was executed"
                        );
                        Ok(())
                    })??;
                    cx.update_window(window.into(), |_, window, cx| -> Result<()> {
                        window.draw(cx).clear(cx);
                        window
                            .render_to_image()?
                            .save(project.join("assistant.png"))?;
                        Ok(())
                    })??;
                    window.update(cx, |harness, _, cx| {
                        harness.assistant.update(cx, |assistant, cx| {
                            assistant.input.update(cx, |input, cx| {
                                input.set_text("Test manual interruption.", cx)
                            });
                            assistant.start(false, cx);
                        });
                    })?;
                    loop {
                        if window.update(cx, |harness, _, cx| {
                            harness
                                .assistant
                                .read(cx)
                                .running
                                .as_ref()
                                .is_some_and(|run| run.approval.is_some())
                        })? {
                            break;
                        }
                        ensure!(Instant::now() < deadline, "approval was not shown");
                        cx.background_executor()
                            .timer(Duration::from_millis(100))
                            .await;
                    }
                    cx.update_window(window.into(), |_, window, cx| -> Result<()> {
                        window.draw(cx).clear(cx);
                        window
                            .render_to_image()?
                            .save(project.join("approval.png"))?;
                        Ok(())
                    })??;
                    window.update(cx, |harness, _, cx| {
                        harness
                            .assistant
                            .update(cx, |assistant, cx| assistant.decide(true, cx));
                    })?;
                    loop {
                        if window.update(cx, |harness, _, cx| {
                            harness
                                .terminal
                                .read(cx)
                                .tail(Some(100))
                                .lines()
                                .any(|line| line.trim() == "AI_SLEEP_STARTED")
                        })? {
                            break;
                        }
                        ensure!(Instant::now() < deadline, "approved command did not start");
                        cx.background_executor()
                            .timer(Duration::from_millis(100))
                            .await;
                    }
                    window.update(cx, |harness, _, cx| {
                        harness.terminal.update(cx, |terminal, cx| {
                            terminal.typed(
                                if cmd {
                                    "echo manual-^takeover"
                                } else {
                                    "Write-Output ('manual'+'-takeover')"
                                },
                                cx,
                            )
                        })
                    })?;
                    let interrupted = Instant::now() + Duration::from_secs(5);
                    loop {
                        let (stopped, text) = window.update(cx, |harness, _, cx| {
                            (
                                harness.assistant.read(cx).running.is_none(),
                                harness.terminal.read(cx).tail(Some(100)),
                            )
                        })?;
                        if stopped && text.contains("manual-takeover") {
                            break;
                        }
                        ensure!(
                            Instant::now() < interrupted,
                            "manual takeover did not finish: {text}"
                        );
                        cx.background_executor()
                            .timer(Duration::from_millis(100))
                            .await;
                    }
                    cx.update_window(window.into(), |_, window, cx| {
                        window.draw(cx).clear(cx);
                        window
                            .render_to_image()?
                            .save(project.join("stopped.png"))?;
                        Ok::<_, anyhow::Error>(())
                    })??;
                    window.update(cx, |harness, _, cx| {
                        harness.assistant.update(cx, |assistant, cx| {
                            assistant.input.update(cx, |input, cx| {
                                input.set_text("Exercise the call limit.", cx)
                            });
                            assistant.start(false, cx);
                        });
                    })?;
                    until(window, cx, |assistant| assistant.running.is_none()).await?;
                    window.update(cx, |harness, _, cx| -> Result<()> {
                        ensure!(
                            harness
                                .assistant
                                .read(cx)
                                .conversation()
                                .is_some_and(|conversation| conversation.resume.is_some()),
                            "30 calls did not pause the task"
                        );
                        harness
                            .assistant
                            .update(cx, |assistant, cx| assistant.start(true, cx));
                        Ok(())
                    })??;
                    until(window, cx, |assistant| assistant.running.is_none()).await?;
                    window.update(cx, |harness, _, cx| -> Result<()> {
                        ensure!(!harness.assistant.read(cx).failed, "continued task failed");
                        ensure!(
                            harness
                                .assistant
                                .read(cx)
                                .conversation()
                                .is_some_and(|conversation| conversation.resume.is_none()),
                            "continued task did not finish"
                        );
                        harness.assistant.update(cx, |assistant, cx| {
                            assistant.input.update(cx, |input, cx| {
                                input.set_text("Exercise declined approval.", cx)
                            });
                            assistant.start(false, cx);
                        });
                        Ok(())
                    })??;
                    until(window, cx, |assistant| {
                        assistant
                            .running
                            .as_ref()
                            .is_some_and(|run| run.approval.is_some())
                    })
                    .await?;
                    window.update(cx, |harness, _, cx| {
                        harness
                            .assistant
                            .update(cx, |assistant, cx| assistant.decide(false, cx))
                    })?;
                    until(window, cx, |assistant| assistant.running.is_none()).await?;
                    ensure!(
                        project.join("must-remain.txt").is_file(),
                        "declined command was executed"
                    );
                    approval::verify(window, &project, cx).await?;
                    editing::verify(window, &project, cx).await?;
                    settings::verify(&project, cx).await?;
                    cx.update_window(window.into(), |_, window, cx| -> Result<()> {
                        window.draw(cx).clear(cx);
                        window
                            .render_to_image()?
                            .save(project.join("assistant-zh.png"))?;
                        Ok(())
                    })??;
                    lifecycle::verify_close(window, &project, cx).await?;
                    Ok(())
                }
                .await;
                *done.lock().unwrap() = Some(result);
                window
                    .update(cx, |_, window, _| window.remove_window())
                    .ok();
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    let result = outcome
        .lock()
        .unwrap()
        .take()
        .expect("native verification completed");
    if result.is_ok() {
        server.join().unwrap();
    }
    result.unwrap();
    eprintln!("Native assistant verification images: {}", root.display());
}
