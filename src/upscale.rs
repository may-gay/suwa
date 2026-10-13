use crate::reader::Decoded;
use anyhow::Result;
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::atomic::AtomicU64};

#[cfg(feature = "cuda")]
use std::sync::atomic::Ordering;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub name: String,
    pub file: String,
    pub sha256: String,
    pub scale: usize,
    pub tile: usize,
    pub padding: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    pub height: u32,
    pub file: String,
    pub sha256: String,
}

impl Model {
    pub fn validate(&self) -> Result<()> {
        ensure!((2..=4).contains(&self.scale), "Model scale must be 2–4");
        ensure!(
            (64..=512).contains(&self.tile) && self.padding <= 64,
            "Invalid tile dimensions"
        );
        ensure!(
            self.sha256.len() == 64 && self.sha256.bytes().all(|c| c.is_ascii_hexdigit()),
            "Model needs a SHA-256 checksum"
        );
        ensure!(
            !self.file.is_empty()
                && Path::new(&self.file)
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))),
            "Model file must be relative to its manifest"
        );
        for variant in &self.variants {
            ensure!(variant.height > 0, "Invalid source height");
            let mut model = self.clone();
            model.variants.clear();
            model.file.clone_from(&variant.file);
            model.sha256.clone_from(&variant.sha256);
            model.validate()?;
        }
        Ok(())
    }

    #[cfg(any(feature = "cuda", test))]
    pub fn for_height(mut self, height: u32) -> Self {
        if let Some(variant) = self
            .variants
            .iter()
            .min_by_key(|v| v.height.abs_diff(height))
        {
            self.file.clone_from(&variant.file);
            self.sha256.clone_from(&variant.sha256);
        }
        self.variants.clear();
        self
    }
}

pub fn install_default(directory: &Path) -> Result<String> {
    let model = directory.join("fast-2x.onnx");
    let manifest = directory.join("fast-2x.json");
    if !model.exists() {
        crate::storage::atomic_write(&model, include_bytes!("../models/fast-2x.onnx"))?;
    }
    if !manifest.exists() {
        crate::storage::atomic_write(&manifest, include_bytes!("../models/fast-2x.json"))?;
    }
    let license = directory.join("LICENSE.waifu2x");
    if !license.exists() {
        crate::storage::atomic_write(&license, include_bytes!("../models/LICENSE.waifu2x"))?;
    }
    Ok(manifest.to_string_lossy().into_owned())
}

#[derive(Default)]
pub struct Upscaler {
    #[cfg(feature = "cuda")]
    session: Option<(String, i32, ort::session::Session, Model)>,
}

impl Upscaler {
    #[cfg(not(feature = "cuda"))]
    pub fn run(
        &mut self,
        _manifest: &str,
        _device: i32,
        _image: &Decoded,
        _generation: &AtomicU64,
        _expected: u64,
    ) -> Result<Decoded> {
        anyhow::bail!(
            "This build has no CUDA backend. Build with --features cuda to enable neural upscaling."
        )
    }

