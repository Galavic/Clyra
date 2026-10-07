//! Asset build tool; never runs in the application.
//! cargo run -p clyra-ui --example export-dot-animations -- frames.ndjson output-dir
use serde::Deserialize;
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Command, Stdio},
    sync::Arc,
};

#[derive(Deserialize)]
struct Clip {
    index: usize,
    state: String,
    duration: f32,
    frames: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    anyhow::ensure!(
        args.len() == 3,
        "expected input NDJSON and output directory"
    );
    let output = Path::new(&args[2]);
    fs::create_dir_all(output)?;
    let renderer = gpui::SvgRenderer::new(Arc::new(()));
    for line in BufReader::new(fs::File::open(&args[1])?).lines() {
        let clip: Clip = serde_json::from_str(&line?)?;
        let filename = output.join(format!("dot-{}-{}.webp", clip.index, clip.state));
        let mut encoder = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "rawvideo",
                "-pixel_format",
                "bgra",
                "-video_size",
                "128x128",
                "-framerate",
                "20",
                "-i",
                "pipe:0",
                "-c:v",
                "libwebp_anim",
                "-lossless",
                "1",
                "-compression_level",
                "4",
                "-loop",
                "0",
            ])
            .arg(&filename)
            .stdin(Stdio::piped())
            .spawn()?;
        let mut input = encoder.stdin.take().unwrap();
        for svg in &clip.frames {
            let image = renderer.render_single_frame(svg.as_bytes(), 0.25)?;
            anyhow::ensure!(
                image.size(0).width.0 == 128 && image.size(0).height.0 == 128,
                "unexpected frame dimensions"
            );
            input.write_all(image.as_bytes(0).unwrap())?;
        }
        drop(input);
        anyhow::ensure!(encoder.wait()?.success(), "WebP encoding failed");
        println!(
            "{}: {} frames, {}s",
            filename.display(),
            clip.frames.len(),
            clip.duration
        );
    }
    Ok(())
}
