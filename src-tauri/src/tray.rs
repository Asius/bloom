//! Read-only discovery of live notification icons. Registry entries are only
//! candidates: Shell_NotifyIconGetRect must confirm each icon before it is used.
mod model;
pub(crate) mod surface;

use model::{icon_identity, normalized_label, OperationGuard, Restoration};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU64, Ordering};
use windows::core::{w, Interface, BOOL, GUID, PCWSTR};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT, RECT},
    System::{Com::*, Registry::*},
    UI::{Accessibility::*, Shell::*, WindowsAndMessaging::*},
};

#[derive(Clone, Serialize)]
pub struct TrayApp {
    pub name: String,
    pub path: String,
    pub tray_ids: Vec<String>,
    pub window_handles: Vec<isize>,
}

/// One notification icon, independent of window history and dock grouping.
/// `id` is a live action token, not a value to persist in settings.
#[derive(Clone, Serialize)]
pub struct TrayIcon {
    pub id: String,
    pub name: String,
    pub path: String,
    pub is_promoted: bool,
    pub is_shell_icon: bool,
    /// Windows' cached artwork, not a guaranteed live status indicator.
    pub icon: Option<String>,
}

#[derive(Serialize)]
pub struct TraySnapshot {
    pub shell_id: String,
    pub icons: Vec<TrayIcon>,
}

// Native pixels plus their original DPI. Refreshed before the taskbar handoff,
// not inferred from the width or scaling of Bloom's own icons.
static NATIVE_INSET: AtomicU64 = AtomicU64::new(0);

pub(crate) fn native_right_inset(scale: f64, monitor_width: u32) -> u32 {
    let stored = NATIVE_INSET.load(Ordering::Relaxed);
    let dpi = (stored >> 32) as u32;
    let inset = stored as u32;
    let logical = if dpi > 0 {
        inset as f64 * 96.0 / dpi as f64
    } else {
        120.0
    };
    (logical * scale).round().clamp(
        0.0,
        monitor_width.saturating_sub(1).min(monitor_width / 3) as f64,
    ) as u32
}

