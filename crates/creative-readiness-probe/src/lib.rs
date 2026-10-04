//! Reference ffmpeg probe. Not a production transcode farm.

use std::path::{Path, PathBuf};
use std::process::Command;

use creative_readiness_core::{PreparedMedia, PreparedRendition, TranscodeLadder};
use sha2::{Digest, Sha256};

pub fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn prepare(
    input: &Path,
    ladder: &TranscodeLadder,
    out_dir: &Path,
) -> Result<PreparedMedia, String> {
    if !ffmpeg_available() {
        return Err("ffmpeg not on PATH (reference probe only)".into());
    }
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let source = std::fs::read(input).map_err(|e| e.to_string())?;
    let source_hash = hex_sha256(&source);
    let mut renditions = Vec::new();
    for rung in &ladder.renditions {
        let dest: PathBuf = out_dir.join(format!("{}.mp4", rung.name));
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-i",
                input.to_str().ok_or("input path")?,
                "-c:v",
                &rung.vcodec,
                "-pix_fmt",
                "yuv420p",
                "-s",
                &format!("{}x{}", rung.width, rung.height),
                "-t",
                "0.2",
                "-movflags",
                "+faststart",
                dest.to_str().ok_or("dest path")?,
            ])
            .output()
            .map_err(|e| e.to_string())?;
        if !status.status.success() {
            return Err(format!(
                "ffmpeg {} failed: {}",
                rung.name,
                String::from_utf8_lossy(&status.stderr)
            ));
        }
        let bytes = std::fs::read(&dest).map_err(|e| e.to_string())?;
        renditions.push(PreparedRendition {
            name: rung.name.clone(),
            sha256: hex_sha256(&bytes),
        });
    }
    Ok(PreparedMedia {
        ladder_id: ladder.id.clone(),
        source_hash,
        renditions,
        packaging: ladder.packaging.clone().or_else(|| Some("mp4".into())),
        trackers_preserved: Some(true),
        mapped_ad_id: None,
    })
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use creative_readiness_core::discover_policy;

    #[test]
    fn prepare_skips_without_ffmpeg() {
        if ffmpeg_available() {
            return;
        }
        let p = discover_policy("live-ssai").unwrap();
        let ladder = p.transcode_ladder.unwrap();
        let err = prepare(Path::new("/dev/null"), &ladder, Path::new("/tmp")).unwrap_err();
        assert!(err.contains("ffmpeg"));
    }

    #[test]
    fn prepare_hashes_are_stable_across_two_runs() {
        if !ffmpeg_available() {
            return;
        }
        let p = discover_policy("live-ssai").unwrap();
        let ladder = p.transcode_ladder.unwrap();
        let root = std::env::temp_dir().join(format!("cr-probe-{}", std::process::id()));
        let src = root.join("src.mp4");
        let a = root.join("a");
        let b = root.join("b");
        std::fs::create_dir_all(&root).unwrap();
        let gen = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=64x64:d=0.2",
                "-an",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                src.to_str().unwrap(),
            ])
            .output()
            .expect("ffmpeg lavfi");
        assert!(
            gen.status.success(),
            "{}",
            String::from_utf8_lossy(&gen.stderr)
        );
        let first = prepare(&src, &ladder, &a).expect("prepare a");
        let second = prepare(&src, &ladder, &b).expect("prepare b");
        assert_eq!(first.source_hash, second.source_hash);
        assert_eq!(first.renditions, second.renditions);
        assert_eq!(first.renditions.len(), 1);
        assert_eq!(first.ladder_id, "live-ssai-default");
        let _ = std::fs::remove_dir_all(&root);
    }
}
