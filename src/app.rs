use crate::{
    api::Server,
    credentials::{Credentials, Key as CredentialKey},
    domain::*,
    engine::Engine,
    reader::{Decoded, PageImage, Reader},
    storage::Store,
};
use iced::widget::{self, scrollable};
use iced::{Element, Subscription, Task, Theme, keyboard, window};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

type Outcome<T> = Result<T, String>;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SettingsPage {
    #[default]
    Main,
    Appearance,
    Reading,
    Upscaling,
    Storage,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    SourceOptions,
    Manage,
    Tracking,
    Backups,
    Library,
    Browse,
    Series,
    Reader,
    Downloads,
    Categories,
    Settings,
    Connection,
}
#[derive(Debug, Clone)]
pub enum Message {
    SourceOptions(crate::source_options::Msg),
    SourceOptionsResult(u64, crate::source_options::Msg),
    Manage(crate::manage::Msg),
    Tracking(crate::tracking::Msg),
    Backup(crate::backups::Msg),
    BackupResult(u64, crate::backups::Msg),
    Tracked(u64, crate::tracking::Msg),
    Managed(u64, crate::manage::Msg),
    Key(keyboard::Key),
    Go(Screen),
    OpenSettings(SettingsPage),
    ToggleLibraryFilters,
    ClearLibraryFilters,
    ToggleSeriesDetails,
    ToggleChapterTools,
    ChapterMenu(Option<i64>),
    ToggleReaderOptions,
    DismissStatus,
    SelectModel(String),
    PreviewModel(&'static crate::model_catalog::Preset),
    InstallModel,
    CancelModelInstall,
    ModelSource,
    ModelSourceResult(Outcome<()>),
    ModelInstalled(String, Outcome<String>),
    ToggleCustomModel,
    ServerText(String),
    Username(String),
    Password(String),
    Headers(String),
    Auth(AuthMode),
    ReadOnly(bool),
    RememberLogin(bool),
    LoadLogin,
    CredentialsLoaded(u64, Outcome<Option<Credentials>>),
    CredentialsForgotten(Outcome<()>),
    Connect,
    Connected(u64, Outcome<(String, Option<String>)>),
    Library(u64, Outcome<Vec<Manga>>),
    Sources(u64, Outcome<Vec<Source>>),
    Categories(u64, Outcome<Vec<Category>>),
    Search(String),
    Shelf(Option<i64>),
    OnlyUnread(bool),
    OnlyBookmarked(bool),
    LibrarySort(LibrarySort),
    LibraryStatus(LibraryStatus),
    ReverseSort(bool),
    ChapterSearch(String),
    ChapterUnread(bool),
    ChapterBookmarked(bool),
    ChapterReverse(bool),
    ChapterPage(i32),
    SelectChapter(i64, bool),
    SelectUnreadChapters,
    ClearChapters,
    ServerDownloadSelected,
    MarkSelectedChapters(bool),
    NextGrid(i32),
    Cover(u64, i64, Outcome<Decoded>),
    RetryCovers,
    Open(i64),
    HistoryManga(u64, i64, Outcome<Box<Manga>>),
    Chapters(u64, i64, Outcome<Vec<Chapter>>),
    Read(i64),
    Pages(u64, i64, Outcome<Vec<String>>),
    Appended(u64, i64, Outcome<Vec<String>>),
    RetryAppend,
    Page(u64, usize, Outcome<Decoded>),
    RetryPage(usize),
    Enhanced(u64, usize, Outcome<Decoded>),
    Step(i32),
    Jump(f32),
    Mode(ReadingMode),
    Fit(Fit),
    Zoom(f32),
    Double(bool),
    Scroll(f32, f32),
    ScrollBy(f32),
    Resize(f32, f32),
    ToggleChrome,
    Fullscreen,
    Incognito(bool),
    Upscale(bool),
    Refresh,
    RefreshSeries,
    LibraryToggle,
    Bookmark(i64, bool),
    MarkRead(i64, bool),
    ServerDownload(i64),
    Action(u64, Outcome<String>),
    Tick,
    Captured(Outcome<()>),
    Sync(u64, Outcome<Vec<Chapter>>),
    KeepProgress(i64),
    UseRemoteProgress(i64),
    ResolvedProgress(u64, i64, Outcome<Box<Chapter>>),
    ReaderCache(f32),
    DiskCache(f32),
    Prefetch(f32),
    ClearCache,
    ThemeName(String),
    Accent(String),
    BrowseSource(String),
    BrowseResults(u64, u64, usize, Outcome<(Vec<Manga>, bool)>),
    Latest(bool),
    MoreBrowse,
    CategoryName(String),
    AddCategory,
    RenameCategory(i64, String),
    SaveCategory(i64),
    DefaultCategory(i64, bool),
    HideCategory(i64, bool),
    MoveCategory(i64, i32),
    DeleteCategory(Option<i64>),
    ConfirmDeleteCategory,
    AssignCategory(i64),
    Dark(bool),
    Oled(bool),
    ModelPath(String),
    SaveSettings,
    StripWidth(f32),
    Noop,
}

pub struct App {
    pub source_options: crate::source_options::Options,
    pub manager: crate::manage::Manager,
    pub tracking: crate::tracking::Tracking,
    pub backups: crate::backups::Backups,
    pub store: Arc<Store>,
    pub engine: Option<Arc<Engine>>,
    pub settings: Settings,
    pub screen: Screen,
    pub password: String,
    pub headers: String,
    pub credential_busy: bool,
    pub credential_status: String,
    login_revision: u64,
    pub status: String,
    pub busy: bool,
    pub session: u64,
    pub connected: bool,
    pub browse_request: u64,
    pub generation: u64,
    pub library: Vec<Manga>,
    pub browse: Vec<Manga>,
    pub sources: Vec<Source>,
    pub categories: Vec<Category>,
    pub hidden_categories: HashSet<i64>,
    pub selected: Option<Manga>,
    pub chapters: Vec<Chapter>,
    pub pending_read: Option<i64>,
    pub reader: Option<Reader>,
    pub covers: HashMap<i64, widget::image::Handle>,
    pub cover_loading: HashSet<i64>,
    pub cover_errors: HashMap<i64, String>,
    pub search: String,
    pub shelf: Option<i64>,
    pub only_unread: bool,
    pub only_bookmarked: bool,
    pub library_status: LibraryStatus,
    pub chapter_search: String,
    pub chapter_unread: bool,
    pub chapter_bookmarked: bool,
    pub chapter_reverse: bool,
    pub chapter_page: usize,
    pub chapter_selection: HashSet<i64>,
    pub grid_page: usize,
    pub sync_busy: bool,
    pub conflicts: Vec<Chapter>,
    pub source: Option<String>,
    pub source_page: usize,
    pub source_more: bool,
    pub latest: bool,
    pub category_name: String,
    pub category_delete: Option<i64>,
    pub incognito: bool,
    pub chrome: bool,
    pub fullscreen: bool,
    pub library_filters: bool,
    pub series_details: bool,
    pub chapter_tools: bool,
    pub chapter_menu: Option<i64>,
    pub reader_options: bool,
    pub settings_page: SettingsPage,
    pub custom_model: bool,
    pub model_choice: &'static crate::model_catalog::Preset,
    pub model_install: Option<(String, Arc<crate::model_catalog::Progress>)>,
    pub model_status: String,
    pub size: (f32, f32),
    capture_path: Option<std::path::PathBuf>,
}

impl App {
    pub fn boot() -> (Self, Task<Message>) {
        let store = Arc::new(Store::open_default().unwrap_or_else(|e| {
            eprintln!("Cannot open Suwa storage: {e}");
            std::process::exit(1)
        }));
        Self::boot_store(store)
    }

