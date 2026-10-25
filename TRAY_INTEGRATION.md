# Windows notification area integration

The dock and the notification area share discovery and native actions.
They use different representations: a dock app can own several notification
icons. A tray icon is not evidence of an open application window.

## Custom tray and dock behavior

The tray lives at the bottom-right of the Bloom dock. Its chevron opens a dark,
bounded hidden-icons panel above it; icons promoted by Windows remain visible
beside the chevron. The notch has no tray button. `get_tray_icons` supplies both
groups from one catalog. The old `bloom-tray-placement` setting is ignored.
Command Center opens the custom tray when it is available, or the native
fallback otherwise. Right-click the chevron for Explorer's native overflow.

While a responding tray surface is available, unpinned apps without windows
leave the dock and remain reachable in the tray. Pinned apps retain a launcher
without an open-window/background dot. Real windows continue to use normal dock
dots/previews. The dock falls back to the previous resident representation if
the custom catalog fails, its window is hidden or its availability lease expires.
The surface renews that lease every two seconds (expires after eight); the dock's
existing ten-second safety poll also converges after a silent failure. Native
actions still revalidate identifiers independently of this availability signal.

The following rules describe that fallback:

An unpinned app remains after its last window closes only if Bloom observed its
window in this session and it still has a live notification icon. Other tray-only
apps do not fill the dock. Pinned apps can show a grey background dot; windowed
apps keep window dots and previews. When the custom tray is unavailable, closing
WhatsApp to the tray therefore still leaves its dock fallback access.

- Click a background app to invoke its notification icon's default action.
- Right-click for Bloom's menu; "App tray menu" opens its native context menu.
- Apps with several tray icons have separate native-menu entries.
- Explorer-hosted icons are excluded from the dock app aggregate, so Bluetooth
  does not become a File Explorer app action.

## Per-icon catalog and action contract

`get_tray_icons` returns `{ shell_id, icons }`; each icon has `id`, `name`, `path`,
`is_promoted`, `is_shell_icon` and optional `icon` PNG data URI. This independent catalog includes attributable
live icons even when Bloom has never seen an app window, including icons hosted
by Explorer and Bloom itself. `get_tray_apps` preserves the dock's grouped contract.
`name` is the executable's display name, not a live tooltip. `icon` comes from
Windows' cached `IconSnapshot`, after PNG validation and limits of 64 KiB and
128×128 pixels. It can lag behind dynamic status; no unread-count or connection
state is inferred from it. Unavailable artwork uses a named initial fallback.
Live tooltip/status extraction remains future work.

`id` is an opaque, transient action token. Pass it unchanged to
`activate_tray_icon` or `show_tray_context_menu`; do not save or parse it.
It binds the candidate to Explorer's taskbar handle, process ID and process
creation time, registry key, executable path and native icon identity. A GUID
identifies an icon independently of HWND/UID; otherwise both HWND and UID are
required. This follows Microsoft's
[NOTIFYICONIDENTIFIER contract](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ns-shellapi-notifyiconidentifier).

`tray.rs` reads candidates from Windows 11's
`HKCU\Control Panel\NotifyIconSettings`. Each must match a running process path
and pass
[Shell_NotifyIconGetRect](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyicongetrect).
Cached registry entries alone never count as running. A changed shell during
catalog construction is an error, not an empty successful snapshot. Actions
re-enumerate and reject stale tokens before invoking UIA. Refresh after errors.

## Native selection and fallback

Promoted icons use their screen rectangle and the existing DPI-aware UIA matcher.
Hidden icons use `UIOrderList` and Explorer's overflow buttons. Both supported
Windows UI Automation button IDs are accepted. Distinct names or cached-tooltip
hints must agree with the order. A changed order/catalog is rejected.
When counts differ, only one label unambiguously naming one known app is accepted.
Several icons from the same executable additionally require unique matching
hints. Ambiguous selection fails; the native tray lets the user select manually.
`InitialTooltip` is only a private matching hint, never a live tooltip or liveness
test, and is not included in the public catalog or diagnostic output.

The native fallback calls `open_system_tray`.
This awaits `Result<bool, String>`: `true` after opening and dismissal, `false`
when there is no supported hidden-icons button, or an error. It finds one enabled,
onscreen chevron by AutomationId, class and control type; it never sends Win+B
and Space or invokes an arbitrary taskbar button. The existing button is disabled
while waiting. A native dialog reports absence/errors and allows retry.

