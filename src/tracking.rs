use crate::ui::{self, button, checkbox, pick_list, text_input};
use crate::{api::Server, domain::Manga};
use anyhow::{Context, Result, ensure};
use iced::widget::{column, container, row, scrollable, text};
use iced::{Element, Length, Task};
use serde::Deserialize;
use serde_json::{Value, json};

const TRACKERS: &str = "id name isLoggedIn authUrl scores statuses{name value} supportsPrivateTracking supportsReadingDates supportsTrackDeletion";
const RECORDS: &str = "id trackerId title lastChapterRead totalChapters displayScore status startDate finishDate private";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tracker {
    id: i64,
    name: String,
    is_logged_in: bool,
    auth_url: Option<String>,
    scores: Vec<String>,
    statuses: Vec<Status>,
    supports_private_tracking: bool,
    supports_reading_dates: bool,
    supports_track_deletion: bool,
}
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Status {
    name: String,
    value: i64,
}
impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    id: i64,
    tracker_id: i64,
    title: String,
    last_chapter_read: f64,
    total_chapters: i64,
    display_score: String,
    status: i64,
    start_date: String,
    finish_date: String,
    private: bool,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    remote_id: String,
    title: String,
    publishing_type: String,
    publishing_status: String,
    total_chapters: i64,
}
#[derive(Debug, Clone)]
pub enum Msg {
    Open(Option<Box<Manga>>),
    Loaded(u64, Result<(Vec<Tracker>, Vec<Record>), String>),
    Select(i64),
    Username(String),
    Password(String),
    Callback(String),
    Credentials,
    OpenBrowser,
    OAuth,
    Logout,
    Search(String),
    Find,
    Found(u64, Result<Vec<Match>, String>),
    Bind(String),
    Edit(i64),
    Chapter(String),
    Score(String),
    Status(Status),
    Start(String),
    Finish(String),
    Private(bool),
    Save,
    Unlink(i64),
    DeleteRemote(bool),
    ConfirmUnlink,
    Cancel,
    Done(u64, Result<String, String>),
}
#[derive(Default)]
pub struct Tracking {
    manga: Option<Manga>,
    trackers: Vec<Tracker>,
    records: Vec<Record>,
    matches: Vec<Match>,
    selected: Option<i64>,
    username: String,
    password: String,
    callback: String,
    search: String,
    edit: Option<Edit>,
    unlink: Option<i64>,
    delete_remote: bool,
    busy: bool,
    status: String,
    request: u64,
}
struct Edit {
    id: i64,
    chapter: String,
    score: String,
    status: Option<Status>,
    start: String,
    finish: String,
    private: bool,
}
impl Tracking {
    fn selected(&self) -> Option<&Tracker> {
        self.trackers.iter().find(|t| Some(t.id) == self.selected)
    }
    fn reload(&mut self, server: Server) -> Task<Msg> {
        self.request += 1;
        self.busy = true;
        let request = self.request;
        let manga = self.manga.as_ref().map(|m| m.id);
        Task::perform(
            async move { load(&server, manga).await.map_err(|e| e.to_string()) },
            move |r| Msg::Loaded(request, r),
        )
    }
    fn action(
        &mut self,
        future: impl std::future::Future<Output = Result<String>> + Send + 'static,
    ) -> Task<Msg> {
        self.request += 1;
        self.busy = true;
        let request = self.request;
        Task::perform(
            async move { future.await.map_err(|e| e.to_string()) },
            move |r| Msg::Done(request, r),
        )
    }
    pub fn update(&mut self, msg: Msg, server: Server) -> Task<Msg> {
        match msg {
            Msg::Open(manga) => {
                self.manga = manga.map(|m| *m);
                self.search = self
                    .manga
                    .as_ref()
                    .map(|m| m.title.clone())
                    .unwrap_or_default();
                self.records.clear();
                self.matches.clear();
                self.edit = None;
                self.unlink = None;
                return self.reload(server);
            }
            Msg::Loaded(id, result) if id == self.request => {
                self.busy = false;
                match result {
                    Ok((trackers, records)) => {
                        self.trackers = trackers;
                        self.records = records;
                        if !self.trackers.iter().any(|t| Some(t.id) == self.selected) {
                            self.selected = self.trackers.first().map(|t| t.id);
                        }
                    }
                    Err(e) => self.status = e,
                }
            }
            Msg::Select(id) => {
                self.request += 1;
                self.busy = false;
                self.selected = Some(id);
                self.matches.clear();
                self.password.clear();
                self.username.clear();
                self.callback.clear();
                self.edit = None;
                self.unlink = None;
                self.status.clear();
            }
            Msg::Username(s) => self.username = s,
            Msg::Password(s) => self.password = s,
            Msg::Callback(s) => self.callback = s,
            Msg::Search(s) => self.search = s,
            Msg::OpenBrowser => {
                if let Some(url) = self.selected().and_then(|t| t.auth_url.clone()) {
                    return self.action(async move {
                        let url = url::Url::parse(&url)?;
                        ensure!(matches!(url.scheme(),"http"|"https"),"Invalid tracker sign-in address");
                        let status = tokio::task::spawn_blocking(move || std::process::Command::new("xdg-open").arg(url.as_str()).status()).await??;
                        ensure!(status.success(),"Could not open the browser");
                        Ok("Finish sign-in in the browser. If needed, paste the final callback URL below.".into())
                    });
                }
            }
            Msg::Credentials | Msg::OAuth => {
                if let Some(id) = self.selected {
                    let username = self.username.clone();
                    let password = std::mem::take(&mut self.password);
                    let callback = std::mem::take(&mut self.callback);
                    return self.action(async move {
                        let data = if matches!(msg,Msg::Credentials) {
                            server.mutate("mutation Login($input:LoginTrackerCredentialsInput!){loginTrackerCredentials(input:$input){isLoggedIn}}",json!({"input":{"trackerId":id,"username":username,"password":password}})).await?
                        } else {
                            ensure!(!callback.trim().is_empty(),"Paste the callback URL after signing in");
                            server.mutate("mutation Login($input:LoginTrackerOAuthInput!){loginTrackerOAuth(input:$input){isLoggedIn}}",json!({"input":{"trackerId":id,"callbackUrl":callback}})).await?
                        };
                        ensure!(data.as_object().is_some_and(|o|o.values().any(|v|v["isLoggedIn"]==true)),"Tracker sign-in failed");
                        Ok("Tracker connected".into())
                    });
                }
            }
            Msg::Logout => {
                if let Some(id) = self.selected {
                    return self.action(async move {
                        server.mutate("mutation Logout($id:Int!){logoutTracker(input:{trackerId:$id}){isLoggedIn}}",json!({"id":id})).await?;
                        Ok("Tracker disconnected".into())
                    });
                }
            }
            Msg::Find => {
                if let Some(id) = self.selected {
                    self.busy = true;
                    self.request += 1;
                    let request = self.request;
                    let query = self.search.clone();
                    return Task::perform(
                        async move {
                            let data = server.query("query Search($input:SearchTrackerInput!){searchTracker(input:$input){trackSearches{remoteId title publishingType publishingStatus totalChapters}}}",json!({"input":{"trackerId":id,"query":query}})).await.map_err(|e|e.to_string())?;
                            serde_json::from_value(data["searchTracker"]["trackSearches"].clone())
                                .map_err(|e| e.to_string())
                        },
                        move |r| Msg::Found(request, r),
                    );
                }
            }
            Msg::Found(id, result) if id == self.request => {
                self.busy = false;
                match result {
                    Ok(matches) => self.matches = matches,
                    Err(e) => self.status = e,
                }
            }
            Msg::Bind(remote) => {
                if let (Some(tracker), Some(manga)) =
                    (self.selected, self.manga.as_ref().map(|m| m.id))
                {
                    self.matches.clear();
                    return self.action(async move {
                        server.mutate("mutation Bind($input:BindTrackInput!){bindTrack(input:$input){trackRecord{id}}}",json!({"input":{"trackerId":tracker,"mangaId":manga,"remoteId":remote}})).await?;
                        Ok("Title linked".into())
                    });
                }
            }
            Msg::Edit(id) => {
                if let Some(r) = self.records.iter().find(|r| r.id == id) {
                    self.selected = Some(r.tracker_id);
                    self.edit = Some(Edit {
                        id,
                        chapter: r.last_chapter_read.to_string(),
                        score: r.display_score.clone(),
                        status: self
                            .selected()
                            .and_then(|t| t.statuses.iter().find(|s| s.value == r.status).cloned()),
                        start: format_date(&r.start_date),
                        finish: format_date(&r.finish_date),
                        private: r.private,
                    });
                }
            }
            Msg::Chapter(v) => {
                if let Some(e) = self.edit.as_mut() {
                    e.chapter = v;
                }
            }
            Msg::Score(v) => {
                if let Some(e) = self.edit.as_mut() {
                    e.score = v;
                }
            }
            Msg::Status(v) => {
                if let Some(e) = self.edit.as_mut() {
                    e.status = Some(v);
                }
            }
            Msg::Start(v) => {
                if let Some(e) = self.edit.as_mut() {
                    e.start = v;
                }
            }
            Msg::Finish(v) => {
                if let Some(e) = self.edit.as_mut() {
                    e.finish = v;
                }
            }
            Msg::Private(v) => {
                if let Some(e) = self.edit.as_mut() {
                    e.private = v;
                }
            }
            Msg::Save => {
                if let (Some(edit), Some(tracker)) = (&self.edit, self.selected()) {
                    let input = edit_input(edit, tracker);
                    match input {
                        Err(e) => self.status = e.to_string(),
                        Ok(input) => return self.action(async move {
                            server.mutate("mutation Update($input:UpdateTrackInput!){updateTrack(input:$input){trackRecord{id}}}",json!({"input":input})).await?;
                            Ok("Tracking entry saved".into())
                        }),
                    }
                }
            }
            Msg::Unlink(id) => {
                self.unlink = Some(id);
                self.delete_remote = false;
            }
            Msg::DeleteRemote(value) => self.delete_remote = value,
            Msg::ConfirmUnlink => {
                if let Some(id) = self.unlink.take() {
                    let delete = self.delete_remote;
                    return self.action(async move {
                        server.mutate("mutation Unlink($id:Int!,$delete:Boolean!){unbindTrack(input:{recordId:$id,deleteRemoteTrack:$delete}){clientMutationId}}",json!({"id":id,"delete":delete})).await?;
                        Ok("Tracking entry unlinked".into())
                    });
                }
            }
            Msg::Cancel => {
                self.edit = None;
                self.unlink = None;
            }
            Msg::Done(id, result) if id == self.request => {
                self.busy = false;
                match result {
                    Ok(status) => {
                        self.status = status;
                        self.edit = None;
                        return self.reload(server);
                    }
                    Err(e) => self.status = e,
                }
            }
            _ => {}
        }
        Task::none()
    }
    pub fn view(&self) -> Element<'_, Msg> {
        let mut body = column![
            text(
                self.manga
                    .as_ref()
                    .map(|m| format!("Tracking · {}", m.title))
                    .unwrap_or_else(|| "Tracking accounts".into())
            )
            .font(ui::DISPLAY)
            .size(24),
            text(&self.status).size(13)
        ]
        .spacing(16);
        let mut tabs = row![].spacing(8);
        for t in &self.trackers {
            tabs = tabs.push(button(text(&t.name)).on_press(Msg::Select(t.id)).style(
                if Some(t.id) == self.selected {
                    ui::primary
                } else {
                    ui::ghost
                },
            ));
        }
        body = body.push(scrollable(tabs).direction(
            iced::widget::scrollable::Direction::Horizontal(Default::default()),
        ));
        if self.busy {
            body = body.push(text("Loading…"));
        }
        let Some(tracker) = self.selected() else {
            return body.into();
        };
        if !tracker.is_logged_in {
            if tracker.auth_url.as_ref().is_some_and(|s| !s.is_empty()) {
                body = body
                    .push(
                        button("Sign in using your browser")
                            .on_press_maybe((!self.busy).then_some(Msg::OpenBrowser)),
                    )
                    .push(
                        text_input("Callback URL after sign-in", &self.callback)
                            .on_input(Msg::Callback)
                            .padding(12),
                    )
                    .push(
                        button("Complete sign-in")
                            .on_press_maybe((!self.busy).then_some(Msg::OAuth)),
                    );
            } else {
                body = body
                    .push(
                        text_input("Username", &self.username)
                            .on_input(Msg::Username)
                            .padding(12),
                    )
                    .push(
                        text_input("Password or token", &self.password)
                            .secure(true)
                            .on_input(Msg::Password)
                            .padding(12),
                    )
                    .push(
                        button("Sign in").on_press_maybe((!self.busy).then_some(Msg::Credentials)),
                    );
            }
            return scrollable(body).into();
        }
        body = body.push(row![
            text("Connected").width(Length::Fill),
            button("Sign out")
                .on_press_maybe((!self.busy).then_some(Msg::Logout))
                .style(ui::ghost)
        ]);
        if let Some(id) = self.unlink {
            let can_delete = self
                .records
                .iter()
                .find(|r| r.id == id)
                .and_then(|r| self.trackers.iter().find(|t| t.id == r.tracker_id))
                .is_some_and(|t| t.supports_track_deletion);
            let mut confirm = column![
                text("Unlink this tracking entry?"),
                row![
                    button("Unlink")
                        .on_press(Msg::ConfirmUnlink)
                        .style(ui::danger),
                    button("Cancel").on_press(Msg::Cancel)
                ]
                .spacing(12)
            ]
            .spacing(10);
            if can_delete {
                confirm = confirm.push(
                    checkbox(self.delete_remote)
                        .label("Also delete the entry from the tracking service")
                        .on_toggle(Msg::DeleteRemote),
                );
            }
            body = body.push(container(confirm).padding(16).style(ui::panel));
        }
        for r in self.records.iter().filter(|r| r.tracker_id == tracker.id) {
            body = body.push(
                container(
                    column![
                        text(&r.title).size(20),
                        text(format!(
                            "Chapter {} / {} · score {}",
                            r.last_chapter_read, r.total_chapters, r.display_score
                        )),
                        row![
                            button("Edit entry").on_press(Msg::Edit(r.id)),
                            button("Unlink")
                                .on_press(Msg::Unlink(r.id))
                                .style(ui::ghost)
                        ]
                        .spacing(12)
                    ]
                    .spacing(10),
                )
                .padding(16)
                .style(ui::panel),
            );
        }
        if let Some(e) = &self.edit {
            let mut form = column![
                text_input("Chapters read", &e.chapter)
                    .on_input(Msg::Chapter)
                    .padding(12),
                row![
                    pick_list(tracker.statuses.as_slice(), e.status.clone(), Msg::Status),
                    pick_list(tracker.scores.as_slice(), Some(e.score.clone()), Msg::Score)
                ]
                .spacing(12)
            ]
            .spacing(12);
            if tracker.supports_reading_dates {
                form = form.push(
                    row![
                        text_input("Started: YYYY-MM-DD", &e.start)
                            .on_input(Msg::Start)
                            .padding(12),
                        text_input("Finished: YYYY-MM-DD", &e.finish)
                            .on_input(Msg::Finish)
                            .padding(12)
                    ]
                    .spacing(12),
                );
            }
            if tracker.supports_private_tracking {
                form = form.push(
                    checkbox(e.private)
                        .label("Private entry")
                        .on_toggle(Msg::Private),
                );
            }
            body = body.push(
                form.push(
                    row![
                        button("Save entry").on_press_maybe((!self.busy).then_some(Msg::Save)),
                        button("Cancel").on_press(Msg::Cancel)
                    ]
                    .spacing(12),
                ),
            );
        }
        if self.manga.is_some() {
            body = body.push(
                row![
                    text_input("Find this title on the tracker", &self.search)
                        .on_input(Msg::Search)
                        .on_submit(Msg::Find)
                        .padding(12),
                    button("Search").on_press_maybe((!self.busy).then_some(Msg::Find))
                ]
                .spacing(12),
            );
            for result in &self.matches {
                body = body.push(
                    container(
                        row![
                            column![
                                text(&result.title),
                                text(format!(
                                    "{} · {} · {} chapters",
                                    result.publishing_type,
                                    result.publishing_status,
                                    result.total_chapters
                                ))
                                .size(12)
                            ]
                            .width(Length::Fill),
                            button("Link title").on_press_maybe(
                                (!self.busy).then_some(Msg::Bind(result.remote_id.clone()))
                            )
                        ]
                        .spacing(12),
                    )
                    .padding(12)
                    .style(ui::panel),
                );
            }
        }
        scrollable(body).into()
    }
}
pub async fn load(server: &Server, manga: Option<i64>) -> Result<(Vec<Tracker>, Vec<Record>)> {
    let query = if manga.is_some() {
        format!(
            "query Tracking($id:Int!){{trackers{{nodes{{{TRACKERS}}}}} manga(id:$id){{trackRecords{{nodes{{{RECORDS}}}}}}}}}"
        )
    } else {
        format!("query Tracking{{trackers{{nodes{{{TRACKERS}}}}}}}")
    };
    let data = server.query(&query, json!({"id":manga})).await?;
    let trackers = serde_json::from_value(data["trackers"]["nodes"].clone())?;
    let records = if manga.is_some() {
        serde_json::from_value(data["manga"]["trackRecords"]["nodes"].clone())?
    } else {
        Vec::new()
    };
    Ok((trackers, records))
}
fn date_format() -> Vec<time::format_description::BorrowedFormatItem<'static>> {
    time::format_description::parse_borrowed::<2>("[year]-[month]-[day]").unwrap()
}
fn format_date(value: &str) -> String {
    value
        .parse::<i64>()
        .ok()
        .filter(|v| *v > 0)
        .and_then(|v| time::OffsetDateTime::from_unix_timestamp(v / 1000).ok())
        .and_then(|d| d.format(&date_format()).ok())
        .unwrap_or_default()
}
fn parse_date(value: &str) -> Result<String> {
    if value.trim().is_empty() {
        return Ok("0".into());
    }
    let date = time::Date::parse(value.trim(), &date_format())
        .context("Use a valid date in YYYY-MM-DD format")?;
    Ok((date.midnight().assume_utc().unix_timestamp() * 1000).to_string())
}
fn edit_input(e: &Edit, t: &Tracker) -> Result<Value> {
    let chapter: f64 = e.chapter.parse().context("Enter a chapter number")?;
    ensure!(
        chapter.is_finite() && chapter >= 0.0,
        "Chapters read must be a nonnegative number"
    );
    let mut input = json!({"recordId":e.id,"lastChapterRead":chapter,"scoreString":e.score});
    if let Some(status) = &e.status {
        input["status"] = json!(status.value);
    }
    if t.supports_reading_dates {
        input["startDate"] = json!(parse_date(&e.start)?);
        input["finishDate"] = json!(parse_date(&e.finish)?);
        if !e.start.trim().is_empty() && !e.finish.trim().is_empty() {
            ensure!(
                e.start <= e.finish,
                "The finish date must not precede the start date"
            );
        }
    }
    if t.supports_private_tracking {
        input["private"] = json!(e.private);
    }
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reading_dates_validate_and_round_trip() {
        let date = "2024-02-29";
        assert_eq!(format_date(&parse_date(date).unwrap()), date);
        assert!(parse_date("2023-02-29").is_err());
        assert_eq!(parse_date("").unwrap(), "0");
        assert_eq!(format_date("0"), "");
    }
}
