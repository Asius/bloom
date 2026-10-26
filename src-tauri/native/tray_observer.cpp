// Bloom's notification-area observer. No patches, registry writes, application
// hooks, network, or heap queues. Only attached to the Shell_TrayWnd thread.
// Protocol layout corroborated against Seelen UI/ManagedShell; implementation
// is local. See TRAY_LIVE.md for sources, lifecycle and limitations.
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <shellapi.h>
#include <stdint.h>
#include <stddef.h>

constexpr wchar_t ReceiverProperty[] = L"Bloom.Tray.Observer.V1";
constexpr ULONG_PTR PacketType = 0x424c5431;
constexpr int Side = 32;

// Shell_NotifyIcon transports a 32-bit wire layout even from 64-bit clients.
struct WireIcon {
    uint32_t size, window, uid, flags, callback, icon;
    wchar_t tip[128];
    uint32_t state, mask;
    wchar_t info[256];
    uint32_t version;
    wchar_t title[64];
    uint32_t infoFlags;
    GUID guid;
    uint32_t balloon;
};
static_assert(sizeof(WireIcon) == 956);
struct ShellMessage { uint32_t magic, operation; WireIcon icon; };

// All pointer-free, fixed size. No balloon/notification content is forwarded.
struct Packet {
    uint32_t protocol, operation, window, uid, flags, callback, state, mask, version, pid, sequence;
    GUID guid;
    wchar_t tip[128];
    uint32_t hasImage;
    unsigned char bgra[Side * Side * 4];
};
static_assert(sizeof(Packet) == 4416);
static LONG sequence = 0;

static bool RenderIcon(HICON icon, unsigned char* output) {
    if (!icon) return false;
    BITMAPINFO bitmap = {};
    bitmap.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bitmap.bmiHeader.biWidth = Side;
    bitmap.bmiHeader.biHeight = -Side;
    bitmap.bmiHeader.biPlanes = 1;
    bitmap.bmiHeader.biBitCount = 32;
    bitmap.bmiHeader.biCompression = BI_RGB;
    void* bits = nullptr;
    HDC dc = CreateCompatibleDC(nullptr);
    if (!dc) return false;
    HBITMAP image = CreateDIBSection(dc, &bitmap, DIB_RGB_COLORS, &bits, nullptr, 0);
    if (!image) { DeleteDC(dc); return false; }
    HGDIOBJ previous = SelectObject(dc, image);
    // Render against black and white to recover alpha for legacy mask icons
    // as well as premultiplied-alpha icons (including black monochrome glyphs).
    unsigned char black[Side * Side * 4];
    ZeroMemory(bits, sizeof(black));
    bool ok = DrawIconEx(dc, 0, 0, icon, Side, Side, 0, nullptr, DI_NORMAL) != FALSE;
    GdiFlush();
    CopyMemory(black, bits, sizeof(black));
    FillMemory(bits, sizeof(black), 255);
    ok = ok && DrawIconEx(dc, 0, 0, icon, Side, Side, 0, nullptr, DI_NORMAL) != FALSE;
    GdiFlush();
    if (ok) {
        const auto* white = static_cast<unsigned char*>(bits);
        for (int i = 0; i < Side * Side * 4; i += 4) {
            int difference = 0;
            for (int c = 0; c < 3; ++c) {
                int d = static_cast<int>(white[i+c]) - black[i+c];
                if (d > difference) difference = d;
            }
            int alpha = 255 - difference;
            output[i+3] = static_cast<unsigned char>(alpha);
            for (int c = 0; c < 3; ++c) {
                int straight = alpha ? (static_cast<int>(black[i+c]) * 255 + alpha/2) / alpha : 0;
                output[i+c] = static_cast<unsigned char>(straight > 255 ? 255 : straight);
            }
        }
    }
    SelectObject(dc, previous);
    DeleteObject(image);
    DeleteDC(dc);
    return ok;
}

static void Observe(const CWPSTRUCT* msg) {
    if (!msg || msg->message != WM_COPYDATA) return;
    wchar_t name[32];
    if (!GetClassNameW(msg->hwnd, name, 32) || lstrcmpW(name, L"Shell_TrayWnd")) return;
    HWND receiver = static_cast<HWND>(GetPropW(msg->hwnd, ReceiverProperty));
    if (!receiver || !IsWindow(receiver)) return;
    const auto* data = reinterpret_cast<const COPYDATASTRUCT*>(msg->lParam);
    // Current Windows appends extra data after the stable 956-byte icon
    // structure (1484-byte envelope observed in the native fixture). Validate
    // the prefix; ignore bounded trailing data, never copy it to Bloom.
    if (!data || data->dwData != 1 || !data->lpData ||
        data->cbData < sizeof(ShellMessage) || data->cbData > 4096) return;
    const auto* message = static_cast<const ShellMessage*>(data->lpData);
    if (message->magic != 0x34753423 || message->icon.size != sizeof(WireIcon)) return;
    if (message->operation != NIM_ADD && message->operation != NIM_MODIFY &&
        message->operation != NIM_DELETE && message->operation != NIM_SETVERSION) return;
    const auto& icon = message->icon;
    Packet packet = {};
    packet.protocol = 1;
    packet.sequence = static_cast<uint32_t>(InterlockedIncrement(&sequence));
    packet.operation = message->operation;
    packet.window = icon.window;
    packet.uid = icon.uid;
    packet.flags = icon.flags;
    packet.callback = icon.callback;
    packet.state = icon.state;
    packet.mask = icon.mask;
    packet.version = message->operation == NIM_SETVERSION ? icon.version : 0;
    DWORD ownerPid = 0;
    GetWindowThreadProcessId(reinterpret_cast<HWND>(static_cast<ULONG_PTR>(icon.window)), &ownerPid);
    packet.pid = ownerPid;
    if (icon.flags & NIF_GUID) packet.guid = icon.guid;
    if (icon.flags & NIF_TIP) CopyMemory(packet.tip, icon.tip, sizeof(packet.tip));
    if ((icon.flags & NIF_ICON) && message->operation != NIM_DELETE)
        packet.hasImage = RenderIcon(reinterpret_cast<HICON>(static_cast<ULONG_PTR>(icon.icon)), packet.bgra);
    COPYDATASTRUCT transfer = {PacketType, sizeof(packet), &packet};
    DWORD_PTR delivered = 0;
    // Receiver only copies into a bounded queue. Never wait indefinitely on
    // Bloom (including when suspended/debugged); always continue Explorer.
    SendMessageTimeoutW(receiver, WM_COPYDATA, reinterpret_cast<WPARAM>(msg->hwnd),
        reinterpret_cast<LPARAM>(&transfer), SMTO_ABORTIFHUNG | SMTO_BLOCK, 20, &delivered);
}

extern "C" __declspec(dllexport) LRESULT CALLBACK BloomTrayObserver(int code, WPARAM w, LPARAM l) {
    if (code >= 0) {
        // WM_COPYDATA may be sent by arbitrary software. Unknown layouts and
        // malformed pointers must not crash Explorer.
        __try { Observe(reinterpret_cast<const CWPSTRUCT*>(l)); }
        __except (EXCEPTION_EXECUTE_HANDLER) { }
    }
    return CallNextHookEx(nullptr, code, w, l);
}