    #[cfg(feature = "cuda")]
    pub fn run(
        &mut self,
        manifest: &str,
        device: i32,
        image: &Decoded,
        generation: &AtomicU64,
        expected: u64,
    ) -> Result<Decoded> {
        use anyhow::Context;
        use ort::{ep, session::Session, value::Tensor};
        use sha2::{Digest, Sha256};
        use std::sync::Arc;
        ensure!(generation.load(Ordering::Relaxed) == expected, "Cancelled");
        let path = Path::new(manifest);
        let model: Model = serde_json::from_slice(
            &std::fs::read(path).context("Open a model manifest (.json) in Settings")?,
        )?;
        model.validate()?;
        let model = model.for_height(image.height);
        let key = format!("{manifest}#{}", model.file);
        if !self
            .session
            .as_ref()
            .is_some_and(|(p, d, _, _)| p == &key && *d == device)
        {
            let file = path
                .parent()
                .context("Missing model directory")?
                .join(&model.file);
            let hash = format!("{:x}", Sha256::digest(std::fs::read(&file)?));
            ensure!(
                hash.eq_ignore_ascii_case(&model.sha256),
                "Model checksum mismatch"
            );
            let runtime = std::env::var("ORT_DYLIB_PATH")
                .unwrap_or_else(|_| "/usr/lib/libonnxruntime.so".into());
            check_runtime(Path::new(&runtime))?;
            ort::init_from(runtime)?.with_name("suwa").commit();
            self.session = None;
            let session = Session::builder()?
                .with_intra_threads(1)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?
                .with_execution_providers([ep::CUDA::default()
                    .with_device_id(device)
                    .with_conv_algorithm_search(ep::cuda::ConvAlgorithmSearch::Heuristic)
                    .with_conv_max_workspace(false)
                    .with_memory_limit(1024 * 1024 * 1024)
                    .build()
                    .error_on_failure()])
                .map_err(|e| anyhow::anyhow!(e.to_string()))?
                .commit_from_file(file)?;
            self.session = Some((key, device, session, model));
        }
        let (_, _, session, model) = self.session.as_mut().unwrap();
        let s = model.scale;
        let w = image.width as usize;
        let h = image.height as usize;
        let ow = w.checked_mul(s).context("Image too wide")?;
        let oh = h.checked_mul(s).context("Image too tall")?;
        let size = ow
            .checked_mul(oh)
            .and_then(|v| v.checked_mul(4))
            .context("Image too large")?;
        ensure!(
            size <= 256 * 1024 * 1024,
            "Enhanced page exceeds the 256 MiB limit; showing the original"
        );
        let mut output = vec![0u8; size];
        let tile = model.tile;
        let pad = model.padding;
        let side = tile + 2 * pad;
        let fp16 = matches!(
            session.inputs()[0].dtype(),
            ort::value::ValueType::Tensor {
                ty: ort::value::TensorElementType::Float16,
                ..
            }
        );
        for y in (0..h).step_by(tile) {
            for x in (0..w).step_by(tile) {
                ensure!(generation.load(Ordering::Relaxed) == expected, "Cancelled");
                let mut input = ndarray::Array4::<f32>::zeros((1, 3, side, side));
                for iy in 0..side {
                    for ix in 0..side {
                        let sx = (x + ix).saturating_sub(pad).min(w - 1);
                        let sy = (y + iy).saturating_sub(pad).min(h - 1);
                        for c in 0..3 {
                            input[[0, c, iy, ix]] =
                                image.rgba[(sy * w + sx) * 4 + c] as f32 / 255.0;
                        }
                    }
                }
                let values = if fp16 {
                    let tensor = Tensor::from_array(input.mapv(half::f16::from_f32))?;
                    session.run(ort::inputs![tensor])?
                } else {
                    let tensor = Tensor::from_array(input)?;
                    session.run(ort::inputs![tensor])?
                };
                let converted;
                let result = if fp16 {
                    converted = values[0]
                        .try_extract_array::<half::f16>()?
                        .mapv(half::f16::to_f32);
                    converted.view()
                } else {
                    values[0].try_extract_array::<f32>()?
                };
                let shape = result.shape();
                ensure!(
                    shape.len() == 4 && shape[0] == 1 && shape[1] == 3,
                    "Expected NCHW RGB model output"
                );
                ensure!(
                    shape[2] <= side * s && shape[3] <= side * s,
                    "Unexpected model output dimensions"
                );
                let trim_y = (side * s - shape[2]) / 2;
                let trim_x = (side * s - shape[3]) / 2;
                ensure!(
                    pad * s >= trim_y && pad * s >= trim_x,
                    "Increase model padding"
                );
                let iy = pad * s - trim_y;
                let ix = pad * s - trim_x;
                let th = (h - y).min(tile) * s;
                let tw = (w - x).min(tile) * s;
                ensure!(
                    iy + th <= shape[2] && ix + tw <= shape[3],
                    "Model tile output is too small"
                );
                for dy in 0..th {
                    for dx in 0..tw {
                        let dest = ((y * s + dy) * ow + x * s + dx) * 4;
                        for c in 0..3 {
                            output[dest + c] = (result[[0, c, iy + dy, ix + dx]].clamp(0.0, 1.0)
                                * 255.0)
                                .round() as u8;
                        }
                        output[dest + 3] = image.rgba[((y + dy / s) * w + x + dx / s) * 4 + 3];
                    }
                }
            }
        }
        Ok(Decoded {
            width: ow as u32,
            height: oh as u32,
            rgba: Arc::new(output),
        })
    }
}

