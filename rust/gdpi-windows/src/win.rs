//! WinDivert FFI ve paket döngüsü (yalnız Windows).

use crate::{addr_outbound, wd_error_msg, windivert_dir, ADDR_SIZE};
use gdpi_core::{filter_str, Backend, Config, Engine, EngineError, LogFn, Stats, Verdict};
use std::ffi::{c_void, CString, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::thread::JoinHandle;
use windows_sys::Win32::Foundation::{GetLastError, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_WITH_ALTERED_SEARCH_PATH,
};

type Handle = *mut c_void;
type RawFn = unsafe extern "system" fn() -> isize;
const LAYER_NETWORK: i32 = 0;
const SHUTDOWN_BOTH: i32 = 3;
const BUF_SIZE: usize = 65535;

type OpenFn = unsafe extern "C" fn(*const i8, i32, i16, u64) -> Handle;
type IoFn = unsafe extern "C" fn(Handle, *mut c_void, u32, *mut u32, *mut c_void) -> i32;
type ShutdownFn = unsafe extern "C" fn(Handle, i32) -> i32;
type CloseFn = unsafe extern "C" fn(Handle) -> i32;
type CalcFn = unsafe extern "C" fn(*mut c_void, u32, *mut c_void, u64) -> i32;

/// Çözümlenmiş WinDivert fonksiyonları. DLL süreç boyunca yüklü kalır.
#[derive(Clone, Copy)]
struct Api {
    open: OpenFn,
    recv: IoFn,
    send: IoFn,
    shutdown: ShutdownFn,
    close: CloseFn,
    calc: CalcFn,
}

/// Açık bir WinDivert tutamacı; iş parçacıkları arasında paylaşılır.
struct Wd {
    api: Api,
    h: Handle,
}

// SAFETY: WinDivert tutamaçları çekirdek nesneleridir ve iş parçacıkları arasında
// eşzamanlı Recv/Send/Shutdown için güvenlidir (WinDivert belgesi).
unsafe impl Send for Wd {}
// SAFETY: yukarıdaki ile aynı gerekçe; tutamaç yalnız stop() içinde, thread join edildikten sonra kapanır.
unsafe impl Sync for Wd {}

fn load_api() -> Result<Api, u32> {
    let dir = windivert_dir();
    let dll = dir.join("WinDivert.dll");
    if !dll.is_file() || !dir.join("WinDivert64.sys").is_file() {
        return Err(2);
    }
    let wide: Vec<u16> = OsStr::new(&dll).encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` NUL ile biten mutlak bir yol; arama yolu kullanılmaz.
    let m = unsafe { LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH) };
    if m.is_null() {
        // SAFETY: parametresiz Win32 çağrısı.
        return Err(unsafe { GetLastError() });
    }
    let sym = |name: &[u8]| -> Result<RawFn, u32> {
        // SAFETY: `m` geçerli bir modül, `name` NUL ile biten ASCII.
        unsafe { GetProcAddress(m, name.as_ptr()) }.ok_or(127)
    };
    // SAFETY: WinDivert 2.2 imzaları yukarıdaki tür takma adlarıyla birebir aynıdır
    // (x64'te cdecl/stdcall farkı yoktur; WINDIVERTEXPORT = __cdecl).
    unsafe {
        Ok(Api {
            open: std::mem::transmute::<RawFn, OpenFn>(sym(b"WinDivertOpen\0")?),
            recv: std::mem::transmute::<RawFn, IoFn>(sym(b"WinDivertRecv\0")?),
            send: std::mem::transmute::<RawFn, IoFn>(sym(b"WinDivertSend\0")?),
            shutdown: std::mem::transmute::<RawFn, ShutdownFn>(sym(b"WinDivertShutdown\0")?),
            close: std::mem::transmute::<RawFn, CloseFn>(sym(b"WinDivertClose\0")?),
            calc: std::mem::transmute::<RawFn, CalcFn>(sym(b"WinDivertHelperCalcChecksums\0")?),
        })
    }
}

impl Wd {
    fn open(filter: &str) -> Result<Wd, EngineError> {
        let err = |e: u32| EngineError::new(e as i64, wd_error_msg(e));
        let api = load_api().map_err(err)?;
        let f = CString::new(filter).map_err(|_| err(87))?;
        // SAFETY: `f` NUL ile biten geçerli bir dize; diğerleri düz değerler.
        let h = unsafe { (api.open)(f.as_ptr(), LAYER_NETWORK, 0, 0) };
        if h.is_null() || h == INVALID_HANDLE_VALUE {
            // SAFETY: parametresiz Win32 çağrısı.
            return Err(err(unsafe { GetLastError() }));
        }
        Ok(Wd { api, h })
    }

    /// Paket alır; tutamaç kapandıysa None.
    fn recv(&self, buf: &mut [u8], addr: &mut [u8; ADDR_SIZE]) -> Option<usize> {
        let mut n: u32 = 0;
        // SAFETY: tamponlar verilen uzunluklarda geçerli ve yazılabilir.
        let ok = unsafe {
            (self.api.recv)(self.h, buf.as_mut_ptr().cast(), buf.len() as u32, &mut n, addr.as_mut_ptr().cast())
        };
        (ok != 0).then_some(n as usize)
    }

    fn calc(&self, pkt: &mut [u8], addr: &mut [u8; ADDR_SIZE]) {
        // SAFETY: `pkt` ve `addr` geçerli, yazılabilir tamponlar.
        unsafe { (self.api.calc)(pkt.as_mut_ptr().cast(), pkt.len() as u32, addr.as_mut_ptr().cast(), 0) };
    }

    fn send(&self, pkt: &[u8], addr: &[u8; ADDR_SIZE]) {
        // SAFETY: WinDivertSend tamponları yalnız okur; işaretçiler uzunluklarıyla geçerli.
        unsafe {
            (self.api.send)(self.h, pkt.as_ptr() as *mut c_void, pkt.len() as u32, std::ptr::null_mut(), addr.as_ptr() as *mut c_void)
        };
    }

    fn shutdown(&self) {
        // SAFETY: `h` açık bir tutamaç; Shutdown bekleyen Recv'i uyandırır.
        unsafe { (self.api.shutdown)(self.h, SHUTDOWN_BOTH) };
    }

    fn close(&self) {
        // SAFETY: yalnız bir kez, worker join edildikten sonra çağrılır.
        unsafe { (self.api.close)(self.h) };
    }
}

fn worker(wd: Arc<Wd>, mut engine: Engine, log: LogFn) {
    let mut buf = vec![0u8; BUF_SIZE];
    let mut addr = [0u8; ADDR_SIZE];
    while let Some(n) = wd.recv(&mut buf, &mut addr) {
        let raw = &buf[..n];
        let out = addr_outbound(&addr);
        let verdict = catch_unwind(AssertUnwindSafe(|| engine.process(raw, out)));
        match verdict {
            Ok(Verdict::Pass) => wd.send(raw, &addr),
            Ok(Verdict::Drop) => {}
            Ok(Verdict::Send(outs)) => {
                for mut o in outs {
                    if o.recalc {
                        wd.calc(&mut o.data, &mut addr);
                    } else {
                        // Sahte paketin bozuk sağlama toplamı korunur; yalnız addr bayrakları güncellenir.
                        let mut scratch = o.data.clone();
                        wd.calc(&mut scratch, &mut addr);
                    }
                    wd.send(&o.data, &addr);
                }
            }
            Err(e) => {
                let m = e
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| e.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "bilinmeyen".into());
                log(&format!("hata: {m}"));
                wd.send(raw, &addr);
            }
        }
    }
}

struct WinBackend {
    wd: Option<Arc<Wd>>,
    thread: Option<JoinHandle<()>>,
}

fn flush_dns_cache() {
    // SAFETY: dnsapi.dll standart Windows sistem kütüphanesidir; DnsFlushResolverCache parametresiz bir API'dir.
    unsafe {
        let dll = windows_sys::Win32::System::LibraryLoader::LoadLibraryA(b"dnsapi.dll\0".as_ptr());
        if !dll.is_null() {
            let func = windows_sys::Win32::System::LibraryLoader::GetProcAddress(dll, b"DnsFlushResolverCache\0".as_ptr());
            if let Some(f) = func {
                let f: unsafe extern "system" fn() -> i32 = std::mem::transmute(f);
                f();
            }
            windows_sys::Win32::Foundation::FreeLibrary(dll);
        }
    }
}

impl Backend for WinBackend {
    fn stop(&mut self) {
        let Some(wd) = self.wd.take() else { return };
        wd.shutdown();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        wd.close();
        flush_dns_cache();
    }
}

impl Drop for WinBackend {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn start(cfg: Config, stats: Arc<Stats>, log: LogFn) -> Result<Box<dyn Backend>, EngineError> {
    flush_dns_cache();
    let wd = Arc::new(Wd::open(&filter_str(&cfg))?);
    let engine = Engine::new(cfg, stats, log.clone());
    let w = wd.clone();
    let thread = std::thread::Builder::new()
        .name("gdpi-windivert".into())
        .spawn(move || worker(w, engine, log));
    match thread {
        Ok(t) => Ok(Box::new(WinBackend { wd: Some(wd), thread: Some(t) })),
        Err(e) => {
            wd.shutdown();
            wd.close();
            Err(EngineError::new(-1, format!("iş parçacığı başlatılamadı: {e}")))
        }
    }
}

/// Yönetici olarak mı çalışıyoruz?
pub fn is_admin() -> bool {
    // SAFETY: parametresiz shell32 çağrısı.
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}
