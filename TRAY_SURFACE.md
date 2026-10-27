# Independent Windows tray surface

Current provider: [TRAY_LIVE.md](TRAY_LIVE.md). The live observer supplies
current images and direct callbacks; registry/UIA remains the fallback. The
approved HWND and bottom curves remain unchanged.

The owner reported that the 3.10.8 tray still took no clicks. Its simulated IPC
tests did not reproduce that native failure. The tray now has a dedicated HWND
and webview entry. This is a deliberate isolation boundary: the dock's global
click-through flag and previews cannot disable tray input. Its visibility follows
the dock's Smart/Peek decision, and hovering or opening the tray keeps both visible.
It costs another webview; no memory or CPU reduction is claimed.

`tray/surface.rs` accepts rectangles only from `tray`, applies a native window
region for the bar and expanded flyout, and keeps that HWND interactive. Empty
desktop outside their union is excluded from the window. The tray is positioned
at the primary monitor bottom, remains proportional to dock height/scale and uses
the same `EdgeSurface` bottom flares. Dock geometry does not contain tray UI.

3.10.14 tray input was confirmed by the owner. Explorer's overflow chevron is
measured with UI Automation before hiding the native taskbar. Bloom anchors
the row there when possible and uses compact 26 logical-pixel cells with 18px
icons, rather than inheriting inconsistent native icon rectangles. Its scale
follows the dock; at the standard 48px base, the visible surface is 6 logical
pixels shorter, stays attached to the bottom edge, and keeps controls aligned
with the dock. Its 18px upper corners and background fallback match the dock;
the wider side insets keep icons from crowding the rounded edge.
The right edge is retained for the current monitor and Explorer session so a
changed icon count or transient measurement cannot move the whole tray. When it
is crowded, the application group scrolls without
hiding the arrow, network, or sound controls.
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
AppBar. The dock sends its visibility state in order; an intentionally hidden
healthy tray remains available for resident-app representation. The last region
is retained so a throttled hidden webview can be revealed and resume its heartbeat.
Menu dispatch, Explorer-session identity and stale-icon safeguards share
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
The UI never asserts Internet connectivity if the status query fails. The sound
control displays the current default endpoint's volume/mute state. Its left
click requests Windows Quick Settings, and right click opens Windows sound
settings directly; neither duplicates all native context-menu commands.

Windows' current icon rectangle is compared with the saved taskbar and chevron
to identify promoted and hidden icons when unambiguous. The registry promotion
setting remains the fallback. A live GUID confirmed by Explorer does not require
an exact executable-name match, which can differ for packaged applications;
duplicate records are deduplicated and a current owner is preferred. An observed
icon with no registry entry is classified from its live rectangle when possible,
otherwise it remains in overflow. The observer begins at attachment and cannot
guarantee an initial snapshot of every application; completeness still needs
real Windows verification and a safe fallback for unmatched identities.

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

The tray's animated content fills the complete host rectangle so its
`EdgeSurface` uses the same base height as the dock instead of collapsing to the
icon row. In Smart mode, a standard maximized-window signal is also a hide
condition; the same dock visibility state drives both surfaces. Physical
Windows verification is still required for maximize/restore transitions and
the approved tray silhouette at the user's DPI.

In 3.10.20 the animated content did not inherit the host's corner radii, so its
`EdgeSurface` appeared rectangular. The content now inherits those radii and
contains the horizontal padding, allowing the background to fill the host while
retaining the icon inset. A headless Edge fixture using the actual styles showed
the rounded surface and 9px inset; the installed Tauri window still needs the
owner's visual confirmation.

## Estabilidad de métricas al arrancar
Dock y tray comparten una altura de respaldo de 42px (base 48 menos 6). El dock comunica la altura derivada de la base nativa, sin medir la caja que Framer modifica durante las animaciones; las respuestas del handshake rechazan alturas no finitas/fuera de rango y callbacks posteriores al desmontaje.
La primera geometría usable fija el espaciado y el borde derecho para esa sesión de Explorer/monitor/DPI/escala de Bloom. Si al arrancar falta una medición nativa se conserva el respaldo de 28px hasta una recalibración explícita; una medición o ancla tardía no desplaza una bandeja visible. Cambiar el número de iconos conserva el borde derecho y adapta el ancho, como antes. Un cambio de escala, DPI, monitor o sesión permite recalibrar. No se alteran clics, proveedor vivo, visibilidad ni menús nativos.
Pruebas automáticas cubren medición tardía, ancla ausente/tardía, menos iconos y cambios de sesión/DPI/escala. Arranque, reinicio, Capturas y maximizar/restaurar en la versión instalada quedan pendientes del propietario.