#[cfg(feature = "cuda")]
fn check_runtime(path: &Path) -> Result<()> {
    use anyhow::Context;
    ensure!(
        path.is_file(),
        "ONNX Runtime was not found at {}. Set ORT_DYLIB_PATH to a CUDA-enabled runtime.",
        path.display()
    );
    // validate the library before ort's global initialization; its error path can reenter initialization.
    unsafe {
        let library =
            libloading::Library::new(path).context("Load ONNX Runtime and its dependencies")?;
        let getter: libloading::Symbol<unsafe extern "C" fn() -> *const ort::sys::OrtApiBase> =
            library.get(b"OrtGetApiBase")?;
        let base = getter();
        ensure!(!base.is_null(), "Missing ONNX Runtime API");
        let version = std::ffi::CStr::from_ptr(((*base).GetVersionString)()).to_string_lossy();
        let minor = version
            .split('.')
            .nth(1)
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        ensure!(
            minor >= ort::MINOR_VERSION,
            "ONNX Runtime {version} is too old; this build requires 1.{} or newer",
            ort::MINOR_VERSION
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsafe_or_unbounded_manifests() {
        let mut m = Model {
            name: "test".into(),
            file: "model.onnx".into(),
            sha256: "a".repeat(64),
            scale: 2,
            tile: 256,
            padding: 32,
            variants: Vec::new(),
        };
        assert!(m.validate().is_ok());
        m.file = "../model.onnx".into();
        assert!(m.validate().is_err());
        m.file = "m.onnx".into();
        m.tile = 100000;
        assert!(m.validate().is_err());
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn cancelled_enhancement_never_loads_a_model() {
        let image = Decoded {
            width: 1,
            height: 1,
            rgba: std::sync::Arc::new(vec![255; 4]),
        };
        let result = Upscaler::default().run("missing.json", 0, &image, &AtomicU64::new(2), 1);
        assert_eq!(result.unwrap_err().to_string(), "Cancelled");
    }

    #[cfg(feature = "cuda")]
    #[test]
    #[ignore = "requires CUDA runtime and SUWA_TEST_MODEL_DIR with installed models"]
    fn cuda_switches_resolution_models_and_preserves_edge_tiles_and_alpha() {
        let directory = std::env::var("SUWA_TEST_MODEL_DIR").unwrap();
        let mut upscaler = Upscaler::default();
        let generation = AtomicU64::new(1);
        for (height, variant) in [(97, 1200), (1588, 1600), (1900, 1920)] {
            let rgba: Vec<_> = (0..height)
                .flat_map(|y| (0..17).flat_map(move |x| [128, 128, 128, ((x + y) % 256) as u8]))
                .collect();
            let image = Decoded {
                width: 17,
                height,
                rgba: std::sync::Arc::new(rgba),
            };
            let output = upscaler
                .run(
                    &format!("{directory}/mangajanai-2x.json"),
                    0,
                    &image,
                    &generation,
                    1,
                )
                .unwrap();
            assert_eq!((output.width, output.height), (34, height * 2));
            assert_eq!(
                upscaler.session.as_ref().unwrap().3.file,
                format!("mangajanai-{variant}.onnx")
            );
            for y in 0..output.height as usize {
                for x in 0..34 {
                    assert_eq!(
                        output.rgba[(y * 34 + x) * 4 + 3],
                        image.rgba[((y / 2) * 17 + x / 2) * 4 + 3]
                    );
                }
            }
        }
        generation.store(2, Ordering::Relaxed);
        let image = Decoded {
            width: 1,
            height: 1,
            rgba: std::sync::Arc::new(vec![255; 4]),
        };
        assert!(
            upscaler
                .run(
                    &format!("{directory}/fast-2x.json"),
                    0,
                    &image,
                    &generation,
                    1
                )
                .is_err()
        );
    }
}
