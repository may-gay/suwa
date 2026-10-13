use crate::{reader, upscale::Upscaler};
use anyhow::{Context, Result, ensure};
use std::sync::atomic::AtomicU64;

pub fn run(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("--version") => println!("Suwa {}", env!("CARGO_PKG_VERSION")),
        Some("--help") => println!(
            "suwa\n  --version\n  --upscale-check MANIFEST INPUT OUTPUT\n  --install-model ID|all DIRECTORY\n\nStart without arguments to open the native reader."
        ),
        Some("--install-model") => {
            ensure!(
                args.len() == 3,
                "usage: suwa --install-model ID|all DIRECTORY"
            );
            let selected: Vec<_> = crate::model_catalog::presets()
                .iter()
                .filter(|p| args[1] == "all" || args[1] == p.id)
                .collect();
            ensure!(!selected.is_empty(), "Unknown model ID");
            let runtime = tokio::runtime::Runtime::new()?;
            for preset in selected {
                let progress = crate::model_catalog::Progress::new(preset);
                let path = runtime.block_on(crate::model_catalog::install(
                    preset,
                    std::path::PathBuf::from(&args[2]),
                    progress,
                ))?;
                println!("{}: {path}", preset.name);
            }
        }
        Some("--upscale-check") => {
            ensure!(
                args.len() == 4,
                "usage: suwa --upscale-check MANIFEST INPUT OUTPUT"
            );
            let input = reader::decode(&std::fs::read(&args[2]).context("Read input image")?)?;
            let output = std::path::Path::new(&args[3]);
            ensure!(!output.exists(), "Output file already exists");
            let mut upscaler = Upscaler::default();
            let generation = AtomicU64::new(1);
            for pass in 0..3 {
                let started = std::time::Instant::now();
                let enhanced = upscaler.run(&args[1], 0, &input, &generation, 1)?;
                println!(
                    "{}: {}x{} -> {}x{} in {:.1} ms",
                    if pass == 0 { "cold" } else { "warm" },
                    input.width,
                    input.height,
                    enhanced.width,
                    enhanced.height,
                    started.elapsed().as_secs_f64() * 1000.0
                );
                if pass == 2 {
                    let file = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(output)?;
                    let mut writer = std::io::BufWriter::new(file);
                    image::write_buffer_with_format(
                        &mut writer,
                        &enhanced.rgba,
                        enhanced.width,
                        enhanced.height,
                        image::ExtendedColorType::Rgba8,
                        image::ImageFormat::Png,
                    )?;
                }
            }
        }
        _ => anyhow::bail!("Unknown option; use --help"),
    }
    Ok(())
}
