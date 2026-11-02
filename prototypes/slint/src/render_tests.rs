//! Optional offscreen visual inspection. Uses a real software rasterizer, no desktop windows.
use super::*;
use slint::Model;
#[test]
fn render_workspace_views() {
    let Some(directory) = std::env::var_os("FOUNTAGE_LAB_RENDER_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("software".into()),
        },
    )))
    .unwrap();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    ui.set_source(SAMPLE.into());
    install_find(&ui);
    install_outline_navigation(&ui);
    ui.set_recovery_status("Recovery copy is current".into());
    let (summary, headings, lines) = derived(SAMPLE, 0);
    apply(&ui, summary, headings, lines, 0.4);
    ui.show().unwrap();
    let mut clean_dividers = None;
    let mut compact_dividers = None;
    for (name, width, height, dark, paper, close) in [
        (
            "shelf-visibility-hidden-light",
            1280,
            860,
            false,
            true,
            false,
        ),
        ("shelf-visibility-hidden-dark", 960, 640, true, true, false),
        ("shelf-capture-light", 1280, 860, false, true, false),
        ("shelf-capture-dark", 960, 640, true, true, false),
        ("shelf-insert-light", 1280, 860, false, true, false),
        ("shelf-replace-dark", 960, 640, true, true, false),
        ("shelf-notes-light", 1280, 860, false, true, false),
        ("shelf-notes-dark", 960, 640, true, true, false),
        ("shelf-notes-edit-dark", 960, 640, true, true, false),
        ("shelf-notes-delete-light", 1280, 860, false, true, false),
        ("shelf-snippets-light", 1280, 860, false, true, false),
        ("shelf-snippets-dark", 960, 640, true, true, false),
        ("shelf-snippets-edit-light", 1280, 860, false, true, false),
        ("shelf-snippets-delete-dark", 960, 640, true, true, false),
        ("shelf-search-light", 1280, 860, false, true, false),
        ("shelf-search-dark", 960, 640, true, true, false),
        ("shelf-search-empty", 960, 640, true, true, false),
        (
            "shelf-delete-checklist-light",
            1280,
            860,
            false,
            true,
            false,
        ),
        ("shelf-delete-checklist-dark", 960, 640, true, true, false),
        ("shelf-delete-checklist-error", 960, 640, true, true, false),
        ("shelf-remove-task-light", 1280, 860, false, true, false),
        ("shelf-remove-task-dark", 960, 640, true, true, false),
        ("shelf-remove-task-error", 960, 640, true, true, false),
        ("shelf-edit-task-light", 1280, 860, false, true, false),
        ("shelf-edit-task-dark", 960, 640, true, true, false),
        ("shelf-edit-task-error", 960, 640, true, true, false),
        ("shelf-add-task-light", 1280, 860, false, true, false),
        ("shelf-add-task-dark", 960, 640, true, true, false),
        ("shelf-add-task-error", 960, 640, true, true, false),
        ("shelf-form-light", 1280, 860, false, true, false),
        ("shelf-form-dark", 960, 640, true, true, false),
        ("shelf-form-error", 960, 640, true, true, false),
        ("shelf-pinned-light", 1280, 860, false, true, false),
        ("shelf-pinned-dark", 960, 640, true, true, false),
        ("shelf-light", 1280, 860, false, true, false),
        ("shelf-dark", 960, 640, true, true, false),
        ("shelf-empty", 960, 640, false, true, false),
        ("shelf-unsupported", 960, 640, true, true, false),
        ("title-page-light", 1280, 860, false, true, false),
        ("title-page-dark", 960, 640, true, true, false),
        ("title-page-error", 960, 640, false, true, false),
        ("scene-numbering-light", 1280, 860, false, true, false),
        ("scene-numbering-dark", 960, 640, true, true, false),
        ("scene-numbering-error", 960, 640, false, true, false),
        ("locations-keyboard-light", 1280, 860, false, true, false),
        ("locations-keyboard-dark", 960, 640, true, true, false),
        ("locations-light", 1280, 860, false, true, false),
        ("locations-dark", 960, 640, true, true, false),
        ("locations-empty", 960, 640, false, true, false),
        ("new-project-academic-light", 1280, 860, false, true, false),
        ("new-project-academic-dark", 960, 640, true, true, false),
        ("new-project-starter-light", 1280, 860, false, true, false),
        ("new-project-starter-dark", 960, 640, true, true, false),
        ("new-project-wide-light", 1280, 860, false, true, false),
        ("new-project-wide-dark", 1280, 860, true, true, false),
        ("new-project-comic", 1280, 860, true, true, false),
        ("new-project-keyboard-light", 960, 640, false, true, false),
        ("new-project-keyboard-dark", 960, 640, true, true, false),
        ("mac-header-light", 1280, 860, false, true, false),
        ("mac-header-dark", 1280, 860, true, true, false),
        ("mac-header-compact", 960, 640, true, false, false),
        ("focus-light", 1280, 860, false, true, false),
        ("focus-dark", 960, 640, true, false, false),
        ("formatted-focus", 1280, 860, true, true, false),
        ("preview-focus", 960, 640, true, true, false),
        ("composition-light", 1280, 860, false, true, false),
        ("composition-dark", 960, 640, true, false, false),
        ("formatted-unicode-light", 1280, 860, false, true, false),
        ("formatted-unicode-dark", 1280, 860, true, false, false),
        ("formatted-emphasis-light", 1280, 860, false, true, false),
        ("formatted-emphasis-dark", 1280, 860, true, false, false),
        ("formatted-light", 1280, 860, false, true, false),
        ("formatted-dark", 1280, 860, true, false, false),
        ("formatted-white-paper", 1280, 860, true, true, false),
        ("formatted-compact", 960, 640, true, true, false),
        ("light", 1280, 860, false, true, false),
        ("dark-white-paper", 1280, 860, true, true, false),
        ("dark-paper", 1280, 860, true, false, false),
        ("preview-styles-light", 1280, 860, false, true, false),
        ("preview-styles-dark", 1280, 860, true, false, false),
        ("preview-styles-compact", 960, 640, true, true, false),
        ("preview-light", 1280, 860, false, true, false),
        ("preview-dark", 1280, 860, true, false, false),
        ("preview-compact", 960, 640, true, true, false),
        ("compact", 960, 640, false, true, false),
        ("project-draft", 960, 640, false, true, false),
        ("edited", 1280, 860, true, true, false),
        ("replace-compact", 960, 640, true, true, false),
        ("find-compact", 960, 640, true, true, false),
        ("long-document-name", 960, 640, true, true, false),
        ("new-project-light", 960, 640, false, true, false),
        ("new-project-dark", 960, 640, true, true, false),
        ("startup-light", 960, 640, false, true, false),
        ("startup-dark", 960, 640, true, true, false),
        ("tasks-removal-light", 1280, 860, false, true, false),
        ("tasks-removal-dark", 960, 640, true, true, false),
        ("tasks-light", 1280, 860, false, true, false),
        ("tasks-dark", 960, 640, true, true, false),
        ("tasks-empty", 960, 640, false, true, false),
        ("notes-light", 1280, 860, false, true, false),
        ("notes-dark", 960, 640, true, true, false),
        ("notes-empty", 960, 640, true, true, false),
        ("notes-filter", 960, 640, true, true, false),
        ("notes-no-results", 960, 640, true, true, false),
        ("project-close", 960, 640, true, true, true),
        ("close-overlay", 960, 640, true, true, true),
    ] {
        ui.window()
            .set_size(slint::PhysicalSize::new(width, height));
        ui.set_integrated_titlebar(name.starts_with("mac-header"));
        ui.invoke_formatted_cancel();
        ui.set_formatted_active(name.starts_with("formatted-") || name.starts_with("composition-"));
        ui.set_focus_mode(name.contains("focus"));
        ui.set_preview_open(name.starts_with("preview-"));
        ui.set_project_mode(name == "project-draft" || name == "project-close");
        let startup = name.starts_with("startup");
        ui.set_source(if name.starts_with("tasks") { if name == "tasks-empty" { "" } else { "Title: The Next Draft\r\n\r\nINT. ROOM - DAY\r\n\r\n[ ] Give Zoë a stronger entrance. [[Private detail]]\r\n[x] Tighten the opening scene.\r\n[ ] Check the final image against the first page.\r\n[[\r\n[ ] Hidden task\r\n]]\r\n" } } else if name.starts_with("preview-styles") { "INT. CAFÉ - DAY\r\n\r\nMARA\r\nA **bold** choice, an *italic* thought, and an _underlined promise_.\r\nCombined ***bold italics*** and **_bold underline_**.\r\nLiteral \\*stars\\*, <u>raw</u>, &amp;, [x](url), 🌙 e\u{301}.\r\nThis is a **longer speech that wraps across the available width without losing its bold style**, while the next phrase returns to normal and the _last phrase stays underlined as it wraps_ across the reading area.\r\n\r\n[[Do not print this note.]]\r\n/* Omitted passage. */\r\n" } else if name.starts_with("formatted-unicode") { "INT. CAFÉ - DAY\r\n\r\nMARA\r\nZoë sees 🌙 👩🏽‍🚀 🏳️‍🌈 🇺🇸.\r\nالعربية · עברית · 世界 · e\u{301}\r\n" } else if name.starts_with("formatted-emphasis") { "INT. CAFÉ - DAY\r\n\r\nMARA\r\n**A bold choice.**\r\n*One quiet thought.*\r\n_An underlined promise._\r\n\r\nZoë watches the moon 🌙.\r\n" } else if startup || name == "locations-empty" { "" } else if name.starts_with("notes") && name != "notes-empty" {
            "Title: Notes QA\n\nINT. ROOM - DAY\n\nZoë watches the moon. [[Check this beat]]\n\n[[Second note\n  \nAdd a pause.]]\n"
        } else { SAMPLE }.into());
        let (summary, headings, lines) = derived(ui.get_source().as_str(), 0);
        ui.set_outline_source(ui.get_source());
        ui.set_outline_category(ui.get_category());
        apply(&ui, summary, headings, lines, 0.4);
        ui.set_navigation_mode(if name.starts_with("tasks") {
            3
        } else if name.starts_with("notes") {
            1
        } else if name.starts_with("locations-") {
            2
        } else {
            0
        });
        ui.set_note_query(
            if name == "notes-filter" {
                "pause"
            } else if name == "notes-no-results" {
                "missing"
            } else {
                ""
            }
            .into(),
        );
        ui.invoke_note_filter_changed();
        ui.set_document_name(if startup { "Untitled draft" } else if name == "long-document-name" {
            "The Last Signal — An extraordinarily long Café 世界 🌙 manuscript title with many words"
        } else if name == "project-draft" || name == "project-close" { "The Last Signal" } else { "Scratch draft" }.into());
        ui.set_dirty(name == "edited");
        ui.set_find_open(name == "find-compact" || name == "replace-compact");
        ui.set_replace_open(name == "replace-compact");
        ui.set_find_mode(if name == "replace-compact" { 2 } else { 0 });
        ui.set_find_regex(name == "replace-compact");
        if name == "find-compact" || name == "replace-compact" {
            ui.set_find_query("MARA".into());
            ui.set_replacement("NINA".into());
            ui.invoke_find_step(false);
        }
        ui.set_status(
            if startup {
                "Start a new project or open an existing draft."
            } else if name == "project-draft" {
                "Project draft · Save a Copy preserves metadata and attachments"
            } else if name == "edited" {
                "Scratch draft · Unsaved edits · Save a Copy to keep changes"
            } else {
                "Scratch draft · Save a copy to keep your draft"
            }
            .into(),
        );
        ui.set_dark(dark);
        ui.set_white_paper(paper);
        ui.set_confirm_close(close);
        ui.set_file_dialog_open(
            name.starts_with("new-project") || startup || name.starts_with("scene-numbering"),
        );
        if name.starts_with("title-page") {
            ui.set_file_dialog_open(true);
        }
        ui.set_title_page_ready(true);
        ui.set_title_page_title("The Last Signal".into());
        ui.set_title_page_credit("Written by".into());
        ui.set_title_page_author("Zoë\nBaz Studios".into());
        ui.set_title_page_contact("writer@example.invalid".into());
        ui.set_title_page_error(
            if name.ends_with("error") {
                "Use consecutive lines within each field. Blank lines end a Fountain title page."
            } else {
                ""
            }
            .into(),
        );
        ui.set_title_page_open(name.starts_with("title-page"));
        ui.set_numbering_open(name.starts_with("scene-numbering"));
        ui.set_task_removal_label("Give Zoë a stronger entrance.".into());
        ui.set_task_removal_open(name.starts_with("tasks-removal"));
        if name.starts_with("tasks-removal") {
            ui.set_file_dialog_open(true);
        }
        ui.set_numbering_selection_available(true);
        ui.set_numbering_selected_only(true);
        ui.set_numbering_start(if name.ends_with("error") { "0" } else { "42" }.into());
        ui.set_numbering_error(
            if name.ends_with("error") {
                "Enter a starting number from 1 to 4294967295."
            } else {
                ""
            }
            .into(),
        );
        ui.set_new_show_open_actions(startup);
        ui.set_new_draft_open(name.starts_with("new-project") || startup);
        ui.set_new_category_choice(if name == "new-project-comic" {
            1
        } else if name.starts_with("new-project-academic") {
            3
        } else {
            0
        });
        ui.set_new_include_starter(name.starts_with("new-project-starter"));
        ui.set_new_academic_style(if name.ends_with("dark") { 2 } else { 3 });
        if name.starts_with("composition-") {
            let _initial_layout = ui.window().take_snapshot().unwrap();
            let a = SAMPLE.find("transmission").unwrap();
            ui.invoke_select_source(a as i32, (a + "transmission".len()) as i32);
            ui.invoke_formatted_preedit("にほん".into(), -1, -1);
            assert_eq!(ui.get_source().as_str(), SAMPLE);
            assert!(ui.get_formatted_composing());
        }
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1000);
        let _settling_frame = ui.window().take_snapshot().unwrap();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1000);
        if name.starts_with("locations-keyboard") {
            ui.invoke_focus_navigation();
            for key in [slint::platform::Key::Home, slint::platform::Key::DownArrow] {
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: key.into() });
            }
            assert!(ui.get_navigator_has_focus());
            assert_eq!(ui.get_outline_selected(), 1);
        }
        if name.starts_with("new-project-keyboard") {
            // Reset focus even when consecutive frames both have the chooser open.
            ui.set_new_draft_open(false);
            ui.set_new_draft_open(true);
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                    text: slint::platform::Key::End.into(),
                });
            assert_eq!(ui.get_new_category_choice(), 7);
        }
        ui.set_shelf_open(name.starts_with("shelf-"));
        ui.set_shelf_draft_open(
            name.starts_with("shelf-form")
                || name.starts_with("shelf-add-task")
                || name.starts_with("shelf-edit-task")
                || name.starts_with("shelf-remove-task")
                || name.starts_with("shelf-delete-checklist"),
        );
        ui.set_shelf_draft_task(
            name.starts_with("shelf-add-task")
                || name.starts_with("shelf-edit-task")
                || name.starts_with("shelf-remove-task"),
        );
        ui.set_shelf_draft_task_edit(
            name.starts_with("shelf-edit-task") || name.starts_with("shelf-remove-task"),
        );
        ui.set_shelf_draft_removing(
            name.starts_with("shelf-remove-task") || name.starts_with("shelf-delete-checklist"),
        );
        ui.set_shelf_draft_task_count(2);
        ui.set_shelf_draft_removal_preview(
            if name.starts_with("shelf-delete-checklist") {
                "[ ] Review Zoë 🌙 [[private]]\r\n[x] Send draft\r\n"
            } else {
                "Review Zoë 🌙 [[private]]"
            }
            .into(),
        );
        ui.set_shelf_draft_target("Revision plan".into());
        ui.set_shelf_draft_title(
            if name.starts_with("shelf-edit-task") {
                "Review Zoë 🌙 [[private]]"
            } else if name.starts_with("shelf-add-task") {
                "Check the observatory scene"
            } else {
                "Revision plan"
            }
            .into(),
        );
        ui.set_shelf_draft_content(
            "[ ] Review the observatory scene\r\n[x] Send draft to Zoë 🌙\r\n".into(),
        );
        ui.set_shelf_draft_existing(
            name == "shelf-form-dark" || name.starts_with("shelf-delete-checklist"),
        );
        if name.starts_with("shelf-remove-task") || name.starts_with("shelf-delete-checklist") {
            ui.invoke_focus_shelf_draft_control(2);
        }
        if name.starts_with("shelf-") {
            ui.set_file_dialog_open(true);
            ui.set_shelf_editable(name != "shelf-unsupported");
            ui.set_shelf_can_undo(name != "shelf-empty");
            ui.set_shelf_status(
                if name.starts_with("shelf-pinned") {
                    "2 checklists · 1 open task"
                } else {
                    "1 checklist · 1 open task"
                }
                .into(),
            );
            ui.set_shelf_error(
                if name == "shelf-delete-checklist-error" {
                    "Recovery could not be written. The checklist has not been deleted."
                } else if name == "shelf-remove-task-error" {
                    "Recovery could not be written. The task has not been removed."
                } else if name == "shelf-edit-task-error" {
                    "The checklist changed. Your task draft is still here."
                } else if name == "shelf-add-task-error" {
                    "Enter a task on one line."
                } else if name == "shelf-form-error" {
                    "Recovery could not be written. Your checklist draft is still here."
                } else if name == "shelf-unsupported" {
                    "This Shelf uses an unsupported format. Its data was left unchanged."
                } else {
                    ""
                }
                .into(),
            );
            ui.set_shelf_rows(ModelRc::new(VecModel::from(
                if name == "shelf-empty" || name == "shelf-unsupported" {
                    vec![]
                } else {
                    let mut sample = vec![
                        ShelfRow {
                            item_id: "plan".into(),
                            title: "Revision plan".into(),
                            ..Default::default()
                        },
                        ShelfRow {
                            task: true,
                            text: "Review the observatory scene".into(),
                            can_move_down: true,
                            ..Default::default()
                        },
                        ShelfRow {
                            task: true,
                            completed: true,
                            text: "Send draft to Zoë".into(),
                            can_move_up: true,
                            ..Default::default()
                        },
                    ];
                    if name.starts_with("shelf-pinned") {
                        sample[0].title = "Observatory ending and final review".into();
                        sample[0].pinned = true;
                        sample[0].group_label = "PINNED".into();
                        sample.push(ShelfRow {
                            item_id: "later".into(),
                            title: "Later plan".into(),
                            group_label: "CHECKLISTS".into(),
                            ..Default::default()
                        });
                        sample.push(ShelfRow {
                            text: "No tasks in this checklist.".into(),
                            ..Default::default()
                        });
                    }
                    sample
                },
            )));
        }
        let item_kind = if name.starts_with("shelf-notes") {
            1
        } else if name.starts_with("shelf-snippets")
            || name == "shelf-insert-light"
            || name == "shelf-replace-dark"
        {
            2
        } else {
            0
        };
        ui.set_shelf_can_capture(name.starts_with("shelf-capture"));
        ui.set_shelf_replace_selection(
            name == "shelf-replace-dark" || name.starts_with("shelf-capture"),
        );
        ui.set_shelf_kind(item_kind);
        ui.set_shelf_draft_kind(item_kind);
        if item_kind != 0 {
            let title = if item_kind == 1 {
                "Observatory research"
            } else {
                "An alternate ending"
            };
            let content =
                "Zoë watches the observatory roof.\r\nKeep this possibility for the next draft. 🌙";
            ui.set_shelf_rows(ModelRc::new(VecModel::from(vec![
                ShelfRow {
                    item_id: "reference".into(),
                    can_insert: name == "shelf-insert-light" || name == "shelf-replace-dark",
                    title: title.into(),
                    pinned: true,
                    group_label: "PINNED".into(),
                    ..Default::default()
                },
                ShelfRow {
                    text: content.into(),
                    ..Default::default()
                },
            ])));
            ui.set_shelf_status(
                if item_kind == 1 {
                    "1 note"
                } else {
                    "1 snippet"
                }
                .into(),
            );
            ui.set_shelf_draft_title(title.into());
            ui.set_shelf_draft_target(title.into());
            ui.set_shelf_draft_content(content.into());
            ui.set_shelf_draft_removal_preview(content.into());
            ui.set_shelf_draft_existing(true);
            ui.set_shelf_draft_open(name.contains("-edit-") || name.contains("-delete-"));
            ui.set_shelf_draft_removing(name.contains("-delete-"));
            if name.contains("-delete-") {
                ui.invoke_focus_shelf_draft_control(2);
            }
        }
        ui.set_shelf_query(
            if name == "shelf-search-empty" {
                "unmatched shelf qa"
            } else if name.starts_with("shelf-search") {
                "observatory"
            } else {
                ""
            }
            .into(),
        );
        if name.starts_with("shelf-search") {
            ui.set_shelf_status(
                if name == "shelf-search-empty" {
                    "0 of 2 checklists · 0 open tasks"
                } else {
                    "1 of 2 checklists · 1 open task"
                }
                .into(),
            );
            if name == "shelf-search-empty" {
                ui.set_shelf_rows(ModelRc::new(VecModel::default()));
            }
            ui.invoke_focus_shelf_search();
        }
        ui.set_shelf_kind_visible(!name.starts_with("shelf-visibility-hidden"));
        if name.starts_with("shelf-visibility-hidden") {
            ui.set_shelf_rows(ModelRc::new(VecModel::default()));
            ui.set_shelf_status("checklists hidden · 2 stored".into());
        }
        let pixels = ui.window().take_snapshot().unwrap();
        assert_eq!((pixels.width(), pixels.height()), (width, height));
        if name.contains("focus") {
            let separator = if dark {
                (0x42, 0x42, 0x42)
            } else {
                (0xd0, 0xd0, 0xce)
            };
            assert!(
                (height - 60..height - 12).any(|y| {
                    (0..width).all(|x| {
                        let pixel = pixels.as_slice()[(y * width + x) as usize];
                        (pixel.r, pixel.g, pixel.b) == separator
                    })
                }),
                "{name}: collapsed sidebars must not push the footer below the viewport"
            );
        }
        let file = std::fs::File::create(directory.join(format!("{name}.png"))).unwrap();
        let mut encoder = png::Encoder::new(file, pixels.width(), pixels.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(pixels.as_bytes())
            .unwrap();
        if matches!(
            name,
            "light" | "dark-white-paper" | "dark-paper" | "preview-light" | "preview-dark"
        ) {
            let expected = if dark && !paper {
                (0x1e, 0x1e, 0x1e)
            } else {
                (0xff, 0xfd, 0xf7)
            };
            for x in [280, 800] {
                let pixel = pixels.as_slice()[((height - 100) * width + x) as usize];
                assert_eq!(
                    (pixel.r, pixel.g, pixel.b),
                    expected,
                    "{name}: active manuscript/preview paper must follow the selected appearance"
                );
            }
        }

        if name == "notes-filter" {
            assert_eq!(ui.get_visible_note_rows().row_count(), 1);
            assert_eq!(ui.get_note_filter_status().as_str(), "1 of 2 notes");
        }
        if name == "notes-no-results" {
            assert_eq!(ui.get_visible_note_rows().row_count(), 0);
            assert_eq!(ui.get_note_filter_status().as_str(), "0 of 2 notes");
        }
        if name == "find-compact" || name == "replace-compact" {
            assert_eq!(
                ui.get_find_status().as_str(),
                "1 of 1 matches",
                "deferred input-change callbacks must not erase a current result"
            );
        }
        if name == "dark-white-paper" || name == "edited" {
            let dividers = (0..width)
                .filter(|x| {
                    (110..height - 60).all(|y| {
                        let pixel = pixels.as_slice()[(y * width + x) as usize];
                        (pixel.r, pixel.g, pixel.b) == (0x42, 0x42, 0x42)
                    })
                })
                .collect::<Vec<_>>();
            assert_eq!(dividers.len(), 2, "only full-height pane dividers count");
            if name == "dark-white-paper" {
                clean_dividers = Some(dividers);
            } else {
                assert_eq!(
                    Some(dividers),
                    clean_dividers,
                    "Edited must not move panel dividers"
                );
            }
        }
        if name == "compact" || name == "long-document-name" {
            let expected = if dark {
                (0x42, 0x42, 0x42)
            } else {
                (0xd0, 0xd0, 0xce)
            };
            let dividers = (0..width)
                .filter(|x| {
                    (110..height - 60).all(|y| {
                        let pixel = pixels.as_slice()[(y * width + x) as usize];
                        (pixel.r, pixel.g, pixel.b) == expected
                    })
                })
                .collect::<Vec<_>>();
            assert_eq!(dividers.len(), 2, "paper borders are not pane dividers");
            if name == "compact" {
                compact_dividers = Some(dividers);
            } else {
                assert_eq!(
                    Some(dividers),
                    compact_dividers,
                    "long document names must not move the pane dividers"
                );
            }
        }
    }
    ui.set_confirm_close(false);
    ui.set_file_dialog_open(false);
    let source = ui.get_source();
    let dirty = ui.get_dirty();
    ui.set_preview_open(true);
    ui.invoke_select_source(0, 0);
    assert!(
        !ui.get_preview_open(),
        "navigation must reveal the editable manuscript"
    );
    assert_eq!(
        ui.get_source(),
        source,
        "switching views must preserve exact source"
    );
    assert_eq!(ui.get_dirty(), dirty, "switching views is not an edit");
    assert_eq!(ui.get_word_count().as_str(), "39");
    assert_eq!(ui.get_cast().row_count(), 2);
    ui.hide().unwrap();
}

