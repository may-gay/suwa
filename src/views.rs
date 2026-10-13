use crate::ui::{self, button, checkbox, pick_list, slider, text_input};
use crate::{
    app::{App, Message, Screen, SettingsPage},
    domain::*,
    reader::Reader,
};
use iced::widget::{Column, Row, Space, column, container, image, row, scrollable, text};
use iced::{Alignment, Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    if app.screen == Screen::Reader {
        return reader_view(app);
    }
    let server_panel = if app.screen == Screen::Manage {
        Some(app.manager.panel)
    } else {
        None
    };
    let mut nav = column![ui::heading("suwa").size(24), Space::new().height(20)].spacing(6);
    for (label, message, active) in [
        (
            "Library",
            Message::Go(Screen::Library),
            matches!(app.screen, Screen::Library | Screen::Series),
        ),
        (
            "Browse",
            Message::Go(Screen::Browse),
            matches!(app.screen, Screen::Browse | Screen::SourceOptions),
        ),
        (
            "Updates",
            Message::Manage(crate::manage::Msg::Open(crate::manage::Panel::Updates)),
            matches!(
                server_panel,
                Some(crate::manage::Panel::Updates | crate::manage::Panel::History)
            ),
        ),
        (
            "Downloads",
            Message::Go(Screen::Downloads),
            server_panel == Some(crate::manage::Panel::ServerDownloads),
        ),
    ] {
        nav = nav.push(
            button(text(label).size(14))
                .on_press(message)
                .width(Length::Fill)
                .padding([12, 14])
                .style(if active { ui::navigation } else { ui::ghost }),
        );
    }
    nav = nav
        .push(Space::new().height(Length::Fill))
        .push(
            button("Settings")
                .on_press(Message::OpenSettings(SettingsPage::Main))
                .width(Length::Fill)
                .padding([12, 14])
                .style(
                    if matches!(
                        app.screen,
                        Screen::Settings
                            | Screen::Categories
                            | Screen::Connection
                            | Screen::Backups
                            | Screen::Tracking
                    ) || matches!(server_panel, Some(crate::manage::Panel::Extensions))
                    {
                        ui::navigation
                    } else {
                        ui::ghost
                    },
                ),
        )
        .push(
            container(ui::caption(if app.connected {
                "Connected"
            } else {
                "Offline"
            }))
            .padding([8, 14]),
        );
    let content = match app.screen {
        Screen::Manage | Screen::Downloads => app.manager.view().map(Message::Manage),
        Screen::SourceOptions => app.source_options.view().map(Message::SourceOptions),
        Screen::Tracking => app.tracking.view().map(Message::Tracking),
        Screen::Backups => app.backups.view().map(Message::Backup),
        Screen::Connection => column![settings_back(), connection(app)].spacing(20).into(),
        Screen::Library | Screen::Browse => library(app),
        Screen::Series => series(app),
        Screen::Categories => column![settings_back(), categories(app)].spacing(20).into(),
        Screen::Settings => settings(app),
        _ => column![].into(),
    };
    let mut main = column![container(content).width(Length::Fill).height(Length::Fill)]
        .spacing(12)
        .width(Length::Fill);
    if !app.status.is_empty()
        && !app.status.starts_with("Connected to ")
        && !app.status.starts_with("Connect your Suwayomi")
    {
        main = main.push(
            container(
                row![
                    ui::caption(&app.status).width(Length::Fill),
                    button("Dismiss")
                        .on_press(Message::DismissStatus)
                        .style(ui::ghost)
                        .padding([4, 8])
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding([8, 14])
            .style(ui::inset),
        );
    }
    container(
        row![
            container(nav)
                .width(168)
                .padding([24, 12])
                .height(Length::Fill),
            container(main)
                .padding(24)
                .width(Length::Fill)
                .height(Length::Fill)
        ]
        .spacing(8),
    )
    .height(Length::Fill)
    .into()
}

fn settings_back<'a>() -> Element<'a, Message> {
    button("‹ Settings")
        .on_press(Message::OpenSettings(SettingsPage::Main))
        .style(ui::ghost)
        .padding([6, 0])
        .into()
}

fn connection(app: &App) -> Element<'_, Message> {
    let mut body = column![
        ui::heading("Connection").size(32),
        ui::caption("Connect your Suwayomi library. Make yourself at home."),
        text("Server address"),
        text_input("http://localhost:4567", &app.settings.server)
            .on_input(Message::ServerText)
            .padding(12),
        pick_list(AuthMode::ALL, Some(app.settings.auth), Message::Auth)
    ]
    .spacing(14)
    .max_width(640);
    if app.settings.auth != AuthMode::None {
        body = body
            .push(
                text_input("Username", &app.settings.username)
                    .on_input(Message::Username)
                    .padding(12),
            )
            .push(
                text_input("Password", &app.password)
                    .secure(true)
                    .on_input(Message::Password)
                    .on_submit(Message::Connect)
                    .padding(12),
            );
    }
    body = body
        .push(text_input("Optional HTTP header: Name: value", &app.headers).secure(true).on_input(Message::Headers).padding(12))
        .push(checkbox(app.settings.remember_login)
            .label("Remember sign-in on this device")
            .on_toggle_maybe((!app.busy && !app.credential_busy).then_some(Message::RememberLogin)))
        .push(ui::caption("Saved in your Linux keyring after connecting. Reconnects automatically next time. Uncheck to forget this account."));
    if !app.credential_status.is_empty() {
        body = body.push(ui::caption(&app.credential_status));
    }
    body = body.push(button("Use saved sign-in")
        .style(ui::ghost)
        .on_press_maybe((!app.busy && !app.credential_busy).then_some(Message::LoadLogin)))
        .push(checkbox(app.settings.read_only).label("Read-only connection for inspecting an existing server").on_toggle(Message::ReadOnly))
        .push(text("Reading uncached chapters and browsing sources fetches data on the server. Turn off read-only to enable those actions.").size(13))
        .push(row![button(if app.busy{"Connecting…"}else{"Connect"}).on_press_maybe((!app.busy && !app.credential_busy).then_some(Message::Connect)).padding([12,24]),button("Open saved library").on_press(Message::Go(Screen::Library)).style(ui::secondary).padding([12,24])].spacing(12).wrap());
    scrollable(container(body).padding(28).style(ui::panel).max_width(700))
        .height(Length::Fill)
        .into()
}
fn library(app: &App) -> Element<'_, Message> {
    let browse = app.screen == Screen::Browse;
    let title = if browse { "Browse" } else { "Library" };
    let mut content = column![
        row![
            ui::heading(title).size(30),
            Space::new().width(Length::Fill),
            if app.cover_errors.is_empty() {
                Element::from(Space::new())
            } else {
                Element::from(
                    button("Retry covers")
                        .on_press(Message::RetryCovers)
                        .style(ui::ghost),
                )
            },
            button(if app.busy { "Loading…" } else { "Refresh" })
                .on_press_maybe((!app.busy).then_some(Message::Refresh))
                .style(ui::secondary)
        ]
        .align_y(Alignment::Center)
    ]
    .spacing(18);
    let search = text_input(
        if browse {
            "Search this source"
        } else {
            "Search your library"
        },
        &app.search,
    )
    .on_input(Message::Search)
    .padding(12);
    content = if browse {
        content.push(
            search.on_submit(
                app.source
                    .clone()
                    .map(Message::BrowseSource)
                    .unwrap_or(Message::Noop),
            ),
        )
    } else {
        let filtered = app.only_unread
            || app.only_bookmarked
            || app.library_status != LibraryStatus::All
            || app.settings.reverse_sort;
        content.push(
            row![
                search,
                button(if filtered { "Filter •" } else { "Filter" })
                    .on_press(Message::ToggleLibraryFilters)
                    .style(if app.library_filters {
                        ui::navigation
                    } else {
                        ui::secondary
                    })
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
    };
    if browse {
        if let Some(source) = &app.source {
            content = content.push(
                button("Filters and source preferences")
                    .on_press(Message::SourceOptions(crate::source_options::Msg::Open(
                        source.clone(),
                    )))
                    .style(ui::secondary),
            );
        }
        let mut sources = Row::new().spacing(8);
        for s in &app.sources {
            sources = sources.push(
                button(text(format!("{} · {}", s.display_name, s.lang)).size(13))
                    .on_press(Message::BrowseSource(s.id.clone()))
                    .style(if app.source.as_ref() == Some(&s.id) {
                        ui::primary
                    } else {
                        ui::secondary
                    }),
            );
        }
        content = content
            .push(
                scrollable(sources).direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default(),
                )),
            )
            .push(
                checkbox(app.latest)
                    .label("Latest releases")
                    .on_toggle(Message::Latest),
            );
    } else {
        let mut cats = row![
            button("All titles")
                .on_press(Message::Shelf(None))
                .style(ui::ghost)
        ]
        .spacing(8);
        for c in &app.categories {
            if app.hidden_categories.contains(&c.id) {
                continue;
            }
            cats = cats.push(
                button(text(&c.name))
                    .on_press(Message::Shelf(Some(c.id)))
                    .style(if app.shelf == Some(c.id) {
                        ui::primary
                    } else {
                        ui::ghost
                    }),
            );
        }
        if !app.categories.is_empty() {
            content = content.push(
                scrollable(cats).direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default(),
                )),
            );
        }
    }
    if !browse && app.library_filters {
        content = content.push(
            container(
                column![
                    row![
                        pick_list(
                            LibrarySort::ALL,
                            Some(app.settings.library_sort),
                            Message::LibrarySort
                        ),
                        pick_list(
                            LibraryStatus::ALL,
                            Some(app.library_status),
                            Message::LibraryStatus
                        )
                    ]
                    .spacing(12)
                    .wrap(),
                    row![
                        checkbox(app.only_unread)
                            .label("Unread")
                            .on_toggle(Message::OnlyUnread),
                        checkbox(app.only_bookmarked)
                            .label("Bookmarked")
                            .on_toggle(Message::OnlyBookmarked),
                        checkbox(app.settings.reverse_sort)
                            .label("Reverse order")
                            .on_toggle(Message::ReverseSort),
                        button("Reset")
                            .on_press(Message::ClearLibraryFilters)
                            .style(ui::ghost)
                    ]
                    .spacing(16)
                    .wrap()
                ]
                .spacing(14),
            )
            .padding(16)
            .style(ui::inset),
        );
    }
    let items = app.filtered();
    let total = items.len();
    let visible: Vec<_> = items
        .into_iter()
        .skip(app.grid_page * 24)
        .take(24)
        .collect();
    let cols = (((app.size.0 - 224.0) / 168.0) as usize).clamp(2, 10);
    let mut grid = Column::new().spacing(18).width(Length::Fill);
    for chunk in visible.chunks(cols) {
        let mut line = Row::new().spacing(16);
        for m in chunk {
            let cover: Element<'_, Message> = if let Some(handle) = app.covers.get(&m.id) {
                image(handle.clone())
                    .width(150)
                    .height(205)
                    .content_fit(iced::ContentFit::Cover)
                    .border_radius(10)
                    .into()
            } else {
                container(
                    column![
                        ui::heading("suwa").size(22),
                        ui::caption(if app.cover_errors.contains_key(&m.id) {
                            "Cover unavailable"
                        } else {
                            "Loading cover…"
                        })
                    ]
                    .spacing(10)
                    .align_x(Alignment::Center),
                )
                .width(150)
                .height(205)
                .center_x(150)
                .center_y(205)
                .style(ui::panel)
                .into()
            };
            let title = if m.title.chars().count() > 36 {
                format!("{}…", m.title.chars().take(35).collect::<String>())
            } else {
                m.title.clone()
            };
            line = line.push(
                button(
                    column![
                        cover,
                        text(title).size(13).height(36),
                        if m.unread_count > 0 {
                            Element::from(
                                container(text(format!("{} unread", m.unread_count)).size(11))
                                    .padding([3, 9])
                                    .style(ui::badge),
                            )
                        } else {
                            Element::from(Space::new().height(18))
                        }
                    ]
                    .spacing(8),
                )
                .padding(0)
                .width(152)
                .on_press(Message::Open(m.id))
                .style(ui::cover_button),
            );
        }
        grid = grid.push(line);
    }
    if total == 0 {
        grid = grid.push(
            container(
                column![
                    text(if browse {
                        "Choose a source to start browsing."
                    } else if !app.hidden_categories.is_empty() {
                        "No visible titles."
                    } else {
                        "Your next chapter starts here."
                    })
                    .size(22),
                    text(if browse {
                        "Sources are managed by your Suwayomi server."
                    } else if !app.hidden_categories.is_empty() {
                        "Check your filters, or unhide a category in Settings."
                    } else {
                        "Connect a server, or add a series from Browse."
                    })
                ]
                .spacing(10),
            )
            .padding(35),
        );
    }
    content = content.push(
        scrollable(
            container(grid)
                .padding(iced::Padding::ZERO.right(16))
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill),
    );
    let mut footer = row![
        button("Previous")
            .on_press_maybe((app.grid_page > 0).then_some(Message::NextGrid(-1)))
            .style(ui::ghost),
        text(format!("{} titles · page {}", total, app.grid_page + 1)).size(12),
        button("Next")
            .on_press_maybe(((app.grid_page + 1) * 24 < total).then_some(Message::NextGrid(1)))
            .style(ui::ghost)
    ]
    .spacing(16)
    .align_y(Alignment::Center);
    if browse && app.source_more {
        footer = footer.push(button("Load more results").on_press(Message::MoreBrowse));
    }
    if total > 24 || app.grid_page > 0 || (browse && app.source_more) {
        content = content.push(footer);
    }
    content.into()
}
fn series(app: &App) -> Element<'_, Message> {
    let Some(m) = &app.selected else {
        return text("Choose a series.").into();
    };
    let first = app
        .chapters
        .iter()
        .find(|c| !c.is_read)
        .or(app.chapters.first())
        .map(|c| c.id);
    let mut info = column![ui::heading(&m.title).size(24)]
        .spacing(12)
        .width(Length::Fill);
    if let Some(author) = &m.author {
        info = info.push(ui::caption(author));
    }
    info = info.push(
        row![
            button("Continue reading").on_press_maybe(first.map(Message::Read)),
            button(if m.in_library {
                "In library"
            } else {
                "Add to library"
            })
            .on_press(Message::LibraryToggle)
            .style(ui::secondary),
            button(if app.series_details {
                "Less"
            } else {
                "Details"
            })
            .on_press(Message::ToggleSeriesDetails)
            .style(ui::ghost)
        ]
        .spacing(8)
        .wrap(),
    );
    let mut header = Row::new().spacing(24).align_y(Alignment::Center);
    if let Some(cover) = app.covers.get(&m.id) {
        header = header.push(
            image(cover.clone())
                .width(100)
                .height(140)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(8),
        );
    }
    header = header.push(info);
    let mut body = column![
        button("‹ Library")
            .on_press(Message::Go(Screen::Library))
            .style(ui::ghost)
            .padding([6, 0]),
        header
    ]
    .spacing(20);
    if app.series_details {
        let mut details = column![text(m.genre.join(" · ")).size(12)].spacing(12);
        if let Some(description) = &m.description {
            details = details.push(scrollable(ui::caption(description)).height(80));
        }
        let mut cats = Row::new().spacing(8);
        for category in &app.categories {
            if category.is_default_category {
                continue;
            }
            cats = cats.push(
                button(text(&category.name).size(12))
                    .on_press(Message::AssignCategory(category.id))
                    .style(if m.categories.nodes.iter().any(|c| c.id == category.id) {
                        ui::navigation
                    } else {
                        ui::secondary
                    }),
            );
        }
        details = details.push(cats.wrap()).push(
            row![
                button("Tracking")
                    .on_press(Message::Tracking(crate::tracking::Msg::Open(Some(
                        Box::new(m.clone())
                    ))))
                    .style(ui::ghost),
                button("Refresh chapters")
                    .on_press(Message::RefreshSeries)
                    .style(ui::ghost),
                button("Remove from library")
                    .on_press_maybe(m.in_library.then_some(Message::LibraryToggle))
                    .style(ui::ghost)
            ]
            .spacing(12)
            .wrap(),
        );
        body = body.push(container(details).padding(16).style(ui::inset));
    }
    body = body.push(
        row![
            ui::heading("Chapters").size(18),
            Space::new().width(Length::Fill),
            button(if app.chapter_tools { "Done" } else { "Edit" })
                .on_press(Message::ToggleChapterTools)
                .style(ui::ghost)
        ]
        .align_y(Alignment::Center),
    );
    if app.chapter_tools {
        let mut tools = column![
            text_input("Search chapters", &app.chapter_search).on_input(Message::ChapterSearch),
            row![
                checkbox(app.chapter_unread)
                    .label("Unread")
                    .on_toggle(Message::ChapterUnread),
                checkbox(app.chapter_bookmarked)
                    .label("Bookmarked")
                    .on_toggle(Message::ChapterBookmarked),
                checkbox(app.chapter_reverse)
                    .label("Newest first")
                    .on_toggle(Message::ChapterReverse)
            ]
            .spacing(16)
            .wrap(),
            row![
                button("Select unread")
                    .on_press(Message::SelectUnreadChapters)
                    .style(ui::ghost),
                button("Clear")
                    .on_press(Message::ClearChapters)
                    .style(ui::ghost),
                text(format!("{} selected", app.chapter_selection.len())).size(12)
            ]
            .spacing(12)
            .align_y(Alignment::Center)
        ]
        .spacing(12);
        if !app.chapter_selection.is_empty() {
            tools = tools.push(
                row![
                    button("Download on server").on_press(Message::ServerDownloadSelected),
                    button("Mark read")
                        .on_press(Message::MarkSelectedChapters(true))
                        .style(ui::ghost),
                    button("Mark unread")
                        .on_press(Message::MarkSelectedChapters(false))
                        .style(ui::ghost)
                ]
                .spacing(8)
                .wrap(),
            );
        }
        body = body.push(container(tools).padding(16).style(ui::inset));
    }
    let filtered = app.filtered_chapters();
    let total = filtered.len();
    let mut list = Column::new().spacing(2);
    for chapter in filtered.into_iter().skip(app.chapter_page * 100).take(100) {
        let id = chapter.id;
        let mut title = column![text(&chapter.name).size(14)].spacing(4);
        let mut detail = Vec::new();
        if chapter.is_read {
            detail.push("Read");
        }
        if chapter.is_bookmarked {
            detail.push("Bookmarked");
        }
        if let Some(scanlator) = &chapter.scanlator
            && !scanlator.is_empty()
        {
            detail.push(scanlator);
        }
        if !detail.is_empty() {
            title = title.push(
                text(detail.join(" · "))
                    .size(11)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(ui::colors(theme).muted),
                    }),
            );
        }
        let mut line = Row::new().spacing(12).align_y(Alignment::Center);
        if app.chapter_tools {
            line = line.push(
                checkbox(app.chapter_selection.contains(&id))
                    .on_toggle(move |v| Message::SelectChapter(id, v)),
            );
        }
        line = line.push(
            button(title)
                .on_press(Message::Read(id))
                .width(Length::Fill)
                .style(ui::ghost)
                .padding([12, 0]),
        );
        if chapter.is_downloaded {
            line = line.push(ui::caption("On server"));
        } else {
            line = line.push(
                iced::widget::tooltip(
                    button("↓")
                        .on_press(Message::ServerDownload(id))
                        .style(ui::ghost)
                        .padding([8, 12]),
                    text("Download on server").size(12),
                    iced::widget::tooltip::Position::Top,
                )
                .style(ui::inset),
            );
        }
        line = line.push(
            button("⋯")
                .on_press(Message::ChapterMenu(if app.chapter_menu == Some(id) {
                    None
                } else {
                    Some(id)
                }))
                .style(ui::ghost)
                .padding([8, 12]),
        );
        let mut entry = column![line];
        if app.chapter_menu == Some(id) {
            entry = entry.push(
                row![
                    button(if chapter.is_bookmarked {
                        "Remove bookmark"
                    } else {
                        "Bookmark"
                    })
                    .on_press(Message::Bookmark(id, !chapter.is_bookmarked))
                    .style(ui::ghost),
                    button(if chapter.is_read {
                        "Mark unread"
                    } else {
                        "Mark read"
                    })
                    .on_press(Message::MarkRead(id, !chapter.is_read))
                    .style(ui::ghost)
                ]
                .spacing(8)
                .wrap(),
            );
        }
        list = list
            .push(container(entry).padding([0, 8]).width(Length::Fill))
            .push(iced::widget::rule::horizontal(1));
    }
    body = body.push(scrollable(list).height(Length::Fill));
    if total > 100 || app.chapter_page > 0 {
        body = body.push(
            row![
                button("Previous")
                    .on_press_maybe((app.chapter_page > 0).then_some(Message::ChapterPage(-1)))
                    .style(ui::ghost),
                text(format!("{total} chapters · page {}", app.chapter_page + 1)).size(12),
                button("Next")
                    .on_press_maybe(
                        ((app.chapter_page + 1) * 100 < total).then_some(Message::ChapterPage(1))
                    )
                    .style(ui::ghost)
            ]
            .spacing(16)
            .align_y(Alignment::Center),
        );
    }
    body.into()
}
fn categories(app: &App) -> Element<'_, Message> {
    let mut rows = column![
        ui::heading("Categories").size(28),
        ui::caption("Hidden categories and their manga stay out of your library and library search on this device. Uncheck here to show them again."),
        row![
            text_input("New category", &app.category_name)
                .on_input(Message::CategoryName)
                .on_submit(Message::AddCategory)
                .padding(12),
            button("Create").on_press(Message::AddCategory).padding(12)
        ]
        .spacing(12)
    ]
    .spacing(16);
    if app.category_delete.is_some() {
        rows = rows.push(
            column![
                text("Delete this category? Its titles will remain in the library."),
                row![
                    button("Delete category")
                        .on_press(Message::ConfirmDeleteCategory)
                        .style(ui::danger),
                    button("Cancel").on_press(Message::DeleteCategory(None))
                ]
                .spacing(12)
            ]
            .spacing(8),
        );
    }
    for c in &app.categories {
        let id = c.id;
        if c.is_default_category {
            rows = rows.push(
                container(
                    column![
                        text(&c.name),
                        checkbox(app.hidden_categories.contains(&id))
                            .label("Hide category and its manga")
                            .on_toggle(move |value| Message::HideCategory(id, value))
                    ]
                    .spacing(12),
                )
                .padding(14)
                .width(Length::Fill)
                .style(ui::panel),
            );
            continue;
        }
        rows = rows.push(
            container(
                column![
                    row![
                        text_input("Category name", &c.name)
                            .on_input(move |v| Message::RenameCategory(id, v))
                            .on_submit(Message::SaveCategory(id))
                            .padding(10),
                        button("Save").on_press(Message::SaveCategory(id)),
                        button("Up").on_press(Message::MoveCategory(id, -1)),
                        button("Down").on_press(Message::MoveCategory(id, 1)),
                        button("Delete")
                            .on_press(Message::DeleteCategory(Some(id)))
                            .style(ui::ghost)
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                    checkbox(c.default)
                        .label("Add new library entries to this category")
                        .on_toggle(move |v| Message::DefaultCategory(id, v)),
                    checkbox(app.hidden_categories.contains(&id))
                        .label("Hide category and its manga")
                        .on_toggle(move |value| Message::HideCategory(id, value))
                ]
                .spacing(10),
            )
            .padding(14)
            .width(Length::Fill)
            .style(ui::panel),
        );
    }
    scrollable(rows).into()
}
fn settings_link<'a>(title: &'a str, detail: &'a str, message: Message) -> Element<'a, Message> {
    button(
        row![
            column![text(title).size(15), ui::caption(detail)]
                .spacing(5)
                .width(Length::Fill),
            text("›").size(22)
        ]
        .align_y(Alignment::Center)
        .spacing(16),
    )
    .on_press(message)
    .style(ui::ghost)
    .padding([14, 16])
    .width(Length::Fill)
    .into()
}

