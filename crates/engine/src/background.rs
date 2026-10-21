// The new-thread background: a copy of the picked picture in the app's own folder, its main color
// for the theme taken from it, and the picture again with its effect drawn in.

use std::path::{Path, PathBuf};

use hyprspace_proto::state::{Background, Effect};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

const PICTURES: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];
const WIDEST: u32 = 2048;

pub fn set(dir: &Path, source: &Path, effect: Effect) -> Result<Background, String> {
    let bytes = std::fs::read(source).map_err(|_| "That picture couldn't be read.".to_string())?;
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    set_bytes(dir, &name, &bytes, effect)
}

pub fn set_bytes(
    dir: &Path,
    name: &str,
    bytes: &[u8],
    effect: Effect,
) -> Result<Background, String> {
    let format = image::guess_format(bytes)
        .map_err(|_| "That file isn't a picture HyprSpace can show.".to_string())?;
    let picture = image::load_from_memory_with_format(bytes, format)
        .map_err(|_| "That picture is damaged or in a format HyprSpace can't read.".to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let id = format!("{:016x}", fnv(bytes));
    let ext = format.extensions_str().first().copied().unwrap_or("png");
    let copy = dir.join(format!("{id}.{ext}"));
    std::fs::write(&copy, bytes).map_err(|e| e.to_string())?;
    let (hue, chroma) = main_color(&picture);
    let mut background = Background {
        source: copy.clone(),
        shown: copy,
        name: name.to_string(),
        hue,
        chroma,
        zoom: 1.0,
        x: 0.5,
        y: 0.5,
    };
    background.shown = draw(dir, &id, &background.source, &picture, effect)?;
    tidy(dir, &background);
    Ok(background)
}

pub fn restyle(
    dir: &Path,
    mut background: Background,
    effect: Effect,
) -> Result<Background, String> {
    let picture = image::open(&background.source)
        .map_err(|_| "The background's copy is gone. Pick the picture again.".to_string())?;
    let id = background
        .source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    background.shown = draw(dir, &id, &background.source, &picture, effect)?;
    tidy(dir, &background);
    Ok(background)
}

pub fn shuffle(
    dir: &Path,
    folder: &Path,
    skip: Option<&str>,
    effect: Effect,
) -> Result<Background, String> {
    let entries = std::fs::read_dir(folder)
        .map_err(|_| "The wallpaper folder couldn't be read. Choose it again.".to_string())?;
    let mut pictures: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && is_picture(p))
        .collect();
    if pictures.len() > 1 {
        pictures.retain(|p| p.file_name().and_then(|n| n.to_str()) != skip);
    }
    if pictures.is_empty() {
        return Err("That folder has no pictures in it.".into());
    }
    let start = seed() as usize % pictures.len();
    pictures.rotate_left(start);
    let mut last = String::new();
    for picture in &pictures {
        match set(dir, picture, effect) {
            Ok(b) => return Ok(b),
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn is_picture(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| PICTURES.contains(&e.to_ascii_lowercase().as_str()))
}

fn seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x100000001b3)
    })
}

