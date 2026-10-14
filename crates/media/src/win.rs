use std::path::Path;
use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HMODULE, RECT, S_OK};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING, D3D11CreateDevice,
    ID3D11Device, ID3D11DeviceContext, ID3D11Multithread, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Media::MediaFoundation::{
    CLSID_MFMediaEngineClassFactory, IMFAttributes, IMFDXGIDeviceManager, IMFMediaEngine,
    IMFMediaEngineClassFactory, IMFMediaEngineNotify, IMFMediaEngineNotify_Impl,
    MF_MEDIA_ENGINE_CALLBACK, MF_MEDIA_ENGINE_DXGI_MANAGER, MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT,
    MF_VERSION, MFARGB, MFCreateAttributes, MFCreateDXGIDeviceManager, MFSTARTUP_FULL, MFShutdown,
    MFStartup,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::core::{BSTR, Interface, implement};

use crate::{Cmd, Frame, State};

// the engine reports through this, and refuses to start without it; the player polls instead
#[implement(IMFMediaEngineNotify)]
struct Notify;

impl IMFMediaEngineNotify_Impl for Notify_Impl {
    fn EventNotify(&self, _: u32, _: usize, _: u32) -> windows::core::Result<()> {
        Ok(())
    }
}

/// HAVE_CURRENT_DATA: a frame at the current position can be shown.
const HAVE_FRAME: u16 = 2;

struct Target {
    size: (u32, u32),
    render: ID3D11Texture2D,
    staging: ID3D11Texture2D,
}

pub struct Engine {
    engine: IMFMediaEngine,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    target: Option<Target>,
    max: (u32, u32),
    last: Option<i64>,
}

impl Engine {
    pub fn open(path: &Path, max: (u32, u32)) -> Result<Engine, String> {
        unsafe { Self::start(path, max) }.map_err(|e| e.to_string())
    }

    unsafe fn start(path: &Path, max: (u32, u32)) -> windows::core::Result<Engine> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
            let mut device = None;
            let mut context = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
            let (device, context): (ID3D11Device, ID3D11DeviceContext) =
                (device.unwrap(), context.unwrap());
            // the engine decodes on its own threads through this device
            let _ = device
                .cast::<ID3D11Multithread>()?
                .SetMultithreadProtected(true);
            let mut token = 0;
            let mut manager: Option<IMFDXGIDeviceManager> = None;
            MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
            let manager = manager.unwrap();
            manager.ResetDevice(&device, token)?;
            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 3)?;
            let attrs = attrs.unwrap();
            let notify: IMFMediaEngineNotify = Notify.into();
            attrs.SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, &notify)?;
            attrs.SetUnknown(&MF_MEDIA_ENGINE_DXGI_MANAGER, &manager)?;
            attrs.SetUINT32(
                &MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT,
                DXGI_FORMAT_B8G8R8A8_UNORM.0 as u32,
            )?;
            let factory: IMFMediaEngineClassFactory =
                CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_INPROC_SERVER)?;
            let engine = factory.CreateInstance(0, &attrs)?;
            engine.SetSource(&BSTR::from(path.to_string_lossy().as_ref()))?;
            Ok(Engine {
                engine,
                device,
                context,
                target: None,
                max,
                last: None,
            })
        }
    }

    pub fn apply(&mut self, cmd: Cmd) {
        let e = &self.engine;
        unsafe {
            let _ = match cmd {
                Cmd::Play => {
                    if e.IsEnded().as_bool() {
                        let _ = e.SetCurrentTime(0.);
                    }
                    e.Play()
                }
                Cmd::Pause => e.Pause(),
                Cmd::Seek(t) => e.SetCurrentTime(t.max(0.)),
                Cmd::Mute(m) => e.SetMuted(m),
                Cmd::Volume(v) => e.SetVolume(v),
            };
        }
    }

    pub fn frame(&mut self) -> Option<Frame> {
        unsafe { self.grab() }.ok().flatten()
    }

    unsafe fn grab(&mut self) -> windows::core::Result<Option<Frame>> {
        unsafe {
            if self.engine.GetReadyState() < HAVE_FRAME {
                return Ok(None);
            }
            // S_FALSE means no frame yet, which the typed binding reports as a success
            let mut pts = 0;
            let tick = (Interface::vtable(&self.engine).OnVideoStreamTick)(
                Interface::as_raw(&self.engine),
                &mut pts,
            );
            if tick != S_OK || self.last == Some(pts) {
                return Ok(None);
            }
            let (mut w, mut h) = (0, 0);
            self.engine.GetNativeVideoSize(Some(&mut w), Some(&mut h))?;
            let size = fit((w, h), self.max);
            if size.0 == 0 {
                return Ok(None);
            }
            if self.target.as_ref().is_none_or(|t| t.size != size) {
                self.target = Some(self.textures(size)?);
            }
            let t = self.target.as_ref().unwrap();
            let rect = RECT {
                left: 0,
                top: 0,
                right: size.0 as i32,
                bottom: size.1 as i32,
            };
            let black = MFARGB {
                rgbBlue: 0,
                rgbGreen: 0,
                rgbRed: 0,
                rgbAlpha: 255,
            };
            self.engine
                .TransferVideoFrame(&t.render, None, &rect, Some(&black))?;
            self.last = Some(pts);
            self.context.CopyResource(&t.staging, &t.render);
            let mut map = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(&t.staging, 0, D3D11_MAP_READ, 0, Some(&mut map))?;
            let row = size.0 as usize * 4;
            let mut bgra = Vec::with_capacity(row * size.1 as usize);
            for y in 0..size.1 as usize {
                let src = (map.pData as *const u8).add(y * map.RowPitch as usize);
                bgra.extend_from_slice(std::slice::from_raw_parts(src, row));
            }
            self.context.Unmap(&t.staging, 0);
            Ok(Some(Frame {
                width: size.0,
                height: size.1,
                bgra,
            }))
        }
    }

    unsafe fn textures(&self, size: (u32, u32)) -> windows::core::Result<Target> {
        let mut desc = D3D11_TEXTURE2D_DESC {
            Width: size.0,
            Height: size.1,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        unsafe {
            let mut render = None;
            self.device
                .CreateTexture2D(&desc, None, Some(&mut render))?;
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            let mut staging = None;
            self.device
                .CreateTexture2D(&desc, None, Some(&mut staging))?;
            Ok(Target {
                size,
                render: render.unwrap(),
                staging: staging.unwrap(),
            })
        }
    }

    pub fn state(&self) -> State {
        let e = &self.engine;
        unsafe {
            let duration = e.GetDuration();
            State {
                position: e.GetCurrentTime(),
                duration: if duration.is_finite() { duration } else { 0. },
                playing: !e.IsPaused().as_bool(),
                ended: e.IsEnded().as_bool(),
                muted: e.GetMuted().as_bool(),
                volume: e.GetVolume(),
                failed: e.GetError().is_ok(),
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        unsafe {
            let _ = self.engine.Shutdown();
            let _ = MFShutdown();
        }
    }
}

pub fn thumbnail(path: &Path, max: (u32, u32)) -> Option<Frame> {
    let mut engine = Engine::open(path, max).ok()?;
    unsafe {
        let _ = engine.engine.SetMuted(true);
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if let Some(f) = engine.frame() {
            return Some(f);
        }
        if engine.state().failed {
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

#[derive(Default)]
struct Shared {
    state: State,
    frame: Option<Frame>,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

/// The engine runs on a thread of its own, which polls it for frames every few milliseconds.
pub struct Player {
    tx: mpsc::Sender<Cmd>,
    shared: Arc<Mutex<Shared>>,
}

impl Player {
    pub fn open(path: &Path, max: (u32, u32)) -> Player {
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (path, out) = (path.to_path_buf(), shared.clone());
        std::thread::spawn(move || {
            let Ok(mut engine) = Engine::open(&path, max) else {
                lock(&out).state.failed = true;
                return;
            };
            engine.apply(Cmd::Play);
            loop {
                loop {
                    match rx.try_recv() {
                        Ok(cmd) => engine.apply(cmd),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => return,
                    }
                }
                let frame = engine.frame();
                let state = engine.state();
                let mut shared = lock(&out);
                if frame.is_some() {
                    shared.frame = frame;
                }
                shared.state = state;
                drop(shared);
                std::thread::sleep(Duration::from_millis(4));
            }
        });
        Player { tx, shared }
    }

    pub fn state(&self) -> State {
        lock(&self.shared).state
    }

    pub fn frame(&self) -> Option<Frame> {
        lock(&self.shared).frame.take()
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }
}

/// `size` scaled down to fit inside `max`, keeping its shape. Never scaled up.
fn fit((w, h): (u32, u32), (mw, mh): (u32, u32)) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (0, 0);
    }
    let scale = (mw as f64 / w as f64).min(mh as f64 / h as f64).min(1.);
    (
        ((w as f64 * scale).round() as u32).max(1),
        ((h as f64 * scale).round() as u32).max(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_fit_inside_the_box_and_keep_their_shape() {
        assert_eq!(fit((3840, 2160), (1920, 1080)), (1920, 1080));
        assert_eq!(fit((1080, 1920), (1920, 1080)), (608, 1080));
        assert_eq!(fit((640, 360), (1920, 1080)), (640, 360));
        assert_eq!(fit((0, 0), (1920, 1080)), (0, 0));
    }
}