Actions share one operation guard and scoped taskbar exposure. The guard preserves
the original rectangle, extended style and layered attributes, and keeps the
native owner alive while a recognized context menu or overflow remains open.
Exit, disabled dock or shell replacement determine the final visibility state;
late cleanup must not hide a taskbar restored on exit. The existing global exit
cleanup also removes temporary transparency if process exit precedes worker Drop.
Normal successful cleanup does not change AppBar state or icon-promotion settings.

Overflow polling is bounded to five minutes; recognized-menu polling retains its
existing one-minute limit. Shell changes/settings/shutdown can end polling early.
UIA connection and transaction timeouts are set to 1s and 2s. These limit requests,
not the whole operation: Microsoft notes that
[Invoke can depend on the provider's implementation](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationinvokepattern-invoke).
An acknowledgment is not proof that an app opened a window.
[ShowContextMenu may invoke an ancestor's menu](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement3-showcontextmenu);
custom menus and each shell-hosted icon still require physical validation.

## Compatibility and limits

Registry layout and Explorer's UIA tree are Windows implementation details, not
a supported enumeration API. The catalog covers live icons attributable through
this adapter, not every process or every system quick-setting control. Missing
metadata or UIA ambiguity can exclude an icon or prevent its direct action.
Windows 10 discovery and alternative shells are not implemented; the catalog
returns an error when its registry source is unavailable. Window-based dock
behavior is preserved. Universal tray coverage, mixed-DPI action correctness and
shell-restart recovery are not proven by pure tests or read-only discovery.

The bottom-right area follows the compact dock's height and scale. The flyout
has a bounded four-column grid that scrolls vertically for more icons. The dock
window region includes the flyout above its normal bottom band; its own hit
rectangles keep the rest of the desktop click-through. No new window or
dependency is introduced. Previously saved notch/right preferences are inert.

Escape, outside press and blur close the custom tray. Default/context actions
close it before opening the native app/menu. Shift+F10 reaches each icon's native
menu. A busy native operation blocks repeated actions. Recognized native menus
stay above the dock, near the clicked icon.
Catalog refresh is shared within the surface: every two seconds while open, ten
while closed, and immediately when opening/after an action. Availability heartbeats
do not themselves re-enumerate Explorer. Empty/error results use a native notice
and preserve the native toggle fallback instead of inventing another layout.

## Validation

```powershell
bun run test
bun run build
cd src-tauri
cargo check --locked
cargo clippy --locked --all-targets
cargo test --locked
# Optional read-only check; does not open menus or expose the taskbar:
cargo test --locked live_tray_catalog -- --ignored --nocapture
```

24 October 2026: 69 Bun tests, 93 Rust tests and build/check/Clippy pass (11 existing
Clippy warnings; two native tests ignored by default). The optional catalog check
found eight live icons, three promoted and zero Explorer-hosted in that snapshot.
It checks uniqueness, stable shell session and readable existing layered attributes.
It does not prove native activation, complete coverage or restoration. Browser
checks with mocked IPC additionally exercise real Dock/SystemTray components:
resident removal and fallback, retained pinned launcher, one default action,
context-menu busy state, location changes, Escape, outside press, 18-icon keyboard
overflow and proportional dimensions at scales 1 and 1.3. These are not installed
Windows tests. The owner confirmed only the native overflow opening in 3.10.5.

25 October 2026: the owner confirmed that WhatsApp and Slack leave the dock
after their last window closes, and reported that a Slack tray click can fail
with a changed-icons error. The accepted revision moves the tray beside the
dock, displays promoted icons continuously, and opens hidden icons above the
chevron. It accepts both observed UIA button identifiers and can use a unique
tooltip when an extra native icon shifts the old order. A mocked-browser check
covered opening, visible icon action, menu busy state, keyboard overflow,
catalog failure/recovery and resident fallback. These checks do not demonstrate
that Slack's real native menu or every Windows icon now works.

The owner performs physical installed tests; these remain pending until reported:

1. Chevron at bottom-right opens the hidden-icons panel; Escape/outside click
   closes it. Promoted icons remain beside the chevron. Notch has no tray button.
2. No hidden icons/all promoted: report absence without opening another app.
3. Background WhatsApp/Slack: default action and native context menu, then Quit;
   include promoted, hidden, multiple icons and app-defined tooltip names.
   Compare the native Windows icon set with Bloom's panel and report any missing
   icon by application name. Open OneDrive and Google Drive flyouts from Bloom.
4. Exit Bloom with overflow/menu open, disable dock, restart Explorer: recover the
   intended taskbar state without transparency. Check pinned/unpinned access.
5. Mixed DPI, multiple monitors, scale changes and permission-denied applications.

Publishing a technically validated test version does not complete these cases.
