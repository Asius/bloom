/** Windows notification icons are distinct from open dock windows. IDs are transient. */
export interface TrayIcon {
	id: string;
	name: string;
	path: string;
	is_promoted: boolean;
	native_rect?: TrayRect | null;
	is_shell_icon: boolean;
	icon: string | null;
	icon_source?: "live" | "cached" | "unavailable";
	tooltip?: string | null;
}

export interface TrayRect {
	x: number;
	y: number;
	width: number;
	height: number;
}

export interface TraySnapshot {
	shell_id: string;
	icons: TrayIcon[];
	native_chevron?: TrayRect | null;
	native_taskbar_height?: number | null;
}

export interface TrayWindowMetrics {
	x: number;
	y: number;
	width: number;
	height: number;
	dpi: number;
}

/** Keep native placement only while the anchor belongs to the current HWND. */
export function nativeTrayLayout(snapshot: TraySnapshot | null, metrics: TrayWindowMetrics | null) {
	if (!snapshot || !metrics || !Number.isFinite(metrics.dpi) || metrics.dpi <= 0) return null;
	const promoted = snapshot.icons.filter((icon) => icon.is_promoted);
	const rects = promoted.flatMap((icon) => icon.native_rect ? [icon.native_rect] : []);
	const firstLeft = rects.length ? Math.min(...rects.map((rect) => rect.x)) : null;
	const anchor = snapshot.native_chevron ?? (firstLeft === null ? null : {
		x: firstLeft - 28 * metrics.dpi, y: 0, width: 25 * metrics.dpi, height: 0
	});
	if (!anchor || anchor.x < metrics.x || anchor.x + anchor.width > metrics.x + metrics.width)
		return null;
	if (snapshot.native_chevron && (anchor.y < metrics.y || anchor.y + anchor.height > metrics.y + metrics.height))
		return null;
	const right = Math.max(anchor.x + anchor.width, ...rects.map((rect) => rect.x + rect.width));
	const ordered = [...rects].sort((a, b) => a.x - b.x);
	const precise = rects.length === promoted.length && rects.every((rect) =>
		rect.width > 0 && rect.height > 0 && rect.x >= anchor.x + anchor.width &&
		rect.x + rect.width <= metrics.x + metrics.width && rect.y >= metrics.y &&
		rect.y + rect.height <= metrics.y + metrics.height
	) && ordered.every((rect, index) => index === 0 || ordered[index - 1].x + ordered[index - 1].width <= rect.x);
	return { anchor, right, precise };
}

/** Keep the tray at the dock's configured height across Bloom scales. */
export function trayHeight(height: number, scale: number) {
	const safeScale = Number.isFinite(scale) && scale > 0 ? scale : 1;
	return Math.max(32, Math.min(height / safeScale || 48, 96));
}

/** A 32px dock-sized cell per control; clip and scroll only the app group. */
export function trayBarGeometry(iconCount: number, scale: number, metrics: TrayWindowMetrics, anchor: TrayRect | undefined) {
	const dpi = Number.isFinite(metrics.dpi) && metrics.dpi > 0 ? metrics.dpi : 1;
	const factor = Number.isFinite(scale) && scale > 0 ? scale : 1;
	const viewport = metrics.width / dpi;
	const width = Math.min(12 + 32 * (Math.max(0, iconCount) + 2) + 6, Math.max(76, viewport * .45 / factor));
	const desired = anchor ? (anchor.x - metrics.x) / dpi : viewport - width * factor - 13;
	const left = Math.max(0, Math.min(desired, viewport - width * factor - 4));
	return { width, left };
}

/** A double click may arrive while the first callback is still completing. */
export function createTrayActionGate<T extends { id: string }>() {
	let busy = false;
	let activeId: string | null = null;
	let pendingDouble: T | null = null;
	return {
		isBusy: () => busy,
		begin(icon?: T, contextMenu = false, doubleClick = false): boolean {
			if (busy) {
				if (icon && doubleClick && !contextMenu && activeId === icon.id) pendingDouble = icon;
				return false;
			}
			busy = true;
			activeId = icon?.id ?? null;
			return true;
		},
		finish(success: boolean): T | null {
			busy = false;
			activeId = null;
			const next = success ? pendingDouble : null;
			pendingDouble = null;
			return next;
		}
	};
}

type NoticeKind = "info" | "error";

/** One native request at a time; all exit paths release the control for retry. */
export function createNativeTrayAction(
	open: () => Promise<boolean>,
	notify: (text: string, kind: NoticeKind) => Promise<unknown>,
	setBusy: (busy: boolean) => void
) {
	let busy = false;
	return async () => {
		if (busy) return;
		busy = true;
		setBusy(true);
		try {
			let opened: boolean;
			try {
				opened = await open();
			} catch (error) {
				const text =
					error instanceof Error
						? error.message
						: typeof error === "string"
							? error
							: "Windows could not open the system tray. Please try again.";
				await notify(text, "error");
				return;
			}
			if (!opened) {
				await notify("No hidden-icons button is currently available in Windows.", "info");
			}
		} finally {
			busy = false;
			setBusy(false);
		}
	};
}
