use crate::domain::*;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

pub struct Store {
    db: Mutex<Connection>,
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Download {
    pub chapter: i64,
    #[cfg(test)]
    pub state: String,
    #[cfg(test)]
    pub done: usize,
    #[cfg(test)]
    pub total: usize,
    #[cfg(test)]
    pub error: String,
}

impl Store {
    pub fn open_default() -> Result<Self> {
        let root = std::env::var_os("SUWA_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                ProjectDirs::from("app", "suwa", "suwa")
                    .expect("home directory")
                    .data_dir()
                    .to_owned()
            });
        Self::open(&root)
    }
    pub fn open(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        let db = Connection::open(root.join("suwa.sqlite3"))?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS kv(scope TEXT NOT NULL, key TEXT NOT NULL, value TEXT NOT NULL, PRIMARY KEY(scope,key));
            CREATE TABLE IF NOT EXISTS progress(scope TEXT NOT NULL, chapter INTEGER NOT NULL, value TEXT NOT NULL, revision INTEGER NOT NULL DEFAULT 1, PRIMARY KEY(scope,chapter));
            CREATE TABLE IF NOT EXISTS downloads(scope TEXT NOT NULL, chapter INTEGER NOT NULL, manga INTEGER NOT NULL, name TEXT NOT NULL, state TEXT NOT NULL, done INTEGER NOT NULL DEFAULT 0, total INTEGER NOT NULL DEFAULT 0, error TEXT NOT NULL DEFAULT '', PRIMARY KEY(scope,chapter));
            CREATE TABLE IF NOT EXISTS tracking_pending(scope TEXT NOT NULL, manga INTEGER NOT NULL, PRIMARY KEY(scope,manga));
            CREATE TABLE IF NOT EXISTS progress_revision(id INTEGER PRIMARY KEY CHECK(id=1), value INTEGER NOT NULL);
            INSERT OR IGNORE INTO progress_revision SELECT 1, COALESCE(MAX(revision),0) FROM progress;
            PRAGMA user_version=1;")?;
        db.execute(
            "UPDATE downloads SET state='queued' WHERE state='downloading'",
            [],
        )?;
        Ok(Self {
            db: Mutex::new(db),
            root: root.to_owned(),
        })
    }
    pub fn put<T: Serialize>(&self, scope: &str, key: &str, value: &T) -> Result<()> {
        self.db.lock().unwrap().execute("INSERT INTO kv VALUES(?1,?2,?3) ON CONFLICT(scope,key) DO UPDATE SET value=excluded.value",params![scope,key,serde_json::to_string(value)?])?;
        Ok(())
    }
    pub fn get<T: DeserializeOwned>(&self, scope: &str, key: &str) -> Result<Option<T>> {
        let raw: Option<String> = self
            .db
            .lock()
            .unwrap()
            .query_row(
                "SELECT value FROM kv WHERE scope=?1 AND key=?2",
                params![scope, key],
                |r| r.get(0),
            )
            .optional()?;
        raw.map(|r| serde_json::from_str(&r).context("Invalid saved data"))
            .transpose()
    }
    pub fn record_progress(&self, scope: &str, p: &Progress) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let mut p = p.clone();
        let pending: Option<String> = tx
            .query_row(
                "SELECT value FROM progress WHERE scope=?1 AND chapter=?2",
                params![scope, p.chapter_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(pending) = pending {
            let previous: Progress = serde_json::from_str(&pending)?;
            p.baseline_page = previous.baseline_page;
            if !p.explicit {
                p.read |= previous.read;
                p.explicit = previous.explicit;
            }
        } else {
            let baseline: Option<String> = tx
                .query_row(
                    "SELECT value FROM kv WHERE scope=?1 AND key=?2",
                    params![scope, format!("baseline:{}", p.chapter_id)],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(baseline) = baseline {
                p.baseline_page = serde_json::from_str(&baseline)?;
            }
        }
        let revision = next_progress_revision(&tx)?;
        tx.execute("INSERT INTO progress(scope,chapter,value,revision) VALUES(?1,?2,?3,?4) ON CONFLICT(scope,chapter) DO UPDATE SET value=excluded.value,revision=excluded.revision",params![scope,p.chapter_id,serde_json::to_string(&p)?,revision])?;
        tx.execute("INSERT INTO kv VALUES(?1,?2,?3) ON CONFLICT(scope,key) DO UPDATE SET value=excluded.value",params![scope,format!("resume:{}",p.chapter_id),serde_json::to_string(&p.page)?])?;
        tx.commit()?;
        Ok(())
    }
    pub fn queue_tracking(&self, scope: &str, manga: i64) -> Result<()> {
        self.db.lock().unwrap().execute(
            "INSERT OR IGNORE INTO tracking_pending VALUES(?1,?2)",
            params![scope, manga],
        )?;
        Ok(())
    }
    pub fn pending_tracking(&self, scope: &str) -> Result<Vec<i64>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare("SELECT manga FROM tracking_pending WHERE scope=?1")?;
        Ok(stmt
            .query_map([scope], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?)
    }
    pub fn acknowledge_tracking(&self, scope: &str, manga: i64) -> Result<()> {
        self.db.lock().unwrap().execute(
            "DELETE FROM tracking_pending WHERE scope=?1 AND manga=?2",
            params![scope, manga],
        )?;
        Ok(())
    }
    pub fn pending_progress(&self, scope: &str) -> Result<Vec<(Progress, i64)>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare("SELECT value,revision FROM progress WHERE scope=?1")?;
        let rows = stmt.query_map([scope], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        rows.map(|row| {
            let (v, r) = row?;
            Ok((serde_json::from_str(&v)?, r))
        })
        .collect()
    }
    pub fn acknowledge(&self, scope: &str, chapter: i64, revision: i64, page: usize) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        tx.execute("INSERT INTO kv VALUES(?1,?2,?3) ON CONFLICT(scope,key) DO UPDATE SET value=excluded.value", params![scope,format!("baseline:{chapter}"),serde_json::to_string(&page)?])?;
        let current: Option<(String, i64)> = tx
            .query_row(
                "SELECT value,revision FROM progress WHERE scope=?1 AND chapter=?2",
                params![scope, chapter],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((value, current_revision)) = current {
            if current_revision == revision {
                tx.execute(
                    "DELETE FROM progress WHERE scope=?1 AND chapter=?2",
                    params![scope, chapter],
                )?;
            } else {
                let mut newer: Progress = serde_json::from_str(&value)?;
                newer.baseline_page = page;
                tx.execute(
                    "UPDATE progress SET value=?3 WHERE scope=?1 AND chapter=?2",
                    params![scope, chapter, serde_json::to_string(&newer)?],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn resolve_local_progress(&self, scope: &str, chapter: i64) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let value: Option<String> = tx
            .query_row(
                "SELECT value FROM progress WHERE scope=?1 AND chapter=?2",
                params![scope, chapter],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(value) = value {
            let mut progress: Progress = serde_json::from_str(&value)?;
            progress.explicit = true;
            let revision = next_progress_revision(&tx)?;
            tx.execute(
                "UPDATE progress SET value=?3,revision=?4 WHERE scope=?1 AND chapter=?2",
                params![scope, chapter, serde_json::to_string(&progress)?, revision],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn resolve_remote_progress(
        &self,
        scope: &str,
        remote: &Chapter,
        revision: i64,
    ) -> Result<()> {
        let chapter = remote.id;
        let page = remote.last_page_read;
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let current: Option<i64> = tx
            .query_row(
                "SELECT revision FROM progress WHERE scope=?1 AND chapter=?2",
                params![scope, chapter],
                |r| r.get(0),
            )
            .optional()?;
        anyhow::ensure!(
            current == Some(revision),
            "New local progress arrived; review this conflict again"
        );
        tx.execute(
            "DELETE FROM progress WHERE scope=?1 AND chapter=?2",
            params![scope, chapter],
        )?;
        for key in [format!("baseline:{chapter}"), format!("resume:{chapter}")] {
            tx.execute("INSERT INTO kv VALUES(?1,?2,?3) ON CONFLICT(scope,key) DO UPDATE SET value=excluded.value",params![scope,key,serde_json::to_string(&page)?])?;
        }
        let key = format!("chapters:{}", remote.manga_id);
        let cached: Option<String> = tx
            .query_row(
                "SELECT value FROM kv WHERE scope=?1 AND key=?2",
                params![scope, key],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(cached) = cached {
            let mut chapters: Vec<Chapter> = serde_json::from_str(&cached)?;
            if let Some(chapter) = chapters.iter_mut().find(|c| c.id == remote.id) {
                *chapter = remote.clone();
                tx.execute(
                    "UPDATE kv SET value=?3 WHERE scope=?1 AND key=?2",
                    params![scope, key, serde_json::to_string(&chapters)?],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn image_path(&self, scope: &str, url: &str) -> PathBuf {
        self.root
            .join("images")
            .join(scope)
            .join(format!("{:x}", Sha256::digest(url.as_bytes())))
    }
    pub fn trim_cache(&self, scope: &str, budget: u64, keep: &Path) -> Result<()> {
        let mut pinned = std::collections::HashSet::new();
        for download in self.downloads(scope)? {
            let urls: Vec<String> = self
                .get(scope, &format!("pages:{}", download.chapter))?
                .unwrap_or_default();
            for url in urls {
                pinned.insert(self.image_path(scope, &url));
            }
        }
        let directory = self.root.join("images").join(scope);
        let entries = match std::fs::read_dir(directory) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let mut files = Vec::new();
        let mut total = 0u64;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some() || pinned.contains(&path) {
                continue;
            }
            if let Ok(metadata) = entry.metadata() {
                if !metadata.is_file() {
                    continue;
                }
                total += metadata.len();
                files.push((
                    metadata.modified().unwrap_or(std::time::UNIX_EPOCH),
                    path,
                    metadata.len(),
                ));
            }
        }
        files.sort_by_key(|(time, _, _)| *time);
        for (_, path, size) in files {
            if total <= budget {
                break;
            }
            if path != keep && std::fs::remove_file(path).is_ok() {
                total = total.saturating_sub(size);
            }
        }
        Ok(())
    }
    #[cfg(test)]
    pub fn queue(&self, scope: &str, c: &Chapter) -> Result<()> {
        self.db.lock().unwrap().execute("INSERT INTO downloads(scope,chapter,manga,name,state) VALUES(?1,?2,?3,?4,'queued') ON CONFLICT(scope,chapter) DO UPDATE SET state=CASE WHEN state='complete' THEN 'complete' ELSE 'queued' END,error=''",params![scope,c.id,c.manga_id,c.name])?;
        Ok(())
    }
    pub fn downloads(&self, scope: &str) -> Result<Vec<Download>> {
        let db = self.db.lock().unwrap();
        let mut stmt=db.prepare("SELECT chapter,manga,name,state,done,total,error FROM downloads WHERE scope=?1 ORDER BY chapter DESC")?;
        Ok(stmt
            .query_map([scope], |r| {
                Ok(Download {
                    chapter: r.get(0)?,
                    #[cfg(test)]
                    state: r.get(3)?,
                    #[cfg(test)]
                    done: r.get::<_, i64>(4)? as usize,
                    #[cfg(test)]
                    total: r.get::<_, i64>(5)? as usize,
                    #[cfg(test)]
                    error: r.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }
    #[cfg(test)]
    pub fn download_state(
        &self,
        scope: &str,
        chapter: i64,
        state: &str,
        done: usize,
        total: usize,
        error: &str,
    ) -> Result<()> {
        self.db.lock().unwrap().execute(
            "UPDATE downloads SET state=?3,done=?4,total=?5,error=?6 WHERE scope=?1 AND chapter=?2",
            params![scope, chapter, state, done as i64, total as i64, error],
        )?;
        Ok(())
    }
}

fn next_progress_revision(tx: &rusqlite::Transaction<'_>) -> Result<i64> {
    tx.execute("UPDATE progress_revision SET value=value+1 WHERE id=1", [])?;
    Ok(tx.query_row(
        "SELECT value FROM progress_revision WHERE id=1",
        [],
        |row| row.get(0),
    )?)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let parent = path.parent().context("Missing parent directory")?;
    std::fs::create_dir_all(parent)?;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let temp = path.with_extension(format!(
        "{}.{}.part",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_acknowledgment_cannot_erase_recreated_progress() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let mut progress = Progress {
            chapter_id: 1,
            page: 2,
            read: false,
            explicit: false,
            baseline_page: 0,
        };
        store.record_progress("a", &progress).unwrap();
        let old = store.pending_progress("a").unwrap()[0].1;
        store.acknowledge("a", 1, old, 2).unwrap();
        drop(store);
        let store = Store::open(dir.path()).unwrap();
        progress.page = 3;
        store.record_progress("a", &progress).unwrap();
        store.acknowledge("a", 1, old, 2).unwrap();
        assert_eq!(store.pending_progress("a").unwrap()[0].0.page, 3);
    }
    #[test]
    fn conflict_resolution_keeps_newer_local_changes() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let mut progress = Progress {
            chapter_id: 7,
            page: 4,
            read: false,
            explicit: false,
            baseline_page: 2,
        };
        store.record_progress("account", &progress).unwrap();
        let remote = Chapter {
            id: 7,
            manga_id: 9,
            last_page_read: 8,
            is_read: true,
            ..Default::default()
        };
        store
            .put(
                "account",
                "chapters:9",
                &vec![Chapter {
                    id: 7,
                    manga_id: 9,
                    last_page_read: 4,
                    ..Default::default()
                }],
            )
            .unwrap();
        let revision = store.pending_progress("account").unwrap()[0].1;
        progress.page = 5;
        store.record_progress("account", &progress).unwrap();
        assert!(
            store
                .resolve_remote_progress("account", &remote, revision)
                .is_err()
        );
        assert_eq!(store.pending_progress("account").unwrap()[0].0.page, 5);
        store.resolve_local_progress("account", 7).unwrap();
        let pending = store.pending_progress("account").unwrap();
        assert!(pending[0].0.explicit);
        progress.page = 6;
        store.record_progress("account", &progress).unwrap();
        let pending = store.pending_progress("account").unwrap();
        assert!(pending[0].0.explicit);
        store
            .resolve_remote_progress("account", &remote, pending[0].1)
            .unwrap();
        assert!(store.pending_progress("account").unwrap().is_empty());
        let cached = store
            .get::<Vec<Chapter>>("account", "chapters:9")
            .unwrap()
            .unwrap();
        assert_eq!(cached[0].last_page_read, 8);
        assert!(cached[0].is_read);
        assert_eq!(store.get::<usize>("account", "resume:7").unwrap(), Some(8));
        progress.page = 9;
        store.record_progress("account", &progress).unwrap();
        assert_eq!(
            store.pending_progress("account").unwrap()[0]
                .0
                .baseline_page,
            8
        );
    }
    #[test]
    fn acknowledgment_cannot_erase_newer_progress() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let mut p = Progress {
            chapter_id: 1,
            page: 2,
            read: false,
            explicit: false,
            baseline_page: 0,
        };
        store.record_progress("a", &p).unwrap();
        let old = store.pending_progress("a").unwrap()[0].1;
        p.page = 3;
        store.record_progress("a", &p).unwrap();
        store.acknowledge("a", 1, old, 2).unwrap();
        assert_eq!(store.pending_progress("a").unwrap()[0].0.page, 3);
        assert_eq!(store.pending_progress("a").unwrap()[0].0.baseline_page, 2);
        p.page = 4;
        store.record_progress("a", &p).unwrap();
        assert_eq!(store.pending_progress("a").unwrap()[0].0.baseline_page, 2);
        assert!(store.pending_progress("b").unwrap().is_empty());
    }
    #[test]
    fn interrupted_downloads_resume_after_restart() {
        let dir = tempfile::tempdir().unwrap();
        {
            let s = Store::open(dir.path()).unwrap();
            s.queue(
                "a",
                &Chapter {
                    id: 1,
                    ..Default::default()
                },
            )
            .unwrap();
            s.download_state("a", 1, "downloading", 2, 5, "").unwrap();
        }
        let s = Store::open(dir.path()).unwrap();
        assert_eq!(s.downloads("a").unwrap()[0].state, "queued");
        assert!(s.downloads("b").unwrap().is_empty());
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    #[test]
    fn eviction_preserves_saved_chapters_and_other_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store
            .queue(
                "a",
                &Chapter {
                    id: 1,
                    ..Default::default()
                },
            )
            .unwrap();
        store.put("a", "pages:1", &vec!["saved-page"]).unwrap();
        for (scope, url) in [("a", "saved-page"), ("a", "transient"), ("b", "private")] {
            atomic_write(&store.image_path(scope, url), b"fixture").unwrap();
        }
        store.trim_cache("a", 0, Path::new("unused")).unwrap();
        assert!(store.image_path("a", "saved-page").exists());
        assert!(!store.image_path("a", "transient").exists());
        assert!(store.image_path("b", "private").exists());
    }
}
