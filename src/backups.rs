use crate::api::Server;
use crate::ui::{self, button, checkbox, text_input};
use iced::widget::{column, container, row, text};
use iced::{Element, Task};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};

const FLAGS: [(&str, &str); 7] = [
    ("includeManga", "Library titles"),
    ("includeCategories", "Categories"),
    ("includeChapters", "Chapters"),
    ("includeHistory", "Reading history"),
    ("includeTracking", "Tracking entries"),
    ("includeClientData", "Client settings"),
    ("includeServerSettings", "Server settings"),
];
#[derive(Debug, Clone)]
pub enum Msg {
    Open,
    Capabilities(u64, Result<Vec<String>, String>),
    Path(String),
    Flag(usize, bool),
    Create,
    Created(u64, Result<(), String>),
    Validate,
    Validated(u64, Result<Value, String>),
    Confirm(bool),
    Restore,
    Started(u64, Result<Value, String>),
    Status(u64, Result<Value, String>),
}
pub struct Backups {
    path: String,
    flags: [bool; 7],
    supported: Vec<String>,
    validation: Option<Value>,
    confirm: bool,
    busy: bool,
    restore: Option<String>,
    status: String,
    request: u64,
}
impl Default for Backups {
    fn default() -> Self {
        Self {
            path: String::new(),
            flags: [true, true, true, true, true, true, false],
            supported: Vec::new(),
            validation: None,
            confirm: false,
            busy: false,
            restore: None,
            status: String::new(),
            request: 0,
        }
    }
}
impl Backups {
    fn begin(&mut self) -> u64 {
        self.busy = true;
        self.request += 1;
        self.request
    }
    fn flags(&self) -> Value {
        let mut value = json!({});
        for (i, (key, _)) in FLAGS.iter().enumerate() {
            if self.supported.iter().any(|s| s == key) {
                value[*key] = json!(self.flags[i]);
            }
        }
        value
    }
    fn poll(&self, server: Server) -> Task<Msg> {
        let Some(id) = self.restore.clone() else {
            return Task::none();
        };
        let request = self.request;
        Task::perform(
            async move {
                tokio::time::sleep(Duration::from_secs(1)).await;
                server.query("query Restore($id:String!){restoreStatus(id:$id){state mangaProgress totalManga}}",json!({"id":id})).await.map_err(|e|e.to_string())
            },
            move |r| Msg::Status(request, r),
        )
    }
    pub fn update(&mut self, msg: Msg, server: Server) -> Task<Msg> {
        match msg {
            Msg::Open => {
                if self.restore.is_some() {
                    self.begin();
                    return self.poll(server);
                }
                let request = self.begin();
                return Task::perform(
                    async move {
                        let data=server.query("query BackupOptions{__type(name:\"PartialBackupFlagsInput\"){inputFields{name}}}",json!({})).await.map_err(|e|e.to_string())?;
                        Ok(data["__type"]["inputFields"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|v| v["name"].as_str().map(str::to_owned))
                            .collect())
                    },
                    move |r| Msg::Capabilities(request, r),
                );
            }
            Msg::Capabilities(id, result) if id == self.request => {
                self.busy = false;
                match result {
                    Ok(fields) => self.supported = fields,
                    Err(e) => self.status = e,
                }
            }
            Msg::Path(path) => {
                self.path = path;
                self.validation = None;
                self.confirm = false;
            }
            Msg::Flag(i, value) => {
                if i < 7 {
                    self.flags[i] = value;
                }
            }
            Msg::Create => {
                let path = PathBuf::from(&self.path);
                let flags = self.flags();
                let request = self.begin();
                self.status = "Creating backup…".into();
                return Task::perform(
                    async move {
                        let data=server.mutate("mutation Backup($flags:PartialBackupFlagsInput!){createBackup(input:{flags:$flags}){url}}",json!({"flags":flags})).await.map_err(|e|e.to_string())?;
                        let url = data["createBackup"]["url"]
                            .as_str()
                            .ok_or("The server returned no backup URL")?;
                        server
                            .save_backup(url, &path)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    move |r| Msg::Created(request, r),
                );
            }
            Msg::Created(id, result) if id == self.request => {
                self.busy = false;
                self.status = match result {
                    Ok(()) => "Backup saved".into(),
                    Err(e) => e,
                };
            }
            Msg::Validate => {
                let path = PathBuf::from(&self.path);
                let request = self.begin();
                self.status = "Checking backup…".into();
                self.validation = None;
                return Task::perform(
                    async move {
                        server
                            .upload_backup(&path, false, json!({}))
                            .await
                            .map_err(|e| e.to_string())
                    },
                    move |r| Msg::Validated(request, r),
                );
            }
            Msg::Validated(id, result) if id == self.request => {
                self.busy = false;
                match result {
                    Ok(data) => {
                        self.validation = Some(data["validateBackup"].clone());
                        self.status =
                            "Backup checked. Review missing sources and trackers before restoring."
                                .into();
                    }
                    Err(e) => self.status = e,
                }
            }
            Msg::Confirm(value) => self.confirm = value,
            Msg::Restore if self.confirm && self.validation.is_some() => {
                let path = PathBuf::from(&self.path);
                let flags = self.flags();
                let request = self.begin();
                self.confirm = false;
                self.status = "Uploading backup…".into();
                return Task::perform(
                    async move {
                        server
                            .upload_backup(&path, true, flags)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    move |r| Msg::Started(request, r),
                );
            }
            Msg::Started(id, result) if id == self.request => match result {
                Ok(data) => {
                    self.restore = data["restoreBackup"]["id"].as_str().map(str::to_owned);
                    if self.restore.is_none() {
                        self.busy = false;
                        self.status = "The server returned no restore ID".into();
                    } else {
                        self.status = "Restoring backup…".into();
                        return self.poll(server);
                    }
                }
                Err(e) => {
                    self.busy = false;
                    self.status = e;
                }
            },
            Msg::Status(id, result) if id == self.request => match result {
                Ok(data) => {
                    let status = &data["restoreStatus"];
                    match status["state"].as_str() {
                        Some("SUCCESS") => {
                            self.busy = false;
                            self.restore = None;
                            self.validation = None;
                            self.status="Restore complete. Refresh your library to load the restored entries.".into();
                        }
                        Some("FAILURE") => {
                            self.busy = false;
                            self.restore = None;
                            self.status="Restore failed. The server may have restored some entries; check its logs.".into();
                        }
                        Some(state) => {
                            self.status = format!(
                                "{} · {} / {} titles",
                                state.replace('_', " ").to_lowercase(),
                                status["mangaProgress"],
                                status["totalManga"]
                            );
                            return self.poll(server);
                        }
                        None => {
                            self.busy = false;
                            self.status =
                                "Restore status unavailable; reopen Backups to retry.".into();
                        }
                    }
                }
                Err(e) => {
                    self.busy = false;
                    self.status =
                        format!("Could not read restore progress: {e}. Reopen Backups to retry.");
                }
            },
            _ => {}
        }
        Task::none()
    }
    pub fn view(&self) -> Element<'_, Msg> {
        let mut body = column![
            ui::heading("Backups").size(28),
            text(&self.status).size(14),
            text_input("Absolute path to a .tachibk backup", &self.path)
                .on_input(Msg::Path)
                .padding(12),
            ui::heading("Contents").size(18)
        ]
        .spacing(16);
        for (i, (key, label)) in FLAGS.iter().enumerate() {
            if self.supported.iter().any(|s| s == key) {
                body = body.push(
                    checkbox(self.flags[i])
                        .label(*label)
                        .on_toggle(move |v| Msg::Flag(i, v)),
                );
            }
        }
        body = body.push(
            row![
                button("Create backup at this path")
                    .on_press_maybe(
                        (!self.busy && !self.path.trim().is_empty()).then_some(Msg::Create)
                    )
                    .padding(12),
                button("Check existing backup")
                    .on_press_maybe(
                        (!self.busy && !self.path.trim().is_empty()).then_some(Msg::Validate)
                    )
                    .padding(12)
            ]
            .spacing(12),
        );
        if let Some(validation) = &self.validation {
            for (key, label) in [
                ("missingSources", "Missing sources"),
                ("missingTrackers", "Missing trackers"),
            ] {
                let missing: Vec<_> = validation[key]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v["name"].as_str())
                    .collect();
                body = body.push(text(format!(
                    "{label}: {}",
                    if missing.is_empty() {
                        "none".into()
                    } else {
                        missing.join(", ")
                    }
                )));
            }
            body = body.push(
                button("Restore this backup")
                    .on_press_maybe((!self.busy).then_some(Msg::Confirm(true)))
                    .padding(12),
            );
        }
        if self.confirm {
            body=body.push(container(column![text("Restore the selected contents into your connected Suwayomi server? This changes server data."),row![button("Restore").on_press(Msg::Restore).style(ui::danger),button("Cancel").on_press(Msg::Confirm(false))].spacing(12)].spacing(12)).padding(16).style(ui::panel));
        }
        ui::scrollable(body).into()
    }
}
