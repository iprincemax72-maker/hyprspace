use std::path::Path;
use std::time::Duration;

use block2::RcBlock;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, MainThreadMarker};
use objc2_av_foundation::{
    AVAsset, AVAssetImageGenerator, AVPlayer, AVPlayerItem, AVPlayerItemStatus,
    AVPlayerItemVideoOutput,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo, CGImageByteOrderInfo,
};
use objc2_core_media::{CMTime, kCMTimeZero};
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight,
    CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferUnlockBaseAddress, kCVPixelFormatType_32BGRA,
};
use objc2_foundation::{NSDictionary, NSError, NSNumber, NSString, NSURL};

use crate::{Cmd, Frame, State};

fn file_url(path: &Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
}

struct Av {
    player: Retained<AVPlayer>,
    item: Retained<AVPlayerItem>,
    output: Retained<AVPlayerItemVideoOutput>,
}

/// AVPlayer only lives on the main thread, so this one runs there and is polled from the UI.
pub struct Player {
    av: Option<Av>,
}

impl Player {
    pub fn open(path: &Path, _max: (u32, u32)) -> Player {
        let av = MainThreadMarker::new().map(|mtm| unsafe {
            let item = AVPlayerItem::playerItemWithURL(&file_url(path), mtm);
            // kCVPixelBufferPixelFormatTypeKey
            let key = NSString::from_str("PixelFormatType");
            let format = NSNumber::new_u32(kCVPixelFormatType_32BGRA);
            let attrs = NSDictionary::<NSString, AnyObject>::from_slices(&[&*key], &[&*format]);
            let output = AVPlayerItemVideoOutput::initWithPixelBufferAttributes(
                AVPlayerItemVideoOutput::alloc(),
                Some(&attrs),
            );
            item.addOutput(&output);
            let player = AVPlayer::playerWithPlayerItem(Some(&item), mtm);
            player.play();
            Av {
                player,
                item,
                output,
            }
        });
        Player { av }
    }

    pub fn state(&self) -> State {
        let Some(av) = &self.av else {
            return State {
                failed: true,
                ..State::default()
            };
        };
        unsafe {
            let position = av.player.currentTime().seconds();
            let duration = av.item.duration().seconds();
            let duration = if duration.is_finite() { duration } else { 0. };
            let playing = av.player.rate() != 0.;
            State {
                position,
                duration,
                playing,
                ended: !playing && duration > 0. && position >= duration - 0.05,
                muted: av.player.isMuted(),
                volume: av.player.volume() as f64,
                failed: av.item.status() == AVPlayerItemStatus::Failed,
            }
        }
    }

    pub fn frame(&self) -> Option<Frame> {
        let av = self.av.as_ref()?;
        unsafe {
            let at = av.player.currentTime();
            if !av.output.hasNewPixelBufferForItemTime(at) {
                return None;
            }
            let buffer = av
                .output
                .copyPixelBufferForItemTime_itemTimeForDisplay(at, std::ptr::null_mut())?;
            CVPixelBufferLockBaseAddress(&buffer, CVPixelBufferLockFlags::ReadOnly);
            let (w, h) = (
                CVPixelBufferGetWidth(&buffer),
                CVPixelBufferGetHeight(&buffer),
            );
            let (base, stride) = (
                CVPixelBufferGetBaseAddress(&buffer) as *const u8,
                CVPixelBufferGetBytesPerRow(&buffer),
            );
            let row = w * 4;
            let mut bgra = Vec::with_capacity(row * h);
            if !base.is_null() {
                for y in 0..h {
                    bgra.extend_from_slice(std::slice::from_raw_parts(base.add(y * stride), row));
                }
            }
            CVPixelBufferUnlockBaseAddress(&buffer, CVPixelBufferLockFlags::ReadOnly);
            (bgra.len() == row * h && h > 0).then_some(Frame {
                width: w as u32,
                height: h as u32,
                bgra,
            })
        }
    }

    pub fn send(&self, cmd: Cmd) {
        let Some(av) = &self.av else {
            return;
        };
        unsafe {
            match cmd {
                Cmd::Play => {
                    if self.state().ended {
                        av.player.seekToTime_toleranceBefore_toleranceAfter(
                            kCMTimeZero,
                            kCMTimeZero,
                            kCMTimeZero,
                        );
                    }
                    av.player.play();
                }
                Cmd::Pause => av.player.pause(),
                Cmd::Seek(t) => av.player.seekToTime_toleranceBefore_toleranceAfter(
                    CMTime::with_seconds(t.max(0.), 600),
                    kCMTimeZero,
                    kCMTimeZero,
                ),
                Cmd::Mute(m) => av.player.setMuted(m),
                Cmd::Volume(v) => av.player.setVolume(v as f32),
            }
        }
    }
}

pub fn thumbnail(path: &Path, max: (u32, u32)) -> Option<Frame> {
    let (tx, rx) = std::sync::mpsc::channel();
    unsafe {
        let asset = AVAsset::assetWithURL(&file_url(path));
        let generator = AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset);
        generator.setAppliesPreferredTrackTransform(true);
        generator.setMaximumSize(CGSize::new(max.0 as f64, max.1 as f64));
        let done = RcBlock::new(move |image: *mut CGImage, _: CMTime, _: *mut NSError| {
            let _ = tx.send(image.as_ref().and_then(bgra));
        });
        generator.generateCGImageAsynchronouslyForTime_completionHandler(kCMTimeZero, &done);
    }
    rx.recv_timeout(Duration::from_secs(5)).ok().flatten()
}

fn bgra(image: &CGImage) -> Option<Frame> {
    let (w, h) = (CGImage::width(Some(image)), CGImage::height(Some(image)));
    if w == 0 || h == 0 {
        return None;
    }
    let mut bgra = vec![0u8; w * h * 4];
    let space = CGColorSpace::new_device_rgb()?;
    let context = unsafe {
        CGBitmapContextCreate(
            bgra.as_mut_ptr().cast(),
            w,
            h,
            8,
            w * 4,
            Some(&space),
            CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0,
        )
    }?;
    let rect = CGRect::new(CGPoint::new(0., 0.), CGSize::new(w as f64, h as f64));
    CGContext::draw_image(Some(&context), rect, Some(image));
    drop(context);
    Some(Frame {
        width: w as u32,
        height: h as u32,
        bgra,
    })
}
