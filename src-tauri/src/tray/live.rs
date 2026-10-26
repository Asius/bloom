//! Live shell notifications. The owner explicitly authorized integration and
//! activation after a controlled fixture test. See TRAY_LIVE.md.
use super::*;
use std::{
    sync::{mpsc, LazyLock, Mutex, OnceLock},
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HANDLE, HINSTANCE, HMODULE, LRESULT, WPARAM},
    System::{DataExchange::COPYDATASTRUCT, LibraryLoader::*},
};

const PROPERTY: PCWSTR = w!("Bloom.Tray.Observer.V1");
const PACKET_TYPE: usize = 0x424c5431;
const DLL: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bloom_tray_observer.dll"));

#[repr(C)]
#[derive(Clone, Copy)]
struct Packet {
    protocol: u32,
    operation: u32,
    window: u32,
    uid: u32,
    flags: u32,
    callback: u32,
    state: u32,
    mask: u32,
    version: u32,
    pid: u32,
    sequence: u32,
    guid: GUID,
    tip: [u16; 128],
    has_image: u32,
    bgra: [u8; 4096],
}
const _: () = assert!(std::mem::size_of::<Packet>() == 4416);

#[derive(Clone)]
pub(super) struct Entry {
    pub token: String,
    pub hwnd: isize,
    pub uid: u32,
    pub guid: GUID,
    pub path: String,
    pub tip: String,
    pub image: Option<String>,
    pub visible: bool,
    callback: Option<u32>,
    version: Option<u32>,
    pid: u32,
    created: u64,
}
impl Entry {
    fn identifier(&self) -> NOTIFYICONIDENTIFIER {
        NOTIFYICONIDENTIFIER {
            cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
            hWnd: HWND(self.hwnd as _),
            uID: self.uid,
            guidItem: self.guid,
        }
    }
    pub fn matches(&self, id: &NOTIFYICONIDENTIFIER) -> bool {
        if id.guidItem != GUID::zeroed() {
            self.guid == id.guidItem
        } else {
            self.hwnd == id.hWnd.0 as isize && self.uid == id.uID
        }
    }
    pub fn actionable(&self) -> bool {
        self.callback.is_some() && self.version.is_some()
    }
    fn owner_alive(&self) -> bool {
        unsafe {
            let mut pid = 0;
            GetWindowThreadProcessId(HWND(self.hwnd as _), Some(&mut pid));
            pid == self.pid && process_created(pid) == Some(self.created)
        }
    }
    pub fn registered(&self) -> bool {
        self.owner_alive() && unsafe { Shell_NotifyIconGetRect(&self.identifier()).is_ok() }
    }
}

#[derive(Default)]
struct Store {
    shell: String,
    next: u64,
    entries: Vec<Entry>,
}
static STORE: LazyLock<Mutex<Store>> = LazyLock::new(|| Mutex::new(Store::default()));
static STARTED: OnceLock<()> = OnceLock::new();
static DROPPED: AtomicBool = AtomicBool::new(false);
static STOP_REQUEST: AtomicBool = AtomicBool::new(false);
static STOPPED: AtomicBool = AtomicBool::new(false);
struct Inbox {
    shell: isize,
    pid: u32,
    tx: mpsc::SyncSender<Packet>,
    sequence: Option<u32>,
}
thread_local! { static INBOX: std::cell::RefCell<Option<Inbox>> = const { std::cell::RefCell::new(None) }; }