pub(crate) fn refresh_native_inset(taskbar: HWND) {
    unsafe {
        let mut rect = RECT::default();
        if !IsWindowVisible(taskbar).as_bool() || GetWindowRect(taskbar, &mut rect).is_err() {
            return;
        }
        let Ok(icons) = live_icons(&process_windows(), true) else {
            return;
        };
        let bounds: Vec<_> = icons
            .iter()
            .filter(|i| i.promoted)
            .filter_map(|i| Shell_NotifyIconGetRect(&i.identifier).ok())
            .collect();
        if let Some(inset) = model::reserved_right(&rect, &bounds) {
            let dpi = GetDpiForWindow(taskbar).max(96);
            let value = ((dpi as u64) << 32) | inset as u64;
            if NATIVE_INSET.swap(value, Ordering::Relaxed) != value {
                crate::diagnostics::record(
                    "tray-position",
                    &format!("native_right_inset={inset} dpi={dpi}"),
                );
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ShellSession {
    hwnd: HWND,
    pid: u32,
    created: u64,
}

impl ShellSession {
    unsafe fn current() -> Result<Self, String> {
        use windows::Win32::Foundation::{CloseHandle, FILETIME};
        use windows::Win32::System::Threading::{
            GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        let hwnd = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null())
            .map_err(|_| "Windows taskbar is unavailable. Please try again.")?;
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|_| "Windows Explorer cannot be queried.")?;
        let mut times = [FILETIME::default(); 4];
        let [created, exited, kernel, user] = &mut times;
        let result = GetProcessTimes(process, created, exited, kernel, user);
        let _ = CloseHandle(process);
        result.map_err(|_| "Windows Explorer's session cannot be queried.")?;
        Ok(Self {
            hwnd,
            pid,
            created: ((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64,
        })
    }

    fn id(&self) -> String {
        format!("{}:{}:{}", self.pid, self.hwnd.0 as isize, self.created)
    }

    unsafe fn is_current(&self) -> bool {
        Self::current().is_ok_and(|current| current == *self)
    }
}

static OPERATION_BUSY: AtomicBool = AtomicBool::new(false);

fn begin_operation() -> Result<OperationGuard<'static>, String> {
    // A click outside a native popup may reach Bloom just before its close is
    // observed. Allow that observation to finish, without overlapping actions.
    for _ in 0..20 {
        if let Ok(guard) = OperationGuard::begin(&OPERATION_BUSY) {
            return Ok(guard);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    OperationGuard::begin(&OPERATION_BUSY)
}

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

unsafe fn open_key(parent: HKEY, name: &str) -> Option<Key> {
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let mut key = HKEY::default();
    RegOpenKeyExW(parent, PCWSTR(name.as_ptr()), Some(0), KEY_READ, &mut key)
        .ok()
        .ok()?;
    Some(Key(key))
}

unsafe fn value(key: HKEY, name: &str, expected: REG_VALUE_TYPE) -> Option<Vec<u8>> {
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let mut kind = REG_VALUE_TYPE::default();
    let mut size = 0;
    RegQueryValueExW(
        key,
        PCWSTR(name.as_ptr()),
        None,
        Some(&mut kind),
        None,
        Some(&mut size),
    )
    .ok()
    .ok()?;
    if kind != expected || size > 65536 {
        return None;
    }
    let mut bytes = vec![0; size as usize];
    RegQueryValueExW(
        key,
        PCWSTR(name.as_ptr()),
        None,
        Some(&mut kind),
        Some(bytes.as_mut_ptr()),
        Some(&mut size),
    )
    .ok()
    .ok()?;
    if kind != expected {
        return None;
    }
    bytes.truncate(size as usize);
    Some(bytes)
}

unsafe fn string(key: HKEY, name: &str) -> Option<String> {
    let bytes = value(key, name, REG_SZ)?;
    let chars: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .take_while(|c| *c != 0)
        .collect();
    String::from_utf16(&chars).ok()
}

fn parse_guid(text: &str) -> Option<GUID> {
    GUID::try_from(text.trim_start_matches('{').trim_end_matches('}')).ok()
}

/// Resolves a shell path that starts with a known-folder GUID
/// (`{7C5A40EF-…}\Steam\steam.exe`) to a file-system path. The notification
/// area settings and the AppsFolder parsing names of classic programs use
/// this form; other paths are returned unchanged.
pub(crate) unsafe fn resolve_path(path: &str) -> Option<String> {
    if !path.starts_with('{') {
        return Some(path.to_owned());
    }
    let end = path.find('}')?;
    let id = parse_guid(&path[..=end])?;
    let folder = SHGetKnownFolderPath(&id, KF_FLAG_DEFAULT, None).ok()?;
    let base = folder.to_string().ok();
    CoTaskMemFree(Some(folder.0 as _));
    Some(format!("{}{}", base?, &path[end + 1..]))
}

struct Icon {
    key: String,
    path: String,
    identifier: NOTIFYICONIDENTIFIER,
    promoted: bool,
    label_hint: Option<String>,
    artwork: Option<String>,
}

impl Icon {
    fn action_id(&self, shell: &ShellSession) -> String {
        let guid = if self.identifier.guidItem == GUID::zeroed() {
            String::new()
        } else {
            format!("{:?}", self.identifier.guidItem)
        };
        icon_identity(
            &shell.id(),
            &self.key,
            &self.path,
            &guid,
            self.identifier.hWnd.0 as isize,
            self.identifier.uID,
        )
    }
}

// Explorer owns system notification icons such as Bluetooth. They share
// explorer.exe with File Explorer windows but are not File Explorer app actions.
fn is_explorer_hosted_icon(path: &str) -> bool {
    std::path::Path::new(path)
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("explorer.exe"))
}

/// The overflow chevron ("Show hidden icons"). Its name is localized, so it is
/// identified by AutomationId and class: the other system buttons sharing the
/// `SystemTrayIcon` id (network, volume, clock) use different classes.
fn is_overflow_chevron(automation_id: &str, class_name: &str) -> bool {
    automation_id == "SystemTrayIcon" && class_name == "SystemTray.NormalButton"
}

/// Buttons in the taskbar's notification area that can belong to an app.
/// Windows 11 26H2 exposes app icons as `NotifyItemIcon`; older builds used
/// `SystemTrayIcon` for every button. The caller still requires exactly one
/// button at the icon's own rectangle.
fn is_tray_button_id(automation_id: &str) -> bool {
    automation_id == "NotifyItemIcon" || automation_id == "SystemTrayIcon"
}

unsafe extern "system" fn collect_window(hwnd: HWND, param: LPARAM) -> windows::core::BOOL {
    let windows = &mut *(param.0 as *mut Vec<(isize, String)>);
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if let Some(path) = crate::commands::process_image_path(pid) {
        windows.push((hwnd.0 as isize, path));
    }
    true.into()
}

unsafe fn process_windows() -> Vec<(isize, String)> {
    let mut windows = Vec::new();
    let param = LPARAM(&mut windows as *mut Vec<(isize, String)> as isize);
    let _ = EnumWindows(Some(collect_window), param);
    // Some apps own their notification icon through a message-only window.
    let mut previous = None;
    while let Ok(hwnd) = FindWindowExW(Some(HWND_MESSAGE), previous, PCWSTR::null(), PCWSTR::null())
    {
        let _ = collect_window(hwnd, param);
        previous = Some(hwnd);
    }
    windows
}

unsafe fn live_icons(windows: &[(isize, String)], include_own: bool) -> Result<Vec<Icon>, String> {
    let root = open_key(HKEY_CURRENT_USER, "Control Panel\\NotifyIconSettings")
        .ok_or("Windows notification-icon discovery is unavailable on this system.")?;
    let own_path = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_lowercase());
    let mut result = Vec::new();
    for index in 0..4096 {
        let mut name = [0u16; 256];
        let mut length = name.len() as u32;
        if RegEnumKeyExW(
            root.0,
            index,
            Some(windows::core::PWSTR(name.as_mut_ptr())),
            &mut length,
            None,
            None,
            None,
            None,
        )
        .is_err()
        {
            break;
        }
        let key_name = String::from_utf16_lossy(&name[..length as usize]);
        let Some(key) = open_key(root.0, &key_name) else {
            continue;
        };
        let Some(path) = string(key.0, "ExecutablePath").and_then(|p| resolve_path(&p)) else {
            continue;
        };
        if !include_own && own_path.as_deref() == Some(&path.to_lowercase()) {
            continue;
        }
        let mut id = NOTIFYICONIDENTIFIER {
            cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
            ..Default::default()
        };
        if let Some(guid) = string(key.0, "IconGuid").and_then(|s| parse_guid(&s)) {
            id.guidItem = guid;
            if !windows
                .iter()
                .any(|(_, exe)| exe.eq_ignore_ascii_case(&path))
                || Shell_NotifyIconGetRect(&id).is_err()
            {
                continue;
            }
        } else if let Some(uid) = value(key.0, "UID", REG_DWORD)
            .and_then(|v| <[u8; 4]>::try_from(v).ok())
            .map(u32::from_le_bytes)
        {
            id.uID = uid;
            let owner = windows.iter().find(|(hwnd, exe)| {
                if !exe.eq_ignore_ascii_case(&path) {
                    return false;
                }
                id.hWnd = HWND(*hwnd as _);
                Shell_NotifyIconGetRect(&id).is_ok()
            });
            if owner.is_none() {
                continue;
            }
        } else {
            continue;
        }
        result.push(Icon {
            key: key_name,
            path,
            identifier: id,
            label_hint: string(key.0, "InitialTooltip"),
            artwork: value(key.0, "IconSnapshot", REG_BINARY)
                .and_then(|bytes| model::snapshot_png(&bytes)),
            promoted: value(key.0, "IsPromoted", REG_DWORD)
                .and_then(|v| <[u8; 4]>::try_from(v).ok())
                .map(u32::from_le_bytes)
                == Some(1),
        });
    }
    Ok(result)
}

pub fn catalog() -> Result<TraySnapshot, String> {
    unsafe {
        let shell = ShellSession::current()?;
        let windows = process_windows();
        let mut names = HashMap::new();
        let order = tray_order().unwrap_or_default();
        let mut live = live_icons(&windows, true)?;
        live.sort_by(|a, b| {
            order
                .get(&a.key)
                .copied()
                .unwrap_or(usize::MAX)
                .cmp(&order.get(&b.key).copied().unwrap_or(usize::MAX))
                .then(a.key.cmp(&b.key))
        });
        let icons: Vec<_> = live
            .into_iter()
            .map(|icon| {
                let name = names
                    .entry(icon.path.to_lowercase())
                    .or_insert_with(|| crate::commands::app_display_name(&icon.path, None))
                    .clone();
                TrayIcon {
                    id: icon.action_id(&shell),
                    name,
                    is_shell_icon: is_explorer_hosted_icon(&icon.path),
                    path: icon.path,
                    is_promoted: icon.promoted,
                    icon: icon.artwork,
                }
            })
            .collect();
        if !shell.is_current() {
            return Err(
                "Windows Explorer restarted while reading the tray. Please try again.".into(),
            );
        }
        Ok(TraySnapshot {
            shell_id: shell.id(),
            icons,
        })
    }
}

pub fn enumerate() -> Vec<TrayApp> {
    unsafe {
        let Ok(shell) = ShellSession::current() else {
            return vec![];
        };
        let windows = process_windows();
        let mut apps: HashMap<String, TrayApp> = HashMap::new();
        // Explorer-hosted system icons still occupy overflow positions, so
        // live_icons keeps them for interact(); they just aren't app trays.
        for icon in live_icons(&windows, false)
            .unwrap_or_default()
            .into_iter()
            .filter(|icon| !is_explorer_hosted_icon(&icon.path))
        {
            let action_id = icon.action_id(&shell);
            let app = apps
                .entry(icon.path.to_lowercase())
                .or_insert_with(|| TrayApp {
                    name: crate::commands::app_display_name(&icon.path, None),
                    window_handles: windows
                        .iter()
                        .filter(|(_, p)| p.eq_ignore_ascii_case(&icon.path))
                        .map(|(h, _)| *h)
                        .collect(),
                    path: icon.path,
                    tray_ids: vec![],
                });
            app.tray_ids.push(action_id);
        }
        if !shell.is_current() {
            return vec![];
        }
        let mut apps: Vec<_> = apps.into_values().collect();
        apps.sort_by(|a, b| a.path.cmp(&b.path));
        apps
    }
}

struct ComApartment;
impl ComApartment {
    unsafe fn new() -> Result<Self, String> {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|e| e.to_string())?;
        Ok(Self)
    }
}
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

unsafe fn automation_client() -> Result<IUIAutomation, String> {
    let automation: IUIAutomation =
        CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?;
    let options = automation
        .cast::<IUIAutomation2>()
        .map_err(|e| e.to_string())?;
    options
        .SetConnectionTimeout(1000)
        .map_err(|e| e.to_string())?;
    options
        .SetTransactionTimeout(2000)
        .map_err(|e| e.to_string())?;
    Ok(automation)
}

/// Restore every temporary Explorer change, including on an early error. No
/// AppBar state or user icon-promotion preferences are changed for this operation.
struct TrayExposure {
    session: ShellSession,
    taskbar: HWND,
    rect: RECT,
    style: i32,
    was_hidden: bool,
    layered: Option<(
        windows::Win32::Foundation::COLORREF,
        u8,
        LAYERED_WINDOW_ATTRIBUTES_FLAGS,
    )>,
    overflow: Option<HWND>,
}

impl TrayExposure {
    unsafe fn new() -> Result<Self, String> {
        use std::sync::atomic::Ordering;
        if crate::state::SHUTTING_DOWN.load(Ordering::Relaxed) {
            return Err("Bloom is shutting down.".into());
        }
        let session = ShellSession::current()?;
        let hwnd = session.hwnd;
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect).map_err(|e| e.to_string())?;
        let style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        let layered = if style & WS_EX_LAYERED.0 as i32 != 0 {
            let mut color = windows::Win32::Foundation::COLORREF(0);
            let mut alpha = 255;
            let mut flags = LAYERED_WINDOW_ATTRIBUTES_FLAGS::default();
            GetLayeredWindowAttributes(hwnd, Some(&mut color), Some(&mut alpha), Some(&mut flags))
                .map_err(|_| "Windows taskbar transparency cannot be preserved.")?;
            Some((color, alpha, flags))
        } else {
            None
        };
        if crate::state::TRAY_INTERACTION_ACTIVE.swap(true, Ordering::SeqCst) {
            return Err("Another tray menu is already opening.".into());
        }
        let guard = Self {
            session,
            taskbar: hwnd,
            rect,
            style,
            was_hidden: crate::state::NATIVE_TASKBAR_HIDDEN.load(Ordering::Relaxed),
            overflow: None,
            layered,
        };
        if guard.was_hidden {
            SetWindowLongW(
                hwnd,
                GWL_EXSTYLE,
                guard.style | WS_EX_LAYERED.0 as i32 | WS_EX_TRANSPARENT.0 as i32,
            );
            // A fully transparent XAML island drops its UI Automation tree.
            // One alpha level is visually imperceptible but keeps it accessible.
            SetLayeredWindowAttributes(hwnd, windows::Win32::Foundation::COLORREF(0), 1, LWA_ALPHA)
                .map_err(|e| e.to_string())?;
            if let Some(rect) = crate::utils::ORIGINAL_TRAY_RECT
                .lock()
                .ok()
                .and_then(|r| *r)
            {
                SetWindowPos(
                    hwnd,
                    None,
                    rect.left,
                    rect.top,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
                )
                .map_err(|e| e.to_string())?;
            }
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        Ok(guard)
    }

    unsafe fn expose_overflow(
        &mut self,
        automation: &IUIAutomation,
    ) -> Result<Option<HWND>, String> {
        let find_overflow = || {
            FindWindowW(w!("TopLevelWindowForOverflowXamlIsland"), PCWSTR::null())
                .or_else(|_| FindWindowW(w!("NotifyIconOverflowWindow"), PCWSTR::null()))
        };
        let has_icons = |hwnd| -> bool {
            let bridge = FindWindowExW(
                Some(hwnd),
                None,
                w!("Windows.UI.Composition.DesktopWindowContentBridge"),
                PCWSTR::null(),
            )
            .or_else(|_| FindWindowExW(Some(hwnd), None, w!("ToolbarWindow32"), PCWSTR::null()))
            .unwrap_or(hwnd);
            let Ok(root) = automation.ElementFromHandle(bridge) else {
                return false;
            };
            let Ok(condition) = automation.CreateTrueCondition() else {
                return false;
            };
            let Ok(elements) = root.FindAll(TreeScope_Descendants, &condition) else {
                return false;
            };
            (0..elements.Length().unwrap_or(0)).any(|index| {
                elements.GetElement(index).is_ok_and(|element| {
                    element
                        .CurrentAutomationId()
                        .is_ok_and(|id| is_tray_button_id(&id.to_string()))
                        && element.CurrentControlType().ok() == Some(UIA_ButtonControlTypeId)
                        && element
                            .CurrentIsOffscreen()
                            .is_ok_and(|offscreen| !offscreen.as_bool())
                })
            })
        };
        // Windows 11's tray controls live in a child XAML bridge, not in the
        // Shell_TrayWnd automation subtree itself. Invoking the chevron also
        // populates the overflow; ShowWindow on its host alone does not.
        let bridge = FindWindowExW(
            Some(self.taskbar),
            None,
            w!("Windows.UI.Composition.DesktopWindowContentBridge"),
            PCWSTR::null(),
        )
        .unwrap_or(self.taskbar);
        let root = automation
            .ElementFromHandle(bridge)
            .map_err(|e| e.to_string())?;
        let condition = automation
            .CreateTrueCondition()
            .map_err(|e| e.to_string())?;
        let elements = root
            .FindAll(TreeScope_Descendants, &condition)
            .map_err(|e| e.to_string())?;
        // Only invoke the known overflow chevron. Win+B/Space can accidentally
        // activate another app when all notification icons are promoted.
        // The chevron is identified by AutomationId and class: its name is
        // localized ("Show Hidden Icons", "Ausgeblendete Symbole einblenden").
        let mut chevrons = Vec::new();
        for index in 0..elements.Length().unwrap_or(0) {
            let Ok(element) = elements.GetElement(index) else {
                continue;
            };
            let is_chevron = is_overflow_chevron(
                &element
                    .CurrentAutomationId()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                &element
                    .CurrentClassName()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
            );
            if is_chevron
                && element.CurrentControlType().ok() == Some(UIA_ButtonControlTypeId)
                && element
                    .CurrentIsEnabled()
                    .is_ok_and(|enabled| enabled.as_bool())
                && element
                    .CurrentIsOffscreen()
                    .is_ok_and(|offscreen| !offscreen.as_bool())
            {
                chevrons.push(element);
            }
        }
        if chevrons.is_empty() {
            return Ok(None);
        }
        if chevrons.len() != 1 {
            return Err("Windows did not identify a unique hidden-icons button.".into());
        }
        // Explorer can retain a hidden overflow host with cached children after
        // all icons are promoted. Never expose that host without a live chevron.
        if let Ok(hwnd) = find_overflow() {
            if !IsWindowVisible(hwnd).as_bool() {
                self.overflow = Some(hwnd);
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
            if IsWindowVisible(hwnd).as_bool() && has_icons(hwnd) {
                return Ok(Some(hwnd));
            }
        }
        let pattern = chevrons
            .remove(0)
            .GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
            .map_err(|e| e.to_string())?;
        // Use visible host/content state in every language. Explorer can
        // retain a stale open state after its host has been hidden.
        for _attempt in 0..2 {
            if !self.session.is_current() {
                return Err("Windows Explorer restarted. Please try again.".into());
            }
            pattern.Invoke().map_err(|e| e.to_string())?;
            for _ in 0..20 {
                if let Ok(hwnd) = find_overflow() {
                    self.overflow = Some(hwnd);
                    if IsWindowVisible(hwnd).as_bool() && has_icons(hwnd) {
                        return Ok(Some(hwnd));
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        }
        Err(
            "Windows could not expose this app's tray icon. Open the system tray and try again."
                .into(),
        )
    }
}

impl Drop for TrayExposure {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering;
        unsafe {
            let same_shell = self.session.is_current();
            if same_shell {
                if let Some(hwnd) = self.overflow {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
            // Exit/settings cleanup owns the final visible state if it changed
            // while UIA was working; never undo a taskbar restoration.
            let restore = model::restoration(
                self.was_hidden,
                crate::state::NATIVE_TASKBAR_HIDDEN.load(Ordering::Relaxed),
                crate::utils::dock_enabled(),
                crate::state::SHUTTING_DOWN.load(Ordering::Relaxed),
                same_shell,
            );
            if restore == Restoration::HideOriginal {
                let _ = ShowWindow(self.taskbar, SW_HIDE);
                if let Some((color, alpha, flags)) = self.layered {
                    let _ = SetLayeredWindowAttributes(self.taskbar, color, alpha, flags);
                }
                SetWindowLongW(self.taskbar, GWL_EXSTYLE, self.style);
                let _ = SetWindowPos(
                    self.taskbar,
                    None,
                    self.rect.left,
                    self.rect.top,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
                );
            }
            crate::state::TRAY_INTERACTION_ACTIVE.store(false, Ordering::SeqCst);
            match restore {
                Restoration::ShowCurrent => {
                    crate::utils::set_taskbar_visibility(true, true);
                }
                Restoration::HideCurrent => {
                    crate::utils::set_taskbar_visibility(false, false);
                }
                _ => {}
            }
        }
    }
}

unsafe fn tray_order() -> Result<HashMap<String, usize>, String> {
    let root = open_key(HKEY_CURRENT_USER, "Control Panel\\NotifyIconSettings")
        .ok_or("Windows tray settings are unavailable.")?;
    let bytes =
        value(root.0, "UIOrderList", REG_BINARY).ok_or("Windows tray order is unavailable.")?;
    model::parse_order(&bytes)
}

/// Shell_NotifyIconGetRect points at the chevron for hidden icons. The live
/// registry order and the overflow's UI Automation button order coincide;
/// require equal counts and reject any contradictory name matches. An icon's
/// tooltip is app-defined and often differs from its executable description
/// (for example PhoneExperienceHost.exe displays "Phone Link - Disconnected"),
/// so a missing name match is not by itself an identity failure.
unsafe fn find_overflow_icon(
    automation: &IUIAutomation,
    icons: &[Icon],
    target: &Icon,
    session: &ShellSession,
) -> Result<IUIAutomationElement3, String> {
    let order = tray_order().unwrap_or_default();
    let mut hidden: Vec<_> = icons.iter().filter(|icon| !icon.promoted).collect();
    hidden.sort_by_key(|icon| order.get(&icon.key).copied().unwrap_or(usize::MAX));
    let position = hidden
        .iter()
        .position(|icon| icon.key == target.key)
        .ok_or("This app's tray icon is not in the overflow.")?;
    let hwnd = FindWindowW(w!("TopLevelWindowForOverflowXamlIsland"), PCWSTR::null())
        .or_else(|_| FindWindowW(w!("NotifyIconOverflowWindow"), PCWSTR::null()))
        .map_err(|_| "Windows tray overflow is unavailable.")?;
    let bridge = FindWindowExW(
        Some(hwnd),
        None,
        w!("Windows.UI.Composition.DesktopWindowContentBridge"),
        PCWSTR::null(),
    )
    .or_else(|_| FindWindowExW(Some(hwnd), None, w!("ToolbarWindow32"), PCWSTR::null()))
    .unwrap_or(hwnd);
    let root = automation
        .ElementFromHandle(bridge)
        .map_err(|e| e.to_string())?;
    // Once overflow is open, some Windows builds expose each hidden icon's
    // actual rectangle. Prefer its live GUID/HWND position over registry order.
    let target_rect = Shell_NotifyIconGetRect(&target.identifier).ok();
    let mut overflow_rect = RECT::default();
    if let Some(rect) = target_rect.filter(|r| {
        GetWindowRect(hwnd, &mut overflow_rect).is_ok() && model::icon_inside(r, &overflow_rect)
    }) {
        let collision = hidden.iter().any(|other| {
            let same = if target.identifier.guidItem != GUID::zeroed() {
                other.identifier.guidItem == target.identifier.guidItem
            } else {
                other.identifier.hWnd == target.identifier.hWnd
                    && other.identifier.uID == target.identifier.uID
            };
            !same && Shell_NotifyIconGetRect(&other.identifier).is_ok_and(|r| r == rect)
        });
        if !collision {
            if let Some(button) = element_at_icon(automation, &root, rect)? {
                if session.is_current()
                    && Shell_NotifyIconGetRect(&target.identifier).ok() == Some(rect)
                {
                    crate::diagnostics::record("tray-selection", "method=live-rectangle");
                    return button.cast().map_err(|e| e.to_string());
                }
            }
        }
    }
    if hidden.iter().any(|icon| !order.contains_key(&icon.key)) {
        return Err("Windows tray order changed. Please try again.".into());
    }
    let condition = automation
        .CreateTrueCondition()
        .map_err(|e| e.to_string())?;
    let elements = root
        .FindAll(TreeScope_Descendants, &condition)
        .map_err(|e| e.to_string())?;
    let mut buttons = Vec::new();
    for index in 0..elements.Length().unwrap_or(0) {
        let Ok(element) = elements.GetElement(index) else {
            continue;
        };
        if element
            .CurrentAutomationId()
            .is_ok_and(|id| is_tray_button_id(&id.to_string()))
            && element.CurrentControlType().ok() == Some(UIA_ButtonControlTypeId)
            && element.CurrentIsOffscreen().is_ok_and(|off| !off.as_bool())
        {
            if !buttons.iter().any(|other| {
                automation
                    .CompareElements(other, &element)
                    .is_ok_and(|same| same.as_bool())
            }) {
                buttons.push(element);
            }
        }
    }
    let labels = buttons
        .iter()
        .map(|button| button.CurrentName().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    let candidates = hidden
        .iter()
        .map(|icon| model::IconLabel {
            path: icon.path.clone(),
            hint: icon.label_hint.clone(),
            names: vec![
                normalized_label(&crate::commands::friendly_process_name(&icon.path)),
                std::path::Path::new(&icon.path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(normalized_label)
                    .unwrap_or_default(),
            ],
        })
        .collect::<Vec<_>>();
    let button =
        model::resolve_overflow_button(&candidates, position, &labels).inspect_err(|_| {
            crate::diagnostics::record(
                "tray-selection",
                &format!(
                    "method=label rejected catalog={} native={}",
                    candidates.len(),
                    labels.len()
                ),
            );
        })?;
    let mut old_ids: Vec<_> = hidden.iter().map(|icon| icon.action_id(session)).collect();
    let mut current_ids: Vec<_> = live_icons(&process_windows(), true)?
        .iter()
        .filter(|icon| !icon.promoted)
        .map(|icon| icon.action_id(session))
        .collect();
    old_ids.sort();
    current_ids.sort();
    if tray_order()? != order || old_ids != current_ids || !session.is_current() {
        return Err("Windows tray changed while selecting the icon. Please try again.".into());
    }
    buttons.remove(button).cast().map_err(|e| e.to_string())
}

unsafe fn element_at_icon(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
    rect: RECT,
) -> Result<Option<IUIAutomationElement>, String> {
    let condition = automation
        .CreateTrueCondition()
        .map_err(|e| e.to_string())?;
    let elements = root
        .FindAll(TreeScope_Descendants, &condition)
        .map_err(|e| e.to_string())?;
    let walker = automation.RawViewWalker().map_err(|e| e.to_string())?;
    let mut found: Vec<IUIAutomationElement> = Vec::new();
    for index in 0..elements.Length().unwrap_or(0) {
        let Ok(element) = elements.GetElement(index) else {
            continue;
        };
        let Ok(bounds) = element.CurrentBoundingRectangle() else {
            continue;
        };
        if !model::icon_inside(&rect, &bounds)
            || bounds.right - bounds.left > 128
            || bounds.bottom - bounds.top > 128
        {
            continue;
        }
        let Some(button) = owning_tray_button(&walker, element) else {
            continue;
        };
        if !found.iter().any(|other| {
            automation
                .CompareElements(other, &button)
                .is_ok_and(|same| same.as_bool())
        }) {
            found.push(button);
        }
    }
    Ok(if found.len() == 1 { found.pop() } else { None })
}

unsafe fn find_promoted_icon(
    automation: &IUIAutomation,
    target: &Icon,
) -> Result<IUIAutomationElement3, String> {
    let icon = Shell_NotifyIconGetRect(&target.identifier)
        .map_err(|_| "This tray icon is no longer running.")?;
    let taskbar = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null())
        .map_err(|_| "Windows taskbar is unavailable.")?;
    let bridge = FindWindowExW(
        Some(taskbar),
        None,
        w!("Windows.UI.Composition.DesktopWindowContentBridge"),
        PCWSTR::null(),
    )
    .unwrap_or(taskbar);
    let root = automation
        .ElementFromHandle(bridge)
        .map_err(|e| e.to_string())?;
    let condition = automation
        .CreateTrueCondition()
        .map_err(|e| e.to_string())?;
    let elements = root
        .FindAll(TreeScope_Descendants, &condition)
        .map_err(|e| e.to_string())?;
    let dpi = GetDpiForWindow(taskbar).max(96) as f64;
    let center_x = (icon.left + icon.right) as f64 / 2.0;
    let center_y = (icon.top + icon.bottom) as f64 / 2.0;
    // UIA can list only an app button's child image instead of the button
    // itself, so a hit is walked up to the tray button that owns it.
    let walker = automation.RawViewWalker().map_err(|e| e.to_string())?;
    let mut matches: Vec<IUIAutomationElement> = Vec::new();
    for index in 0..elements.Length().unwrap_or(0) {
        let Ok(element) = elements.GetElement(index) else {
            continue;
        };
        let Ok(bounds) = element.CurrentBoundingRectangle() else {
            continue;
        };
        if bounds.right - bounds.left > 128 || bounds.bottom - bounds.top > 128 {
            continue;
        }
        let hit = |x: f64, y: f64| {
            x >= bounds.left as f64
                && x < bounds.right as f64
                && y >= bounds.top as f64
                && y < bounds.bottom as f64
        };
        if !(hit(center_x, center_y) || hit(center_x * 96.0 / dpi, center_y * 96.0 / dpi)) {
            continue;
        }
        let Some(button) = owning_tray_button(&walker, element) else {
            continue;
        };
        let duplicate = matches.iter().any(|known| {
            automation
                .CompareElements(known, &button)
                .is_ok_and(|same| same.as_bool())
        });
        if !duplicate {
            matches.push(button);
        }
    }
    // The icon's position already identifies exactly one button, so it's
    // used even when its app set no tooltip for it.
    if matches.len() != 1 {
        return Err("Windows could not identify this app's tray icon.".into());
    }
    matches.remove(0).cast().map_err(|e| e.to_string())
}

/// The notification-area button an element belongs to: the element itself or
/// one of its two nearest ancestors.
unsafe fn owning_tray_button(
    walker: &IUIAutomationTreeWalker,
    element: IUIAutomationElement,
) -> Option<IUIAutomationElement> {
    let mut current = element;
    for _ in 0..3 {
        if current
            .CurrentAutomationId()
            .is_ok_and(|id| is_tray_button_id(&id.to_string()))
        {
            return Some(current);
        }
        current = walker.GetParentElement(&current).ok()?;
    }
    None
}

/// Visible top-level windows.
unsafe fn visible_windows() -> Vec<HWND> {
    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let windows = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
        if unsafe { IsWindowVisible(hwnd) }.as_bool() {
            windows.push(hwnd);
        }
        BOOL(1)
    }
    let mut windows = Vec::new();
    let _ = EnumWindows(Some(collect), LPARAM(&mut windows as *mut _ as isize));
    windows
}

// Standard menus and captionless popup windows cover native, WinForms,
// SDL and Qt tray menus. The caller also requires a newly visible window
// from the app's process family, so ordinary captioned clients stay put.
fn is_tray_menu_window(class: &str, style: u32) -> bool {
    // WS_CAPTION combines WS_BORDER and WS_DLGFRAME; a menu can have a
    // border alone without having a title bar.
    class == "#32768" || (style & WS_POPUP.0 != 0 && style & WS_CAPTION.0 != WS_CAPTION.0)
}

fn is_same_app_family(process_path: &str, app_path: &str) -> bool {
    if process_path.eq_ignore_ascii_case(app_path) {
        return true;
    }
    let Some(directory) = std::path::Path::new(app_path).parent() else {
        return false;
    };
    let directory = format!("{}\\", directory.to_string_lossy().to_lowercase());
    let system_root = format!(
        "{}\\",
        std::env::var("SystemRoot")
            .unwrap_or_else(|_| "C:\\Windows".into())
            .to_lowercase()
    );
    // Shared install roots must not claim menus from unrelated applications.
    let is_shared_root = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
        .iter()
        .filter_map(|key| std::env::var(key).ok())
        .any(|root| directory == format!("{}\\", root.to_lowercase()));
    directory.len() > 3
        && !is_shared_root
        && !directory.starts_with(&system_root)
        && process_path.to_lowercase().starts_with(&directory)
}

fn dock_bar_top() -> Option<i32> {
    use tauri::Manager;
    let app = crate::state::DISPLAY_MONITOR_HANDLE.get()?;
    let scale = app.get_webview_window("dock")?.scale_factor().ok()?;
    let (position, _) = (*crate::state::DOCK_WINDOW_RECT.lock().ok()?)?;
    let bar = (*crate::state::DOCK_RECT.lock().ok()?)?;
    Some(position.y + (bar.y as f64 * scale) as i32)
}

/// Apps open their tray menu at their notification icon, which stays in the
/// hidden taskbar's corner while Bloom replaces the taskbar. Waits briefly for
/// the menu the app opens and moves it to `anchor` (the dock icon that was
/// clicked), above the pointer and kept on that monitor. The menu is found by
/// its process's executable: many icons are identified by GUID and carry no
/// window to take the process from.
unsafe fn move_new_menu_to(app_path: &str, before: &[HWND], anchor: POINT) -> Option<(HWND, i32)> {
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1200);
    while std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(25));
        let Some(menu) = visible_windows().into_iter().find(|hwnd| {
            if before.contains(hwnd) {
                return false;
            }
            let mut class_name = [0u8; 256];
            let len =
                windows::Win32::UI::WindowsAndMessaging::GetClassNameA(*hwnd, &mut class_name);
            let class = std::str::from_utf8(&class_name[..len as usize]).unwrap_or("");
            if !is_tray_menu_window(class, GetWindowLongW(*hwnd, GWL_STYLE) as u32) {
                return false;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(*hwnd, Some(&mut pid));
            crate::commands::process_image_path(pid)
                .is_some_and(|path| is_same_app_family(&path, app_path))
        }) else {
            continue;
        };
        let mut rect = RECT::default();
        if GetWindowRect(menu, &mut rect).is_err() {
            return None;
        }
        let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let monitor = MonitorFromPoint(anchor, MONITOR_DEFAULTTONEAREST);
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        let area = info.rcMonitor;
        let x = anchor
            .x
            .clamp(area.left, (area.right - width).max(area.left));
        let y = model::menu_y(anchor.y, height, area.top, area.bottom, dock_bar_top());
        let _ = SetWindowPos(
            menu,
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        return Some((menu, y + height));
    }
    None
}

/// Keep the dock's native-menu hold until the external popup closes. UIA's
/// ShowContextMenu returns when the popup opens, not when the user dismisses it.
unsafe fn wait_for_menu_close(menu: HWND, bottom: i32) {
    let mut owner = 0;
    GetWindowThreadProcessId(menu, Some(&mut owner));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while std::time::Instant::now() < deadline
        && IsWindow(Some(menu)).as_bool()
        && IsWindowVisible(menu).as_bool()
        && !crate::state::SHUTTING_DOWN.load(std::sync::atomic::Ordering::Relaxed)
    {
        let mut current_owner = 0;
        GetWindowThreadProcessId(menu, Some(&mut current_owner));
        if current_owner != owner {
            break;
        }
        // Steam can reposition its popup after UIA returns. Keep its bottom
        // above the bar while retaining the existing native-menu hover hold.
        let mut rect = RECT::default();
        if GetWindowRect(menu, &mut rect).is_ok() && rect.bottom > bottom {
            let _ = SetWindowPos(
                menu,
                None,
                rect.left,
                bottom - (rect.bottom - rect.top),
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

pub fn interact(key: &str, context_menu: bool) -> Result<(), String> {
    let _operation = begin_operation()?;
    unsafe {
        let _com = ComApartment::new()?;
        let session = ShellSession::current()?;
        let windows = process_windows();
        let icons = live_icons(&windows, true)?;
        let icon = icons
            .iter()
            .find(|i| i.action_id(&session) == key)
            .ok_or("This tray icon is no longer running.")?;
        let automation = automation_client()?;
        let mut exposure = TrayExposure::new()?;
        if exposure.session != session {
            return Err("Windows Explorer restarted. Refresh the tray and try again.".into());
        }
        let element = if icon.promoted {
            find_promoted_icon(&automation, icon)?
        } else {
            exposure.expose_overflow(&automation)?.ok_or(
                "Windows is not showing this icon in the overflow. Refresh the tray and try again.",
            )?;
            find_overflow_icon(&automation, &icons, icon, &session)?
        };
        if !session.is_current() {
            return Err("Windows Explorer restarted. Refresh the tray and try again.".into());
        }
        Shell_NotifyIconGetRect(&icon.identifier)
            .map_err(|_| "This tray icon is no longer running.")?;
        if context_menu {
            // The pointer is on the dock icon that was right-clicked.
            let mut cursor = POINT::default();
            let _ = GetCursorPos(&mut cursor);
            let before = visible_windows();
            element
                .ShowContextMenu()
                .map_err(|_| "Windows could not open this app's native tray menu.".to_string())?;
            let menu = move_new_menu_to(&icon.path, &before, cursor);
            if let Some((menu, bottom)) = menu {
                // Hiding the native owner can dismiss a shell-hosted menu.
                wait_for_menu_close(menu, bottom);
            } else if let Some(overflow) = exposure
                .overflow
                .filter(|hwnd| IsWindowVisible(*hwnd).as_bool())
            {
                // Custom menus may not expose an attributable HWND. Preserve
                // their native owner without moving an unrelated popup.
                wait_for_overflow_close(&exposure, overflow)?;
            }
            Ok(())
        } else {
            let element: IUIAutomationElement = element.cast().map_err(|e| e.to_string())?;
            let pattern = element
                .GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
                .map_err(|e| e.to_string())?;
            pattern
                .Invoke()
                .map_err(|_| "Windows could not activate this tray icon.".to_string())
        }
    }
}

/// Opens Explorer's actual hidden-icons flyout through its own chevron. There
/// is deliberately no keyboard macro or arbitrary-button fallback. The guard
/// stays on this worker until dismissal, settings change, shell restart or timeout.
pub fn open_native_overflow() -> Result<bool, String> {
    let _operation = begin_operation()?;
    unsafe {
        let _com = ComApartment::new()?;
        let automation = automation_client()?;
        let mut exposure = TrayExposure::new()?;
        let Some(overflow) = exposure.expose_overflow(&automation)? else {
            return Ok(false);
        };
        wait_for_overflow_close(&exposure, overflow)?;
        Ok(true)
    }
}

unsafe fn wait_for_overflow_close(exposure: &TrayExposure, overflow: HWND) -> Result<(), String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    while IsWindow(Some(overflow)).as_bool() && IsWindowVisible(overflow).as_bool() {
        if !exposure.session.is_current() {
            return Err("Windows Explorer restarted. Open the tray again.".into());
        }
        if crate::state::SHUTTING_DOWN.load(std::sync::atomic::Ordering::Relaxed)
            || (exposure.was_hidden
                && (!crate::utils::dock_enabled()
                    || !crate::state::NATIVE_TASKBAR_HIDDEN
                        .load(std::sync::atomic::Ordering::Relaxed)))
        {
            break;
        }
        if std::time::Instant::now() >= deadline {
            return Err("The Windows tray did not close. Please try again.".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_tray_menu_detection_excludes_normal_client_windows() {
        assert!(is_tray_menu_window("#32768", 0));
        assert!(is_tray_menu_window("SDL_app", WS_POPUP.0));
        assert!(is_tray_menu_window(
            "WindowsForms10.Window.8.app",
            WS_POPUP.0
        ));
        assert!(is_tray_menu_window(
            "WindowsForms10.Window.8.app",
            WS_POPUP.0 | WS_BORDER.0
        ));
        assert!(is_tray_menu_window(
            "Qt6QWindowPopupDropShadowSaveBits",
            WS_POPUP.0
        ));
        assert!(!is_tray_menu_window("SDL_app", WS_POPUP.0 | WS_CAPTION.0));
        assert!(!is_tray_menu_window(
            "WindowsForms10.Window.8.app",
            WS_OVERLAPPEDWINDOW.0
        ));
        assert!(!is_tray_menu_window("OtherApp", 0));
    }

    #[test]
    fn known_folder_paths_expand_and_keep_the_relative_executable() {
        unsafe {
            let resolved =
                resolve_path("{7C5A40EF-A0FB-4BFC-874A-C0F2E0B9FA8E}\\Steam\\Steam.exe").unwrap();
            let root = std::env::var("ProgramFiles(x86)").unwrap();
            assert_eq!(
                resolved.to_lowercase(),
                format!("{root}\\Steam\\Steam.exe").to_lowercase()
            );
            assert_eq!(
                resolve_path("C:\\Apps\\Steam.exe").as_deref(),
                Some("C:\\Apps\\Steam.exe")
            );
            assert!(resolve_path("{not-a-guid}\\app.exe").is_none());
        }
    }
    #[test]
    fn chevron_is_identified_without_its_localized_name() {
        assert!(is_overflow_chevron(
            "SystemTrayIcon",
            "SystemTray.NormalButton"
        ));
        assert!(!is_overflow_chevron(
            "NotifyItemIcon",
            "SystemTray.NormalButton"
        ));
        assert!(!is_overflow_chevron(
            "SystemTrayIcon",
            "SystemTray.OmniButton"
        ));
        assert!(!is_overflow_chevron(
            "SystemTrayIcon",
            "SystemTray.AccentButton"
        ));
    }

    #[test]
    fn tray_menus_of_helper_processes_belong_to_their_app() {
        let steam = "C:\\Program Files (x86)\\Steam\\steam.exe";
        assert!(is_same_app_family(steam, steam));
        assert!(is_same_app_family(
            "C:\\Program Files (x86)\\Steam\\bin\\cef\\cef.win64\\steamwebhelper.exe",
            steam
        ));
        assert!(!is_same_app_family(
            "C:\\Program Files\\Apps\\GHelper.exe",
            steam
        ));
        assert!(!is_same_app_family(
            "C:\\Program Files (x86)\\SteamTools\\helper.exe",
            steam
        ));
        // Apps in the Windows folder must not claim every system process.
        assert!(!is_same_app_family(
            "C:\\Windows\\System32\\notepad.exe",
            "C:\\Windows\\System32\\SecurityHealthSystray.exe"
        ));
    }

    #[test]
    fn promoted_app_icons_use_notify_item_icon_on_26h2() {
        assert!(is_tray_button_id("NotifyItemIcon"));
        assert!(is_tray_button_id("SystemTrayIcon"));
        assert!(!is_tray_button_id(""));
        assert!(!is_tray_button_id("TaskbarFrame"));
    }

    #[test]
    #[ignore = "read-only check against the current Windows desktop"]
    fn live_tray_discovery() {
        let apps = enumerate();
        for app in &apps {
            assert!(!app.tray_ids.is_empty());
            assert!(!app.path.is_empty());
            println!("{}: {} live tray icon(s)", app.name, app.tray_ids.len());
        }
        println!("{} live tray apps", apps.len());
    }

    #[test]
    #[ignore = "read-only check against the current Windows desktop"]
    fn live_tray_catalog() {
        let first = catalog().expect("the current Windows shell must expose its tray catalog");
        let ids: std::collections::HashSet<_> = first.icons.iter().map(|icon| &icon.id).collect();
        assert_eq!(ids.len(), first.icons.len());
        assert!(first
            .icons
            .iter()
            .all(|icon| !icon.path.is_empty() && !icon.id.is_empty()));
        let second = catalog().expect("reading the catalog twice must not change the shell");
        assert_eq!(first.shell_id, second.shell_id);
        unsafe {
            let shell = ShellSession::current().unwrap();
            if GetWindowLongW(shell.hwnd, GWL_EXSTYLE) & WS_EX_LAYERED.0 as i32 != 0 {
                let mut alpha = 0;
                GetLayeredWindowAttributes(shell.hwnd, None, Some(&mut alpha), None).expect(
                    "the existing taskbar transparency must be readable before any exposure",
                );
            }
        }
        // Do not log tooltip hints, application paths, messages or HWNDs.
        println!(
            "{} live icons; {} shell-hosted; {} promoted",
            first.icons.len(),
            first.icons.iter().filter(|icon| icon.is_shell_icon).count(),
            first.icons.iter().filter(|icon| icon.is_promoted).count()
        );
    }

    #[test]
    #[ignore = "owner must open the native Windows overflow first; read-only, never invokes apps"]
    fn native_slack_selection_readonly() {
        unsafe {
            let _com = ComApartment::new().unwrap();
            let panel = FindWindowW(w!("TopLevelWindowForOverflowXamlIsland"), PCWSTR::null())
                .or_else(|_| FindWindowW(w!("NotifyIconOverflowWindow"), PCWSTR::null()))
                .unwrap();
            assert!(
                IsWindowVisible(panel).as_bool(),
                "Owner must open Windows overflow first"
            );
            let session = ShellSession::current().unwrap();
            let icons = live_icons(&process_windows(), true).unwrap();
            let automation = automation_client().unwrap();
            let root = automation.ElementFromHandle(panel).unwrap();
            let elements = root
                .FindAll(
                    TreeScope_Descendants,
                    &automation.CreateTrueCondition().unwrap(),
                )
                .unwrap();
            let mut native_count = 0;
            let mut slack_labels = 0;
            for index in 0..elements.Length().unwrap_or(0) {
                let element = elements.GetElement(index).unwrap();
                if element
                    .CurrentAutomationId()
                    .is_ok_and(|id| is_tray_button_id(&id.to_string()))
                {
                    native_count += 1;
                    if element
                        .CurrentName()
                        .is_ok_and(|name| normalized_label(&name.to_string()).contains("slack"))
                    {
                        slack_labels += 1;
                    }
                }
            }
            println!(
                "catalog_hidden={} native_nodes={native_count} native_slack_labels={slack_labels}",
                icons.iter().filter(|i| !i.promoted).count()
            );
            let mut resolved = 0;
            let mut rejected = 0;
            for icon in icons.iter().filter(|i| {
                !i.promoted
                    && std::path::Path::new(&i.path)
                        .file_name()
                        .is_some_and(|name| name.eq_ignore_ascii_case("slack.exe"))
            }) {
                match find_overflow_icon(&automation, &icons, icon, &session) {
                    Ok(_) => resolved += 1,
                    Err(error) => {
                        rejected += 1;
                        println!("selection rejected: {error}");
                    }
                }
            }
            println!(
                "slack_identifiers={} resolved={resolved} rejected={rejected}",
                resolved + rejected
            );
            assert!(
                resolved > 0 && rejected == 0,
                "Native Slack mapping is not verified"
            );
        }
    }
    #[test]
    fn accepts_braced_guid_and_rejects_bad_identity() {
        assert!(parse_guid("{699E423D-73DA-42BA-B94E-881FF8672467}").is_some());
        assert!(parse_guid("not-an-icon").is_none());
    }

    #[test]
    fn explorer_hosted_system_icons_are_not_app_trays() {
        assert!(is_explorer_hosted_icon("C:\\Windows\\explorer.exe"));
        assert!(is_explorer_hosted_icon("C:\\WINDOWS\\EXPLORER.EXE"));
        assert!(!is_explorer_hosted_icon(
            "C:\\Windows\\System32\\SecurityHealthSystray.exe"
        ));
        assert!(!is_explorer_hosted_icon("C:\\Apps\\Discord.exe"));
    }
}
