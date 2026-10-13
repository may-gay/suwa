use super::*;
use std::path::Path;
use tokio::io::AsyncWriteExt;

impl Server {
    pub async fn upload_backup(&self, path: &Path, restore: bool, flags: Value) -> Result<Value> {
        ensure!(
            !restore || !self.read_only,
            "Read-only connection: backup restore is disabled"
        );
        let metadata = tokio::fs::metadata(path)
            .await
            .context("Open backup file")?;
        ensure!(
            metadata.is_file() && metadata.len() <= 512 * 1024 * 1024,
            "Choose a backup file no larger than 512 MiB"
        );
        let query = if restore {
            "mutation Restore($backup:Upload!,$flags:PartialBackupFlagsInput!){restoreBackup(input:{backup:$backup,flags:$flags}){id status{state mangaProgress totalManga}}}"
        } else {
            "query Validate($backup:Upload!){validateBackup(input:{backup:$backup}){missingSources{id name} missingTrackers{name}}}"
        };
        let mut input = json!({"backup":null});
        if restore {
            input["flags"] = flags;
        }
        let mut token = self.tokens.read().await.0.clone();
        for attempt in 0..2 {
            let file = tokio::fs::File::open(path).await?;
            let stream = tokio_util::io::ReaderStream::new(file);
            let part = reqwest::multipart::Part::stream_with_length(
                reqwest::Body::wrap_stream(stream),
                metadata.len(),
            )
            .file_name("backup.tachibk")
            .mime_str("application/octet-stream")?;
            let form = reqwest::multipart::Form::new()
                .text(
                    "operations",
                    json!({"query":query,"variables":input}).to_string(),
                )
                .text("map", json!({"0":["variables.backup"]}).to_string())
                .part("0", part);
            let response = self
                .authorize(
                    self.http
                        .post(self.endpoint("api/graphql")?)
                        .multipart(form),
                    &token,
                )
                .send()
                .await?;
            if attempt == 0
                && response.status() == reqwest::StatusCode::UNAUTHORIZED
                && self.renew(&token).await?
            {
                token = self.tokens.read().await.0.clone();
                continue;
            }
            return response_data(response).await;
        }
        unreachable!()
    }
    pub async fn save_backup(&self, url: &str, destination: &Path) -> Result<()> {
        ensure!(destination.is_absolute(), "Choose an absolute backup path");
        ensure!(
            !destination.exists(),
            "The destination already exists; choose a new filename"
        );
        let url = self.endpoint(url)?;
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
            "Backup download returned HTTP {}",
            response.status()
        );
        let parent = destination.parent().context("Missing backup directory")?;
        tokio::fs::create_dir_all(parent).await?;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let temporary = destination.with_extension(format!(
            "{}.{}.part",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .await?;
        let result: Result<()> = async {
            let mut size = 0;
            while let Some(chunk) = response.chunk().await? {
                size += chunk.len();
                ensure!(
                    size <= 512 * 1024 * 1024,
                    "Backup exceeds the 512 MiB limit"
                );
                file.write_all(&chunk).await?;
            }
            file.sync_all().await?;
            tokio::fs::hard_link(&temporary, destination)
                .await
                .context("Save backup without replacing an existing file")?;
            Ok(())
        }
        .await;
        drop(file);
        let _ = tokio::fs::remove_file(temporary).await;
        result
    }
}
