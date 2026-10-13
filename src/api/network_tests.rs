use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;

struct Fixture {
    address: String,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Fixture {
    fn new(replies: Vec<(u16, Value, &'static str)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback sockets are required");
        listener.set_nonblocking(true).unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            for (status, body, expected) in replies {
                let deadline = std::time::Instant::now() + Duration::from_secs(10);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                std::time::Instant::now() < deadline,
                                "request never arrived: {expected}"
                            );
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut data = Vec::new();
                let mut chunk = [0; 4096];
                loop {
                    let count = stream.read(&mut chunk).unwrap();
                    assert!(count > 0);
                    data.extend_from_slice(&chunk[..count]);
                    if let Some(end) = data.windows(4).position(|b| b == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&data[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|n| n.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if data.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                let request = String::from_utf8(data).unwrap();
                assert!(
                    request.contains(expected),
                    "unexpected request; expected {expected}"
                );
                let body = body.to_string();
                write!(stream, "HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        Self {
            address,
            worker: Some(worker),
        }
    }
    fn server(&self) -> Server {
        Server::new(
            &Settings {
                server: self.address.clone(),
                read_only: false,
                ..Default::default()
            },
            "",
            "",
        )
        .unwrap()
    }
    fn finish(mut self) {
        self.worker.take().unwrap().join().unwrap();
    }
}

#[tokio::test]
#[ignore = "requires permission for loopback sockets"]
async fn paginates_library_and_uses_actual_offset() {
    let fixture = Fixture::new(vec![
        (
            200,
            json!({"data":{"mangas":{"nodes":[{"id":1,"title":"First"},{"id":2,"title":"Second"}],"pageInfo":{"hasNextPage":true}}}}),
            "\"offset\":0",
        ),
        (
            200,
            json!({"data":{"mangas":{"nodes":[{"id":3,"title":"Third"}],"pageInfo":{"hasNextPage":false}}}}),
            "\"offset\":2",
        ),
    ]);
    let items = fixture.server().library().await.unwrap();
    assert_eq!(
        items.iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    fixture.finish();
}

#[tokio::test]
#[ignore = "requires permission for loopback sockets"]
async fn expired_access_token_is_renewed_before_retry() {
    let fixture = Fixture::new(vec![
        (401, json!({}), "Bearer expired"),
        (
            200,
            json!({"data":{"refreshToken":{"accessToken":"renewed"}}}),
            "mutation Renew",
        ),
        (
            200,
            json!({"data":{"chapter":{"id":5,"lastPageRead":7}}}),
            "Bearer renewed",
        ),
    ]);
    let server = fixture.server();
    *server.tokens.write().await = ("expired".into(), "fixture-refresh".into());
    assert_eq!(server.chapter(5).await.unwrap().last_page_read, 7);
    fixture.finish();
}

#[tokio::test]
#[ignore = "requires permission for loopback sockets"]
async fn graphql_errors_and_broken_pagination_are_reported() {
    let fixture = Fixture::new(vec![
        (
            200,
            json!({"data":null,"errors":[{"message":"source unavailable"}]}),
            "mutation Pages",
        ),
        (
            200,
            json!({"data":{"mangas":{"nodes":[],"pageInfo":{"hasNextPage":true}}}}),
            "query Library",
        ),
    ]);
    let server = fixture.server();
    assert!(
        server
            .pages(3)
            .await
            .unwrap_err()
            .to_string()
            .contains("source unavailable")
    );
    assert!(
        server
            .library()
            .await
            .unwrap_err()
            .to_string()
            .contains("no progress")
    );
    fixture.finish();
}

#[tokio::test]
#[ignore = "requires the isolated fixture server started by scripts/test-server.sh"]
async fn isolated_suwayomi_roundtrip() {
    let endpoint =
        std::env::var("SUWA_TEST_SERVER_URL").expect("start this test with scripts/test-server.sh");
    let parsed = url::Url::parse(&endpoint).unwrap();
    assert_eq!(parsed.host_str(), Some("127.0.0.1"));
    assert!(parsed.port().is_some());
    let server = Server::new(
        &Settings {
            server: endpoint,
            read_only: false,
            ..Default::default()
        },
        "",
        "",
    )
    .unwrap();
    let source = crate::source_options::load(&server, "0").await.unwrap();
    assert!(source["filters"].is_array());
    assert!(source["preferences"].is_array());
    let (mangas, _) = server
        .browse("0", "Suwa Integration Fixture", 1, false, Vec::new())
        .await
        .unwrap();
    let manga = mangas
        .iter()
        .find(|m| m.title == "Suwa Integration Fixture")
        .expect("fixture manga is missing");
    server.refresh_manga(manga.id).await.unwrap();
    server.set_library(manga.id, true).await.unwrap();
    assert!(
        server
            .library()
            .await
            .unwrap()
            .iter()
            .any(|m| m.id == manga.id)
    );
    let (trackers, records) = crate::tracking::load(&server, Some(manga.id))
        .await
        .unwrap();
    assert!(!trackers.is_empty());
    assert!(records.is_empty());
    let chapters = server.chapters(manga.id).await.unwrap();
    assert_eq!(
        chapters
            .iter()
            .map(|c| c.chapter_number)
            .collect::<Vec<_>>(),
        vec![1.0, 2.0, 3.0]
    );
    let c = &chapters[0];
    let pages = server.pages(c.id).await.unwrap();
    assert_eq!(pages.len(), 4);
    let image = crate::reader::decode(&server.image(&pages[0]).await.unwrap()).unwrap();
    assert_eq!(image.width, 600);
    server
        .patch_chapter(
            c.id,
            json!({"lastPageRead":2,"isRead":false,"isBookmarked":true}),
        )
        .await
        .unwrap();
    let updated = server.chapter(c.id).await.unwrap();
    assert_eq!(updated.last_page_read, 2);
    assert!(updated.is_bookmarked);
    let category = server.mutate("mutation FixtureCategory{createCategory(input:{name:\"Suwa fixture category\"}){category{id}}}",json!({})).await.unwrap();
    let category_id = category["createCategory"]["category"]["id"]
        .as_i64()
        .unwrap();
    server.mutate("mutation Assign($id:Int!,$category:Int!){updateMangaCategories(input:{id:$id,patch:{addToCategories:[$category]}}){manga{id}}}",json!({"id":manga.id,"category":category_id})).await.unwrap();
    assert!(
        server
            .categories()
            .await
            .unwrap()
            .iter()
            .any(|c| c.id == category_id)
    );
    server.mutate("mutation Rename($id:Int!,$name:String!){updateCategory(input:{id:$id,patch:{name:$name,default:true}}){category{id}}}",json!({"id":category_id,"name":"Renamed fixture"})).await.unwrap();
    server.mutate("mutation Order($id:Int!){updateCategoryOrder(input:{id:$id,position:1}){categories{id}}}",json!({"id":category_id})).await.unwrap();
    let categories = server.categories().await.unwrap();
    let updated_category = categories.iter().find(|c| c.id == category_id).unwrap();
    assert_eq!(updated_category.name, "Renamed fixture");
    assert!(updated_category.default);
    server
        .mutate(
            "mutation Delete($id:Int!){deleteCategory(input:{categoryId:$id}){category{id}}}",
            json!({"id":category_id}),
        )
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(crate::storage::Store::open(dir.path()).unwrap());
    store.queue(&server.scope, c).unwrap();
    let engine = crate::engine::Engine::new(store.clone(), server.clone());
    assert!(engine.download_next().await.unwrap());
    let downloads = store.downloads(&server.scope).unwrap();
    assert_eq!(downloads[0].state, "complete", "{}", downloads[0].error);
    let generation = engine.next_generation();
    assert_eq!(engine.page(&pages[0], generation).await.unwrap().width, 600);
    store
        .record_progress(
            &server.scope,
            &Progress {
                chapter_id: c.id,
                page: 3,
                read: true,
                explicit: false,
                baseline_page: 2,
            },
        )
        .unwrap();
    assert!(engine.sync_progress().await.unwrap().is_empty());
    assert!(server.chapter(c.id).await.unwrap().is_read);
    assert!(store.pending_progress(&server.scope).unwrap().is_empty());
    let backup_path = dir.path().join("fixture.tachibk");
    let capabilities = server
        .query(
            "query BackupOptions{__type(name:\"PartialBackupFlagsInput\"){inputFields{name}}}",
            json!({}),
        )
        .await
        .unwrap();
    let mut flags = json!({});
    for field in capabilities["__type"]["inputFields"].as_array().unwrap() {
        let name = field["name"].as_str().unwrap();
        flags[name] = json!(name != "includeServerSettings");
    }
    let created = server.mutate("mutation Backup($flags:PartialBackupFlagsInput!){createBackup(input:{flags:$flags}){url}}",json!({"flags":flags})).await.unwrap();
    server
        .save_backup(
            created["createBackup"]["url"].as_str().unwrap(),
            &backup_path,
        )
        .await
        .unwrap();
    assert!(backup_path.metadata().unwrap().len() > 0);
    assert!(
        server
            .save_backup(
                created["createBackup"]["url"].as_str().unwrap(),
                &backup_path
            )
            .await
            .is_err()
    );
    let validation = server
        .upload_backup(&backup_path, false, json!({}))
        .await
        .unwrap();
    assert!(validation["validateBackup"]["missingSources"].is_array());
    server.set_library(manga.id, false).await.unwrap();
    let restored = server
        .upload_backup(&backup_path, true, flags)
        .await
        .unwrap();
    let restore_id = restored["restoreBackup"]["id"].as_str().unwrap();
    let mut finished = false;
    for _ in 0..100 {
        let status = server
            .query(
                "query Restore($id:String!){restoreStatus(id:$id){state mangaProgress totalManga}}",
                json!({"id":restore_id}),
            )
            .await
            .unwrap();
        match status["restoreStatus"]["state"].as_str() {
            Some("SUCCESS") => {
                finished = true;
                break;
            }
            Some("FAILURE") => panic!("fixture restore failed"),
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    assert!(finished, "fixture restore did not finish");
    assert!(server.manga(manga.id).await.unwrap().in_library);
    if let Some(path) = std::env::var_os("SUWA_TEST_EXPORT") {
        let path = std::path::PathBuf::from(path);
        std::fs::create_dir(&path).expect("export must use a new directory");
        let saved = Arc::new(crate::storage::Store::open(&path).unwrap());
        let fixture = crate::engine::Engine::new(saved.clone(), server.clone());
        let library = fixture.library().await.unwrap();
        let chapters = fixture.chapters(manga.id).await.unwrap();
        for chapter in &chapters {
            saved.queue(&server.scope, chapter).unwrap();
            fixture.download_next().await.unwrap();
        }
        for manga in library {
            if let Some(url) = manga.thumbnail_url {
                fixture.bytes(&url).await.unwrap();
            }
        }
        saved
            .put(
                "app",
                "settings",
                &Settings {
                    server: parsed.to_string(),
                    read_only: true,
                    ..Default::default()
                },
            )
            .unwrap();
    }
}