fn settings(app: &App) -> Element<'_, Message> {
    let themes: Vec<String> = std::iter::once("Pastel goth".to_owned())
        .chain(iced::Theme::ALL.iter().map(ToString::to_string))
        .collect();
    let appearance = ui::section(
        "Appearance",
        "A little darkness, a softer palette.",
        column![
            ui::caption("Color theme"),
            pick_list(themes, Some(app.settings.theme.clone()), Message::ThemeName)
                .width(Length::Fill),
            checkbox(app.settings.dark)
                .label("Dark background")
                .on_toggle(Message::Dark),
            checkbox(app.settings.oled)
                .label("Pure black background")
                .on_toggle(Message::Oled),
            ui::caption("Pastel goth accent"),
            text_input("#c8a4e8", &app.settings.accent).on_input(Message::Accent),
        ]
        .spacing(14),
    );
    let reading = ui::section(
        "Reading",
        "Settle into your preferred way to read.",
        column![
            ui::caption("Direction and page fit"),
            pick_list(ReadingMode::ALL, Some(app.settings.mode), Message::Mode).width(Length::Fill),
            pick_list(Fit::ALL, Some(app.settings.fit), Message::Fit).width(Length::Fill),
            checkbox(app.settings.double_page)
                .label("Two pages side by side")
                .on_toggle(Message::Double),
            text(format!(
                "Webtoon width · {} px",
                app.settings.strip_width as u32
            ))
            .size(14),
            slider(
                400.0..=1600.0,
                app.settings.strip_width,
                Message::StripWidth
            )
            .step(25.0_f32),
        ]
        .spacing(14),
    );
    let cache = ui::section(
        "Storage",
        "Temporary pages and memory use.",
        column![
            text(format!("Page memory · {} MiB", app.settings.cache_mib)).size(14),
            slider(
                64.0..=1024.0,
                app.settings.cache_mib as f32,
                Message::ReaderCache
            )
            .step(32.0_f32),
            text(format!("Prefetch · {} pages", app.settings.prefetch)).size(14),
            slider(1.0..=12.0, app.settings.prefetch as f32, Message::Prefetch).step(1.0_f32),
            text(format!(
                "Temporary cache · {} MiB",
                app.settings.disk_cache_mib
            ))
            .size(14),
            slider(
                128.0..=8192.0,
                app.settings.disk_cache_mib as f32,
                Message::DiskCache
            )
            .step(128.0_f32),
            button("Clear cache")
                .on_press(Message::ClearCache)
                .style(ui::secondary)
        ]
        .spacing(16),
    );
    let preset = app.model_choice;
    let directory = app.model_directory();
    let installed = preset.installed(&directory);
    let current = crate::model_catalog::presets()
        .iter()
        .find(|p| {
            std::path::Path::new(&app.settings.model_path)
                .file_stem()
                .is_some_and(|name| name == p.id.as_str())
        })
        .map_or("Custom model", |p| p.name.as_str());
    let mut model_details = column![
        pick_list(
            crate::model_catalog::presets().iter().collect::<Vec<_>>(),
            Some(preset),
            Message::PreviewModel,
        )
        .width(Length::Fill),
        text(&preset.description).size(14),
        ui::caption(&preset.note),
        button("Model source")
            .on_press(Message::ModelSource)
            .style(ui::ghost),
        ui::caption(format!(
            "{} · {}",
            preset.license,
            if installed {
                "Installed".into()
            } else {
                format!(
                    "{:.1} MiB download",
                    preset.download_bytes() as f64 / (1024.0 * 1024.0)
                )
            }
        )),
    ]
    .spacing(12);
    if let Some((id, progress)) = &app.model_install {
        let name = crate::model_catalog::presets()
            .iter()
            .find(|p| &p.id == id)
            .map_or("Model", |p| p.name.as_str());
        let cancelling = progress.cancel.is_cancelled();
        let done = progress.done.load(std::sync::atomic::Ordering::Relaxed);
        model_details = model_details
            .push(ui::caption(format!(
                "{} · {} · {:.1} / {:.1} MiB",
                if cancelling {
                    "Cancelling"
                } else {
                    "Installing"
                },
                name,
                done as f64 / 1048576.0,
                progress.total as f64 / 1048576.0,
            )))
            .push(iced::widget::progress_bar(
                0.0..=progress.total.max(1) as f32,
                done as f32,
            ))
            .push(
                button("Cancel download")
                    .on_press_maybe((!cancelling).then_some(Message::CancelModelInstall))
                    .style(ui::ghost),
            );
    } else {
        let selected =
            std::path::Path::new(&app.settings.model_path) == preset.manifest(&directory);
        let action = if installed {
            Message::SelectModel(preset.manifest(&directory).to_string_lossy().into_owned())
        } else {
            Message::InstallModel
        };
        model_details = model_details.push(
            button(if selected {
                "Selected"
            } else if installed {
                "Use this model"
            } else {
                "Install and use"
            })
            .on_press_maybe((!selected).then_some(action))
            .style(ui::secondary),
        );
    }
    if !app.model_status.is_empty() {
        model_details = model_details.push(text(&app.model_status).size(14));
    }
    let mut enhancement = column![
        checkbox(app.settings.upscale)
            .label("Upscale pages")
            .on_toggle(Message::Upscale),
        ui::caption(format!("Using {current}")),
        model_details,
        button(if app.custom_model {
            "Hide custom model"
        } else {
            "Custom model…"
        })
        .on_press(Message::ToggleCustomModel)
        .style(ui::ghost),
    ]
    .spacing(20);
    if app.custom_model {
        enhancement = enhancement
            .push(
                text_input("Model manifest (.json)", &app.settings.model_path)
                    .on_input(Message::ModelPath)
                    .on_submit(Message::SaveSettings),
            )
            .push(
                button("Save model")
                    .on_press(Message::SaveSettings)
                    .style(ui::secondary),
            );
    }
    let enhancement = ui::section(
        "Upscaling",
        "Original pages appear immediately. Enhancement follows in the background.",
        enhancement,
    );
    let mut body = Column::new().spacing(20).width(Length::Fill).max_width(760);
    if app.settings_page == SettingsPage::Main {
        body = body
            .push(ui::heading("Settings").size(28))
            .push(
                container(
                    column![
                        settings_link(
                            "Appearance",
                            "Theme and colors",
                            Message::OpenSettings(SettingsPage::Appearance)
                        ),
                        settings_link(
                            "Reading",
                            "Direction, page fit and shortcuts",
                            Message::OpenSettings(SettingsPage::Reading)
                        ),
                        settings_link(
                            "Upscaling",
                            "GPU enhancement and model",
                            Message::OpenSettings(SettingsPage::Upscaling)
                        ),
                        settings_link(
                            "Categories",
                            "Organize and hide library categories",
                            Message::Go(Screen::Categories)
                        ),
                        settings_link(
                            "Storage",
                            "Cache and memory",
                            Message::OpenSettings(SettingsPage::Storage)
                        ),
                    ]
                    .spacing(2),
                )
                .padding(8)
                .style(ui::panel),
            )
            .push(ui::heading("Server").size(18))
            .push(
                container(
                    column![
                        settings_link(
                            "Connection",
                            "Sign-in and server address",
                            Message::Go(Screen::Connection)
                        ),
                        settings_link(
                            "Sources",
                            "Extensions and repositories",
                            Message::Manage(crate::manage::Msg::Open(
                                crate::manage::Panel::Extensions
                            ))
                        ),
                        settings_link(
                            "Tracking",
                            "Accounts and reading progress",
                            Message::Manage(crate::manage::Msg::Open(
                                crate::manage::Panel::Trackers
                            ))
                        ),
                        settings_link(
                            "Backups",
                            "Export and restore",
                            Message::Manage(crate::manage::Msg::Open(
                                crate::manage::Panel::Backups
                            ))
                        ),
                    ]
                    .spacing(2),
                )
                .padding(8)
                .style(ui::panel),
            );
    } else {
        body = body.push(settings_back()).push(match app.settings_page {
            SettingsPage::Appearance => appearance,
            SettingsPage::Reading => reading,
            SettingsPage::Upscaling => enhancement,
            SettingsPage::Storage => cache,
            SettingsPage::Main => unreachable!(),
        });
        if app.settings_page == SettingsPage::Reading {
            body = body.push(ui::section(
                "Controls",
                "",
                column![
                    checkbox(app.incognito)
                        .label("Incognito reading")
                        .on_toggle(Message::Incognito),
                    ui::caption("← / →  Turn pages     Space  Next page"),
                    ui::caption("F11  Fullscreen     H  Hide controls     Esc  Back"),
                ]
                .spacing(14),
            ));
        }
    }
    if !app.conflicts.is_empty() {
        body = body.push(ui::heading("Reading position conflicts").size(20));
        let pending = app
            .engine
            .as_ref()
            .and_then(|e| app.store.pending_progress(&e.server.scope).ok())
            .unwrap_or_default();
        for remote in &app.conflicts {
            let local = pending
                .iter()
                .find(|(p, _)| p.chapter_id == remote.id)
                .map(|(p, _)| p.page + 1);
            let id = remote.id;
            body = body.push(
                container(
                    column![
                        text(&remote.name),
                        text(format!(
                            "This device: page {} · Server: page {}{}",
                            local
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "unknown".into()),
                            remote.last_page_read + 1,
                            if remote.is_read { " (read)" } else { "" }
                        ))
                        .size(13),
                        row![
                            button("Keep this device").on_press(Message::KeepProgress(id)),
                            button("Use server position")
                                .on_press(Message::UseRemoteProgress(id))
                                .style(ui::secondary)
                        ]
                        .spacing(10)
                    ]
                    .spacing(10),
                )
                .padding(14)
                .width(Length::Fill)
                .style(ui::panel),
            );
        }
    }
    scrollable(
        container(body)
            .padding(iced::Padding::ZERO.right(16))
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .into()
}
fn reader_view(app: &App) -> Element<'_, Message> {
    let mut body = Column::new().spacing(0);
    if app.chrome {
        body = body.push(
            container(
                row![
                    button("Back")
                        .on_press(Message::Go(Screen::Series))
                        .style(ui::ghost),
                    text(
                        app.selected
                            .as_ref()
                            .map(|m| m.title.as_str())
                            .unwrap_or("Reader")
                    )
                    .size(15)
                    .width(Length::Fill),
                    button(if app.reader_options {
                        "Done"
                    } else {
                        "Options"
                    })
                    .on_press(Message::ToggleReaderOptions)
                    .style(ui::ghost),
                    button("Hide")
                        .on_press(Message::ToggleChrome)
                        .style(ui::ghost)
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding(12)
            .style(ui::reader_bar),
        );
    }
    let Some(r) = &app.reader else {
        return body
            .push(
                container(text(if app.busy {
                    "Opening chapter…"
                } else {
                    &app.status
                }))
                .center_x(Length::Fill)
                .center_y(Length::Fill),
            )
            .into();
    };
    let area: Element<'_, Message> = if r.mode.strip() {
        let range = r.visible();
        let top = r.offset(range.start);
        let bottom = r.offset(r.urls.len()) - r.offset(range.end);
        let mut pages = Column::new().spacing(0).align_x(Alignment::Center);
        if top > 0.0 {
            pages = pages.push(Space::new().height(top));
        }
        for i in range {
            pages = pages.push(page_view(r, i, r.width * r.zoom, r.page_height(i)));
            if r.gap() > 0.0 {
                pages = pages.push(Space::new().height(r.gap()));
            }
        }
        if bottom > 0.0 {
            pages = pages.push(Space::new().height(bottom));
        }
        if let Some(error) = &r.append_error {
            pages = pages.push(
                column![
                    text(error).size(13),
                    button("Retry next chapter").on_press(Message::RetryAppend)
                ]
                .spacing(10)
                .padding(16),
            );
        } else if r.append_loading.is_some() {
            pages = pages.push(text("Loading next chapter…").size(13));
        }
        scrollable(container(pages).center_x(Length::Fill))
            .id("reader")
            .height(Length::Fill)
            .on_scroll(|v| Message::Scroll(v.absolute_offset().y, v.bounds().height))
            .into()
    } else {
        let count = if app.settings.double_page { 2 } else { 1 };
        let width = (app.size.0 - 32.0) / count as f32;
        let height = app.size.1
            - if app.chrome {
                ui::reader_chrome_height(app.reader_options)
            } else {
                20.0
            };
        let mut indexes: Vec<_> = (r.current..(r.current + count).min(r.urls.len())).collect();
        if r.mode == ReadingMode::RightToLeft {
            indexes.reverse();
        }
        let mut pages = Row::new().spacing(0).align_y(Alignment::Center);
        let mut content_width = 0.0_f32;
        let mut content_height = 0.0_f32;
        for i in indexes {
            let (w, h) = r.fitted_size(i, (width.max(1.0), height.max(1.0)));
            content_width += w;
            content_height = content_height.max(h);
            pages = pages.push(page_view(r, i, w, h));
        }
        scrollable(
            container(pages)
                .center_x(app.size.0.max(content_width))
                .center_y(height.max(content_height)),
        )
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::default(),
            horizontal: scrollable::Scrollbar::default(),
        })
        .id("paged-reader")
        .height(Length::Fill)
        .into()
    };
    body = body.push(area);
    if app.chrome {
        let mut controls = column![
            row![
                button("‹").on_press(Message::Step(-1)).style(ui::ghost),
                slider(
                    0.0..=r.span_at(r.current).count.saturating_sub(1).max(1) as f32,
                    r.local_page() as f32,
                    move |v| Message::Jump(v + r.span_at(r.current).start as f32)
                )
                .step(1.0_f32),
                text(format!(
                    "{} / {}",
                    r.local_page() + 1,
                    r.span_at(r.current).count
                ))
                .size(12),
                button("›").on_press(Message::Step(1)).style(ui::ghost)
            ]
            .spacing(16)
            .align_y(Alignment::Center)
        ]
        .spacing(12);
        if app.reader_options {
            controls = controls
                .push(
                    row![
                        pick_list(ReadingMode::ALL, Some(app.settings.mode), Message::Mode),
                        pick_list(Fit::ALL, Some(r.fit), Message::Fit),
                        button("Fullscreen")
                            .on_press(Message::Fullscreen)
                            .style(ui::ghost)
                    ]
                    .spacing(12)
                    .wrap(),
                )
                .push(
                    row![
                        button("−")
                            .on_press(Message::Zoom(r.zoom - 0.25))
                            .style(ui::ghost),
                        text(format!("{}%", (r.zoom * 100.0) as u32)).size(12),
                        button("+")
                            .on_press(Message::Zoom(r.zoom + 0.25))
                            .style(ui::ghost),
                        checkbox(app.settings.double_page)
                            .label("Spread")
                            .on_toggle(Message::Double),
                        checkbox(app.settings.upscale)
                            .label("Upscale")
                            .on_toggle(Message::Upscale),
                        checkbox(app.incognito)
                            .label("Incognito")
                            .on_toggle(Message::Incognito)
                    ]
                    .spacing(16)
                    .align_y(Alignment::Center)
                    .wrap(),
                );
        }
        body = body.push(container(controls).padding([12, 20]).style(ui::reader_bar));
    }
    if app.chrome
        && let Some(notice) = &r.notice
    {
        body = body.push(container(text(notice).size(12)).padding(10));
    }
    body.height(Length::Fill).into()
}
fn page_view(r: &Reader, i: usize, w: f32, h: f32) -> Element<'_, Message> {
    if let Some(img) = r.images.get(&i) {
        image(img.handle.clone())
            .width(w)
            .height(h)
            .content_fit(iced::ContentFit::Fill)
            .into()
    } else {
        container(
            column![
                text(
                    r.errors
                        .get(&i)
                        .map(String::as_str)
                        .unwrap_or("Loading page…")
                )
                .size(14),
                button("Retry page")
                    .on_press_maybe(r.errors.contains_key(&i).then_some(Message::RetryPage(i)))
            ]
            .spacing(12)
            .align_x(Alignment::Center),
        )
        .width(w)
        .height(h)
        .center_x(w)
        .center_y(h)
        .into()
    }
}
