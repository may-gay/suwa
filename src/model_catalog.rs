use crate::upscale::Model;
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

#[derive(Debug, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub note: String,
    pub license: String,
    pub source: String,
    pub model: Model,
    pub downloads: Vec<Artifact>,
}

impl std::fmt::Display for &'static Preset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}
impl PartialEq for Preset {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Preset {}

#[derive(Debug, Deserialize)]
pub struct Artifact {
    pub file: String,
    pub url: String,
    pub bytes: u64,
    pub sha256: String,
    pub range: Option<ArchiveRange>,
}
#[derive(Debug, Deserialize)]
pub struct ArchiveRange {
    pub offset: u64,
    pub length: u64,
    pub deflate: bool,
}

pub fn presets() -> &'static [Preset] {
    static CATALOG: OnceLock<Vec<Preset>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../models/catalog.json")).expect("model catalog")
    })
}

impl Preset {
    pub fn manifest(&self, directory: &Path) -> PathBuf {
        directory.join(format!("{}.json", self.id))
    }
    pub fn installed(&self, directory: &Path) -> bool {
        self.manifest(directory).is_file()
            && if self.downloads.is_empty() {
                directory.join(&self.model.file).is_file()
            } else {
                self.downloads.iter().all(|a| {
                    std::fs::metadata(directory.join(&a.file)).is_ok_and(|m| m.len() == a.bytes)
                })
            }
    }
    pub fn download_bytes(&self) -> u64 {
        self.downloads
            .iter()
            .map(|a| a.range.as_ref().map_or(a.bytes, |r| r.length))
            .sum()
    }
}

pub struct Progress {
    pub done: AtomicU64,
    pub total: u64,
    pub cancel: tokio_util::sync::CancellationToken,
}
impl Progress {
    pub fn new(preset: &Preset) -> Arc<Self> {
        Arc::new(Self {
            done: AtomicU64::new(0),
            total: preset.download_bytes(),
            cancel: tokio_util::sync::CancellationToken::new(),
        })
    }
    pub fn check(&self) -> Result<()> {
        ensure!(!self.cancel.is_cancelled(), "Model download cancelled");
        Ok(())
    }
}