unsafe extern "system" fn receiver(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_COPYDATA && l.0 != 0 {
        return INBOX.with(|slot| {
            let mut slot = slot.borrow_mut();
            let Some(inbox) = slot.as_mut() else {
                return LRESULT(0);
            };
            if w.0 as isize != inbox.shell {
                return LRESULT(0);
            }
            let mut pid = 0;
            GetWindowThreadProcessId(HWND(inbox.shell as _), Some(&mut pid));
            if pid != inbox.pid {
                return LRESULT(0);
            }
            // User32 marshals WM_COPYDATA for the call. No pointer escapes it.
            let data = &*(l.0 as *const COPYDATASTRUCT);
            if data.dwData != PACKET_TYPE
                || data.cbData as usize != std::mem::size_of::<Packet>()
                || data.lpData.is_null()
            {
                return LRESULT(0);
            }
            let packet = std::ptr::read_unaligned(data.lpData as *const Packet);
            if packet.protocol != 1 || !matches!(packet.operation, 0 | 1 | 2 | 4) {
                return LRESULT(0);
            }
            if inbox
                .sequence
                .is_some_and(|previous| packet.sequence != previous.wrapping_add(1))
            {
                DROPPED.store(true, Ordering::Relaxed);
            }
            inbox.sequence = Some(packet.sequence);
            if inbox.tx.try_send(packet).is_err() {
                DROPPED.store(true, Ordering::Relaxed);
            }
            LRESULT(1)
        });
    }
    DefWindowProcW(hwnd, msg, w, l)
}

unsafe fn process_created(pid: u32) -> Option<u64> {
    use windows::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::Threading::*,
    };
    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let mut time = [FILETIME::default(); 4];
    let [created, exited, kernel, user] = &mut time;
    let result = GetProcessTimes(handle, created, exited, kernel, user);
    let _ = CloseHandle(handle);
    result.ok()?;
    Some(((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64)
}

fn png(packet: &Packet) -> Option<String> {
    use base64::Engine;
    if packet.has_image != 1 {
        return None;
    }
    let mut rgba = packet.bgra;
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let image = image::RgbaImage::from_raw(32, 32, rgba.to_vec())?;
    let mut output = std::io::Cursor::new(Vec::new());
    image.write_to(&mut output, image::ImageFormat::Png).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(output.into_inner())
    ))
}

fn ingest(store: &mut Store, packet: Packet) -> bool {
    let guid = if packet.flags & NIF_GUID.0 != 0 {
        packet.guid
    } else {
        GUID::zeroed()
    };
    let index = store.entries.iter().position(|entry| {
        if guid != GUID::zeroed() {
            entry.guid == guid
        } else {
            entry.hwnd == packet.window as isize && entry.uid == packet.uid
        }
    });
    if packet.operation == NIM_DELETE.0 {
        if let Some(index) = index {
            store.entries.remove(index);
            return true;
        }
        return false;
    }
    if packet.window == 0 || packet.pid == 0 {
        return false;
    }
    let hwnd = HWND(packet.window as usize as _);
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid != packet.pid {
        return false;
    }
    let Some(created) = (unsafe { process_created(pid) }) else {
        return false;
    };
    let Some(path) = (unsafe { crate::commands::process_image_path(pid) }) else {
        return false;
    };
    let index = if let Some(index) = index {
        index
    } else {
        if store.entries.len() >= 512 {
            return false;
        }
        store.next += 1;
        store.entries.push(Entry {
            token: format!("live:{}:{}", store.shell, store.next),
            hwnd: packet.window as isize,
            uid: packet.uid,
            guid,
            pid,
            created,
            path: path.clone(),
            tip: String::new(),
            image: None,
            callback: None,
            version: None,
            visible: true,
        });
        store.entries.len() - 1
    };
    let entry = &mut store.entries[index];
    if entry.pid != pid || entry.created != created || entry.hwnd != packet.window as isize {
        // GUIDs can survive app restart. Invalidate every old action token.
        store.next += 1;
        entry.token = format!("live:{}:{}", store.shell, store.next);
        entry.callback = None;
        entry.version = None;
        entry.image = None;
    }
    entry.pid = pid;
    entry.created = created;
    entry.path = path;
    entry.hwnd = packet.window as isize;
    entry.uid = packet.uid;
    if guid != GUID::zeroed() {
        entry.guid = guid;
    }
    if packet.operation == NIM_ADD.0 {
        entry.version = Some(0);
        entry.visible = true;
    }
    // The union is uTimeout on ADD/MODIFY, NOT a protocol version.
    if packet.operation == NIM_SETVERSION.0 {
        if packet.version <= 4 {
            entry.version = Some(packet.version);
        }
        return true;
    }
    if packet.flags & NIF_MESSAGE.0 != 0 {
        entry.callback =
            (packet.callback >= WM_USER && packet.callback <= 0xffff).then_some(packet.callback);
    }
    if packet.flags & NIF_STATE.0 != 0 && packet.mask & NIS_HIDDEN.0 != 0 {
        entry.visible = packet.state & NIS_HIDDEN.0 == 0;
    }
    if packet.flags & NIF_TIP.0 != 0 {
        let len = packet
            .tip
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(packet.tip.len());
        entry.tip = String::from_utf16_lossy(&packet.tip[..len]);
    }
    if packet.flags & NIF_ICON.0 != 0 {
        entry.image = png(&packet);
    }
    true
}

