use crate::domain::*;
use anyhow::{Context, Result, bail, ensure};
use reqwest::{
    Client,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;
use url::Url;

const MANGA_FIELDS: &str = "id title thumbnailUrl author description genre status inLibrary inLibraryAt unreadCount downloadCount bookmarkCount chapters{totalCount} latestReadChapter{lastReadAt} latestUploadedChapter{uploadDate} categories { nodes { id name } }";
const CHAPTER_FIELDS: &str = "id mangaId name chapterNumber sourceOrder scanlator isRead isBookmarked isDownloaded lastPageRead pageCount";

#[derive(Clone)]
pub struct Server {
    http: Client,
    base: Url,
    pub scope: String,
    pub read_only: bool,
    basic: Option<(String, String)>,
    tokens: Arc<RwLock<(String, String)>>,
    renewal: Arc<tokio::sync::Mutex<()>>,
}

pub fn normalize_url(input: &str) -> Result<Url> {
    let text = input.trim();
    ensure!(!text.is_empty(), "Enter a server address");
    let mut url = Url::parse(&if text.contains("://") {
        text.to_owned()
    } else {
        format!("http://{text}")
    })?;
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "Use an HTTP or HTTPS server address"
    );
    ensure!(
        url.host_str().is_some() && url.username().is_empty() && url.password().is_none(),
        "Enter credentials in the login fields"
    );
    ensure!(
        url.query().is_none() && url.fragment().is_none(),
        "The server address must not contain a query or fragment"
    );
    if !text.contains("://") && url.port().is_none() {
        let _ = url.set_port(Some(4567));
    }
    url.set_path(&format!("{}/", url.path().trim_end_matches('/')));
    Ok(url)
}

pub fn account_scope(settings: &Settings) -> Result<String> {
    let base = normalize_url(&settings.server)?;
    Ok(format!(
        "{:x}",
        Sha256::digest(format!("{}\0{:?}\0{}", base, settings.auth, settings.username).as_bytes())
    ))
}

