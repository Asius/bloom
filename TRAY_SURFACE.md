# Independent Windows tray surface

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

3.10.9 input was confirmed by the owner. 3.10.10 preserves the native reserved
right-hand space (system controls and clock): measure promoted-icon rectangles
before hiding Windows' taskbar and scale that inset with DPI. If measurement is
unavailable, reserve 120 logical pixels until the next handoff; this is a fallback,
not a claim of exact native geometry.

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

Icons are Windows `IconSnapshot` artwork verified as live candidates through
`Shell_NotifyIconGetRect`, not invented app shortcuts. Cached artwork is not a
guaranteed live state indicator; some shell controls may not be exposed through
this catalog. This implementation does not reparent Explorer's controls or
inject a DLL into Explorer. Activation/context menus still depend on native UI
Automation and must be tested with the owner's actual Windows build/apps.
The owner's grayscale images were confirmed to originate in the Windows cache.
COM exploration did not enumerate live images during the initial bounded probe;
no experimental live-image provider is enabled in this version.

Diagnostics distinguish `tray-surface` readiness/open changes from `tray-input`
(`toggle`, `activate`, `context`). They do not record icon names or paths.

Validation includes a test-owned invisible Win32 HWND: real `SetWindowRgn`,
`GetWindowRgn` and `PtInRegion` confirm bar/flyout membership, desktop-gap exclusion,
and a layered style without `WS_EX_TRANSPARENT`. This does not prove that the
installed WebView receives real mouse input. Owner tests remain mandatory:
arrow, left/right clicks, native menus, Capturas, fullscreen recovery, scale/DPI,
disable/enable and native taskbar restoration on exit.