pub(super) fn snapshot(shell: &str) -> Vec<Entry> {
    let Ok(mut store) = STORE.lock() else {
        return vec![];
    };
    if store.shell != shell {
        return vec![];
    }
    store.entries.retain(Entry::owner_alive);
    store.entries.clone()
}

struct Observer {
    hook: HHOOK,
    module: HMODULE,
    shell: HWND,
    receiver: HWND,
    _verified_file: std::fs::File,
}
impl Drop for Observer {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.hook);
            if GetPropW(self.shell, PROPERTY).0 == self.receiver.0 {
                let _ = RemovePropW(self.shell, PROPERTY);
            }
            let _ = windows::Win32::Foundation::FreeLibrary(self.module);
        }
    }
}

fn library_path(directory: &std::path::Path) -> Result<std::path::PathBuf, String> {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    DLL.hash(&mut hash);
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let path = directory.join(format!(
        "tray-observer-{}-{:x}.dll",
        env!("CARGO_PKG_VERSION"),
        hash.finish()
    ));
    if !path.exists() {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        file.write_all(DLL).map_err(|e| e.to_string())?;
    }
    // Full byte verification; filename hash is only for side-by-side versions.
    if std::fs::read(&path).map_err(|e| e.to_string())? != DLL {
        return Err("Tray observer does not match this build".into());
    }
    Ok(path)
}

unsafe fn attach(
    path: &std::path::Path,
    shell: ShellSession,
    receiver: HWND,
) -> Result<Observer, String> {
    use std::{io::Read, os::windows::fs::OpenOptionsExt};
    // Deny write/delete sharing through verification, LoadLibrary and unhook.
    let mut verified_file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(path)
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    verified_file
        .by_ref()
        .take(DLL.len() as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes != DLL {
        return Err("Tray observer does not match this build".into());
    }
    let previous = HWND(GetPropW(shell.hwnd, PROPERTY).0);
    if !previous.is_invalid() && IsWindow(Some(previous)).as_bool() {
        return Err("Another tray observer is active".into());
    }
    let wide: Vec<u16> = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let module = LoadLibraryExW(
        PCWSTR(wide.as_ptr()),
        None,
        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
    )
    .map_err(|e| e.to_string())?;
    let Some(proc) = GetProcAddress(module, windows::core::s!("BloomTrayObserver")) else {
        let _ = windows::Win32::Foundation::FreeLibrary(module);
        return Err("Missing tray observer export".into());
    };
    if let Err(error) = SetPropW(shell.hwnd, PROPERTY, Some(HANDLE(receiver.0))) {
        let _ = windows::Win32::Foundation::FreeLibrary(module);
        return Err(error.to_string());
    }
    let tid = GetWindowThreadProcessId(shell.hwnd, None);
    if tid == 0 {
        let _ = RemovePropW(shell.hwnd, PROPERTY);
        let _ = windows::Win32::Foundation::FreeLibrary(module);
        return Err("Explorer thread unavailable".into());
    }
    let hook = SetWindowsHookExW(
        WH_CALLWNDPROC,
        Some(std::mem::transmute::<
            unsafe extern "system" fn() -> isize,
            unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT,
        >(proc)),
        Some(HINSTANCE(module.0)),
        tid,
    );
    match hook {
        Ok(hook) => Ok(Observer {
            hook,
            module,
            shell: shell.hwnd,
            receiver,
            _verified_file: verified_file,
        }),
        Err(error) => {
            let _ = RemovePropW(shell.hwnd, PROPERTY);
            let _ = windows::Win32::Foundation::FreeLibrary(module);
            Err(error.to_string())
        }
    }
}

unsafe fn request_registrations() {
    let message = RegisterWindowMessageW(w!("TaskbarCreated"));
    if message != 0 {
        let _ = SendNotifyMessageW(HWND_BROADCAST, message, WPARAM(0), LPARAM(0));
    }
}

pub(crate) fn start(app: &tauri::AppHandle) {
    use tauri::Manager;
    if STARTED.set(()).is_err() {
        return;
    }
    let Ok(directory) = app.path().app_local_data_dir() else {
        return;
    };
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(error) = run(&directory.join("native"), Some(app), true, None) {
            crate::diagnostics::record("tray-provider", &format!("unavailable: {error}"));
        }
        STOPPED.store(true, Ordering::Release);
    });
}