impl Server {
    pub fn new(settings: &Settings, password: &str, custom_headers: &str) -> Result<Self> {
        let base = normalize_url(&settings.server)?;
        let mut headers = HeaderMap::new();
        for line in custom_headers.lines().filter(|l| !l.trim().is_empty()) {
            let (name, value) = line
                .split_once(':')
                .context("Headers must use Name: value, one per line")?;
            headers.insert(
                HeaderName::from_bytes(name.trim().as_bytes())?,
                HeaderValue::from_str(value.trim())?,
            );
        }
        let http = Client::builder()
            .default_headers(headers)
            .cookie_store(true)
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(45))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("Suwa/0.1")
            .build()?;
        let scope = account_scope(settings)?;
        Ok(Self {
            http,
            base,
            scope,
            read_only: settings.read_only,
            basic: (settings.auth == AuthMode::Basic)
                .then(|| (settings.username.clone(), password.to_owned())),
            tokens: Default::default(),
            renewal: Default::default(),
        })
    }

    fn endpoint(&self, path: &str) -> Result<Url> {
        ensure!(
            !path.starts_with("//"),
            "Protocol-relative URLs are not supported"
        );
        let relative = if path.starts_with(self.base.path()) && self.base.path() != "/" {
            path
        } else {
            path.trim_start_matches('/')
        };
        let url = self.base.join(relative)?;
        ensure!(
            url.origin() == self.base.origin()
                && url.username().is_empty()
                && url.password().is_none(),
            "Server returned a URL on a different origin"
        );
        Ok(url)
    }

    fn authorize(&self, request: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
        let request = if let Some((u, p)) = &self.basic {
            request.basic_auth(u, Some(p))
        } else {
            request
        };
        if token.is_empty() {
            request
        } else {
            request.bearer_auth(token)
        }
    }
    async fn renew(&self, expired: &str) -> Result<bool> {
        let _guard = self.renewal.lock().await;
        let (access, refresh) = self.tokens.read().await.clone();
        if access != expired {
            return Ok(true);
        }
        if refresh.is_empty() {
            return Ok(false);
        }
        let response = self.http.post(self.endpoint("api/graphql")?)
            .json(&json!({"query":"mutation Renew($token:String!){refreshToken(input:{refreshToken:$token}){accessToken}}","variables":{"token":refresh}}))
            .send().await?.error_for_status()?;
        let data: Value = response.json().await?;
        let token = data["data"]["refreshToken"]["accessToken"]
            .as_str()
            .context("Session expired; reconnect to sign in")?;
        self.tokens.write().await.0 = token.into();
        Ok(true)
    }
    async fn request(&self, query: &str, variables: Value) -> Result<Value> {
        let body = json!({"query":query,"variables":variables});
        let token = self.tokens.read().await.0.clone();
        let mut response = self
            .authorize(
                self.http.post(self.endpoint("api/graphql")?).json(&body),
                &token,
            )
            .send()
            .await
            .context("Could not reach the server")?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED && self.renew(&token).await? {
            let token = self.tokens.read().await.0.clone();
            response = self
                .authorize(
                    self.http.post(self.endpoint("api/graphql")?).json(&body),
                    &token,
                )
                .send()
                .await?;
        }
        response_data(response).await
    }

    pub async fn query(&self, query: &str, variables: Value) -> Result<Value> {
        ensure!(
            query.trim_start().starts_with("query"),
            "Expected a query operation"
        );
        self.request(query, variables).await
    }
    pub async fn mutate(&self, query: &str, variables: Value) -> Result<Value> {
        ensure!(
            !self.read_only,
            "Read-only connection: enable server changes in Connection to use this action"
        );
        self.request(query, variables).await
    }

    pub async fn login(&self, settings: &Settings, password: &str) -> Result<String> {
        if settings.auth == AuthMode::UiLogin {
            let data = self.request("mutation Login($username:String!,$password:String!){ login(input:{username:$username,password:$password}) { accessToken refreshToken } }",json!({"username":settings.username,"password":password})).await?;
            *self.tokens.write().await = (
                data["login"]["accessToken"]
                    .as_str()
                    .context("Missing access token")?
                    .into(),
                data["login"]["refreshToken"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
            );
        } else if settings.auth == AuthMode::SimpleLogin {
            let response = self
                .http
                .post(self.endpoint("login.html")?)
                .form(&[("user", settings.username.as_str()), ("pass", password)])
                .send()
                .await?;
            ensure!(
                response.status().is_success() || response.status().is_redirection(),
                "Sign-in failed"
            );
        }
        let info = self
            .query("query About { aboutServer { name version } }", json!({}))
            .await?;
        Ok(format!(
            "{} {}",
            info["aboutServer"]["name"].as_str().unwrap_or("Suwayomi"),
            info["aboutServer"]["version"].as_str().unwrap_or_default()
        ))
    }

    pub async fn library(&self) -> Result<Vec<Manga>> {
        let query = format!(
            "query Library($offset:Int!) {{ mangas(condition:{{inLibrary:true}},first:100,offset:$offset) {{ nodes {{ {MANGA_FIELDS} }} pageInfo {{ hasNextPage }} }} }}"
        );
        self.paginate(&query, "mangas", json!({})).await
    }
    async fn paginate<T: DeserializeOwned>(
        &self,
        query: &str,
        field: &str,
        mut vars: Value,
    ) -> Result<Vec<T>> {
        let mut all = Vec::new();
        loop {
            vars["offset"] = json!(all.len());
            let data = self.query(query, vars.clone()).await?;
            let page: Vec<T> = serde_json::from_value(data[field]["nodes"].clone())?;
            let more = data[field]["pageInfo"]["hasNextPage"]
                .as_bool()
                .unwrap_or(false);
            ensure!(
                !more || !page.is_empty(),
                "Server pagination made no progress"
            );
            all.extend(page);
            if !more {
                return Ok(all);
            }
        }
    }
    pub async fn chapters(&self, id: i64) -> Result<Vec<Chapter>> {
        let query = format!(
            "query Chapters($id:Int!,$offset:Int!) {{ chapters(condition:{{mangaId:$id}},first:100,offset:$offset,order:[{{by:SOURCE_ORDER,byType:ASC}}]) {{ nodes {{ {CHAPTER_FIELDS} }} pageInfo {{ hasNextPage }} }} }}"
        );
        self.paginate(&query, "chapters", json!({"id":id})).await
    }
    pub async fn manga(&self, id: i64) -> Result<Manga> {
        let data = self
            .query(
                &format!("query Manga($id:Int!){{manga(id:$id){{{MANGA_FIELDS}}}}}"),
                json!({"id":id}),
            )
            .await?;
        Ok(serde_json::from_value(data["manga"].clone())?)
    }
    pub async fn chapter(&self, id: i64) -> Result<Chapter> {
        let data = self
            .query(
                &format!("query Chapter($id:Int!) {{ chapter(id:$id) {{ {CHAPTER_FIELDS} }} }}"),
                json!({"id":id}),
            )
            .await?;
        Ok(serde_json::from_value(data["chapter"].clone())?)
    }
    pub async fn sources(&self) -> Result<Vec<Source>> {
        let d = self
            .query(
                "query Sources { sources { nodes { id displayName lang supportsLatest } } }",
                json!({}),
            )
            .await?;
        Ok(serde_json::from_value(d["sources"]["nodes"].clone())?)
    }
    pub async fn categories(&self) -> Result<Vec<Category>> {
        let d = self
            .query(
                "query Categories { categories { nodes { id name default order } } }",
                json!({}),
            )
            .await?;
        let mut categories: Vec<Category> =
            serde_json::from_value(d["categories"]["nodes"].clone())?;
        for category in &mut categories {
            category.is_default_category = category.id == 0;
        }
        Ok(categories)
    }
    pub async fn browse(
        &self,
        source: &str,
        term: &str,
        page: usize,
        latest: bool,
        filters: Vec<Value>,
    ) -> Result<(Vec<Manga>, bool)> {
        let query = format!(
            "mutation Browse($input:FetchSourceMangaInput!) {{ fetchSourceManga(input:$input) {{ mangas {{ {MANGA_FIELDS} }} hasNextPage }} }}"
        );
        let data=self.mutate(&query,json!({"input":{"source":source,"type":if !term.is_empty() || !filters.is_empty(){"SEARCH"}else if latest{"LATEST"}else{"POPULAR"},"query":term,"page":page,"filters":filters}})).await?;
        Ok((
            serde_json::from_value(data["fetchSourceManga"]["mangas"].clone())?,
            data["fetchSourceManga"]["hasNextPage"]
                .as_bool()
                .unwrap_or(false),
        ))
    }
    pub async fn pages(&self, id: i64) -> Result<Vec<String>> {
        let data = self
            .mutate(
                "mutation Pages($id:Int!){ fetchChapterPages(input:{chapterId:$id}) { pages } }",
                json!({"id":id}),
            )
            .await?;
        Ok(serde_json::from_value(
            data["fetchChapterPages"]["pages"].clone(),
        )?)
    }
    pub async fn image(&self, path: &str) -> Result<Vec<u8>> {
        let url = self.endpoint(path)?;
        let token = self.tokens.read().await.0.clone();
        let mut response = self
            .authorize(self.http.get(url.clone()), &token)
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED && self.renew(&token).await? {
            let token = self.tokens.read().await.0.clone();
            response = self.authorize(self.http.get(url), &token).send().await?;
        }
        ensure!(
            response.status().is_success(),
            "Image request returned HTTP {}",
            response.status()
        );
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                bytes.len() + chunk.len() <= 64 * 1024 * 1024,
                "Image exceeds 64 MiB"
            );
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
    pub async fn set_library(&self, id: i64, in_library: bool) -> Result<()> {
        self.mutate("mutation Library($id:Int!,$value:Boolean!){updateManga(input:{id:$id,patch:{inLibrary:$value}}){manga{id}}}",json!({"id":id,"value":in_library})).await?;
        Ok(())
    }
    pub async fn patch_chapter(&self, id: i64, patch: Value) -> Result<()> {
        self.mutate("mutation Chapter($input:UpdateChapterInput!){updateChapter(input:$input){chapter{id}}}",json!({"input":{"id":id,"patch":patch}})).await?;
        Ok(())
    }
    pub async fn refresh_manga(&self, id: i64) -> Result<()> {
        self.mutate("mutation Refresh($id:Int!){fetchManga(input:{id:$id}){manga{id}} fetchChapters(input:{mangaId:$id}){chapters{id}}}",json!({"id":id})).await?;
        Ok(())
    }
    pub async fn enqueue(&self, ids: &[i64]) -> Result<()> {
        self.mutate("mutation Download($ids:[Int!]!){enqueueChapterDownloads(input:{ids:$ids}){downloadStatus{state}}}",json!({"ids":ids})).await?;
        Ok(())
    }
    pub async fn update_library(&self) -> Result<()> {
        self.mutate(
            "mutation Update {updateLibrary(input:{}){clientMutationId}}",
            json!({}),
        )
        .await?;
        Ok(())
    }
}

