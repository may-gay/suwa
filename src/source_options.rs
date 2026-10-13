use crate::api::Server;
use crate::ui::{self, button, checkbox, pick_list, text_input};
use anyhow::Result;
use iced::widget::{column, container, row, scrollable, text};
use iced::{Element, Length, Task};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub enum Msg {
    Open(String),
    Loaded(u64, Result<Value, String>),
    Filter(Vec<usize>, Value),
    Preference(usize, Value),
    SavePreference(usize),
    Saved(u64, usize, Result<Value, String>),
    Reset,
    Apply,
    Back,
}

#[derive(Default)]
pub struct Options {
    pub source: String,
    name: String,
    filters: Vec<Value>,
    preferences: Vec<Value>,
    changes: BTreeMap<Vec<usize>, Value>,
    drafts: BTreeMap<usize, Value>,
    pub status: String,
    busy: bool,
    request: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Choice {
    index: usize,
    label: String,
}
impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}
fn choices(values: &Value) -> Vec<Choice> {
    values
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, v)| Choice {
            index,
            label: v.as_str().unwrap_or("").to_owned(),
        })
        .collect()
}
fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}
fn filter_state(filter: &Value) -> (&'static str, Value) {
    let (field, default) = match string(filter, "__typename") {
        "CheckBoxFilter" => ("checkBoxState", "boolDefault"),
        "TextFilter" => ("textState", "textDefault"),
        "SelectFilter" => ("selectState", "intDefault"),
        "SortFilter" => ("sortState", "sortDefault"),
        "TriStateFilter" => ("triState", "triDefault"),
        _ => ("", ""),
    };
    (field, filter[default].clone())
}
fn preference_field(preference: &Value) -> &'static str {
    match string(preference, "__typename") {
        "CheckBoxPreference" => "checkBoxState",
        "SwitchPreference" => "switchState",
        "ListPreference" => "listState",
        "MultiSelectListPreference" => "multiSelectState",
        _ => "editTextState",
    }
}
impl Options {
    pub fn filter_changes(&self) -> Vec<Value> {
        self.changes
            .iter()
            .map(|(path, change)| {
                let mut value = change.clone();
                for (depth, position) in path.iter().enumerate().rev() {
                    if depth == path.len() - 1 {
                        value["position"] = json!(position);
                    } else {
                        value = json!({"position":position,"groupChange":value});
                    }
                }
                value
            })
            .collect()
    }
    pub fn update(&mut self, msg: Msg, server: Server) -> Task<Msg> {
        match msg {
            Msg::Open(source) => {
                if source != self.source {
                    self.changes.clear();
                    self.filters.clear();
                    self.preferences.clear();
                }
                self.source = source.clone();
                self.drafts.clear();
                self.request += 1;
                self.busy = true;
                self.status = "Loading source options…".into();
                let request = self.request;
                return Task::perform(
                    async move { load(&server, &source).await.map_err(|e| e.to_string()) },
                    move |r| Msg::Loaded(request, r),
                );
            }
            Msg::Loaded(request, result) if request == self.request => {
                self.busy = false;
                match result {
                    Ok(source) => {
                        self.name = string(&source, "displayName").into();
                        self.filters = source["filters"].as_array().cloned().unwrap_or_default();
                        self.preferences = source["preferences"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default();
                        self.status.clear();
                    }
                    Err(e) => self.status = e,
                }
            }
            Msg::Filter(path, value) => {
                self.changes.insert(path, value);
            }
            Msg::Preference(position, value) => {
                self.drafts.insert(position, value);
            }
            Msg::SavePreference(position) if !self.busy => {
                if let (Some(preference), Some(value)) =
                    (self.preferences.get(position), self.drafts.get(&position))
                {
                    let source = self.source.clone();
                    let mut change = json!({"position":position});
                    change[preference_field(preference)] = value.clone();
                    self.request += 1;
                    self.busy = true;
                    let request = self.request;
                    return Task::perform(
                        async move {
                            let response = server.mutate(&"mutation Preference($input:UpdateSourcePreferenceInput!){updateSourcePreference(input:$input){preferences{FIELDS}}}".replace("FIELDS", &preference_fields()), json!({"input":{"source":source,"change":change}})).await.map_err(|e|e.to_string())?;
                            Ok(response["updateSourcePreference"]["preferences"].clone())
                        },
                        move |r| Msg::Saved(request, position, r),
                    );
                }
            }
            Msg::Saved(request, position, result) if request == self.request => {
                self.busy = false;
                match result {
                    Ok(mut preferences) => {
                        normalize_preferences(&mut preferences);
                        self.preferences = preferences.as_array().cloned().unwrap_or_default();
                        self.drafts.remove(&position);
                        self.status = "Source preference saved".into();
                    }
                    Err(e) => self.status = e,
                }
            }
            Msg::Reset => self.changes.clear(),
            _ => {}
        }
        Task::none()
    }
    fn filter<'a>(&'a self, filter: &'a Value, path: Vec<usize>) -> Element<'a, Msg> {
        let name = string(filter, "name");
        let (field, default) = filter_state(filter);
        let value = self
            .changes
            .get(&path)
            .map(|v| &v[field])
            .unwrap_or(&default);
        match string(filter, "__typename") {
            "GroupFilter" => {
                let mut group = column![text(name).size(18)].spacing(10);
                for (index, child) in filter["filters"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    let mut child_path = path.clone();
                    child_path.push(index);
                    group = group.push(self.filter(child, child_path));
                }
                container(group)
                    .padding(12)
                    .style(ui::panel)
                    .width(Length::Fill)
                    .into()
            }
            "CheckBoxFilter" => checkbox(value.as_bool().unwrap_or(false))
                .label(name)
                .on_toggle(move |v| Msg::Filter(path.clone(), json!({field:v})))
                .into(),
            "TextFilter" => column![
                text(name),
                text_input("", value.as_str().unwrap_or(""))
                    .on_input(move |v| Msg::Filter(path.clone(), json!({field:v})))
                    .padding(10)
            ]
            .spacing(6)
            .into(),
            "SelectFilter" | "SortFilter" => {
                let sort = string(filter, "__typename") == "SortFilter";
                let options = choices(&filter["values"]);
                let index = if sort {
                    value["index"].as_u64()
                } else {
                    value.as_u64()
                }
                .map(|v| v as usize);
                let selected = options.iter().find(|c| Some(c.index) == index).cloned();
                let ascending = value["ascending"].as_bool().unwrap_or(true);
                let selection_path = path.clone();
                let mut body = column![
                    text(name),
                    pick_list(options, selected, move |v: Choice| {
                        Msg::Filter(
                            selection_path.clone(),
                            if sort {
                                json!({field:{"index":v.index,"ascending":ascending}})
                            } else {
                                json!({field:v.index})
                            },
                        )
                    })
                ]
                .spacing(6);
                if sort {
                    body = body.push(checkbox(ascending).label("Ascending").on_toggle(move |v| {
                        Msg::Filter(
                            path.clone(),
                            json!({field:{"index":index.unwrap_or(0),"ascending":v}}),
                        )
                    }));
                }
                body.into()
            }
            "TriStateFilter" => {
                let selected = match value.as_str().unwrap_or("IGNORE") {
                    "INCLUDE" => "Include",
                    "EXCLUDE" => "Exclude",
                    _ => "Ignore",
                };
                row![text(name).width(Length::Fill),pick_list(["Ignore","Include","Exclude"],Some(selected),move |v|Msg::Filter(path.clone(),json!({field:match v {"Include"=>"INCLUDE","Exclude"=>"EXCLUDE",_=>"IGNORE"}})))].spacing(12).into()
            }
            "SeparatorFilter" => iced::widget::rule::horizontal(1).into(),
            _ => text(name).size(16).into(),
        }
    }
    pub fn view(&self) -> Element<'_, Msg> {
        let mut body = column![
            row![
                button("Back").on_press(Msg::Back).style(ui::ghost),
                text(if self.name.is_empty() {
                    "Source options"
                } else {
                    &self.name
                })
                .font(ui::DISPLAY)
                .size(24)
            ]
            .spacing(14),
            text(&self.status).size(13),
            ui::heading("Search filters").size(20),
        ]
        .spacing(18)
        .max_width(780);
        if self.filters.is_empty() {
            body = body.push(text("This source has no search filters."));
        }
        for (position, filter) in self.filters.iter().enumerate() {
            body = body.push(self.filter(filter, vec![position]));
        }
        body = body.push(
            row![
                button("Search with filters").on_press_maybe((!self.busy).then_some(Msg::Apply)),
                button("Reset filters")
                    .on_press(Msg::Reset)
                    .style(ui::secondary)
            ]
            .spacing(12),
        );
        body = body.push(ui::heading("Source preferences").size(20));
        if self.preferences.is_empty() {
            body = body.push(text("This source has no preferences."));
        }
        for (position, preference) in self.preferences.iter().enumerate() {
            if preference["visible"] == false {
                continue;
            }
            let enabled = preference["enabled"] != false && !self.busy;
            let field = preference_field(preference);
            let current = self.drafts.get(&position).unwrap_or_else(|| {
                if preference["current"].is_null() {
                    &preference["fallback"]
                } else {
                    &preference["current"]
                }
            });
            let mut content = column![
                text(string(preference, "title")).size(17),
                text(string(preference, "summary")).size(13)
            ]
            .spacing(10);
            if field == "checkBoxState" || field == "switchState" {
                content = content.push(
                    checkbox(current.as_bool().unwrap_or(false))
                        .label("Enabled")
                        .on_toggle_maybe(
                            enabled.then_some(move |v| Msg::Preference(position, json!(v))),
                        ),
                );
            } else if field == "listState" {
                let options = choices(&preference["entries"]);
                let selected = options
                    .iter()
                    .find(|c| preference["entryValues"][c.index] == *current)
                    .cloned();
                let values = preference["entryValues"].clone();
                content = content.push(pick_list(options, selected, move |v: Choice| {
                    Msg::Preference(position, values[v.index].clone())
                }));
            } else if field == "multiSelectState" {
                for option in choices(&preference["entries"]) {
                    let key = preference["entryValues"][option.index].clone();
                    let selected = current.as_array().cloned().unwrap_or_default();
                    let checked = selected.contains(&key);
                    content = content.push(checkbox(checked).label(option.label).on_toggle_maybe(
                        enabled.then_some(move |v| {
                            let mut next = selected.clone();
                            next.retain(|x| x != &key);
                            if v {
                                next.push(key.clone());
                            }
                            Msg::Preference(position, json!(next))
                        }),
                    ));
                }
            } else {
                let hint = format!(
                    "{} {}",
                    string(preference, "title"),
                    string(preference, "key")
                )
                .to_lowercase();
                let secret = [
                    "password", "token", "secret", "api key", "api_key", "apikey", "cookie",
                ]
                .iter()
                .any(|word| hint.contains(word));
                content = content.push(
                    text_input("", current.as_str().unwrap_or(""))
                        .secure(secret)
                        .on_input_maybe(
                            enabled.then_some(move |v| Msg::Preference(position, json!(v))),
                        )
                        .padding(10),
                );
            }
            content = content.push(
                button("Save preference")
                    .on_press_maybe(
                        (enabled && self.drafts.contains_key(&position))
                            .then_some(Msg::SavePreference(position)),
                    )
                    .style(ui::secondary),
            );
            body = body.push(
                container(content)
                    .padding(14)
                    .style(ui::panel)
                    .width(Length::Fill),
            );
        }
        scrollable(body).into()
    }
}

