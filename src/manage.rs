use crate::ui::{self, button, text_input};
use crate::{api::Server, storage::atomic_write};
use anyhow::Result;
use iced::widget::{Column, Space, column, container, row, scrollable, text};
use iced::{Element, Length, Task};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Panel {
    #[default]
    Updates,
    History,
    Extensions,
    ServerDownloads,
    Trackers,
    Backups,
}
impl Panel {
    pub fn name(self) -> &'static str {
        match self {
            Self::Updates => "Updates",
            Self::History => "History",
            Self::Extensions => "Extensions",
            Self::ServerDownloads => "Server downloads",
            Self::Trackers => "Tracking",
            Self::Backups => "Backups",
        }
    }
}
#[derive(Debug, Clone)]
pub enum Msg {
    Open(Panel),
    ReadChapter(i64, i64),
    Loaded(u64, Result<Value, String>),
    Search(String),
    Path(String),
    Repository(String),
    Run(Action),
    Done(u64, Result<String, String>),
    Confirm,
    Cancel,
}
#[derive(Debug, Clone)]
pub enum Action {
    UpdateLibrary,
    StopUpdate,
    RefreshExtensions,
    Install(String),
    Upgrade(String),
    Uninstall(String),
    StartDownloads,
    StopDownloads,
    CancelDownload(i64),
    Backup,
    AddRepository,
    RemoveRepository(String),
}
#[derive(Default)]
pub struct Manager {
    pub panel: Panel,
    pub data: Value,
    pub search: String,
    pub path: String,
    pub repository: String,
    pub status: String,
    pub busy: bool,
    request: u64,
    confirm: Option<Action>,
}
impl Manager {
    pub fn update(&mut self, msg: Msg, server: Server) -> Task<Msg> {
        match msg {
            Msg::Open(panel) => {
                self.panel = panel;
                self.request += 1;
                let id = self.request;
                self.busy = true;
                self.data = Value::Null;
                self.confirm = None;
                return Task::perform(
                    async move { load(&server, panel).await.map_err(|e| e.to_string()) },
                    move |r| Msg::Loaded(id, r),
                );
            }
            Msg::Loaded(id, result) if id == self.request => {
                self.busy = false;
                match result {
                    Ok(data) => self.data = data,
                    Err(e) => self.status = e,
                }
            }
            Msg::Search(s) => self.search = s,
            Msg::Path(s) => self.path = s,
            Msg::Repository(s) => self.repository = s,
            Msg::Run(action) => {
                if matches!(action, Action::Uninstall(_) | Action::RemoveRepository(_)) {
                    self.confirm = Some(action);
                    return Task::none();
                }
                return self.run(action, server);
            }
            Msg::Confirm => {
                if let Some(action) = self.confirm.take() {
                    return self.run(action, server);
                }
            }
            Msg::Cancel => self.confirm = None,
            Msg::Done(id, result) if id == self.request => {
                self.busy = false;
                match result {
                    Ok(status) => {
                        self.status = status;
                        return self.update(Msg::Open(self.panel), server);
                    }
                    Err(e) => self.status = e,
                }
            }
            _ => {}
        }
        Task::none()
    }
    fn run(&mut self, action: Action, server: Server) -> Task<Msg> {
        self.busy = true;
        self.request += 1;
        let id = self.request;
        let path = self.path.clone();
        let repo = self.repository.clone();
        Task::perform(
            async move {
                execute(&server, action, &path, &repo)
                    .await
                    .map_err(|e| e.to_string())
            },
            move |r| Msg::Done(id, r),
        )
    }
    pub fn view(&self) -> Element<'_, Msg> {
        let mut body = column![ui::heading(self.panel.name()).size(28),].spacing(16);
        if matches!(self.panel, Panel::Updates | Panel::History) {
            body = body.push(
                row![
                    button("Updates")
                        .on_press(Msg::Open(Panel::Updates))
                        .style(ui::ghost),
                    button("History")
                        .on_press(Msg::Open(Panel::History))
                        .style(ui::ghost)
                ]
                .spacing(8),
            );
        }
        if !self.status.is_empty() {
            body = body.push(ui::caption(&self.status));
        }
        if self.confirm.is_some() {
            body = body.push(
                container(
                    column![
                        text("Remove this extension or repository from the server?"),
                        row![
                            button("Remove").on_press(Msg::Confirm).style(ui::danger),
                            button("Cancel").on_press(Msg::Cancel)
                        ]
                        .spacing(12)
                    ]
                    .spacing(12),
                )
                .padding(16)
                .style(ui::panel),
            );
        }
        if self.busy {
            body = body.push(text("Loading…"));
        }
        match self.panel {
            Panel::Updates | Panel::History => {
                if self.panel == Panel::Updates {
                    body = body.push(
                        row![
                            button("Check library for new chapters")
                                .on_press(Msg::Run(Action::UpdateLibrary)),
                            button("Stop update")
                                .on_press(Msg::Run(Action::StopUpdate))
                                .style(ui::secondary)
                        ]
                        .spacing(12),
                    );
                }
                let mut rows = Column::new().spacing(8);
                for item in nodes(&self.data, "chapters") {
                    rows = rows.push(
                        button(
                            column![
                                text(label(&item["manga"], "title")).size(16),
                                text(label(item, "name")).size(13)
                            ]
                            .spacing(4),
                        )
                        .on_press(Msg::ReadChapter(
                            item["manga"]["id"].as_i64().unwrap_or_default(),
                            item["id"].as_i64().unwrap_or_default(),
                        ))
                        .padding(12)
                        .width(Length::Fill)
                        .style(ui::secondary),
                    );
                }
                body = body.push(scrollable(rows).height(Length::Fill));
            }
            Panel::Extensions => {
                body = body.push(
                    row![
                        text_input("Filter extensions", &self.search)
                            .on_input(Msg::Search)
                            .padding(10),
                        button("Check for updates").on_press(Msg::Run(Action::RefreshExtensions))
                    ]
                    .spacing(12),
                );
                body = body.push(
                    row![
                        text_input("Extension repository URL", &self.repository)
                            .on_input(Msg::Repository)
                            .padding(10),
                        button("Add repository").on_press(Msg::Run(Action::AddRepository))
                    ]
                    .spacing(12),
                );
                for store in nodes(&self.data, "extensionStores") {
                    body = body.push(
                        row![
                            text(label(store, "name")).width(Length::Fill),
                            button("Remove repository")
                                .on_press(Msg::Run(Action::RemoveRepository(
                                    label(store, "indexUrl").to_owned()
                                )))
                                .style(ui::ghost)
                        ]
                        .spacing(12),
                    );
                }
                let mut rows = Column::new().spacing(6);
                for item in nodes(&self.data, "extensions")
                    .iter()
                    .filter(|v| {
                        label(v, "name")
                            .to_lowercase()
                            .contains(&self.search.to_lowercase())
                    })
                    .take(150)
                {
                    let pkg = label(item, "pkgName").to_owned();
                    let installed = item["isInstalled"].as_bool().unwrap_or(false);
                    let action = if installed {
                        Action::Uninstall(pkg.clone())
                    } else {
                        Action::Install(pkg.clone())
                    };
                    let mut line = row![
                        column![
                            text(label(item, "name")),
                            text(format!(
                                "{} · {}",
                                label(item, "lang"),
                                label(item, "versionName")
                            ))
                            .size(12)
                        ]
                        .width(Length::Fill),
                        button(if installed { "Uninstall" } else { "Install" })
                            .on_press(Msg::Run(action))
                            .style(ui::secondary)
                    ]
                    .spacing(12);
                    if item["hasUpdate"] == true {
                        line = line.push(button("Update").on_press(Msg::Run(Action::Upgrade(pkg))));
                    }
                    rows = rows.push(container(line).padding(12).style(ui::panel));
                }
                body = body.push(scrollable(rows).height(Length::Fill));
            }
            Panel::ServerDownloads => {
                body = body.push(
                    row![
                        button("Resume queue").on_press(Msg::Run(Action::StartDownloads)),
                        button("Pause queue")
                            .on_press(Msg::Run(Action::StopDownloads))
                            .style(ui::secondary)
                    ]
                    .spacing(12),
                );
                let mut rows = Column::new().spacing(8);
                if let Some(queue) = self.data["downloadStatus"]["queue"].as_array() {
                    for d in queue {
                        rows = rows.push(
                            container(
                                row![
                                    column![
                                        text(label(&d["manga"], "title")),
                                        text(label(&d["chapter"], "name")).size(13)
                                    ]
                                    .width(Length::Fill),
                                    text(format!(
                                        "{} · {:.0}%",
                                        label(d, "state"),
                                        d["progress"].as_f64().unwrap_or(0.0) * 100.0
                                    )),
                                    button("Cancel")
                                        .on_press(Msg::Run(Action::CancelDownload(
                                            d["chapter"]["id"].as_i64().unwrap_or(0)
                                        )))
                                        .style(ui::ghost)
                                ]
                                .spacing(12),
                            )
                            .padding(12)
                            .style(ui::panel),
                        );
                    }
                }
                body = body.push(scrollable(rows).height(Length::Fill));
            }
            Panel::Trackers => {
                for t in nodes(&self.data, "trackers") {
                    body = body.push(
                        container(row![
                            text(label(t, "name")).width(Length::Fill),
                            text(if t["isLoggedIn"] == true {
                                "Connected"
                            } else {
                                "Not connected"
                            })
                        ])
                        .padding(16)
                        .style(ui::panel),
                    );
                }
            }
            Panel::Backups => {
                body=body.push(text("Create a Suwayomi backup containing categories, chapters, history, and tracking."))
                    .push(text_input("Save backup to an absolute path",&self.path).on_input(Msg::Path).padding(12))
                    .push(button("Create and save backup").on_press(Msg::Run(Action::Backup)).padding(12));
            }
        }
        body.push(Space::new().height(1)).into()
    }
}
fn label<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn nodes<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v[key]["nodes"].as_array().map(Vec::as_slice).unwrap_or(&[])
}
async fn load(s: &Server, p: Panel) -> Result<Value> {
    let query = match p {
        Panel::Updates => {
            "query Updates{chapters(first:100,filter:{manga:{inLibrary:{equalTo:true}}},order:[{by:FETCHED_AT,byType:DESC}]){nodes{id name manga{id title}}}}"
        }
        Panel::History => {
            "query History{chapters(first:100,filter:{lastReadAt:{greaterThan:\"0\"}},order:[{by:LAST_READ_AT,byType:DESC}]){nodes{id name manga{id title}}}}"
        }
        Panel::Extensions => {
            "query Extensions{extensions{nodes{pkgName name lang versionName isInstalled hasUpdate isObsolete}} extensionStores{nodes{name indexUrl}}}"
        }
        Panel::ServerDownloads => {
            "query Downloads{downloadStatus{state queue{state progress chapter{id name} manga{id title}}}}"
        }
        Panel::Trackers => "query Trackers{trackers{nodes{id name isLoggedIn authUrl}}}",
        Panel::Backups => "query About{aboutServer{name version}}",
    };
    s.query(query, json!({})).await
}
async fn execute(s: &Server, a: Action, path: &str, repository: &str) -> Result<String> {
    match a {
        Action::UpdateLibrary => s.update_library().await?,
        Action::StopUpdate => {
            s.mutate(
                "mutation Stop{updateStop(input:{}){clientMutationId}}",
                json!({}),
            )
            .await?;
        }
        Action::RefreshExtensions => {
            s.mutate(
                "mutation Refresh{fetchExtensions(input:{}){extensions{pkgName}}}",
                json!({}),
            )
            .await?;
        }
        Action::Install(ref id) | Action::Upgrade(ref id) | Action::Uninstall(ref id) => {
            let patch = match a {
                Action::Install(_) => json!({"install":true}),
                Action::Upgrade(_) => json!({"update":true}),
                _ => json!({"uninstall":true}),
            };
            s.mutate("mutation Extension($input:UpdateExtensionInput!){updateExtension(input:$input){extension{pkgName}}}",json!({"input":{"id":id,"patch":patch}})).await?;
        }
        Action::StartDownloads => {
            s.mutate(
                "mutation Start{startDownloader(input:{}){downloadStatus{state}}}",
                json!({}),
            )
            .await?;
        }
        Action::StopDownloads => {
            s.mutate(
                "mutation Stop{stopDownloader(input:{}){downloadStatus{state}}}",
                json!({}),
            )
            .await?;
        }
        Action::CancelDownload(id) => {
            s.mutate("mutation Cancel($id:Int!){dequeueChapterDownload(input:{id:$id}){downloadStatus{state}}}",json!({"id":id})).await?;
        }
        Action::AddRepository => {
            s.mutate("mutation Repository($url:String!){addExtensionStore(input:{indexUrl:$url}){extensionStore{name}}}",json!({"url":repository})).await?;
        }
        Action::RemoveRepository(url) => {
            s.mutate("mutation Repository($url:String!){removeExtensionStore(input:{indexUrl:$url}){extensionStore{name}}}",json!({"url":url})).await?;
        }
        Action::Backup => {
            let path = std::path::PathBuf::from(path);
            anyhow::ensure!(path.is_absolute(), "Choose an absolute backup path");
            anyhow::ensure!(
                !path.exists(),
                "That file already exists; choose a new filename"
            );
            let result = s
                .mutate("mutation Backup{createBackup(input:{}){url}}", json!({}))
                .await?;
            let url = result["createBackup"]["url"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing backup URL"))?;
            let bytes = s.image(url).await?;
            tokio::task::spawn_blocking(move || atomic_write(&path, &bytes)).await??;
            return Ok("Backup saved".into());
        }
    }
    Ok("Done".into())
}