struct Temporary(PathBuf);
impl Temporary {
    fn create(destination: &Path) -> Result<(Self, std::fs::File)> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let path = destination.with_extension(format!("{}.{nonce}.part", std::process::id()));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok((Self(path), file))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn digest(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn verify_download(
    artifact: &Artifact,
    downloaded: &Path,
    destination: &Path,
    progress: &Progress,
) -> Result<()> {
    progress.check()?;
    let file = std::fs::File::open(downloaded)?;
    let mut source: Box<dyn Read> = if artifact.range.as_ref().is_some_and(|r| r.deflate) {
        Box::new(flate2::read::DeflateDecoder::new(file))
    } else {
        Box::new(file)
    };
    let (temp, mut output) = Temporary::create(destination)?;
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0; 65536];
    loop {
        progress.check()?;
        let n = source.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        ensure!(
            bytes <= artifact.bytes,
            "Model file exceeded its expected size"
        );
        hash.update(&buffer[..n]);
        output.write_all(&buffer[..n])?;
    }
    ensure!(bytes == artifact.bytes, "Incomplete model file");
    ensure!(
        format!("{:x}", hash.finalize()) == artifact.sha256,
        "Model checksum mismatch"
    );
    output.sync_all()?;
    drop(output);
    progress.check()?;
    std::fs::rename(&temp.0, destination)?;
    Ok(())
}

pub async fn install(
    preset: &'static Preset,
    directory: PathBuf,
    progress: Arc<Progress>,
) -> Result<String> {
    preset.model.validate()?;
    progress.check()?;
    std::fs::create_dir_all(&directory)?;
    if preset.downloads.is_empty() {
        return crate::upscale::install_default(&directory);
    }
    let client = reqwest::Client::builder()
        .user_agent("Suwa/0.1 model installer")
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(300))
        .build()?;
    for artifact in &preset.downloads {
        progress.check()?;
        let destination = directory.join(&artifact.file);
        let path = destination.clone();
        let expected = artifact.sha256.clone();
        let valid =
            tokio::task::spawn_blocking(move || digest(&path).is_ok_and(|h| h == expected)).await?;
        let length = artifact.range.as_ref().map_or(artifact.bytes, |r| r.length);
        if valid {
            progress.done.fetch_add(length, Ordering::Relaxed);
            continue;
        }
        let mut request = client.get(&artifact.url);
        if let Some(range) = &artifact.range {
            request = request.header(
                reqwest::header::RANGE,
                format!("bytes={}-{}", range.offset, range.offset + range.length - 1),
            );
        }
        let mut response = tokio::select! {
            _ = progress.cancel.cancelled() => anyhow::bail!("Model download cancelled"),
            response = request.send() => response?.error_for_status()?,
        };
        if let Some(range) = &artifact.range {
            ensure!(
                response.status() == reqwest::StatusCode::PARTIAL_CONTENT,
                "The model host did not support partial downloads"
            );
            let expected = format!(
                "bytes {}-{}/",
                range.offset,
                range.offset + range.length - 1
            );
            ensure!(
                response
                    .headers()
                    .get(reqwest::header::CONTENT_RANGE)
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|v| v.starts_with(&expected)),
                "The model host returned an unexpected range"
            );
        }
        ensure!(
            response.content_length().is_none_or(|n| n == length),
            "Unexpected model download size"
        );
        let (temp, file) = Temporary::create(&destination)?;
        let mut file = tokio::fs::File::from_std(file);
        let mut received = 0;
        loop {
            progress.check()?;
            let chunk = tokio::select! {
                _ = progress.cancel.cancelled() => anyhow::bail!("Model download cancelled"),
                chunk = tokio::time::timeout(Duration::from_secs(30), response.chunk()) => chunk.context("Model download timed out")??,
            };
            let Some(chunk) = chunk else {
                break;
            };
            received += chunk.len() as u64;
            ensure!(
                received <= length,
                "Model download exceeded its expected size"
            );
            use tokio::io::AsyncWriteExt;
            file.write_all(&chunk).await?;
            progress
                .done
                .fetch_add(chunk.len() as u64, Ordering::Relaxed);
        }
        ensure!(received == length, "Incomplete model download");
        file.sync_all().await?;
        drop(file);
        let download = temp.0.clone();
        let progress = progress.clone();
        tokio::task::spawn_blocking(move || {
            verify_download(artifact, &download, &destination, &progress)
        })
        .await??;
    }
    progress.check()?;
    let manifest = preset.manifest(&directory);
    let model = serde_json::to_vec_pretty(&preset.model)?;
    let provenance = format!(
        "{}\n{}\nLicense: {}\nSource: {}\n\nDownloads:\n{}\n",
        preset.name,
        preset.description,
        preset.license,
        preset.source,
        preset
            .downloads
            .iter()
            .map(|a| format!("{}\nSHA-256: {}\n{}", a.file, a.sha256, a.url))
            .collect::<Vec<_>>()
            .join("\n\n")
    );
    crate::storage::atomic_write(
        &directory.join(format!("{}.source.txt", preset.id)),
        provenance.as_bytes(),
    )?;
    let license = if preset.id.starts_with("swin-") || preset.id == "cunet-2x" {
        include_str!("../models/LICENSE.waifu2x").to_owned()
    } else if preset.id.starts_with("mangajanai-") || preset.id.starts_with("illustration-") {
        include_str!("../models/licenses/MangaJaNai.txt").to_owned()
    } else if preset.id.starts_with("realesrgan-") {
        format!(
            "{}\n{}",
            include_str!("../models/licenses/Real-ESRGAN.txt"),
            include_str!("../models/licenses/HWEnc.txt")
        )
    } else if preset.id.starts_with("anime4k-") {
        format!(
            "{}\n{}",
            include_str!("../models/licenses/Anime4K.txt"),
            include_str!("../models/licenses/HWEnc.txt")
        )
    } else {
        include_str!("../models/licenses/AnimeSharp.txt").to_owned()
    };
    crate::storage::atomic_write(
        &directory.join(format!("{}.license.txt", preset.id)),
        license.as_bytes(),
    )?;
    crate::storage::atomic_write(&manifest, &model)?;
    Ok(manifest.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_has_safe_files_and_pinned_hashes() {
        let mut ids = std::collections::HashSet::new();
        for preset in presets() {
            assert!(ids.insert(&preset.id));
            preset.model.validate().unwrap();
            for a in &preset.downloads {
                assert!(a.bytes < 96 * 1024 * 1024);
                assert!(a.url.starts_with("https://"));
                assert!(!a.file.contains('/') && !a.file.contains('\\'));
                assert_eq!(a.sha256.len(), 64);
                assert!(a.sha256.bytes().all(|b| b.is_ascii_hexdigit()));
                assert!(
                    a.range
                        .as_ref()
                        .is_none_or(|r| r.length > 0 && r.length < 96 * 1024 * 1024)
                );
            }
        }
    }
    #[test]
    fn rejects_corrupted_and_cancelled_installs() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input");
        let output = dir.path().join("model.onnx");
        let a = Artifact {
            file: "model.onnx".into(),
            url: String::new(),
            bytes: 4,
            sha256: format!("{:x}", Sha256::digest(b"good")),
            range: None,
        };
        let progress = Progress::new(&presets()[0]);
        std::fs::write(&input, b"bad!").unwrap();
        assert!(verify_download(&a, &input, &output, &progress).is_err());
        assert!(!output.exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        std::fs::write(&input, b"good").unwrap();
        progress.cancel.cancel();
        assert!(verify_download(&a, &input, &output, &progress).is_err());
        let progress = Progress::new(&presets()[0]);
        verify_download(&a, &input, &output, &progress).unwrap();
        assert_eq!(std::fs::read(output).unwrap(), b"good");
    }
    #[test]
    fn extracts_only_bounded_deflate_payloads() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input");
        let output = dir.path().join("model.onnx");
        let mut compressed =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        compressed.write_all(b"model weights").unwrap();
        std::fs::write(&input, compressed.finish().unwrap()).unwrap();
        let mut a = Artifact {
            file: "model.onnx".into(),
            url: String::new(),
            bytes: 13,
            sha256: format!("{:x}", Sha256::digest(b"model weights")),
            range: Some(ArchiveRange {
                offset: 0,
                length: 20,
                deflate: true,
            }),
        };
        let progress = Progress::new(&presets()[0]);
        a.bytes = 4;
        assert!(verify_download(&a, &input, &output, &progress).is_err());
        a.bytes = 13;
        verify_download(&a, &input, &output, &progress).unwrap();
        assert_eq!(std::fs::read(output).unwrap(), b"model weights");
    }
    #[test]
    fn manga_resolution_selection_keeps_the_pinned_file_and_hash_together() {
        let preset = presets().iter().find(|p| p.id == "mangajanai-2x").unwrap();
        for (height, expected) in [
            (900, 1200),
            (1370, 1400),
            (1588, 1600),
            (1900, 1920),
            (12000, 2048),
        ] {
            let model = preset.model.clone().for_height(height);
            let variant = preset
                .model
                .variants
                .iter()
                .find(|v| v.height == expected)
                .unwrap();
            assert_eq!(model.file, variant.file);
            assert_eq!(model.sha256, variant.sha256);
            assert!(model.variants.is_empty());
        }
        let mut model = preset.model.clone();
        model.variants[0].file = "../unsafe.onnx".into();
        assert!(model.validate().is_err());
    }
}