fn filter_fields(depth: usize) -> String {
    let mut fields = "__typename ... on CheckBoxFilter{name boolDefault:default} ... on TextFilter{name textDefault:default} ... on SelectFilter{name intDefault:default values} ... on SortFilter{name sortDefault:default{index ascending} values} ... on TriStateFilter{name triDefault:default} ... on HeaderFilter{name} ... on SeparatorFilter{name}".to_owned();
    if depth > 0 {
        fields.push_str(&format!(
            " ... on GroupFilter{{name filters{{{}}}}}",
            filter_fields(depth - 1)
        ));
    }
    fields
}
fn preference_fields() -> String {
    // aliases stay distinct across union members with incompatible value types.
    ["CheckBoxPreference","SwitchPreference","EditTextPreference","ListPreference","MultiSelectListPreference"].iter().enumerate().map(|(index,kind)|format!("... on {kind}{{title summary key enabled visible value{index}:currentValue default{index}:default {}}}",if matches!(*kind,"ListPreference"|"MultiSelectListPreference"){"entries entryValues"}else{""})).fold("__typename ".into(),|mut all,part|{all.push_str(&part);all})
}
fn normalize_preferences(preferences: &mut Value) {
    for preference in preferences.as_array_mut().into_iter().flatten() {
        let index = match string(preference, "__typename") {
            "CheckBoxPreference" => 0,
            "SwitchPreference" => 1,
            "EditTextPreference" => 2,
            "ListPreference" => 3,
            _ => 4,
        };
        preference["current"] = preference[format!("value{index}")].clone();
        preference["fallback"] = preference[format!("default{index}")].clone();
    }
}
pub async fn load(server: &Server, source: &str) -> Result<Value> {
    let mut data = server.query(&"query SourceOptions($id:LongString!){source(id:$id){displayName filters{FILTERS} preferences{PREFERENCES}}}".replace("FILTERS", &filter_fields(4)).replace("PREFERENCES", &preference_fields()),json!({"id":source})).await?;
    let mut source = data["source"].take();
    normalize_preferences(&mut source["preferences"]);
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grouped_changes_preserve_positions_and_siblings() {
        let mut options = Options::default();
        options
            .changes
            .insert(vec![2, 0], json!({"checkBoxState":true}));
        options
            .changes
            .insert(vec![2, 3], json!({"triState":"EXCLUDE"}));
        options
            .changes
            .insert(vec![5], json!({"sortState":{"index":1,"ascending":false}}));
        assert_eq!(
            options.filter_changes(),
            vec![
                json!({"position":2,"groupChange":{"position":0,"checkBoxState":true}}),
                json!({"position":2,"groupChange":{"position":3,"triState":"EXCLUDE"}}),
                json!({"position":5,"sortState":{"index":1,"ascending":false}})
            ]
        );
    }
}