fn tidy(dir: &Path, background: &Background) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path != background.source && path != background.shown {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn draw(
    dir: &Path,
    id: &str,
    source: &Path,
    picture: &DynamicImage,
    effect: Effect,
) -> Result<PathBuf, String> {
    if effect == Effect::None {
        return Ok(source.to_path_buf());
    }
    let picture = if picture.width() > WIDEST {
        picture.resize(WIDEST, u32::MAX, image::imageops::FilterType::Triangle)
    } else {
        picture.clone()
    };
    let out = match effect {
        Effect::None => unreachable!(),
        Effect::Dither => dither(&picture.to_rgba8()),
        Effect::Ascii => ascii(&picture.to_rgba8()),
        Effect::Halftone => halftone(&picture.to_rgba8()),
        Effect::Scanlines => scanlines(&picture.to_rgba8()),
    };
    let path = dir.join(format!("{id}-{}.png", effect.label().to_ascii_lowercase()));
    out.save_with_format(&path, ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn main_color(picture: &DynamicImage) -> (f32, f32) {
    let small = picture.thumbnail(64, 64).to_rgba8();
    let mut bins = vec![(0.0_f64, [0.0_f64; 3]); 4096];
    for Rgba([r, g, b, a]) in small.pixels().copied() {
        if a < 128 {
            continue;
        }
        let spread = (r.max(g).max(b) - r.min(g).min(b)) as f64 / 255.0;
        let weight = 0.02 + spread * spread;
        let ix = ((r as usize >> 4) << 8) | ((g as usize >> 4) << 4) | (b as usize >> 4);
        let (count, sum) = &mut bins[ix];
        *count += weight;
        for (s, c) in sum.iter_mut().zip([r, g, b]) {
            *s += c as f64 * weight;
        }
    }
    let Some((count, sum)) = bins.into_iter().max_by(|a, b| a.0.total_cmp(&b.0)) else {
        return (264.0, 0.0);
    };
    if count <= 0.0 {
        return (264.0, 0.0);
    }
    let rgb = sum.map(|s| s / count / 255.0);
    oklch(rgb)
}

fn oklch([r, g, b]: [f64; 3]) -> (f32, f32) {
    let lin = |c: f64| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (lin(r), lin(g), lin(b));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    let a = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
    let bb = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;
    let chroma = (a * a + bb * bb).sqrt();
    let hue = bb.atan2(a).to_degrees().rem_euclid(360.0);
    (hue as f32, chroma as f32)
}

fn luma(p: &Rgba<u8>) -> f32 {
    (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0
}

fn opaque([r, g, b]: [u8; 3]) -> Rgba<u8> {
    Rgba([r, g, b, 255])
}

fn mix(picture: u8, over: f32) -> u8 {
    (picture as f32 * 0.6 + over * 0.4) as u8
}

fn dither(src: &RgbaImage) -> RgbaImage {
    const BAYER: [[f32; 4]; 4] = [
        [0.0, 8.0, 2.0, 10.0],
        [12.0, 4.0, 14.0, 6.0],
        [3.0, 11.0, 1.0, 9.0],
        [15.0, 7.0, 13.0, 5.0],
    ];
    let (w, h) = src.dimensions();
    RgbaImage::from_fn(w, h, |x, y| {
        let p = src.get_pixel((x | 1).min(w - 1), (y | 1).min(h - 1));
        let peak = p[0].max(p[1]).max(p[2]) as f32;
        let t = (BAYER[(y / 2 % 4) as usize][(x / 2 % 4) as usize] + 0.5) / 16.0;
        let gain = if peak / 255.0 > t {
            255.0 / peak.max(1.0)
        } else {
            0.08
        };
        opaque([0, 1, 2].map(|i| (p[i] as f32 * gain).round() as u8))
    })
}

fn ascii(src: &RgbaImage) -> RgbaImage {
    // 5x7 glyphs, one row per byte, high bit on the left: " .:-=+*#%@"
    const GLYPHS: [[u8; 7]; 10] = [
        [0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0b00100],
        [0, 0b00100, 0, 0, 0, 0b00100, 0],
        [0, 0, 0, 0b11111, 0, 0, 0],
        [0, 0, 0b11111, 0, 0b11111, 0, 0],
        [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
        [0, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0],
        [0b01010, 0b11111, 0b01010, 0b01010, 0b11111, 0b01010, 0],
        [0b11001, 0b11010, 0b00100, 0b01000, 0b10110, 0b10011, 0],
        [
            0b01110, 0b10001, 0b10111, 0b10101, 0b10110, 0b10000, 0b01111,
        ],
    ];
    let (w, h) = src.dimensions();
    RgbaImage::from_fn(w, h, |x, y| {
        let cell = src.get_pixel((x / 6 * 6 + 3).min(w - 1), (y / 8 * 8 + 4).min(h - 1));
        let glyph = &GLYPHS[((luma(cell).sqrt() * 9.0) as usize).min(9)];
        let ink = x % 6 < 5 && y % 8 < 7 && glyph[(y % 8) as usize] & (0b10000 >> (x % 6)) != 0;
        let p = src.get_pixel(x, y);
        opaque([0, 1, 2].map(|i| mix(p[i], if ink { cell[i] as f32 } else { 0.0 })))
    })
}

fn halftone(src: &RgbaImage) -> RgbaImage {
    let (w, h) = src.dimensions();
    RgbaImage::from_fn(w, h, |x, y| {
        let dot = src.get_pixel((x / 4 * 4 + 2).min(w - 1), (y / 4 * 4 + 2).min(h - 1));
        let radius = 2.0 * (0.3 + 0.7 * luma(dot).sqrt());
        let (dx, dy) = ((x % 4) as f32 - 1.5, (y % 4) as f32 - 1.5);
        let cover = (radius + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
        let p = src.get_pixel(x, y);
        opaque([0, 1, 2].map(|i| mix(p[i], dot[i] as f32 * cover)))
    })
}

fn scanlines(src: &RgbaImage) -> RgbaImage {
    let mut out = src.clone();
    for (_, y, p) in out.enumerate_pixels_mut() {
        if y % 3 == 0 {
            for i in 0..3 {
                p[i] = (p[i] as f32 * 0.52) as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(dir: &Path, name: &str, color: [u8; 3]) -> PathBuf {
        let path = dir.join(name);
        RgbaImage::from_pixel(40, 30, Rgba([color[0], color[1], color[2], 255]))
            .save(&path)
            .unwrap();
        path
    }

    #[test]
    fn a_picture_is_copied_in_with_its_color_and_effect() {
        let tmp = tempfile::tempdir().unwrap();
        let red = picture(tmp.path(), "red.png", [220, 30, 40]);
        let store = tmp.path().join("store");
        let b = set(&store, &red, Effect::Halftone).unwrap();
        assert_eq!(b.name, "red.png");
        assert!(b.source.starts_with(&store) && b.source.exists());
        assert!(b.shown != b.source && b.shown.exists());
        assert!(b.hue < 40.0 || b.hue > 340.0, "red hue, got {}", b.hue);
        assert!(b.chroma > 0.15);
        let plain = restyle(&store, b, Effect::None).unwrap();
        assert_eq!(plain.shown, plain.source);
        assert_eq!(std::fs::read_dir(&store).unwrap().count(), 1);
    }

    #[test]
    fn shuffle_skips_the_one_on_screen_and_says_when_a_folder_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let pics = tmp.path().join("pics");
        std::fs::create_dir_all(&pics).unwrap();
        picture(&pics, "a.png", [10, 200, 30]);
        picture(&pics, "b.png", [10, 30, 200]);
        std::fs::write(pics.join("notes.txt"), "not a picture").unwrap();
        let store = tmp.path().join("store");
        for _ in 0..4 {
            let b = shuffle(&store, &pics, Some("a.png"), Effect::None).unwrap();
            assert_eq!(b.name, "b.png");
        }
        let empty = tmp.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(shuffle(&store, &empty, None, Effect::None).is_err());
    }

    #[test]
    fn every_effect_draws() {
        let tmp = tempfile::tempdir().unwrap();
        let pic = picture(tmp.path(), "p.png", [120, 180, 60]);
        let store = tmp.path().join("store");
        for effect in Effect::ALL {
            let b = set(&store, &pic, effect).unwrap();
            let shown = image::open(&b.shown).unwrap();
            assert!(shown.width() > 0 && shown.height() > 0, "{effect:?}");
        }
    }
}