pub(crate) fn stop() {
    STOP_REQUEST.store(true, Ordering::Release);
    let deadline = Instant::now() + Duration::from_millis(500);
    while STARTED.get().is_some() && !STOPPED.load(Ordering::Acquire) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn run(
    directory: &std::path::Path,
    app: Option<tauri::AppHandle>,
    refresh: bool,
    duration: Option<Duration>,
) -> Result<(), String> {
    use tauri::Emitter;
    let path = library_path(directory)?;
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class = w!("Bloom.Tray.Observer.Receiver.V1");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(receiver),
            hInstance: HINSTANCE(instance.0),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!(""),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(HINSTANCE(instance.0)),
            None,
        )
        .map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::sync_channel(128);
        let mut observer: Option<Observer> = None;
        let mut session: Option<ShellSession> = None;
        let start = Instant::now();
        let mut next_attach = Instant::now();
        let mut last_emit = Instant::now();
        let mut last_session_check = Instant::now();
        let mut dirty = false;
        while !STOP_REQUEST.load(Ordering::Acquire)
            && !crate::state::SHUTTING_DOWN.load(Ordering::Relaxed)
            && duration.is_none_or(|d| start.elapsed() < d)
        {
            if last_session_check.elapsed() >= Duration::from_millis(500) {
                if session.is_some_and(|s| !s.is_current()) {
                    observer.take();
                    session = None;
                    STORE.lock().unwrap().entries.clear();
                    dirty = true;
                }
                last_session_check = Instant::now();
            }
            if observer.is_none() && Instant::now() >= next_attach {
                next_attach = Instant::now() + Duration::from_secs(5);
                if let Ok(shell) = ShellSession::current() {
                    for _ in rx.try_iter() {}
                    INBOX.with(|slot| {
                        *slot.borrow_mut() = Some(Inbox {
                            shell: shell.hwnd.0 as isize,
                            pid: shell.pid,
                            tx: tx.clone(),
                            sequence: None,
                        })
                    });
                    match attach(&path, shell, hwnd) {
                        Ok(value) => {
                            let mut store = STORE.lock().unwrap();
                            store.shell = shell.id();
                            store.entries.clear();
                            drop(store);
                            observer = Some(value);
                            session = Some(shell);
                            crate::diagnostics::record("tray-provider", "observer attached");
                            if refresh {
                                request_registrations();
                            }
                        }
                        Err(error) => crate::diagnostics::record(
                            "tray-provider",
                            &format!("attach failed: {error}"),
                        ),
                    }
                }
            }
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            // Encoding/store work happens outside Explorer and its send.
            for packet in rx.try_iter().take(128) {
                if session.is_some_and(|s| s.is_current()) {
                    dirty |= ingest(&mut STORE.lock().unwrap(), packet);
                }
            }
            if DROPPED.swap(false, Ordering::Relaxed) {
                STORE.lock().unwrap().entries.clear();
                dirty = true;
                crate::diagnostics::record(
                    "tray-provider",
                    "message gap/queue overflow; invalidated live state",
                );
                observer.take();
                session = None;
                next_attach = Instant::now() + Duration::from_secs(5);
            }
            if dirty && last_emit.elapsed() >= Duration::from_millis(250) {
                if let Some(app) = &app {
                    let _ = app.emit("tray-icons-changed", ());
                }
                dirty = false;
                last_emit = Instant::now();
            }
            let _ = MsgWaitForMultipleObjectsEx(None, 100, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        }
        drop(observer);
        STORE.lock().unwrap().entries.clear();
        INBOX.with(|slot| *slot.borrow_mut() = None);
        let _ = DestroyWindow(hwnd);
    }
    Ok(())
}

