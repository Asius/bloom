# Live notification-area provider

Bloom 3.10.12 keeps the approved tray layout and adds a live data/action path.
Registry history is no longer the only image source. Native UI Automation is
retained for icons whose registration/callback is unavailable.

## Data flow

`build.rs` compiles `native/tray_observer.cpp` using MSVC. The DLL is embedded
in Bloom, extracted under its local app data directory with a build-specific
name, verified byte-for-byte, and opened with write/delete sharing denied for
the duration of the attachment. It is loaded by absolute path with a restricted
DLL dependency search. No downloaded modules or Windhawk mods are loaded.

`tray/live.rs` owns a message-only receiver and a hook restricted to the
`Shell_TrayWnd` thread. It requests registration once per attachment using
`TaskbarCreated`. The observer only reads notification-area `WM_COPYDATA`:
ADD, MODIFY, DELETE and SETVERSION. It validates the known 956-byte icon prefix,
message type/magic and bounded envelope; this PC uses a 1484-byte envelope.
Trailing data and balloon content are ignored. Unknown layouts are rejected.

The observer captures the current HICON at 32×32 while its owner is still in
the shell call, with black/white rendering for alpha and legacy mask icons.
Only a fixed 4416-byte sequenced packet is sent to Bloom. Its receiver copies to a queue
of 128 entries, then returns; PNG encoding happens in Bloom. Sending is bounded
to 20 ms so an unresponsive receiver cannot indefinitely hold Explorer.
The observer always calls the next hook; it does not patch Explorer functions.

The store holds at most 512 entries. Identity is GUID or HWND+UID within a shell
generation, plus owner PID and process creation time. ADD defaults to protocol
0; only SETVERSION interprets the timeout/version union. State masks distinguish
an application-hidden icon from an icon merely in the Windows overflow. Stale
owners/tokens are rejected; sequence gaps/queue overflow invalidate live state and reconnect.
Explorer restart reconnects with a fresh inventory. Exit/restart detaches the
hook and removes the receiver property; the OS also removes hooks on process exit.

The registry contributes ordering and visible/overflow preferences. Each image
reports `live`, `cached` or `unavailable`; an unavailable native callback retains
the UIA fallback. Events refresh the frontend at most four times per second.
No tooltip, notification text or executable path is written to diagnostics.

## Actions

Left, double and right clicks send versioned callbacks asynchronously to the
validated owner window. Version 4 packs screen coordinates and UID; older
versions use the legacy parameters. A slow app/menu loop does not disable all
Bloom tray buttons. Activation no longer requires finding Slack by its position
or label. Native app popups are retained at their own on-screen position; a
newly appearing popup belonging to the clicked application is moved near the
click only when its original position is outside that monitor.

The UIA fallback was also repaired: never show a hidden cached flyout HWND as
though it were current, require a Button rather than its image child, allow
bounded layout settling before selection, and preserve left-click popups.

## Verification and limits

The owner explicitly authorized integration, a controlled fixture test and
activation after checks. Ignored native tests must be run deliberately:

```
cargo test --locked native_observer -- --ignored --nocapture --test-threads=1
```

The fixture creates its own temporary notification icon, checks live image
changes, protocol version, left/double/context callbacks, removal, stale-token
rejection and observer-property cleanup. It does not invoke user applications.
The initial-inventory test requests registrations and reads aggregate coverage.
An earlier native run found 6 icons, 6 live images, 5 direct callbacks, 0 cached
images. These counts are observations, not a coverage guarantee for every app.

Owner testing of installed Slack, WhatsApp, OneDrive/Drive menus, actual input,
DPI, Explorer restart, fullscreen, Capturas and update/exit remains separate.
This does not implement network/volume/clock controls, drag reorder, native
hover cards or notification balloons. It does not claim Windows-wide parity.
The private wire prefix can change in a Windows update; fallback stays required.

## References and independent implementation

- [Seelen tray manager and callbacks](https://github.com/eythaann/Seelen-UI/blob/master/src/background/modules/system_tray/application/tray_icon.rs)
- [Seelen observer and wire layout](https://github.com/eythaann/Seelen-UI/blob/master/src/hook_dll/src/lib.rs)
- [Seelen scoped hook loader](https://github.com/eythaann/Seelen-UI/blob/master/src/background/modules/system_tray/application/tray_hook_loader.rs)
- [Windhawk architecture](https://github.com/ramensoftware/windhawk#technical-details)
- [Microsoft notification contract](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyiconw)
- [Microsoft NOTIFYICONDATAW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ns-shellapi-notifyicondataw)

The integration is written for Bloom. Seelen's AGPL implementation and
Windhawk's global injection engine are not vendored into this GPL application.