async fn response_data(response: reqwest::Response) -> Result<Value> {
    let status = response.status();
    ensure!(
        status.is_success(),
        "Server returned HTTP {status}; check the address and sign-in"
    );
    let body: Value = response
        .json()
        .await
        .context("The server did not return GraphQL JSON")?;
    if let Some(errors) = body
        .get("errors")
        .and_then(Value::as_array)
        .filter(|v| !v.is_empty())
    {
        bail!(
            "{}",
            errors
                .iter()
                .filter_map(|e| e["message"].as_str())
                .map(|message| message.lines().next().unwrap_or(message))
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    body.get("data")
        .cloned()
        .context("Missing GraphQL response data")
}

mod transfer;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_hosts_and_preserves_proxy_paths() {
        assert_eq!(normalize_url("nas").unwrap().as_str(), "http://nas:4567/");
        assert_eq!(
            normalize_url("https://host/manga").unwrap().as_str(),
            "https://host/manga/"
        );
        assert!(normalize_url("file:///etc/passwd").is_err());
        assert!(normalize_url("https://user:secret@host").is_err());
    }
    #[test]
    fn proxy_urls_never_duplicate_the_prefix_or_leak_credentials() {
        let s = Server::new(
            &Settings {
                server: "https://example.test/suwayomi".into(),
                ..Default::default()
            },
            "",
            "",
        )
        .unwrap();
        for path in ["api/page", "/api/page", "/suwayomi/api/page"] {
            assert_eq!(
                s.endpoint(path).unwrap().as_str(),
                "https://example.test/suwayomi/api/page"
            );
        }
        assert!(s.endpoint("https://elsewhere.test/page").is_err());
        assert!(s.endpoint("//elsewhere.test/page").is_err());
        assert!(
            s.endpoint("https://user:password@example.test/page")
                .is_err()
        );
    }
    #[tokio::test]
    async fn read_only_blocks_mutation_before_network() {
        let s = Server::new(&Settings::default(), "", "").unwrap();
        assert!(
            s.enqueue(&[1])
                .await
                .unwrap_err()
                .to_string()
                .contains("Read-only")
        );
    }
}

#[cfg(test)]
mod network_tests;
