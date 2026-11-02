mod document;
mod storage;

use iced::widget::{button, column, container, row, scrollable, space, text, text_editor};
use iced::{
    Element, Font,
    Length::{Fill, FillPortion},
    Subscription, Task, Theme, keyboard, window,
};
use std::path::PathBuf;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .window_size((1280., 840.))
        .exit_on_close_request(false)
        .run()
}

struct App {
    content: text_editor::Content,
    outline: document::Outline,
    path: Option<PathBuf>,
    saved: String,
    undo: Vec<(String, text_editor::Cursor)>,
    redo: Vec<(String, text_editor::Cursor)>,
    pending: Option<Intent>,
    busy: bool,
    status: String,
    focus: bool,
}
#[derive(Debug, Clone)]
enum Intent {
    New,
    Open,
    Close(window::Id),
}
#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    Request(Intent),
    Discard,
    Cancel,
    Save(bool),
    Opened(Result<Option<(PathBuf, String)>, String>),
    Saved(Result<Option<(PathBuf, String)>, String>),
    Jump(usize),
    Undo,
    Redo,
    Focus,
}
impl App {
    fn new() -> Self {
        let content =
            text_editor::Content::with_text(include_str!("../examples/first-light.fountain"));
        let saved = content.text();
        Self {
            outline: document::outline(&saved),
            content,
            path: None,
            saved,
            undo: vec![],
            redo: vec![],
            pending: None,
            busy: false,
            status: "FIRST LIGHT  ·  A sample screenplay. Make yourself at home.".into(),
            focus: false,
        }
    }
    fn dirty(&self) -> bool {
        self.content.text() != self.saved
    }
    fn name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or("Untitled.fountain".into())
    }
    fn title(&self) -> String {
        format!(
            "{}{} — Fountage by Baz Studios",
            self.name(),
            if self.dirty() { " •" } else { "" }
        )
    }
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            window::close_requests().map(|id| Message::Request(Intent::Close(id))),
            keyboard::listen().filter_map(|event| {
                if let keyboard::Event::KeyPressed { key, modifiers, .. } = event {
                    shortcut(&key, modifiers)
                } else {
                    None
                }
            }),
        ])
    }
    fn execute(&mut self, intent: Intent) -> Task<Message> {
        match intent {
            Intent::New => {
                self.replace(None, String::new());
                self.status =
                    "New screenplay. Start with a scene heading, such as INT. ROOM - DAY.".into();
                Task::none()
            }
            Intent::Close(id) => window::close(id),
            Intent::Open => {
                self.busy = true;
                Task::perform(
                    async {
                        let Some(file) = rfd::AsyncFileDialog::new()
                            .add_filter("Fountain screenplay", &["fountain", "txt"])
                            .pick_file()
                            .await
                        else {
                            return Ok(None);
                        };
                        let path = file.path().to_path_buf();
                        let bytes = file.read().await;
                        let source = String::from_utf8(bytes).map_err(|_| "This file is not UTF-8 text. Highland packages are not supported yet.".to_string())?;
                        Ok(Some((path, source)))
                    },
                    Message::Opened,
                )
            }
        }
    }
    fn replace(&mut self, path: Option<PathBuf>, source: String) {
        self.path = path;
        self.content = text_editor::Content::with_text(&source);
        self.saved = self.content.text();
        self.outline = document::outline(&self.saved);
        self.undo.clear();
        self.redo.clear();
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Opened(result) => {
                self.busy = false;
                match result {
                    Ok(Some((path, source))) => {
                        self.replace(Some(path), source);
                        self.status = "Screenplay opened.".into();
                    }
                    Ok(None) => {}
                    Err(e) => self.status = format!("Could not open: {e}"),
                }
            }
            Message::Saved(result) => {
                self.busy = false;
                match result {
                    Ok(Some((path, source))) => {
                        self.path = Some(path);
                        self.saved = source;
                        self.status = "Screenplay saved.".into();
                        if let Some(intent) = self.pending.take() {
                            return self.execute(intent);
                        }
                    }
                    Ok(None) => self.status = "Save cancelled. Your changes are still here.".into(),
                    Err(e) => self.status = format!("Could not save: {e}"),
                }
            }
            _ if self.busy => {}
            Message::Cancel => self.pending = None,
            Message::Discard => {
                if let Some(intent) = self.pending.take() {
                    return self.execute(intent);
                }
            }
            Message::Save(save_as) => {
                self.busy = true;
                let path = if save_as { None } else { self.path.clone() };
                let source = self.content.text();
                let name = self.name();
                return Task::perform(
                    async move {
                        let path = match path {
                            Some(p) => p,
                            None => {
                                let Some(file) = rfd::AsyncFileDialog::new()
                                    .set_file_name(&name)
                                    .add_filter("Fountain screenplay", &["fountain"])
                                    .save_file()
                                    .await
                                else {
                                    return Ok(None);
                                };
                                file.path().to_path_buf()
                            }
                        };
                        storage::save(&path, &source)?;
                        Ok(Some((path, source)))
                    },
                    Message::Saved,
                );
            }
            _ if self.pending.is_some() => {}
            Message::Request(intent) => {
                if self.dirty() {
                    self.pending = Some(intent);
                } else {
                    return self.execute(intent);
                }
            }
            Message::Edit(action) => {
                if action.is_edit() {
                    self.undo.push((self.content.text(), self.content.cursor()));
                    if self.undo.len() > 500 {
                        self.undo.remove(0);
                    }
                    self.redo.clear();
                }
                self.content.perform(action);
                self.outline = document::outline(&self.content.text());
            }
            Message::Jump(line) => {
                self.content.move_to(text_editor::Cursor {
                    position: text_editor::Position { line, column: 0 },
                    selection: None,
                });
                return iced::widget::operation::focus("screenplay");
            }
            Message::Undo | Message::Redo => {
                let (from, to) = if matches!(message, Message::Undo) {
                    (&mut self.undo, &mut self.redo)
                } else {
                    (&mut self.redo, &mut self.undo)
                };
                if let Some((source, cursor)) = from.pop() {
                    to.push((self.content.text(), self.content.cursor()));
                    self.content = text_editor::Content::with_text(&source);
                    self.content.move_to(cursor);
                    self.outline = document::outline(&source);
                }
            }
            Message::Focus => self.focus = !self.focus,
        }
        Task::none()
    }
    fn theme(&self) -> Theme {
        Theme::custom(
            "Fountage · Paper",
            iced::theme::Palette {
                background: rgb(244, 242, 237),
                text: rgb(42, 48, 44),
                primary: rgb(53, 79, 65),
                success: rgb(53, 79, 65),
                warning: rgb(168, 125, 64),
                danger: rgb(158, 69, 54),
            },
        )
    }
    fn view(&self) -> Element<'_, Message> {
        let muted = rgb(117, 120, 109);
        let ink = rgb(42, 48, 44);
        let accent = rgb(53, 79, 65);
        let serif = Font::with_name("Georgia");
        let toolbar = row![
            column![
                text("Fountage").font(serif).size(32).color(accent),
                text("by Baz Studios").size(11).color(muted)
            ]
            .spacing(3),
            space().width(32),
            button(text("New screenplay").size(13))
                .padding([10, 14])
                .style(quiet)
                .on_press(Message::Request(Intent::New)),
            button(text("Open…").size(13))
                .padding([10, 14])
                .style(quiet)
                .on_press(Message::Request(Intent::Open)),
            space().width(Fill),
            button(
                text(if self.focus {
                    "Leave focus"
                } else {
                    "Focus mode"
                })
                .size(13)
            )
            .padding([10, 14])
            .style(quiet)
            .on_press(Message::Focus),
            button(text("Save as…").size(13))
                .padding([10, 14])
                .style(quiet)
                .on_press(Message::Save(true)),
            button(text("Save screenplay").size(13))
                .padding([10, 20])
                .style(primary)
                .on_press(Message::Save(false))
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        let source = self.content.text();
        let title = source
            .lines()
            .find_map(|line| line.strip_prefix("Title:").map(str::trim))
            .filter(|title| !title.is_empty())
            .unwrap_or("Untitled screenplay");
        let editor = text_editor(&self.content)
            .id("screenplay")
            .font(Font::with_name("Menlo"))
            .size(17)
            .line_height(1.65)
            .padding(iced::Padding {
                top: 30.,
                right: 42.,
                bottom: 30.,
                left: 42.,
            })
            .height(Fill)
            .style(move |_, _| text_editor::Style {
                background: rgb(255, 254, 251).into(),
                border: iced::Border::default(),
                placeholder: muted,
                value: ink,
                selection: rgb(213, 225, 211),
            })
            .on_action(Message::Edit)
            .key_binding(|key| {
                if let Some(message) = shortcut(&key.key, key.modifiers) {
                    Some(text_editor::Binding::Custom(message))
                } else {
                    text_editor::Binding::from_key_press(key)
                }
            });
        let paper = container(editor)
            .padding(1)
            .height(Fill)
            .style(|_| container::Style {
                background: Some(rgb(255, 254, 251).into()),
                border: iced::Border {
                    color: rgb(222, 222, 212),
                    width: 1.,
                    radius: 4.into(),
                },
                shadow: iced::Shadow {
                    color: iced::Color::from_rgba(0.18, 0.21, 0.16, 0.06),
                    offset: iced::Vector::new(0., 5.),
                    blur_radius: 18.,
                },
                ..Default::default()
            });
        let manuscript = column![
            row![
                column![
                    text("THE MANUSCRIPT").size(10).color(muted),
                    text(title.to_owned()).font(serif).size(29)
                ]
                .spacing(8),
                space().width(Fill),
                text(if self.dirty() {
                    "●  Unsaved changes"
                } else {
                    "Fountain draft"
                })
                .size(11)
                .color(muted)
            ]
            .align_y(iced::Alignment::Center),
            paper
        ]
        .spacing(22);
        let active_scene = self
            .outline
            .scenes
            .iter()
            .rposition(|s| s.line <= self.content.cursor().position.line);
        let mut workspace = row![].spacing(28).height(Fill);
        if !self.focus {
            let mut scenes = column![
                text("STORY STRUCTURE").size(10).color(muted),
                row![
                    text("Scenes").font(serif).size(25),
                    space().width(Fill),
                    text(format!("{:02}", self.outline.scenes.len()))
                        .size(13)
                        .color(muted)
                ]
                .align_y(iced::Alignment::Center),
                space().height(12)
            ]
            .spacing(12);
            if self.outline.scenes.is_empty() {
                scenes = scenes.push(
                    text("Every story starts somewhere. Add your first scene heading to begin.")
                        .size(13)
                        .color(muted),
                );
            }
            for (i, scene) in self.outline.scenes.iter().enumerate() {
                let selected = active_scene == Some(i);
                scenes = scenes.push(
                    button(
                        column![
                            text(format!("SCENE {:02}", i + 1))
                                .size(10)
                                .color(if selected { accent } else { muted }),
                            text(&scene.heading).size(13),
                            text(format!("Line {}", scene.line + 1))
                                .size(11)
                                .color(muted)
                        ]
                        .spacing(9),
                    )
                    .width(Fill)
                    .padding(16)
                    .style(move |_, status| button::Style {
                        background: Some(
                            if selected {
                                rgb(229, 234, 223)
                            } else if matches!(status, button::Status::Hovered) {
                                rgb(235, 235, 226)
                            } else {
                                rgb(244, 242, 237)
                            }
                            .into(),
                        ),
                        text_color: ink,
                        border: iced::Border {
                            radius: 8.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .on_press(Message::Jump(scene.line)),
                );
            }
            workspace = workspace.push(container(scrollable(scenes)).width(210).height(Fill));
        }
        workspace = workspace.push(container(manuscript).width(FillPortion(5)).height(Fill));
        if !self.focus {
            let mut insight = column![
                text("IN THIS DRAFT").size(10).color(muted),
                text(self.outline.words.to_string())
                    .font(serif)
                    .size(40)
                    .color(accent),
                text("words on the page").size(12).color(muted),
                space().height(22),
                text("THE CAST").size(10).color(muted),
                text("Characters").font(serif).size(23),
                space().height(4)
            ]
            .spacing(12);
            for (name, count) in &self.outline.characters {
                insight = insight.push(
                    row![
                        text(name).size(12),
                        space().width(Fill),
                        text(format!("{} cues", count)).size(11).color(muted)
                    ]
                    .spacing(8),
                );
            }
            insight = insight
                .push(space().height(28))
                .push(text("Room for a story.").font(serif).size(20).color(accent))
                .push(
                    text("A quiet place to find the next line.")
                        .size(13)
                        .color(muted),
                );
            workspace = workspace.push(container(scrollable(insight)).width(155).height(Fill));
        }
        let cursor = self.content.cursor().position;
        let footer = row![
            text(if self.busy {
                "Working…"
            } else {
                &self.status
            })
            .size(11)
            .color(muted),
            space().width(Fill),
            text(format!(
                "LINE {}   /   COL {}",
                cursor.line + 1,
                cursor.column + 1
            ))
            .size(10)
            .color(muted)
        ]
        .spacing(16);
        let mut page = column![toolbar, space().height(14)].spacing(20);
        if self.pending.is_some() {
            page = page.push(
                container(
                    column![
                        text("Keep the words you’ve written.").font(serif).size(23),
                        text("Save your changes before continuing?").size(13),
                        row![
                            button("Save & continue")
                                .padding([10, 16])
                                .style(primary)
                                .on_press(Message::Save(false)),
                            button("Discard changes")
                                .padding([10, 16])
                                .style(quiet)
                                .on_press(Message::Discard),
                            button("Cancel")
                                .padding([10, 16])
                                .style(quiet)
                                .on_press(Message::Cancel)
                        ]
                        .spacing(12)
                    ]
                    .spacing(12),
                )
                .padding(20)
                .width(Fill)
                .style(container::bordered_box),
            );
        }
        page.push(workspace).push(footer).padding(28).into()
    }
}
fn rgb(r: u8, g: u8, b: u8) -> iced::Color {
    iced::Color::from_rgb8(r, g, b)
}
fn quiet(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then(|| rgb(230, 231, 222).into()),
        text_color: rgb(66, 78, 67),
        border: iced::Border {
            radius: 7.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
fn primary(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(
            if matches!(status, button::Status::Hovered) {
                rgb(69, 99, 81)
            } else {
                rgb(53, 79, 65)
            }
            .into(),
        ),
        text_color: rgb(253, 252, 246),
        border: iced::Border {
            radius: 7.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn shortcut(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Message> {
    if !modifiers.command() {
        return None;
    }
    match key.as_ref() {
        keyboard::Key::Character("s") => Some(Message::Save(modifiers.shift())),
        keyboard::Key::Character("o") => Some(Message::Request(Intent::Open)),
        keyboard::Key::Character("n") => Some(Message::Request(Intent::New)),
        keyboard::Key::Character("z") => Some(if modifiers.shift() {
            Message::Redo
        } else {
            Message::Undo
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dirty_new_requires_a_decision_and_cancel_keeps_text() {
        let mut app = App::new();
        let _ = app.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Insert('x'),
        )));
        let edited = app.content.text();
        assert!(app.dirty());
        let _ = app.update(Message::Request(Intent::New));
        assert!(app.pending.is_some());
        let _ = app.update(Message::Cancel);
        assert_eq!(app.content.text(), edited);
        assert!(app.pending.is_none());
        let _ = app.update(Message::Request(Intent::New));
        let _ = app.update(Message::Discard);
        assert!(app.content.text().trim().is_empty());
        assert!(!app.dirty());
    }
    #[test]
    fn undo_redo_preserves_document_and_dirty_state() {
        let mut app = App::new();
        let original = app.content.text();
        let _ = app.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Insert('é'),
        )));
        let edited = app.content.text();
        let _ = app.update(Message::Undo);
        assert_eq!(app.content.text(), original);
        assert!(!app.dirty());
        let _ = app.update(Message::Redo);
        assert_eq!(app.content.text(), edited);
        assert!(app.dirty());
    }
    #[test]
    fn cancelled_or_failed_save_never_discards_pending_changes() {
        let mut app = App::new();
        let _ = app.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Insert('x'),
        )));
        let edited = app.content.text();
        app.pending = Some(Intent::New);
        let _ = app.update(Message::Saved(Ok(None)));
        assert!(app.pending.is_some());
        assert_eq!(app.content.text(), edited);
        let _ = app.update(Message::Saved(Err("Disk full".into())));
        assert!(app.pending.is_some());
        assert!(app.dirty());
        assert_eq!(app.content.text(), edited);
    }
}
