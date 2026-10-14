use std::path::Path;

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
use mac as platform;
#[cfg(windows)]
mod win;
#[cfg(windows)]
use win as platform;
#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use super::*;

    pub struct Player;

    impl Player {
        pub fn open(_: &Path, _: (u32, u32)) -> Player {
            Player
        }
        pub fn state(&self) -> State {
            State {
                failed: true,
                ..State::default()
            }
        }
        pub fn frame(&self) -> Option<Frame> {
            None
        }
        pub fn send(&self, _: Cmd) {}
    }

    pub fn thumbnail(_: &Path, _: (u32, u32)) -> Option<Frame> {
        None
    }
}

pub const EXTENSIONS: &[&str] = &["mp4", "mov", "m4v"];

pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub position: f64,
    pub duration: f64,
    pub playing: bool,
    pub ended: bool,
    pub muted: bool,
    pub volume: f64,
    pub failed: bool,
}

pub(crate) enum Cmd {
    Play,
    Pause,
    Seek(f64),
    Mute(bool),
    Volume(f64),
}

/// A video playing. Open it and call it from one thread, the main one on macOS, where the system
/// player has to live. Dropping it stops the playback.
pub struct Player(platform::Player);

impl Player {
    /// Starts `path` playing, with frames scaled down to fit inside `max`.
    pub fn open(path: &Path, max: (u32, u32)) -> Player {
        Player(platform::Player::open(path, max))
    }

    pub fn state(&self) -> State {
        self.0.state()
    }

    /// The newest frame, once. `None` until the next one is decoded.
    pub fn frame(&self) -> Option<Frame> {
        self.0.frame()
    }

    pub fn play(&self) {
        self.0.send(Cmd::Play);
    }

    pub fn pause(&self) {
        self.0.send(Cmd::Pause);
    }

    pub fn seek(&self, seconds: f64) {
        self.0.send(Cmd::Seek(seconds));
    }

    pub fn mute(&self, muted: bool) {
        self.0.send(Cmd::Mute(muted));
    }

    pub fn set_volume(&self, volume: f64) {
        self.0.send(Cmd::Volume(volume.clamp(0., 1.)));
    }
}

pub fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// The first frame of a video, scaled down to fit inside `max`. Blocks while it decodes.
pub fn thumbnail(path: &Path, max: (u32, u32)) -> Option<Frame> {
    platform::thumbnail(path, max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn clip() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/clip.mp4")
    }

    fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(
                start.elapsed() < Duration::from_secs(8),
                "timed out waiting for {what}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    #[ignore = "needs the OS video engine, which CI's Windows Server runners lack"]
    fn a_clip_plays_pauses_seeks_and_ends() {
        let player = Player::open(&clip(), (1920, 1080));
        let mut shown = None;
        wait_for("a frame", || {
            shown = player.frame();
            shown.is_some()
        });
        let f = shown.unwrap();
        assert_eq!((f.width, f.height), (320, 180));
        assert_eq!(f.bgra.len(), 320 * 180 * 4);
        assert!(
            f.bgra
                .chunks(4)
                .any(|p| p[0] > 40 || p[1] > 40 || p[2] > 40)
        );
        wait_for("playback to move", || player.state().position > 0.3);
        assert!((player.state().duration - 2.).abs() < 0.1);

        player.pause();
        wait_for("the pause", || !player.state().playing);
        let at = player.state().position;
        std::thread::sleep(Duration::from_millis(300));
        assert!((player.state().position - at).abs() < 0.05);

        player.set_volume(0.25);
        wait_for("the volume", || (player.state().volume - 0.25).abs() < 0.01);

        player.frame();
        player.seek(1.5);
        wait_for("the seek", || (player.state().position - 1.5).abs() < 0.1);
        wait_for("the frame after the seek", || player.frame().is_some());

        player.play();
        wait_for("the end", || player.state().ended);
        player.play();
        wait_for("a replay from the start", || {
            let s = player.state();
            s.playing && s.position < 1.
        });
    }

    #[test]
    #[ignore = "needs the OS video engine, which CI's Windows Server runners lack"]
    fn a_thumbnail_is_the_first_frame_fitted() {
        let f = thumbnail(&clip(), (160, 160)).expect("a thumbnail");
        assert_eq!((f.width, f.height), (160, 90));
        assert_eq!(f.bgra.len(), 160 * 90 * 4);
        assert!(
            f.bgra
                .chunks(4)
                .any(|p| p[0] > 40 || p[1] > 40 || p[2] > 40)
        );
    }

    #[test]
    fn a_missing_file_fails_instead_of_hanging() {
        let player = Player::open(Path::new("no/such/clip.mp4"), (640, 360));
        wait_for("the failure", || player.state().failed);
        assert!(thumbnail(Path::new("no/such/clip.mp4"), (64, 64)).is_none());
    }

    #[test]
    fn videos_are_known_by_extension() {
        assert!(is_video(Path::new("C:/clips/Roblox.MP4")));
        assert!(is_video(Path::new("a.mov")));
        assert!(!is_video(Path::new("a.png")));
        assert!(!is_video(Path::new("mp4")));
    }
}
