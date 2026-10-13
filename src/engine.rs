use crate::{
    api::Server,
    domain::*,
    reader::{Decoded, decode},
    storage::{Store, atomic_write},
    upscale::Upscaler,
};
use anyhow::{Context, Result, ensure};
use serde_json::json;
#[cfg(test)]
use std::sync::atomic::AtomicBool;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use tokio::sync::Semaphore;

pub struct Engine {
    pub store: Arc<Store>,
    pub server: Server,
    pub generation: Arc<AtomicU64>,
    #[cfg(test)]
    pub paused: AtomicBool,
    pub cache_budget: AtomicU64,
    cache_writes: AtomicUsize,
    image_slots: Semaphore,
    decode_slots: Semaphore,
    inference_slot: Semaphore,
    upscale: Arc<Mutex<Upscaler>>,
}

impl Engine {
    pub fn new(store: Arc<Store>, server: Server) -> Self {
        Self {
            store,
            server,
            generation: Arc::new(AtomicU64::new(0)),
            #[cfg(test)]
            paused: AtomicBool::new(false),
            cache_budget: AtomicU64::new(1024 * 1024 * 1024),
            cache_writes: AtomicUsize::new(0),
            image_slots: Semaphore::new(4),
            decode_slots: Semaphore::new(2),
            inference_slot: Semaphore::new(1),
            upscale: Arc::new(Mutex::new(Upscaler::default())),
        }
    }
    pub fn next_generation(&self) -> u64 {
        {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            self.generation.store(id, Ordering::Relaxed);
            id
        }
    }
    pub async fn library(&self) -> Result<Vec<Manga>> {
        let library = self.server.library().await?;
        self.store.put(&self.server.scope, "library", &library)?;
        Ok(library)
    }
    pub async fn chapters(&self, id: i64) -> Result<Vec<Chapter>> {
        let mut chapters: Vec<Chapter> = match self.server.chapters(id).await {
            Ok(c) => c,
            Err(e) => self
                .store
                .get(&self.server.scope, &format!("chapters:{id}"))?
                .ok_or(e)?,
        };
        chapters.sort_by_key(|chapter| chapter.source_order);
        let pending: std::collections::HashMap<_, _> = self
            .store
            .pending_progress(&self.server.scope)?
            .into_iter()
            .map(|(p, _)| (p.chapter_id, p))
            .collect();
        for chapter in &mut chapters {
            if let Some(p) = pending.get(&chapter.id) {
                chapter.is_read = p.read;
                chapter.last_page_read = p.page;
            }
        }
        self.store
            .put(&self.server.scope, &format!("chapters:{id}"), &chapters)?;
        Ok(chapters)
    }
    pub async fn pages(&self, id: i64) -> Result<Vec<String>> {
        if let Some(p) = self.store.get(&self.server.scope, &format!("pages:{id}"))? {
            return Ok(p);
        }
        let pages = self.server.pages(id).await?;
        ensure!(!pages.is_empty(), "This chapter has no pages");
        self.store
            .put(&self.server.scope, &format!("pages:{id}"), &pages)?;
        Ok(pages)
    }
    pub async fn bytes(&self, url: &str) -> Result<Vec<u8>> {
        let path = self.store.image_path(&self.server.scope, url);
        if let Ok(bytes) = tokio::fs::read(&path).await {
            return Ok(bytes);
        }
        let _slot = self.image_slots.acquire().await?;
        let bytes = self.server.image(url).await?;
        let b = bytes.clone();
        let store = self.store.clone();
        let scope = self.server.scope.clone();
        let prune = self
            .cache_writes
            .fetch_add(1, Ordering::Relaxed)
            .is_multiple_of(32);
        let budget = self.cache_budget.load(Ordering::Relaxed);
        tokio::task::spawn_blocking(move || -> Result<()> {
            atomic_write(&path, &b)?;
            if prune {
                store.trim_cache(&scope, budget, &path)?;
            }
            Ok(())
        })
        .await??;
        Ok(bytes)
    }
    pub async fn page(&self, url: &str, generation: u64) -> Result<Decoded> {
        let _decode = self.decode_slots.acquire().await?;
        ensure!(
            self.generation.load(Ordering::Relaxed) == generation,
            "Cancelled"
        );
        let bytes = self.bytes(url).await?;
        ensure!(
            self.generation.load(Ordering::Relaxed) == generation,
            "Cancelled"
        );
        let result = tokio::task::spawn_blocking(move || decode(&bytes)).await?;
        if result.is_err() {
            let _ = tokio::fs::remove_file(self.store.image_path(&self.server.scope, url)).await;
        }
        result
    }
    pub async fn cover(&self, url: &str) -> Result<Decoded> {
        let _decode = self.decode_slots.acquire().await?;
        let bytes = self.bytes(url).await?;
        let result = tokio::task::spawn_blocking(move || {
            let original = decode(&bytes)?;
            let rgba = image::RgbaImage::from_raw(
                original.width,
                original.height,
                original.rgba.as_ref().clone(),
            )
            .context("Invalid cover")?;
            let small = image::DynamicImage::ImageRgba8(rgba)
                .thumbnail(240, 340)
                .into_rgba8();
            Ok(Decoded {
                width: small.width(),
                height: small.height(),
                rgba: Arc::new(small.into_raw()),
            })
        })
        .await?;
        if result.is_err() {
            let _ = tokio::fs::remove_file(self.store.image_path(&self.server.scope, url)).await;
        }
        result
    }
    pub async fn enhance(
        &self,
        original: Decoded,
        settings: Settings,
        generation: u64,
    ) -> Result<Decoded> {
        let _inference = self.inference_slot.acquire().await?;
        ensure!(
            self.generation.load(Ordering::Relaxed) == generation,
            "Cancelled"
        );
        let up = self.upscale.clone();
        let cancel = self.generation.clone();
        tokio::task::spawn_blocking(move || {
            up.lock().unwrap().run(
                &settings.model_path,
                settings.cuda_device,
                &original,
                &cancel,
                generation,
            )
        })
        .await?
    }
    pub async fn sync_progress(&self) -> Result<Vec<Chapter>> {
        if self.server.read_only {
            return Ok(Vec::new());
        }
        let pending = self.store.pending_progress(&self.server.scope)?;
        let mut conflicts = Vec::new();
        for (p, revision) in pending {
            let remote = self.server.chapter(p.chapter_id).await?;
            match reconcile(&p, &remote) {
                Reconcile::Conflict => conflicts.push(remote),
                Reconcile::Apply { page, read } => {
                    self.server
                        .patch_chapter(p.chapter_id, json!({"lastPageRead":page,"isRead":read}))
                        .await?;
                    if read {
                        self.store
                            .queue_tracking(&self.server.scope, remote.manga_id)?;
                    }
                    self.store
                        .acknowledge(&self.server.scope, p.chapter_id, revision, page)?;
                }
            }
        }
        for manga in self.store.pending_tracking(&self.server.scope)? {
            self.server.mutate("mutation Progress($id:Int!){trackProgress(input:{mangaId:$id}){trackRecords{id}}}",json!({"id":manga})).await?;
            self.store.acknowledge_tracking(&self.server.scope, manga)?;
        }
        Ok(conflicts)
    }
    #[cfg(test)]
    pub async fn download_next(&self) -> Result<bool> {
        if self.paused.load(Ordering::Relaxed) {
            return Ok(false);
        }
        let Some(job) = self
            .store
            .downloads(&self.server.scope)?
            .into_iter()
            .find(|d| d.state == "queued")
        else {
            return Ok(false);
        };
        let result: Result<()> = async {
            let pages = self.pages(job.chapter).await?;
            self.store.download_state(
                &self.server.scope,
                job.chapter,
                "downloading",
                0,
                pages.len(),
                "",
            )?;
            for (i, url) in pages.iter().enumerate() {
                if !self
                    .store
                    .downloads(&self.server.scope)?
                    .iter()
                    .any(|d| d.chapter == job.chapter)
                {
                    return Ok(());
                }
                if self.paused.load(Ordering::Relaxed) {
                    self.store.download_state(
                        &self.server.scope,
                        job.chapter,
                        "queued",
                        i,
                        pages.len(),
                        "",
                    )?;
                    return Ok(());
                }
                self.bytes(url).await?;
                self.store.download_state(
                    &self.server.scope,
                    job.chapter,
                    "downloading",
                    i + 1,
                    pages.len(),
                    "",
                )?;
            }
            self.store.download_state(
                &self.server.scope,
                job.chapter,
                "complete",
                pages.len(),
                pages.len(),
                "",
            )?;
            Ok(())
        }
        .await;
        if let Err(e) = result {
            self.store.download_state(
                &self.server.scope,
                job.chapter,
                "failed",
                job.done,
                job.total,
                &e.to_string(),
            )?;
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn failed_cover_decode_does_not_poison_the_cache() {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(root.path()).unwrap());
        let server = Server::new(&Settings::default(), "", "").unwrap();
        let path = store.image_path(&server.scope, "/fixture-cover");
        atomic_write(&path, b"invalid cached image").unwrap();
        let engine = Engine::new(store, server);
        assert!(engine.cover("/fixture-cover").await.is_err());
        assert!(!path.exists());
        atomic_write(&path, include_bytes!("../assets/test/cover.avif")).unwrap();
        let cover = engine.cover("/fixture-cover").await.unwrap();
        assert_eq!((cover.width, cover.height), (240, 320));
        assert!(path.exists());
    }

    #[test]
    #[ignore = "requires SUWA_TEST_COVER_DIR; reads cached image files without modifying them"]
    fn cached_thumbnail_formats_decode() {
        let root = std::env::var_os("SUWA_TEST_COVER_DIR").expect("set SUWA_TEST_COVER_DIR");
        let mut decoded = 0;
        let mut errors = Vec::new();
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if !path.is_file() || path.extension().is_some() {
                continue;
            }
            match decode(&std::fs::read(&path).unwrap()) {
                Ok(_) => decoded += 1,
                Err(error) => errors.push(format!(
                    "{}: {error}",
                    path.file_name().unwrap().to_string_lossy()
                )),
            }
        }
        assert!(decoded > 0, "no images found");
        assert!(errors.is_empty(), "{}", errors.join("\n"));
        println!("Decoded {decoded} cached images");
    }
}