fn callback_params(version: u32, uid: u32, event: u32, point: POINT) -> (WPARAM, LPARAM) {
    if version >= 4 {
        (
            WPARAM(((point.x as u16 as u32) | ((point.y as u16 as u32) << 16)) as usize),
            LPARAM(((event & 0xffff) | ((uid & 0xffff) << 16)) as isize),
        )
    } else {
        (WPARAM(uid as usize), LPARAM(event as isize))
    }
}

pub(super) fn interact(token: &str, context: bool) -> Option<Result<(), String>> {
    dispatch(token, context, false)
}

pub(super) fn double_click(token: &str) -> Option<Result<(), String>> {
    dispatch(token, false, true)
}

fn dispatch(token: &str, context: bool, double: bool) -> Option<Result<(), String>> {
    if !token.starts_with("live:") {
        return None;
    }
    Some((|| unsafe {
        let shell = ShellSession::current()?;
        let entry = snapshot(&shell.id())
            .into_iter()
            .find(|i| i.token == token)
            .ok_or("This tray icon changed or closed. Open the tray again.")?;
        if !entry.visible || !entry.registered() {
            return Err("This tray icon is no longer available.".into());
        }
        let callback = entry
            .callback
            .ok_or("This icon has not supplied its callback yet.")?;
        let version = entry
            .version
            .ok_or("This icon has not supplied its protocol version yet.")?;
        let mut point = POINT::default();
        GetCursorPos(&mut point).map_err(|e| e.to_string())?;
        let _ = AllowSetForegroundWindow(entry.pid);
        let events = if double {
            [WM_LBUTTONDBLCLK, WM_LBUTTONUP]
        } else if context {
            [WM_RBUTTONDOWN, WM_RBUTTONUP]
        } else {
            [WM_LBUTTONDOWN, WM_LBUTTONUP]
        };
        let before = visible_windows();
        for event in events
            .into_iter()
            .chain((version >= 3 && !double).then_some(if context {
                WM_CONTEXTMENU
            } else {
                NIN_SELECT
            }))
        {
            let (w, l) = callback_params(version, entry.uid, event, point);
            PostMessageW(Some(HWND(entry.hwnd as _)), callback, w, l).map_err(|e| e.to_string())?;
        }
        // Some applications locate a popup through their native icon rectangle
        // instead of the v4 click coordinates. Recover a newly shown popup only
        // when it is offscreen, without holding the tray buttons disabled.
        let path = entry.path;
        let before: Vec<isize> = before.iter().map(|h| h.0 as isize).collect();
        std::thread::spawn(move || {
            let before: Vec<HWND> = before.iter().map(|h| HWND(*h as _)).collect();
            let _ = move_new_menu_to(&path, &before, point);
        });
        Ok(())
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    #[test]
    fn callbacks_preserve_legacy_uid_and_v4_signed_coordinates() {
        let p = POINT { x: -120, y: 1000 };
        let (w, l) = callback_params(0, 0x123456, WM_LBUTTONUP, p);
        assert_eq!(w.0, 0x123456);
        assert_eq!(l.0, WM_LBUTTONUP as isize);
        let (w, l) = callback_params(4, 42, WM_CONTEXTMENU, p);
        assert_eq!(w.0 as u16 as i16, -120);
        assert_eq!((w.0 >> 16) as u16, 1000);
        assert_eq!((l.0 as u32) >> 16, 42);
        assert_eq!((l.0 as u32) & 0xffff, WM_CONTEXTMENU);
    }

    static SELECTED: AtomicU64 = AtomicU64::new(0);
    static CONTEXT: AtomicU64 = AtomicU64::new(0);
    static DOUBLE: AtomicU64 = AtomicU64::new(0);
    const CALLBACK: u32 = WM_APP + 71;
    unsafe extern "system" fn fixture_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if msg == CALLBACK {
            match l.0 as u32 & 0xffff {
                NIN_SELECT => {
                    SELECTED.fetch_add(1, Ordering::Relaxed);
                }
                WM_CONTEXTMENU => {
                    CONTEXT.fetch_add(1, Ordering::Relaxed);
                }
                WM_LBUTTONDBLCLK => {
                    DOUBLE.fetch_add(1, Ordering::Relaxed);
                }
                _ => {}
            }
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, msg, w, l)
    }
    struct Fixture {
        hwnd: HWND,
        icon: NOTIFYICONDATAW,
    }
    impl Fixture {
        unsafe fn new() -> Self {
            let instance = GetModuleHandleW(None).unwrap();
            let class = w!("Bloom.Tray.OwnedFixture.V1");
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(fixture_proc),
                hInstance: HINSTANCE(instance.0),
                lpszClassName: class,
                ..Default::default()
            });
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                w!(""),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(HINSTANCE(instance.0)),
                None,
            )
            .unwrap();
            let mut icon = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 42,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_GUID,
                uCallbackMessage: CALLBACK,
                guidItem: CoCreateGuid().unwrap(),
                hIcon: LoadIconW(None, IDI_INFORMATION).unwrap(),
                ..Default::default()
            };
            let tip: Vec<u16> = "Bloom controlled tray fixture".encode_utf16().collect();
            icon.szTip[..tip.len()].copy_from_slice(&tip);
            Self { hwnd, icon }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            unsafe {
                let _ = Shell_NotifyIconW(NIM_DELETE, &self.icon);
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
    fn pump_until(mut check: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            unsafe {
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            if check() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn state_masks_version_union_and_recycled_owner_tokens() {
        unsafe {
            let fixture = Fixture::new();
            let mut packet: Packet = std::mem::zeroed();
            packet.window = fixture.hwnd.0 as usize as u32;
            packet.pid = std::process::id();
            packet.operation = NIM_ADD.0;
            packet.uid = 42;
            packet.flags = NIF_MESSAGE.0;
            packet.callback = CALLBACK;
            packet.version = 30000; // ADD's timeout must not become version.
            let mut store = Store::default();
            assert!(ingest(&mut store, packet));
            assert_eq!(store.entries[0].version, Some(0));
            packet.operation = NIM_SETVERSION.0;
            packet.version = 4;
            ingest(&mut store, packet);
            assert_eq!(store.entries[0].version, Some(4));
            packet.operation = NIM_MODIFY.0;
            packet.flags = NIF_STATE.0;
            packet.state = NIS_HIDDEN.0;
            packet.mask = 0;
            ingest(&mut store, packet);
            assert!(store.entries[0].visible);
            packet.mask = NIS_HIDDEN.0;
            ingest(&mut store, packet);
            assert!(!store.entries[0].visible);
            let token = store.entries[0].token.clone();
            store.entries[0].created = 0; // recycled process identity
            ingest(&mut store, packet);
            assert_ne!(store.entries[0].token, token);
            assert!(!store.entries[0].actionable());
            packet.operation = NIM_DELETE.0;
            ingest(&mut store, packet);
            assert!(store.entries.is_empty());
        }
    }

    #[test]
    #[ignore = "Attaches reviewed observer to Explorer; creates only a test-owned icon; requires explicit owner authorization"]
    fn native_observer_fixture_lifecycle() {
        let directory = std::env::current_dir()
            .unwrap()
            .join("target/tray-observer-fixture");
        let worker =
            std::thread::spawn(move || run(&directory, None, false, Some(Duration::from_secs(18))));
        let outcome = std::panic::catch_unwind(|| unsafe {
            let shell = ShellSession::current().unwrap();
            assert!(
                pump_until(|| !GetPropW(shell.hwnd, PROPERTY).is_invalid()),
                "observer did not attach"
            );
            let mut fixture = Fixture::new();
            assert!(Shell_NotifyIconW(NIM_ADD, &fixture.icon).as_bool());
            let get = || {
                snapshot(&shell.id())
                    .into_iter()
                    .find(|e| e.hwnd == fixture.hwnd.0 as isize && e.uid == 42)
            };
            assert!(
                pump_until(|| get().is_some_and(|e| e.image.is_some() && e.actionable())),
                "ADD/image/callback not observed"
            );
            let original = get().unwrap();
            assert_eq!(original.version, Some(0));
            fixture.icon.Anonymous.uVersion = 4;
            assert!(Shell_NotifyIconW(NIM_SETVERSION, &fixture.icon).as_bool());
            assert!(
                pump_until(|| get().is_some_and(|e| e.version == Some(4))),
                "version not observed"
            );
            fixture.icon.hIcon = LoadIconW(None, IDI_WARNING).unwrap();
            fixture.icon.uFlags = NIF_ICON | NIF_GUID;
            assert!(Shell_NotifyIconW(NIM_MODIFY, &fixture.icon).as_bool());
            assert!(
                pump_until(|| get().is_some_and(|e| e.image.is_some() && e.image != original.image)),
                "live image did not change"
            );
            let selected = SELECTED.load(Ordering::Relaxed);
            interact(&original.token, false).unwrap().unwrap();
            assert!(
                pump_until(|| SELECTED.load(Ordering::Relaxed) > selected),
                "left click did not reach fixture"
            );
            let context = CONTEXT.load(Ordering::Relaxed);
            interact(&original.token, true).unwrap().unwrap();
            assert!(
                pump_until(|| CONTEXT.load(Ordering::Relaxed) > context),
                "context callback did not reach fixture"
            );
            let double = DOUBLE.load(Ordering::Relaxed);
            double_click(&original.token).unwrap().unwrap();
            assert!(
                pump_until(|| DOUBLE.load(Ordering::Relaxed) > double),
                "double click did not reach fixture"
            );
            assert!(Shell_NotifyIconW(NIM_DELETE, &fixture.icon).as_bool());
            assert!(pump_until(|| get().is_none()), "DELETE not observed");
            assert!(
                interact(&original.token, false).unwrap().is_err(),
                "stale click accepted"
            );
            println!("native fixture: add, live image modify, version 4, left/double/context callbacks, delete and stale-token rejection passed");
        });
        worker.join().unwrap().unwrap();
        unsafe {
            let shell = ShellSession::current().unwrap();
            assert!(
                GetPropW(shell.hwnd, PROPERTY).is_invalid(),
                "observer property leaked"
            );
        }
        if let Err(panic) = outcome {
            std::panic::resume_unwind(panic);
        }
    }

    #[test]
    #[ignore = "Authorized startup-refresh probe; reads current icons without invoking any application"]
    fn native_observer_initial_inventory() {
        let directory = std::env::current_dir()
            .unwrap()
            .join("target/tray-observer-fixture");
        let worker =
            std::thread::spawn(move || run(&directory, None, true, Some(Duration::from_secs(8))));
        std::thread::sleep(Duration::from_secs(5));
        let result = catalog();
        worker.join().unwrap().unwrap();
        let snapshot = result.unwrap();
        let live_images = snapshot
            .icons
            .iter()
            .filter(|icon| icon.icon_source == "live")
            .count();
        let direct_actions = snapshot
            .icons
            .iter()
            .filter(|icon| icon.id.starts_with("live:"))
            .count();
        println!(
            "startup inventory: total={} live_images={} direct_actions={} cached_images={}",
            snapshot.icons.len(),
            live_images,
            direct_actions,
            snapshot
                .icons
                .iter()
                .filter(|i| i.icon_source == "cached")
                .count()
        );
        assert!(live_images > 0, "startup did not receive live images");
        assert!(direct_actions > 0, "startup did not receive callbacks");
        unsafe {
            assert!(GetPropW(ShellSession::current().unwrap().hwnd, PROPERTY).is_invalid());
        }
    }
}