slint::slint! {
    export component PreviewStyleProbe inherits Window {
        width: 400px; height: 90px; background: white;
        in property <styled-text> content;
        StyledText { x: 12px; y: 12px; width: 376px; height: 66px; text: root.content; default-font-family: "Courier"; default-font-size: 32px; default-color: black; }
    }
}
#[test]
fn styled_preview_raster_applies_bold_italic_and_underline_not_just_text_labels() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("software".into()),
        },
    )))
    .unwrap();
    let ui = PreviewStyleProbe::new().unwrap();
    ui.set_content(slint::StyledText::from_plain_text("MMMMMMMM"));
    ui.show().unwrap();
    let plain = ui.window().take_snapshot().unwrap();
    let ink_count = |pixels: &slint::SharedPixelBuffer<slint::Rgba8Pixel>| {
        pixels
            .as_slice()
            .iter()
            .filter(|p| p.r < 128 && p.g < 128 && p.b < 128)
            .count()
    };
    for (bold, italic, underline) in [
        (true, false, false),
        (false, true, false),
        (false, false, true),
        (true, true, true),
    ] {
        ui.set_content(
            slint::StyledText::from_spans(
                "MMMMMMMM",
                &[i_slint_core::styled_text::StyledTextSpan {
                    range: 0..8,
                    bold,
                    italic,
                    underline,
                }],
            )
            .unwrap(),
        );
        let styled = ui.window().take_snapshot().unwrap();
        let changed = plain
            .as_slice()
            .iter()
            .zip(styled.as_slice())
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            changed > 30,
            "style must change actual raster: {bold}/{italic}/{underline}"
        );
        if underline && !bold && !italic {
            assert!(ink_count(&styled) > ink_count(&plain) + 30);
        }
    }
    ui.hide().unwrap();
}
