// The picture in the sidebar's top corner: the picked one fitted to the corner and faded out from
// it, drawn once here so the sidebar only paints a file.

use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{ImageFormat, RgbaImage};

const WIDE: u32 = 360;
const TALL: u32 = 420;

/// The faded copy of `source` in `dir`, the only file kept there. None clears the corner.
pub fn set(dir: &Path, source: Option<&Path>) -> Result<Option<PathBuf>, String> {
    let Some(source) = source else {
        keep_only(dir, None);
        return Ok(None);
    };
    let bytes = std::fs::read(source).map_err(|_| "That picture couldn't be read.".to_string())?;
    let picture = image::load_from_memory(&bytes)
        .map_err(|_| "That file isn't a picture HyprSpace can show.".to_string())?;
    let wide = picture.width().clamp(1, WIDE);
    let fitted = picture.resize(wide, u32::MAX, FilterType::Triangle);
    let tall = fitted.height().min(TALL);
    let mut art = fitted.crop_imm(0, 0, wide, tall).to_rgba8();
    fade(&mut art);
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{:016x}.png", fnv(&bytes)));
    art.save_with_format(&path, ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    keep_only(dir, Some(&path));
    Ok(Some(path))
}

/// Solid in the corner, gone by most of the way across, along an ellipse a little wider and
/// taller than the picture.
fn fade(art: &mut RgbaImage) {
    let (w, h) = (art.width() as f32 * 1.2, art.height() as f32 * 1.15);
    for (x, y, p) in art.enumerate_pixels_mut() {
        let d = ((x as f32 / w).powi(2) + (y as f32 / h).powi(2)).sqrt();
        let t = ((d - 0.16) / (0.7 - 0.16)).clamp(0., 1.);
        let keep = 1. - t * t * (3. - 2. * t);
        p[3] = (p[3] as f32 * keep).round() as u8;
    }
}

fn keep_only(dir: &Path, keep: Option<&Path>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if Some(path.as_path()) != keep {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_corner_stays_and_the_far_side_fades_away() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("wide.png");
        RgbaImage::from_pixel(1200, 800, image::Rgba([200, 40, 40, 255]))
            .save(&source)
            .unwrap();
        let out = dir.path().join("art");
        let path = set(&out, Some(&source)).unwrap().unwrap();
        let art = image::open(&path).unwrap().to_rgba8();
        assert_eq!((art.width(), art.height()), (WIDE, 240));
        assert_eq!(art.get_pixel(0, 0)[3], 255);
        assert_eq!(art.get_pixel(WIDE - 1, 239)[3], 0);
        assert_eq!(set(&out, None).unwrap(), None);
        assert!(!path.exists());
    }
}