    fn boot_store(store: Arc<Store>) -> (Self, Task<Message>) {
        let mut settings: Settings = store
            .get("app", "settings")
            .ok()
            .flatten()
            .unwrap_or_default();
        if settings.theme == "Suwa" {
            settings.theme = "Pastel goth".into();
            if settings.accent == "#a1bc98" {
                settings.accent = "#c8a4e8".into();
            }
        }
        if settings.model_path.is_empty()
            && let Ok(path) = std::env::var("SUWA_DEFAULT_MODEL")
        {
            settings.model_path = path;
        }
        if settings.model_path.is_empty() {
            settings.model_path =
                crate::upscale::install_default(&store.root.join("models")).unwrap_or_default();
        }
        let model_choice = crate::model_catalog::presets()
            .iter()
            .find(|preset| {
                std::path::Path::new(&settings.model_path)
                    .file_stem()
                    .is_some_and(|name| name == preset.id.as_str())
            })
            .unwrap_or(&crate::model_catalog::presets()[0]);
        let engine = Server::new(&settings, "", "")
            .ok()
            .map(|s| Arc::new(Engine::new(store.clone(), s)));
        if let Some(engine) = &engine {
            engine.cache_budget.store(
                settings.disk_cache_mib as u64 * 1024 * 1024,
                Ordering::Relaxed,
            );
        }
        let library: Vec<Manga> = engine
            .as_ref()
            .and_then(|e| store.get(&e.server.scope, "library").ok().flatten())
            .unwrap_or_default();
        let categories = engine
            .as_ref()
            .and_then(|e| store.get(&e.server.scope, "categories").ok().flatten())
            .unwrap_or_default();
        let hidden_categories = engine
            .as_ref()
            .and_then(|e| {
                store
                    .get(&e.server.scope, "hidden_categories")
                    .ok()
                    .flatten()
            })
            .unwrap_or_default();
        let screen = if library.is_empty() {
            Screen::Connection
        } else {
            Screen::Library
        };
        let mut app = Self {
            source_options: Default::default(),
            manager: Default::default(),
            tracking: Default::default(),
            backups: Default::default(),
            store,
            engine,
            settings,
            screen,
            password: String::new(),
            headers: String::new(),
            credential_busy: false,
            credential_status: String::new(),
            login_revision: 0,
            status: "Connect your Suwayomi server, or open your saved library offline.".into(),
            busy: false,
            session: 0,
            connected: false,
            browse_request: 0,
            generation: 0,
            library,
            browse: Vec::new(),
            sources: Vec::new(),
            categories,
            hidden_categories,
            selected: None,
            chapters: Vec::new(),
            pending_read: None,
            reader: None,
            covers: HashMap::new(),
            cover_loading: HashSet::new(),
            cover_errors: HashMap::new(),
            search: String::new(),
            shelf: None,
            only_unread: false,
            only_bookmarked: false,
            library_status: LibraryStatus::All,
            chapter_search: String::new(),
            chapter_unread: false,
            chapter_bookmarked: false,
            chapter_reverse: false,
            chapter_page: 0,
            chapter_selection: HashSet::new(),
            grid_page: 0,
            sync_busy: false,
            conflicts: Vec::new(),
            source: None,
            source_page: 1,
            source_more: false,
            latest: false,
            category_name: String::new(),
            category_delete: None,
            incognito: false,
            chrome: true,
            fullscreen: false,
            library_filters: false,
            series_details: false,
            chapter_tools: false,
            chapter_menu: None,
            reader_options: false,
            settings_page: SettingsPage::Main,
            model_choice,
            model_install: None,
            model_status: String::new(),
            custom_model: false,
            size: (1180.0, 820.0),
            capture_path: std::env::var_os("SUWA_CAPTURE").map(Into::into),
        };
        if app.capture_path.is_some() {
            match std::env::var("SUWA_CAPTURE_VIEW").as_deref() {
                Ok("settings") => app.screen = Screen::Settings,
                Ok("upscaling") => {
                    app.screen = Screen::Settings;
                    app.settings_page = SettingsPage::Upscaling;
                }
                Ok("library-filters") => app.library_filters = true,
                Ok("categories") => app.screen = Screen::Categories,
                Ok(
                    view @ ("series" | "series-edit" | "series-details" | "reader"
                    | "reader-options" | "webtoon"),
                ) => {
                    if let Some(manga) = app.library.first() {
                        let id = manga.id;
                        if !view.starts_with("series") {
                            app.settings.mode = if view == "webtoon" {
                                ReadingMode::Webtoon
                            } else {
                                ReadingMode::RightToLeft
                            };
                            app.incognito = true;
                            app.pending_read = app
                                .engine
                                .as_ref()
                                .and_then(|e| {
                                    app.store
                                        .get::<Vec<Chapter>>(
                                            &e.server.scope,
                                            &format!("chapters:{id}"),
                                        )
                                        .ok()
                                        .flatten()
                                })
                                .and_then(|chapters| chapters.first().map(|c| c.id));
                        }
                        let task = app.update(Message::Open(id));
                        app.series_details = view == "series-details";
                        app.chapter_tools = view == "series-edit";
                        let covers = app.covers();
                        return (app, Task::batch([task, covers]));
                    }
                }
                _ => {}
            }
        }
        let covers = if app.screen == Screen::Library {
            app.covers()
        } else {
            Task::none()
        };
        let credentials = if app.settings.remember_login && app.capture_path.is_none() {
            app.update(Message::LoadLogin)
        } else {
            Task::none()
        };
        (app, Task::batch([covers, credentials]))
    }
    pub fn view(&self) -> Element<'_, Message> {
        crate::views::view(self)
    }
    pub fn theme(&self) -> Theme {
        crate::ui::theme(&self.settings)
    }
    pub fn model_directory(&self) -> std::path::PathBuf {
        std::env::var_os("SUWA_MODEL_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| self.store.root.join("models"))
    }
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_secs(3)).map(|_| Message::Tick),
            if self.model_install.is_some() {
                iced::time::every(Duration::from_millis(200)).map(|_| Message::Noop)
            } else {
                Subscription::none()
            },
            iced::event::listen_with(|event, status, _| {
                if status == iced::event::Status::Captured {
                    return None;
                }
                match event {
                    iced::Event::Window(window::Event::Resized(s)) => {
                        Some(Message::Resize(s.width, s.height))
                    }
                    iced::Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => {
                        Some(Message::Key(key))
                    }
                    _ => None,
                }
            }),
        ])
    }

    fn save(&mut self) {
        if let Err(e) = self.store.put("app", "settings", &self.settings) {
            self.status = e.to_string();
        }
    }
    fn error(&mut self, e: String) {
        self.status = e;
        self.busy = false;
    }
    fn refresh(&mut self) -> Task<Message> {
        let Some(e) = self.engine.clone() else {
            return Task::none();
        };
        self.busy = true;
        let sid = self.session;
        Task::perform(
            async move { e.library().await.map_err(|e| e.to_string()) },
            move |r| Message::Library(sid, r),
        )
    }
    fn covers(&mut self) -> Task<Message> {
        let Some(e) = self.engine.clone() else {
            return Task::none();
        };
        let sid = self.session;
        let items: Vec<_> = self
            .filtered()
            .into_iter()
            .skip(self.grid_page * 24)
            .take(24)
            .filter_map(|m| m.thumbnail_url.as_ref().map(|url| (m.id, url.clone())))
            .collect();
        if self.covers.len() > 144 {
            self.covers
                .retain(|id, _| items.iter().any(|(i, _)| i == id));
        }
        let mut jobs = Vec::new();
        for (id, url) in items {
            if self.covers.contains_key(&id) || !self.cover_loading.insert(id) {
                continue;
            }
            let e = e.clone();
            jobs.push(Task::perform(
                async move { e.cover(&url).await.map_err(|e| e.to_string()) },
                move |r| Message::Cover(sid, id, r),
            ));
        }
        Task::batch(jobs)
    }
    pub fn filtered(&self) -> Vec<&Manga> {
        let search = self.search.to_lowercase();
        let default_category = self
            .categories
            .iter()
            .find(|c| c.is_default_category)
            .map(|c| c.id);
        let mut items: Vec<_> = if self.screen == Screen::Browse {
            &self.browse
        } else {
            &self.library
        }
        .iter()
        .filter(|m| {
            (self.screen == Screen::Browse || m.title.to_lowercase().contains(&search))
                && (self.screen == Screen::Browse || !self.only_unread || m.unread_count > 0)
                && (self.screen == Screen::Browse || !self.only_bookmarked || m.bookmark_count > 0)
                && (self.screen == Screen::Browse || self.library_status.matches(&m.status))
                && (self.screen == Screen::Browse
                    || m.visible_in_library(&self.hidden_categories, default_category))
                && (self.screen == Screen::Browse
                    || self
                        .shelf
                        .is_none_or(|id| m.in_category(id, default_category)))
        })
        .collect();
        if self.screen != Screen::Browse {
            items.sort_by_cached_key(|m| m.title.to_lowercase());
            match self.settings.library_sort {
                LibrarySort::Title => {}
                LibrarySort::RecentlyRead => items.sort_by_key(|m| {
                    std::cmp::Reverse(
                        m.latest_read_chapter
                            .as_ref()
                            .and_then(|c| c.last_read_at.parse::<i64>().ok())
                            .unwrap_or(0),
                    )
                }),
                LibrarySort::LatestChapter => items.sort_by_key(|m| {
                    std::cmp::Reverse(
                        m.latest_uploaded_chapter
                            .as_ref()
                            .and_then(|c| c.upload_date.parse::<i64>().ok())
                            .unwrap_or(0),
                    )
                }),
                LibrarySort::ChapterCount => {
                    items.sort_by_key(|m| std::cmp::Reverse(m.chapters.total_count))
                }
                LibrarySort::UnreadCount => {
                    items.sort_by_key(|m| std::cmp::Reverse(m.unread_count))
                }
                LibrarySort::Added => items.sort_by_key(|m| {
                    std::cmp::Reverse(m.in_library_at.parse::<i64>().unwrap_or(0))
                }),
            }
            if self.settings.reverse_sort {
                items.reverse();
            }
        }
        items
    }
    pub fn filtered_chapters(&self) -> Vec<&Chapter> {
        let query = self.chapter_search.to_lowercase();
        let mut chapters: Vec<_> = self
            .chapters
            .iter()
            .filter(|c| {
                (!self.chapter_unread || !c.is_read)
                    && (!self.chapter_bookmarked || c.is_bookmarked)
                    && (query.is_empty()
                        || c.name.to_lowercase().contains(&query)
                        || c.chapter_number.to_string().contains(&query))
            })
            .collect();
        if self.chapter_reverse {
            chapters.reverse();
        }
        chapters
    }
    fn scroll_reader(&self) -> Task<Message> {
        let Some(r) = &self.reader else {
            return Task::none();
        };
        if !r.mode.strip() {
            return Task::none();
        }
        widget::operation::scroll_to(
            "reader",
            scrollable::AbsoluteOffset {
                x: 0.0,
                y: r.scroll,
            },
        )
    }
    fn load_pages(&mut self) -> Task<Message> {
        let Some(e) = self.engine.clone() else {
            return Task::none();
        };
        let Some(r) = self.reader.as_mut() else {
            return Task::none();
        };
        let generation = self.generation;
        let wanted = if r.mode.strip() {
            let v = r.visible();
            v.start.saturating_sub(1)..(v.end + self.settings.prefetch).min(r.urls.len())
        } else {
            r.wanted(self.settings.prefetch)
        };
        let mut jobs = Vec::new();
        for i in wanted {
            if r.images.contains_key(&i) || r.errors.contains_key(&i) || !r.loading.insert(i) {
                continue;
            }
            let url = r.urls[i].clone();
            let e = e.clone();
            jobs.push(Task::perform(
                async move { e.page(&url, generation).await.map_err(|e| e.to_string()) },
                move |result| Message::Page(generation, i, result),
            ));
        }
        if r.mode.strip()
            && r.current + self.settings.prefetch + 2 >= r.urls.len()
            && r.append_loading.is_none()
            && r.append_error.is_none()
            && let Some(last) = r.spans.last()
            && let Some(position) = self.chapters.iter().position(|c| c.id == last.id)
            && let Some(next) = self.chapters.get(position + 1)
        {
            let id = next.id;
            r.append_loading = Some(id);
            jobs.push(Task::perform(
                async move { e.pages(id).await.map_err(|e| e.to_string()) },
                move |r| Message::Appended(generation, id, r),
            ));
        }
        Task::batch(jobs)
    }
    fn enhance_next(&mut self) -> Task<Message> {
        if !self.settings.upscale {
            return Task::none();
        }
        let (Some(engine), Some(r)) = (self.engine.clone(), self.reader.as_mut()) else {
            return Task::none();
        };
        if !r.enhancing.is_empty() {
            return Task::none();
        }
        let wanted = if r.mode.strip() {
            r.visible()
        } else {
            r.wanted(self.settings.prefetch)
        };
        let candidate = r
            .images
            .iter()
            .filter(|(index, image)| {
                wanted.contains(index) && !image.enhanced && image.original.is_some()
            })
            .min_by_key(|(index, _)| index.abs_diff(r.current))
            .map(|(index, image)| (*index, image.original.as_ref().unwrap().clone()));
        let Some((index, original)) = candidate else {
            return Task::none();
        };
        r.enhancing.insert(index);
        let settings = self.settings.clone();
        let generation = self.generation;
        Task::perform(
            async move {
                engine
                    .enhance(original, settings, generation)
                    .await
                    .map_err(|e| e.to_string())
            },
            move |r| Message::Enhanced(generation, index, r),
        )
    }
    fn record_position(&mut self) {
        if self.incognito {
            return;
        }
        let (Some(e), Some(r)) = (&self.engine, &self.reader) else {
            return;
        };
        if e.server.read_only {
            if let Err(err) = self.store.put(
                &e.server.scope,
                &format!("resume:{}", r.chapter_id),
                &r.local_page(),
            ) {
                self.status = err.to_string();
            }
            return;
        }
        let baseline = self
            .chapters
            .iter()
            .find(|c| c.id == r.chapter_id)
            .map(|c| c.last_page_read)
            .unwrap_or(0);
        let baseline = self
            .store
            .get::<usize>(&e.server.scope, &format!("baseline:{}", r.chapter_id))
            .ok()
            .flatten()
            .unwrap_or(baseline);
        let p = Progress {
            chapter_id: r.chapter_id,
            page: if r.finished() {
                r.span_at(r.current).count.saturating_sub(1)
            } else {
                r.local_page()
            },
            read: r.finished()
                || (self.settings.double_page
                    && !r.mode.strip()
                    && r.local_page() + 2 >= r.span_at(r.current).count),
            explicit: false,
            baseline_page: baseline,
        };
        if let Err(err) = self.store.record_progress(&e.server.scope, &p) {
            self.status = err.to_string();
        }
    }
    fn set_reader_page(&mut self, index: usize) {
        if let Some(r) = self.reader.as_mut() {
            let old = r.span_at(r.current).clone();
            if r.span_at(index).id != old.id && index > r.current {
                r.current = old.start + old.count.saturating_sub(1);
                self.record_position();
            }
        }
        if let Some(r) = self.reader.as_mut() {
            r.set_current(index);
        }
    }
    fn action<F>(&mut self, f: F) -> Task<Message>
    where
        F: std::future::Future<Output = anyhow::Result<String>> + Send + 'static,
    {
        self.busy = true;
        let sid = self.session;
        Task::perform(
            async move { f.await.map_err(|e| e.to_string()) },
            move |r| Message::Action(sid, r),
        )
    }
    fn browse_source(&mut self) -> Task<Message> {
        let (Some(e), Some(source)) = (self.engine.clone(), self.source.clone()) else {
            return Task::none();
        };
        self.busy = true;
        let query = self.search.clone();
        let page = self.source_page;
        let latest = self.latest;
        let filters = if self.source_options.source == source {
            self.source_options.filter_changes()
        } else {
            Vec::new()
        };
        self.browse_request += 1;
        let request = self.browse_request;
        let sid = self.session;
        Task::perform(
            async move {
                e.server
                    .browse(&source, &query, page, latest, filters)
                    .await
                    .map_err(|e| e.to_string())
            },
            move |r| Message::BrowseResults(sid, request, page, r),
        )
    }
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenSettings(page) => {
                self.settings_page = page;
                return self.update(Message::Go(Screen::Settings));
            }
            Message::ToggleLibraryFilters => self.library_filters = !self.library_filters,
            Message::ClearLibraryFilters => {
                self.only_unread = false;
                self.only_bookmarked = false;
                self.library_status = LibraryStatus::All;
                self.settings.library_sort = LibrarySort::Title;
                self.settings.reverse_sort = false;
                self.grid_page = 0;
                self.save();
                return self.covers();
            }
            Message::ToggleSeriesDetails => self.series_details = !self.series_details,
            Message::ToggleChapterTools => {
                self.chapter_tools = !self.chapter_tools;
                self.chapter_menu = None;
                if !self.chapter_tools {
                    self.chapter_selection.clear();
                }
            }
            Message::ChapterMenu(id) => self.chapter_menu = id,
            Message::ToggleReaderOptions => {
                self.reader_options = !self.reader_options;
                if let Some(reader) = &mut self.reader {
                    reader.viewport = (self.size.1
                        - crate::ui::reader_chrome_height(self.reader_options))
                    .max(1.0);
                }
            }
            Message::DismissStatus => self.status.clear(),
            Message::SelectModel(path) => {
                self.settings.model_path = path;
                self.save();
                return self.update(Message::Upscale(self.settings.upscale));
            }
            Message::PreviewModel(preset) => {
                self.model_choice = preset;
                self.model_status.clear();
            }
            Message::InstallModel => {
                if self.model_install.is_some() {
                    return Task::none();
                }
                let preset = self.model_choice;
                let directory = self.model_directory();
                let progress = crate::model_catalog::Progress::new(preset);
                self.model_install = Some((preset.id.clone(), progress.clone()));
                self.model_status.clear();
                return Task::perform(
                    async move {
                        crate::model_catalog::install(preset, directory, progress)
                            .await
                            .map_err(|e| format!("{e:#}"))
                    },
                    move |result| Message::ModelInstalled(preset.id.clone(), result),
                );
            }
            Message::CancelModelInstall => {
                if let Some((_, progress)) = &self.model_install {
                    progress.cancel.cancel();
                }
            }
            Message::ModelSource => {
                let source = self.model_choice.source.clone();
                return Task::perform(
                    async move {
                        let status = tokio::process::Command::new("xdg-open")
                            .arg(source)
                            .status()
                            .await
                            .map_err(|e| e.to_string())?;
                        if status.success() {
                            Ok(())
                        } else {
                            Err("Could not open model source".into())
                        }
                    },
                    Message::ModelSourceResult,
                );
            }
            Message::ModelSourceResult(Err(error)) => self.model_status = error,
            Message::ModelSourceResult(Ok(())) => {}
            Message::ModelInstalled(id, result) => {
                if !self
                    .model_install
                    .as_ref()
                    .is_some_and(|(pending, _)| pending == &id)
                {
                    return Task::none();
                }
                let cancelled = self.model_install.take().unwrap().1.cancel.is_cancelled();
                if cancelled {
                    self.model_status =
                        "Download cancelled. Verified weights are kept for retry.".into();
                } else {
                    match result {
                        Ok(path) => {
                            self.model_status = "Installed and selected.".into();
                            return self.update(Message::SelectModel(path));
                        }
                        Err(error) => self.model_status = error,
                    }
                }
            }
            Message::ToggleCustomModel => self.custom_model = !self.custom_model,
            Message::Go(Screen::Downloads) => {
                return self.update(Message::Manage(crate::manage::Msg::Open(
                    crate::manage::Panel::ServerDownloads,
                )));
            }
            Message::SourceOptions(crate::source_options::Msg::Back) => {
                self.screen = Screen::Browse;
            }
            Message::SourceOptions(crate::source_options::Msg::Apply) => {
                self.screen = Screen::Browse;
                self.source_page = 1;
                self.latest = false;
                return self.browse_source();
            }
            Message::SourceOptions(msg) => {
                self.screen = Screen::SourceOptions;
                if let Some(e) = &self.engine {
                    let sid = self.session;
                    return self
                        .source_options
                        .update(msg, e.server.clone())
                        .map(move |m| Message::SourceOptionsResult(sid, m));
                }
            }
            Message::SourceOptionsResult(sid, msg) if sid == self.session => {
                if let Some(e) = &self.engine {
                    let sid = self.session;
                    return self
                        .source_options
                        .update(msg, e.server.clone())
                        .map(move |m| Message::SourceOptionsResult(sid, m));
                }
            }
            Message::Manage(crate::manage::Msg::Open(crate::manage::Panel::Backups)) => {
                return self.update(Message::Backup(crate::backups::Msg::Open));
            }
            Message::Backup(msg) => {
                self.screen = Screen::Backups;
                if let Some(e) = &self.engine {
                    let sid = self.session;
                    return self
                        .backups
                        .update(msg, e.server.clone())
                        .map(move |m| Message::BackupResult(sid, m));
                }
            }
            Message::BackupResult(sid, msg) if sid == self.session => {
                if let Some(e) = &self.engine {
                    return self
                        .backups
                        .update(msg, e.server.clone())
                        .map(move |m| Message::BackupResult(sid, m));
                }
            }
            Message::Manage(crate::manage::Msg::Open(crate::manage::Panel::Trackers)) => {
                return self.update(Message::Tracking(crate::tracking::Msg::Open(None)));
            }
            Message::Tracking(msg) => {
                self.screen = Screen::Tracking;
                if let Some(e) = &self.engine {
                    let sid = self.session;
                    return self
                        .tracking
                        .update(msg, e.server.clone())
                        .map(move |m| Message::Tracked(sid, m));
                }
            }
            Message::Tracked(sid, msg) if sid == self.session => {
                if let Some(e) = &self.engine {
                    return self
                        .tracking
                        .update(msg, e.server.clone())
                        .map(move |m| Message::Tracked(sid, m));
                }
            }
            Message::Manage(crate::manage::Msg::ReadChapter(manga, chapter)) => {
                if let Some(e) = self.engine.clone() {
                    let sid = self.session;
                    self.busy = true;
                    return Task::perform(
                        async move {
                            e.server
                                .manga(manga)
                                .await
                                .map(Box::new)
                                .map_err(|e| e.to_string())
                        },
                        move |r| Message::HistoryManga(sid, chapter, r),
                    );
                }
            }
            Message::HistoryManga(sid, chapter, result)
                if sid == self.session && self.screen == Screen::Manage =>
            {
                match result {
                    Ok(manga) => {
                        let id = manga.id;
                        self.browse.retain(|m| m.id != id);
                        self.browse.push(*manga);
                        self.pending_read = Some(chapter);
                        return self.update(Message::Open(id));
                    }
                    Err(e) => self.error(e),
                }
            }
            Message::Manage(msg) => {
                self.screen = Screen::Manage;
                if let Some(e) = &self.engine {
                    let sid = self.session;
                    return self
                        .manager
                        .update(msg, e.server.clone())
                        .map(move |m| Message::Managed(sid, m));
                }
            }
            Message::Managed(sid, msg) if sid == self.session => {
                if let Some(e) = &self.engine {
                    return self
                        .manager
                        .update(msg, e.server.clone())
                        .map(move |m| Message::Managed(sid, m));
                }
            }

            Message::Key(key) => {
                if key == keyboard::Key::Named(keyboard::key::Named::F11) {
                    return self.update(Message::Fullscreen);
                }
                if self.screen != Screen::Reader {
                    return Task::none();
                }
                let rtl = self.settings.mode == ReadingMode::RightToLeft;
                if let Some(r) = &self.reader
                    && r.mode.strip()
                {
                    let delta = match key.as_ref() {
                        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some(100.0),
                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some(-100.0),
                        keyboard::Key::Named(
                            keyboard::key::Named::PageDown | keyboard::key::Named::Space,
                        ) => Some(r.viewport * 0.85),
                        keyboard::Key::Named(keyboard::key::Named::PageUp) => {
                            Some(-r.viewport * 0.85)
                        }
                        _ => None,
                    };
                    if let Some(delta) = delta {
                        return self.update(Message::ScrollBy(delta));
                    }
                }
                let action = match key.as_ref() {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        Message::Go(Screen::Series)
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                        Message::Step(if rtl { -1 } else { 1 })
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                        Message::Step(if rtl { 1 } else { -1 })
                    }
                    keyboard::Key::Named(
                        keyboard::key::Named::PageDown
                        | keyboard::key::Named::ArrowDown
                        | keyboard::key::Named::Space,
                    ) => Message::Step(1),
                    keyboard::Key::Named(
                        keyboard::key::Named::PageUp | keyboard::key::Named::ArrowUp,
                    ) => Message::Step(-1),
                    keyboard::Key::Character("h") => Message::ToggleChrome,
                    _ => Message::Noop,
                };
                return self.update(action);
            }
            Message::Go(screen) => {
                if self.screen == Screen::Connection && screen == Screen::Library {
                    self.login_revision += 1;
                    self.credential_status.clear();
                }
                if self.screen == Screen::Reader {
                    self.record_position();
                    if let Some(e) = &self.engine {
                        self.generation = e.next_generation();
                    }
                    self.reader = None;
                }
                self.pending_read = None;
                self.screen = screen;
                self.search.clear();
                self.grid_page = 0;
                self.busy = false;
                if screen == Screen::Browse
                    && let Some(e) = self.engine.clone()
                {
                    let sid = self.session;
                    return Task::perform(
                        async move { e.server.sources().await.map_err(|e| e.to_string()) },
                        move |r| Message::Sources(sid, r),
                    );
                }
                if screen == Screen::Library {
                    return self.covers();
                }
            }
            Message::ServerText(v) => {
                self.settings.server = v;
                self.password.clear();
                self.headers.clear();
                self.credential_status.clear();
                self.login_revision += 1;
            }
            Message::Username(v) => {
                self.settings.username = v;
                self.password.clear();
                self.headers.clear();
                self.credential_status.clear();
                self.login_revision += 1;
            }
            Message::Password(v) => {
                self.password = v;
                self.login_revision += 1;
            }
            Message::Headers(v) => {
                self.headers = v;
                self.login_revision += 1;
            }
            Message::Auth(v) => {
                self.settings.auth = v;
                self.password.clear();
                self.headers.clear();
                self.credential_status.clear();
                self.login_revision += 1;
            }
            Message::ReadOnly(v) => self.settings.read_only = v,
            Message::LoadLogin => {
                if self.busy || self.credential_busy {
                    return Task::none();
                }
                if let Ok(scope) = crate::api::account_scope(&self.settings) {
                    let key = CredentialKey::new(&self.store.root, scope);
                    let revision = self.login_revision;
                    self.credential_busy = true;
                    self.credential_status = "Opening the Linux keyring…".into();
                    return Task::perform(
                        async move { key.load().await.map_err(|e| e.to_string()) },
                        move |result| Message::CredentialsLoaded(revision, result),
                    );
                }
            }
            Message::CredentialsLoaded(revision, result) => {
                self.credential_busy = false;
                if revision != self.login_revision {
                    self.credential_status.clear();
                    return Task::none();
                }
                match result {
                    Ok(Some(credentials)) => {
                        self.password = credentials.password;
                        self.headers = credentials.headers;
                        self.credential_status = "Saved sign-in loaded".into();
                        return self.update(Message::Connect);
                    }
                    Ok(None) => {
                        self.credential_status = "No saved sign-in for this account yet".into()
                    }
                    Err(e) => self.credential_status = e,
                }
            }
            Message::RememberLogin(value) => {
                if self.busy || self.credential_busy {
                    return Task::none();
                }
                if value {
                    self.settings.remember_login = true;
                    self.save();
                    self.credential_status =
                        "Sign-in will be saved after a successful connection".into();
                } else {
                    match crate::api::account_scope(&self.settings) {
                        Ok(scope) => {
                            let key = CredentialKey::new(&self.store.root, scope);
                            self.credential_busy = true;
                            self.credential_status = "Removing saved sign-in…".into();
                            return Task::perform(
                                async move { key.forget().await.map_err(|e| e.to_string()) },
                                Message::CredentialsForgotten,
                            );
                        }
                        Err(e) => self.credential_status = e.to_string(),
                    }
                }
            }
            Message::CredentialsForgotten(result) => {
                self.credential_busy = false;
                match result {
                    Ok(()) => {
                        self.settings.remember_login = false;
                        self.save();
                        self.credential_status =
                            "Saved sign-in removed; these details stay in this session only".into();
                    }
                    Err(e) => self.credential_status = e,
                }
            }
            Message::Connect if self.busy || self.credential_busy => return Task::none(),
            Message::Connect => match Server::new(&self.settings, &self.password, &self.headers) {
                Err(e) => self.error(e.to_string()),
                Ok(server) => {
                    self.login_revision += 1;
                    if let Some(e) = &self.engine {
                        e.next_generation();
                    }
                    self.connected = false;
                    self.session += 1;
                    self.browse_request += 1;
                    self.browse.clear();
                    self.sources.clear();
                    self.selected = None;
                    self.source = None;
                    self.source_page = 1;
                    self.source_more = false;
                    self.source_options = Default::default();
                    self.manager = Default::default();
                    self.tracking = Default::default();
                    self.backups = Default::default();
                    self.shelf = None;
                    self.category_name.clear();
                    self.category_delete = None;
                    self.search.clear();
                    self.grid_page = 0;
                    self.generation = 0;
                    self.covers.clear();
                    self.cover_loading.clear();
                    self.cover_errors.clear();
                    self.pending_read = None;
                    self.chapters.clear();
                    self.reader = None;

                    self.sync_busy = false;
                    self.conflicts.clear();
                    self.library = self
                        .store
                        .get(&server.scope, "library")
                        .ok()
                        .flatten()
                        .unwrap_or_default();
                    self.categories = self
                        .store
                        .get(&server.scope, "categories")
                        .ok()
                        .flatten()
                        .unwrap_or_default();
                    self.hidden_categories = self
                        .store
                        .get(&server.scope, "hidden_categories")
                        .ok()
                        .flatten()
                        .unwrap_or_default();

                    self.engine = Some(Arc::new(Engine::new(self.store.clone(), server.clone())));
                    self.engine.as_ref().unwrap().cache_budget.store(
                        self.settings.disk_cache_mib as u64 * 1024 * 1024,
                        Ordering::Relaxed,
                    );
                    let settings = self.settings.clone();
                    let password = self.password.clone();
                    let credentials = Credentials {
                        password: password.clone(),
                        headers: self.headers.clone(),
                    };
                    let key = CredentialKey::new(&self.store.root, server.scope.clone());
                    let sid = self.session;
                    self.busy = true;
                    self.credential_busy = true;
                    self.status = "Connecting…".into();
                    self.save();
                    return Task::perform(
                        async move {
                            let name = server
                                .login(&settings, &password)
                                .await
                                .map_err(|e| e.to_string())?;
                            let warning = if settings.remember_login {
                                key.save(&credentials).await.err().map(|e| e.to_string())
                            } else {
                                None
                            };
                            Ok((name, warning))
                        },
                        move |r| Message::Connected(sid, r),
                    );
                }
            },
            Message::Connected(sid, result) if sid == self.session => {
                self.busy = false;
                self.credential_busy = false;
                match result {
                    Err(e) => self.error(e),
                    Ok((name, warning)) => {
                        self.connected = true;
                        self.status = format!("Connected to {name}");
                        if let Some(warning) = warning {
                            self.credential_status = format!("Sign-in was not saved: {warning}");
                            self.status
                                .push_str(&format!(" · {}", self.credential_status));
                        } else if self.settings.remember_login {
                            self.credential_status = "Sign-in saved in the Linux keyring".into();
                        }
                        self.screen = Screen::Library;
                        let e = self.engine.clone().unwrap();
                        let categories = Task::perform(
                            async move { e.server.categories().await.map_err(|e| e.to_string()) },
                            move |r| Message::Categories(sid, r),
                        );
                        return Task::batch([self.refresh(), categories]);
                    }
                }
            }
            Message::Library(sid, result) if sid == self.session => {
                self.busy = false;
                match result {
                    Ok(items) => {
                        if let Some(selected) = self.selected.as_mut() {
                            if let Some(updated) = items.iter().find(|m| m.id == selected.id) {
                                *selected = updated.clone();
                            } else {
                                selected.in_library = false;
                            }
                        }
                        self.library = items;
                        return self.covers();
                    }
                    Err(e) => self.error(format!("{e}. Saved library is available offline.")),
                }
            }
            Message::Sources(sid, result) if sid == self.session => {
                self.busy = false;
                match result {
                    Ok(s) => self.sources = s,
                    Err(e) => self.error(e),
                }
            }
            Message::Categories(sid, result) if sid == self.session => match result {
                Ok(mut c) => {
                    c.sort_by_key(|c| c.order);
                    if let Some(e) = &self.engine {
                        let _ = self.store.put(&e.server.scope, "categories", &c);
                    }
                    self.categories = c
                }
                Err(e) => self.error(e),
            },
            Message::Search(s) => {
                self.search = s;
                self.grid_page = 0;
                if self.screen == Screen::Library {
                    return self.covers();
                }
            }
            Message::Shelf(id) => {
                self.shelf = id.filter(|id| !self.hidden_categories.contains(id));
                self.grid_page = 0;
                return self.covers();
            }
            Message::OnlyUnread(v) => {
                self.only_unread = v;
                self.grid_page = 0;
                return self.covers();
            }
            Message::OnlyBookmarked(v) => {
                self.only_bookmarked = v;
                self.grid_page = 0;
                return self.covers();
            }
            Message::LibrarySort(v) => {
                self.settings.library_sort = v;
                self.grid_page = 0;
                self.save();
                return self.covers();
            }
            Message::LibraryStatus(v) => {
                self.library_status = v;
                self.grid_page = 0;
                return self.covers();
            }
            Message::ReverseSort(v) => {
                self.settings.reverse_sort = v;
                self.grid_page = 0;
                self.save();
                return self.covers();
            }
            Message::ChapterSearch(v) => {
                self.chapter_search = v;
                self.chapter_page = 0;
            }
            Message::ChapterUnread(v) => {
                self.chapter_unread = v;
                self.chapter_page = 0;
            }
            Message::ChapterBookmarked(v) => {
                self.chapter_bookmarked = v;
                self.chapter_page = 0;
            }
            Message::ChapterReverse(v) => {
                self.chapter_reverse = v;
                self.chapter_page = 0;
            }
            Message::ChapterPage(delta) => {
                self.chapter_page = self.chapter_page.saturating_add_signed(delta as isize)
            }
            Message::SelectChapter(id, value) => {
                if value {
                    self.chapter_selection.insert(id);
                } else {
                    self.chapter_selection.remove(&id);
                }
            }
            Message::SelectUnreadChapters => {
                self.chapter_selection = self
                    .chapters
                    .iter()
                    .filter(|c| !c.is_read)
                    .map(|c| c.id)
                    .collect()
            }
            Message::ClearChapters => self.chapter_selection.clear(),
            Message::ServerDownloadSelected => {
                if let Some(engine) = self.engine.clone() {
                    let ids: Vec<_> = self
                        .chapters
                        .iter()
                        .filter(|c| self.chapter_selection.contains(&c.id) && !c.is_downloaded)
                        .map(|c| c.id)
                        .collect();
                    if !ids.is_empty() {
                        return self.action(async move {
                            engine.server.enqueue(&ids).await?;
                            Ok("Selected chapters queued on server".into())
                        });
                    }
                }
            }
            Message::MarkSelectedChapters(read) => {
                if let Some(e) = self.engine.clone() {
                    if e.server.read_only {
                        self.error(
                            "Read-only connection: reading status changes are disabled".into(),
                        );
                        return Task::none();
                    }
                    let selected: Vec<_> = self
                        .chapters
                        .iter()
                        .filter(|c| self.chapter_selection.contains(&c.id))
                        .cloned()
                        .collect();
                    return self.action(async move {
                        tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
                            for c in selected {
                                e.store.record_progress(
                                    &e.server.scope,
                                    &Progress {
                                        chapter_id: c.id,
                                        page: if read {
                                            c.page_count.saturating_sub(1)
                                        } else {
                                            0
                                        },
                                        read,
                                        explicit: true,
                                        baseline_page: c.last_page_read,
                                    },
                                )?;
                            }
                            Ok("Reading status saved; queued for synchronization".into())
                        })
                        .await?
                    });
                }
            }
            Message::NextGrid(delta) => {
                self.grid_page = self.grid_page.saturating_add_signed(delta as isize);
                return self.covers();
            }
            Message::Cover(sid, id, result) if sid == self.session => {
                self.cover_loading.remove(&id);
                match result {
                    Ok(img) => {
                        self.cover_errors.remove(&id);
                        self.covers.insert(id, img.handle());
                    }
                    Err(error) => {
                        self.cover_errors.insert(id, error);
                    }
                }
            }
            Message::RetryCovers => return self.covers(),
            Message::Open(id) => {
                if self.selected.as_ref().map(|m| m.id) != Some(id) {
                    self.series_details = false;
                    self.chapter_tools = false;
                    self.chapter_menu = None;
                }
                self.selected = self
                    .library
                    .iter()
                    .chain(self.browse.iter())
                    .find(|m| m.id == id)
                    .cloned();
                self.screen = Screen::Series;
                self.chapter_page = 0;
                self.chapter_search.clear();
                self.chapter_selection.clear();
                if let Some(e) = &self.engine {
                    self.generation = e.next_generation();
                }
                self.reader = None;
                self.chapters.clear();
                self.busy = true;
                if let Some(e) = self.engine.clone() {
                    let sid = self.session;
                    return Task::perform(
                        async move { e.chapters(id).await.map_err(|e| e.to_string()) },
                        move |r| Message::Chapters(sid, id, r),
                    );
                }
            }
            Message::Chapters(sid, id, result)
                if sid == self.session && self.selected.as_ref().is_some_and(|m| m.id == id) =>
            {
                self.busy = false;
                match result {
                    Ok(c) => {
                        self.chapters = c;
                        if let Some(chapter) = self.pending_read.take() {
                            return self.update(Message::Read(chapter));
                        }
                    }
                    Err(e) => self.error(e),
                }
            }
            Message::Read(id) => {
                self.record_position();
                self.screen = Screen::Reader;
                self.reader = None;
                self.busy = true;
                self.chrome = true;
                self.reader_options = self.capture_path.is_some()
                    && std::env::var("SUWA_CAPTURE_VIEW").as_deref() == Ok("reader-options");
                if let Some(e) = self.engine.clone() {
                    self.generation = e.next_generation();
                    let gen_id = self.generation;
                    return Task::perform(
                        async move { e.pages(id).await.map_err(|e| e.to_string()) },
                        move |r| Message::Pages(gen_id, id, r),
                    );
                }
            }
            Message::Pages(gen_id, id, result) if gen_id == self.generation => {
                self.busy = false;
                match result {
                    Err(e) => self.error(e),
                    Ok(urls) => {
                        let remote = self
                            .chapters
                            .iter()
                            .find(|c| c.id == id)
                            .map(|c| c.last_page_read)
                            .unwrap_or(0);
                        let local =
                            self.engine
                                .as_ref()
                                .filter(|e| {
                                    !self.connected
                                        || e.server.read_only
                                        || self.store.pending_progress(&e.server.scope).is_ok_and(
                                            |p| p.iter().any(|(p, _)| p.chapter_id == id),
                                        )
                                })
                                .and_then(|e| {
                                    self.store
                                        .get(&e.server.scope, &format!("resume:{id}"))
                                        .ok()
                                        .flatten()
                                });
                        let mut reader = Reader::new(
                            id,
                            urls,
                            local.unwrap_or(remote),
                            self.settings.mode,
                            self.settings.fit,
                        );
                        reader.width = self.settings.strip_width.min(self.size.0 - 40.0);
                        reader.viewport = (self.size.1
                            - crate::ui::reader_chrome_height(self.reader_options))
                        .max(1.0);
                        let offset = reader.offset(reader.current);
                        reader.scroll = offset;
                        self.reader = Some(reader);
                        self.record_position();
                        return Task::batch([
                            self.load_pages(),
                            widget::operation::scroll_to(
                                "reader",
                                scrollable::AbsoluteOffset { x: 0.0, y: offset },
                            ),
                        ]);
                    }
                }
            }
            Message::Appended(gen_id, id, result) if gen_id == self.generation => {
                if let Some(r) = self.reader.as_mut() {
                    r.append_loading = None;
                    match result {
                        Ok(urls) => r.append(id, urls),
                        Err(e) => r.append_error = Some(e),
                    }
                }
                return self.load_pages();
            }
            Message::RetryAppend => {
                if let Some(r) = self.reader.as_mut() {
                    r.append_error = None;
                }
                return self.load_pages();
            }
            Message::Page(gen_id, index, result) if gen_id == self.generation => {
                let mut reposition = Task::none();
                if let Some(r) = self.reader.as_mut() {
                    r.loading.remove(&index);
                    match result {
                        Err(e) => {
                            r.errors.insert(index, e);
                        }
                        Ok(img) => {
                            let old_scroll = r.scroll;
                            r.insert(
                                index,
                                PageImage {
                                    handle: img.handle(),
                                    width: img.width,
                                    height: img.height,
                                    bytes: img.rgba.len(),
                                    enhanced: false,
                                    original: Some(img),
                                },
                                self.settings.cache_mib * 1024 * 1024,
                            );
                            if r.mode.strip() && (r.scroll - old_scroll).abs() > 0.5 {
                                reposition = widget::operation::scroll_to(
                                    "reader",
                                    scrollable::AbsoluteOffset {
                                        x: 0.0,
                                        y: r.scroll,
                                    },
                                );
                            }
                        }
                    }
                }
                return Task::batch([reposition, self.enhance_next()]);
            }
            Message::RetryPage(index) => {
                if let Some(r) = self.reader.as_mut() {
                    r.errors.remove(&index);
                }
                return self.load_pages();
            }
            Message::Enhanced(gen_id, index, result) if gen_id == self.generation => {
                if let Some(r) = self.reader.as_mut() {
                    r.enhancing.remove(&index);
                    match result {
                        Err(e) => {
                            r.notice = Some(e.clone());
                            self.status = e;
                            self.settings.upscale = false;
                        }
                        Ok(img) if self.settings.upscale => {
                            if let Some(original) = r.images.get(&index) {
                                let (w, h) = (original.width, original.height);
                                r.insert(
                                    index,
                                    PageImage {
                                        handle: img.handle(),
                                        width: w,
                                        height: h,
                                        bytes: img.rgba.len(),
                                        enhanced: true,
                                        original: None,
                                    },
                                    self.settings.cache_mib * 1024 * 1024,
                                );
                            }
                        }
                        Ok(_) => {}
                    }
                }
                return self.enhance_next();
            }
            Message::Step(delta) => {
                if let Some(r) = self.reader.as_ref() {
                    let step = if self.settings.double_page && !r.mode.strip() {
                        2
                    } else {
                        1
                    };
                    let next = r.current as i64 + delta as i64 * step;
                    if next < 0 || next >= r.urls.len() as i64 {
                        let chapter = r.chapter_id;
                        if next >= r.urls.len() as i64 {
                            let r = self.reader.as_mut().unwrap();
                            r.set_current(r.urls.len().saturating_sub(1));
                            r.scroll = r.offset(r.urls.len());
                            self.record_position();
                        }
                        if let Some(pos) = self.chapters.iter().position(|c| c.id == chapter) {
                            let target = pos as i64 + delta as i64;
                            if target >= 0 && (target as usize) < self.chapters.len() {
                                return self
                                    .update(Message::Read(self.chapters[target as usize].id));
                            }
                        }
                    } else {
                        return self.update(Message::Jump(next as f32));
                    }
                }
            }
            Message::Jump(value) => {
                let Some(r) = self.reader.as_mut() else {
                    return Task::none();
                };
                let index = (value as usize).min(r.urls.len().saturating_sub(1));
                r.scroll = r.offset(index);
                self.set_reader_page(index);
                self.record_position();
                return Task::batch([
                    self.load_pages(),
                    self.enhance_next(),
                    self.scroll_reader(),
                    widget::operation::scroll_to(
                        "paged-reader",
                        scrollable::AbsoluteOffset { x: 0.0, y: 0.0 },
                    ),
                ]);
            }
            Message::Mode(mode) => {
                self.settings.mode = mode;
                if let Some(r) = self.reader.as_mut() {
                    r.mode = mode;
                    r.scroll = r.offset(r.current);
                }
                self.save();
                return Task::batch([self.load_pages(), self.scroll_reader()]);
            }
            Message::Fit(fit) => {
                self.settings.fit = fit;
                if let Some(r) = self.reader.as_mut() {
                    r.fit = fit;
                }
                self.save();
            }
            Message::Zoom(z) => {
                if let Some(r) = self.reader.as_mut() {
                    let anchor = r.locate(r.scroll);
                    let fraction = (r.scroll - r.offset(anchor)) / r.page_height(anchor);
                    r.zoom = z.clamp(0.25, 4.0);
                    r.scroll = r.offset(anchor) + fraction * r.page_height(anchor);
                }
                return Task::batch([self.load_pages(), self.scroll_reader()]);
            }
            Message::Double(v) => {
                self.settings.double_page = v;
                self.save();
            }
            Message::ScrollBy(delta) => {
                if let Some(r) = self.reader.as_ref() {
                    let y = (r.scroll + delta)
                        .clamp(0.0, (r.offset(r.urls.len()) - r.viewport).max(0.0));
                    let height = r.viewport;
                    let loading = self.update(Message::Scroll(y, height));
                    return Task::batch([loading, self.scroll_reader()]);
                }
            }
            Message::Scroll(y, height) => {
                if let Some(r) = self.reader.as_mut() {
                    let was_finished = r.finished();
                    r.scroll = y;
                    r.viewport = height;
                    let current = r.locate(y + height * 0.3);
                    let changed = current != r.current || was_finished != r.finished();
                    self.set_reader_page(current);
                    if changed {
                        self.record_position();
                    }
                    return Task::batch([self.load_pages(), self.enhance_next()]);
                }
            }
            Message::Resize(w, h) => {
                self.size = (w, h);
                if let Some(r) = self.reader.as_mut() {
                    let anchor = r.locate(r.scroll);
                    let fraction = (r.scroll - r.offset(anchor)) / r.page_height(anchor);
                    r.width = self.settings.strip_width.min((w - 40.0).max(1.0));
                    r.viewport = (h - if self.chrome {
                        crate::ui::reader_chrome_height(self.reader_options)
                    } else {
                        20.0
                    })
                    .max(1.0);
                    r.scroll = r.offset(anchor) + fraction * r.page_height(anchor);
                }
                return Task::batch([self.load_pages(), self.scroll_reader()]);
            }
            Message::ToggleChrome => {
                self.chrome = !self.chrome;
                if let Some(reader) = &mut self.reader {
                    reader.viewport = (self.size.1
                        - if self.chrome {
                            crate::ui::reader_chrome_height(self.reader_options)
                        } else {
                            20.0
                        })
                    .max(1.0);
                }
            }
            Message::Fullscreen => {
                self.fullscreen = !self.fullscreen;
                let mode = if self.fullscreen {
                    window::Mode::Fullscreen
                } else {
                    window::Mode::Windowed
                };
                return window::latest().then(move |id| {
                    if let Some(id) = id {
                        window::set_mode(id, mode)
                    } else {
                        Task::none()
                    }
                });
            }
            Message::Incognito(v) => self.incognito = v,
            Message::Upscale(v) => {
                self.settings.upscale = v;
                self.save();
                if let (Some(e), Some(r)) = (&self.engine, self.reader.as_mut()) {
                    self.generation = e.next_generation();
                    r.images.clear();
                    r.loading.clear();
                    r.enhancing.clear();
                    r.errors.clear();
                    r.notice = None;
                }
                return self.load_pages();
            }
            Message::Refresh => {
                if self.screen == Screen::Browse {
                    self.source_page = 1;
                    return self.browse_source();
                }
                return self.refresh();
            }
            Message::RefreshSeries => {
                if let (Some(e), Some(m)) = (self.engine.clone(), self.selected.clone()) {
                    return self.action(async move {
                        e.server.refresh_manga(m.id).await?;
                        Ok("Series refreshed".into())
                    });
                }
            }
            Message::LibraryToggle => {
                if let (Some(e), Some(m)) = (self.engine.clone(), self.selected.clone()) {
                    return self.action(async move {
                        e.server.set_library(m.id, !m.in_library).await?;
                        Ok("Library updated".into())
                    });
                }
            }
            Message::Bookmark(id, value) => {
                if let Some(e) = self.engine.clone() {
                    return self.action(async move {
                        e.server
                            .patch_chapter(id, json!({"isBookmarked":value}))
                            .await?;
                        Ok("Bookmark updated".into())
                    });
                }
            }
            Message::MarkRead(id, value) => {
                let previous = std::mem::take(&mut self.chapter_selection);
                self.chapter_selection.insert(id);
                let task = self.update(Message::MarkSelectedChapters(value));
                self.chapter_selection = previous;
                return task;
            }
            Message::ServerDownload(id) => {
                if let Some(e) = self.engine.clone() {
                    return self.action(async move {
                        e.server.enqueue(&[id]).await?;
                        Ok("Queued on server".into())
                    });
                }
            }
            Message::Action(sid, result) if sid == self.session => {
                self.busy = false;
                match result {
                    Err(e) => self.error(e),
                    Ok(text) => {
                        self.status = text;
                        if self.screen == Screen::Categories {
                            self.category_name.clear();
                            let e = self.engine.clone().unwrap();
                            let sid = self.session;
                            return Task::batch([
                                self.refresh(),
                                Task::perform(
                                    async move {
                                        e.server.categories().await.map_err(|e| e.to_string())
                                    },
                                    move |r| Message::Categories(sid, r),
                                ),
                            ]);
                        }
                        if self.screen == Screen::Series
                            && let Some(m) = &self.selected
                        {
                            let id = m.id;
                            let chapters = self.update(Message::Open(id));
                            return Task::batch([chapters, self.refresh()]);
                        }
                        return self.refresh();
                    }
                }
            }
            Message::Captured(Err(e)) => self.status = e,
            Message::Tick => {
                if self.capture_path.is_some()
                    && self.settings.upscale
                    && self
                        .reader
                        .as_ref()
                        .is_some_and(|r| r.images.get(&r.current).is_none_or(|page| !page.enhanced))
                {
                    return Task::none();
                }
                if let Some(path) = self.capture_path.take() {
                    return window::latest().then(move |id| {
                        let path = path.clone();
                        if let Some(id) = id {
                            window::screenshot(id).then(move |shot| {
                                let path = path.clone();
                                Task::perform(
                                    async move {
                                        tokio::task::spawn_blocking(
                                            move || -> anyhow::Result<()> {
                                                let file = std::fs::OpenOptions::new()
                                                    .write(true)
                                                    .create_new(true)
                                                    .open(path)?;
                                                image::write_buffer_with_format(
                                                    &mut std::io::BufWriter::new(file),
                                                    &shot.rgba,
                                                    shot.size.width,
                                                    shot.size.height,
                                                    image::ExtendedColorType::Rgba8,
                                                    image::ImageFormat::Png,
                                                )?;
                                                Ok(())
                                            },
                                        )
                                        .await
                                        .map_err(|e| e.to_string())?
                                        .map_err(|e| e.to_string())
                                    },
                                    Message::Captured,
                                )
                            })
                        } else {
                            Task::none()
                        }
                    });
                }
                if !self.connected {
                    return Task::none();
                }
                if let Some(e) = self.engine.clone() {
                    let sid = self.session;
                    let mut jobs = Vec::new();

                    if !self.sync_busy && !self.incognito && !e.server.read_only {
                        self.sync_busy = true;
                        jobs.push(Task::perform(
                            async move { e.sync_progress().await.map_err(|e| e.to_string()) },
                            move |r| Message::Sync(sid, r),
                        ));
                    }
                    return Task::batch(jobs);
                }
            }
            Message::Sync(sid, result) if sid == self.session => {
                self.sync_busy = false;
                match result {
                    Ok(conflicts) => {
                        if !conflicts.is_empty() {
                            self.status="Reading positions changed on another device. Review conflicts in Settings.".into();
                        }
                        self.conflicts = conflicts;
                    }
                    Err(e) => {
                        self.status = format!(
                            "Synchronization unfinished; pending changes are saved locally. {e}"
                        )
                    }
                }
            }
            Message::KeepProgress(id) => {
                if let Some(e) = &self.engine {
                    match self.store.resolve_local_progress(&e.server.scope, id) {
                        Ok(()) => {
                            self.conflicts.retain(|c| c.id != id);
                            return self.update(Message::Tick);
                        }
                        Err(e) => self.error(e.to_string()),
                    }
                }
            }
            Message::UseRemoteProgress(id) => {
                if let Some(e) = self.engine.clone() {
                    let sid = self.session;
                    return Task::perform(
                        async move {
                            let pending = e
                                .store
                                .pending_progress(&e.server.scope)
                                .map_err(|e| e.to_string())?;
                            let revision = pending
                                .iter()
                                .find(|(p, _)| p.chapter_id == id)
                                .map(|(_, revision)| *revision)
                                .ok_or("This conflict is already resolved")?;
                            let remote = e.server.chapter(id).await.map_err(|e| e.to_string())?;
                            e.store
                                .resolve_remote_progress(&e.server.scope, &remote, revision)
                                .map_err(|e| e.to_string())?;
                            Ok(Box::new(remote))
                        },
                        move |r| Message::ResolvedProgress(sid, id, r),
                    );
                }
            }
            Message::ResolvedProgress(sid, id, result) if sid == self.session => match result {
                Ok(remote) => {
                    if let Some(chapter) = self.chapters.iter_mut().find(|c| c.id == id) {
                        *chapter = *remote;
                    }
                    self.conflicts.retain(|c| c.id != id);
                    self.status = "Server reading position kept".into();
                }
                Err(e) => self.error(e),
            },
            Message::ReaderCache(value) => {
                self.settings.cache_mib = value as usize;
                self.save();
            }
            Message::DiskCache(value) => {
                self.settings.disk_cache_mib = value as usize;
                if let Some(e) = &self.engine {
                    e.cache_budget
                        .store(value as u64 * 1024 * 1024, Ordering::Relaxed);
                }
                self.save();
            }
            Message::Prefetch(value) => {
                self.settings.prefetch = value as usize;
                self.save();
            }
            Message::ClearCache => {
                if let Some(e) = self.engine.clone() {
                    return self.action(async move {
                        tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
                            e.store
                                .trim_cache(&e.server.scope, 0, std::path::Path::new(""))?;
                            Ok("Unpinned image cache cleared".into())
                        })
                        .await?
                    });
                }
            }
            Message::ThemeName(name) => {
                self.settings.theme = name;
                self.save();
            }
            Message::Accent(value) => {
                self.settings.accent = value;
                self.save();
            }
            Message::BrowseSource(source) => {
                self.source = Some(source);
                self.source_page = 1;
                self.browse.clear();
                return self.browse_source();
            }
            Message::BrowseResults(sid, request, page, result)
                if sid == self.session && request == self.browse_request =>
            {
                self.busy = false;
                match result {
                    Ok((items, more)) => {
                        if page == 1 {
                            self.browse = items
                        } else {
                            self.browse.extend(items)
                        }
                        self.source_more = more;
                        return self.covers();
                    }
                    Err(e) => {
                        self.source_page = page.saturating_sub(1).max(1);
                        self.error(e);
                    }
                }
            }
            Message::Latest(v) => {
                self.latest = v;
                self.source_page = 1;
                return self.browse_source();
            }
            Message::MoreBrowse => {
                self.source_page += 1;
                return self.browse_source();
            }
            Message::HideCategory(id, hidden) => {
                if let Some(engine) = &self.engine {
                    let mut ids = self.hidden_categories.clone();
                    if hidden {
                        ids.insert(id);
                    } else {
                        ids.remove(&id);
                    }
                    if let Err(e) = self
                        .store
                        .put(&engine.server.scope, "hidden_categories", &ids)
                    {
                        self.error(format!("Could not save hidden categories: {e}"));
                        return Task::none();
                    }
                    self.hidden_categories = ids;
                    if self.shelf == Some(id) {
                        self.shelf = None;
                    }
                    self.grid_page = 0;
                }
            }
            Message::CategoryName(v) => self.category_name = v,
            Message::AddCategory => {
                if let Some(e) = self.engine.clone() {
                    let name = self.category_name.trim().to_owned();
                    if !name.is_empty() {
                        return self.action(async move{e.server.mutate("mutation Category($name:String!){createCategory(input:{name:$name}){category{id}}}",json!({"name":name})).await?;Ok("Category created".into())});
                    }
                }
            }
            Message::RenameCategory(id, name) => {
                if let Some(c) = self.categories.iter_mut().find(|c| c.id == id) {
                    c.name = name;
                }
            }
            Message::SaveCategory(id) => {
                if let (Some(e), Some(c)) = (
                    self.engine.clone(),
                    self.categories.iter().find(|c| c.id == id),
                ) {
                    let name = c.name.trim().to_owned();
                    if name.is_empty() {
                        self.error("Category name cannot be empty".into());
                        return Task::none();
                    }
                    return self.action(async move { e.server.mutate("mutation Rename($id:Int!,$name:String!){updateCategory(input:{id:$id,patch:{name:$name}}){category{id}}}",json!({"id":id,"name":name})).await?; Ok("Category renamed".into()) });
                }
            }
            Message::DefaultCategory(id, value) => {
                if let Some(e) = self.engine.clone() {
                    return self.action(async move { e.server.mutate("mutation Default($id:Int!,$value:Boolean!){updateCategory(input:{id:$id,patch:{default:$value}}){category{id}}}",json!({"id":id,"value":value})).await?; Ok("Default category updated".into()) });
                }
            }
            Message::MoveCategory(id, delta) => {
                if let (Some(e), Some(position)) = (
                    self.engine.clone(),
                    self.categories
                        .iter()
                        .filter(|c| !c.is_default_category)
                        .position(|c| c.id == id),
                ) {
                    let count = self
                        .categories
                        .iter()
                        .filter(|c| !c.is_default_category)
                        .count();
                    let position = position
                        .saturating_add_signed(delta as isize)
                        .min(count.saturating_sub(1))
                        + 1;
                    return self.action(async move { e.server.mutate("mutation Order($id:Int!,$position:Int!){updateCategoryOrder(input:{id:$id,position:$position}){categories{id}}}",json!({"id":id,"position":position})).await?; Ok("Category order updated".into()) });
                }
            }
            Message::DeleteCategory(id) => self.category_delete = id,
            Message::ConfirmDeleteCategory => {
                if let (Some(e), Some(id)) = (self.engine.clone(), self.category_delete.take()) {
                    return self.action(async move { e.server.mutate("mutation Delete($id:Int!){deleteCategory(input:{categoryId:$id}){category{id}}}",json!({"id":id})).await?; Ok("Category deleted".into()) });
                }
            }
            Message::AssignCategory(id) => {
                if let (Some(e), Some(m)) = (self.engine.clone(), self.selected.clone()) {
                    let add = !m.categories.nodes.iter().any(|c| c.id == id);
                    return self.action(async move{e.server.mutate("mutation Categories($input:UpdateMangaCategoriesInput!){updateMangaCategories(input:$input){manga{id}}}",json!({"input":{"id":m.id,"patch":if add{json!({"addToCategories":[id]})}else{json!({"removeFromCategories":[id]})}}})).await?;Ok("Category updated".into())});
                }
            }
            Message::Dark(v) => {
                self.settings.dark = v;
                self.save()
            }
            Message::Oled(v) => {
                self.settings.oled = v;
                self.save()
            }
            Message::ModelPath(v) => self.settings.model_path = v,
            Message::SaveSettings => {
                self.save();
                self.status = "Settings saved".into();
            }
            Message::StripWidth(v) => {
                self.settings.strip_width = v;
                self.save();
            }
            _ => {}
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(root: &std::path::Path) -> App {
        let store = Arc::new(Store::open(root).unwrap());
        let settings = Settings {
            remember_login: false,
            model_path: "fixture-model.json".into(),
            ..Default::default()
        };
        let scope = crate::api::account_scope(&settings).unwrap();
        let private = Category {
            id: 7,
            name: "Private".into(),
            ..Default::default()
        };
        let public = Category {
            id: 9,
            name: "Public".into(),
            ..Default::default()
        };
        let library = vec![
            Manga {
                id: 1,
                title: "Hidden title".into(),
                categories: Nodes {
                    nodes: vec![private.clone(), public.clone()],
                },
                ..Default::default()
            },
            Manga {
                id: 2,
                title: "Visible title".into(),
                categories: Nodes {
                    nodes: vec![public.clone()],
                },
                ..Default::default()
            },
            Manga {
                id: 3,
                title: "Uncategorized title".into(),
                ..Default::default()
            },
        ];
        store.put("app", "settings", &settings).unwrap();
        store.put(&scope, "library", &library).unwrap();
        store
            .put(
                &scope,
                "categories",
                &vec![
                    private,
                    public,
                    Category {
                        id: 42,
                        name: "Default".into(),
                        is_default_category: true,
                        ..Default::default()
                    },
                ],
            )
            .unwrap();
        app_from_store(store)
    }

    fn app_from_store(store: Arc<Store>) -> App {
        App::boot_store(store).0
    }

    #[tokio::test]
    async fn hidden_categories_persist_and_filter_overlapping_and_uncategorized_titles() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = fixture(dir.path());
        let scope = app.engine.as_ref().unwrap().server.scope.clone();
        app.shelf = Some(7);
        let _ = app.update(Message::HideCategory(7, true));
        assert_eq!(app.shelf, None);
        assert_eq!(app.filtered().len(), 2);
        app.search = "Hidden".into();
        assert!(app.filtered().is_empty());
        let mut app = app_from_store(Arc::new(Store::open(dir.path()).unwrap()));
        assert!(app.hidden_categories.contains(&7));
        app.shelf = Some(9);
        assert_eq!(
            app.filtered().iter().map(|m| m.id).collect::<Vec<_>>(),
            vec![2]
        );
        app.shelf = None;
        let _ = app.update(Message::HideCategory(42, true));
        assert_eq!(app.filtered().len(), 1);
        let _ = app.update(Message::HideCategory(7, false));
        assert_eq!(app.filtered().len(), 2);
        let _ = app.update(Message::HideCategory(42, false));
        assert_eq!(app.filtered().len(), 3);
        let original: Vec<Manga> = app.store.get(&scope, "library").unwrap().unwrap();
        assert_eq!(original.len(), 3);
        assert_eq!(original[0].categories.nodes.len(), 2);
    }

    #[tokio::test]
    async fn hidden_categories_are_separate_for_each_account() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = fixture(dir.path());
        let old_scope = app.engine.as_ref().unwrap().server.scope.clone();
        let _ = app.update(Message::HideCategory(7, true));
        app.settings.username = "another-account".into();
        app.save();
        let app = app_from_store(app.store.clone());
        assert!(app.hidden_categories.is_empty());
        let old: HashSet<i64> = app
            .store
            .get(&old_scope, "hidden_categories")
            .unwrap()
            .unwrap();
        assert!(old.contains(&7));
    }

    #[tokio::test]
    async fn delayed_credentials_never_replace_an_edited_account() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = fixture(dir.path());
        let _ = app.update(Message::Password("old password".into()));
        let _ = app.update(Message::Headers("X-Test: old".into()));
        let revision = app.login_revision;
        let _ = app.update(Message::Username("another-account".into()));
        let _ = app.update(Message::CredentialsLoaded(
            revision,
            Ok(Some(Credentials {
                password: "old saved password".into(),
                headers: "X-Test: old saved".into(),
            })),
        ));
        assert!(app.password.is_empty());
        assert!(app.headers.is_empty());
        assert!(!app.connected);
        assert_eq!(app.session, 0);
    }

    #[tokio::test]
    async fn restored_credentials_start_reconnecting_without_entering_the_settings_database() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = fixture(dir.path());
        let _ = app.update(Message::CredentialsLoaded(
            app.login_revision,
            Ok(Some(Credentials {
                password: "fixture-secret-only".into(),
                headers: "X-Fixture: header-secret".into(),
            })),
        ));
        assert_eq!(app.password, "fixture-secret-only");
        assert_eq!(app.headers, "X-Fixture: header-secret");
        assert!(app.busy && app.credential_busy);
        assert_eq!(app.session, 1);
        let saved: serde_json::Value = app.store.get("app", "settings").unwrap().unwrap();
        let saved = saved.to_string();
        assert!(!saved.contains("fixture-secret-only"));
        assert!(!saved.contains("header-secret"));
    }
}
