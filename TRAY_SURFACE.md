# Independent Windows tray surface

Current provider: [TRAY_LIVE.md](TRAY_LIVE.md). The live observer supplies
current images and direct callbacks; registry/UIA remains the fallback. The
approved HWND and bottom curves remain unchanged.

The owner reported that the 3.10.8 tray still took no clicks. Its simulated IPC
tests did not reproduce that native failure. The tray now has a dedicated HWND
and webview entry. This is a deliberate isolation boundary: the dock's global
click-through flag, hover/reveal geometry and previews cannot disable tray input.
It costs another webview; no memory or CPU reduction is claimed.

`tray/surface.rs` accepts rectangles only from `tray`, applies a native window
region for the bar and expanded flyout, and keeps that HWND interactive. Empty
desktop outside their union is excluded from the window. The tray is positioned
at the primary monitor bottom, remains proportional to dock height/scale and uses
the same `EdgeSurface` bottom flares. Dock geometry does not contain tray UI.

3.10.14 tray input was confirmed by the owner. Explorer's overflow chevron is
measured with UI Automation before hiding the native taskbar. Bloom anchors
the row there when possible and uses uniform 32 logical-pixel cells to match the
dock, rather than inheriting smaller native icon rectangles. The row scales with
the dock and is shifted only as needed to fit the monitor; when it is crowded,
the application group scrolls without hiding the arrow or network control.
If the anchor is unavailable the right-edge fallback is approximate. The hidden
panel opens above and toward the left. This adjusted layout and DPI behavior
still require owner verification in the installed app.

The HWND covers the full primary-monitor width; its native region still limits
painting and input to the bar and flyout. The original horizontal taskbar height
supplies the dock, tray and compact notch base height in logical pixels; 48
logical pixels is the fallback if unavailable.
The bottom flares retain the approved style.
The hidden-icons grid grows with its icon count, caps at five columns, and puts
the sixth icon on the next row. Its keyboard up/down step follows the actual
column count. The panel has no drop shadow; the approved Bloom bottom-edge
curves are retained.

The tray webview initializes Bloom's shared theme on its own. System mode follows
Windows/WebView2's `prefers-color-scheme`, setting changes are applied from the
event payload immediately, and the background opacity slider updates the tray
surface. The initial opacity is 0.92 with a subtle acrylic-style blur; CSS cannot
read Windows' exact composited taskbar material, so this is a visual default and
not a claim of pixel-identical transparency. Light mode preserves Windows'
multicolor Start icon.

For hidden-icon actions, prefer a live icon rectangle inside the overflow and
revalidate it. Chevron-only rectangles and collisions cannot identify an icon.
The label fallback resolves a unique current label across count/order changes;
multiple icons of one app require distinct exact tooltip labels. Ambiguous
matches remain errors rather than invoking a different app.

The surface follows dock enablement/native taskbar handoff, capture selection,
fullscreen state, monitor changes and shutdown. It does not register another
AppBar. Menu dispatch, Explorer-session identity and stale-icon safeguards share
the existing tray catalog; the catalog uses Windows UI order when available,
with a deterministic registry-key fallback if that value is absent.

Icons are Windows notification icons, not invented app shortcuts. The live
provider supplies current artwork when Explorer publishes it; registry snapshots
remain the fallback, so some icons may still have cached art. Activation/context
menus use the live callback where available and UI Automation as a bounded
fallback. If the registry catalogue fails, validated live-observer icons remain
accessible in overflow, where their promotion state is unknown. A double click
on the same icon is queued while its first callback is completing. The owner
must verify appearance, positioning, single/double clicks and app menus on the
installed Windows build.

The network control reads connectivity from Windows Network List Manager and
uses the default Windows connection profile to distinguish Wi-Fi. Left click
invokes the Windows Quick Settings shortcut; right click opens Windows network
settings directly. This does not recreate every native context-menu item.
The UI never asserts Internet connectivity if the status query fails.

Newly visible popups identified as belonging to the clicked application are
repositioned only if they are outside the monitor or overlap the tray bar.
Their bounds are rechecked while they remain visible, since an app can move a
popup after creation. Unknown or unattributable popup windows are not moved.

Diagnostics distinguish `tray-surface` readiness/open changes from `tray-input`
(`toggle`, `activate`, `context`). They do not record icon names or paths.

Validation includes a test-owned invisible Win32 HWND: real `SetWindowRgn`,
`GetWindowRgn` and `PtInRegion` confirm bar/flyout membership, desktop-gap exclusion,
and a layered style without `WS_EX_TRANSPARENT`. This does not prove that the
installed WebView receives real mouse input. Owner tests remain mandatory:
arrow, left/right clicks, native menus, Capturas, fullscreen recovery, scale/DPI,
disable/enable and native taskbar restoration on exit.
