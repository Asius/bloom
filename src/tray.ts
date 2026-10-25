/** Shared contract for the two approved tray placements. Action IDs are transient. */
export interface TrayIcon {
	id: string;
	name: string;
	path: string;
	is_promoted: boolean;
	is_shell_icon: boolean;
	icon: string | null;
}

export interface TraySnapshot {
	shell_id: string;
	icons: TrayIcon[];
}

export type TrayPlacement = "notch" | "right";
export const TRAY_PLACEMENT_KEY = "bloom-tray-placement";
export const trayPlacement = (value: unknown): TrayPlacement =>
	value === "right" ? "right" : "notch";

/** Do not invent a persisted key when settings.json uses the default. */
export function mirrorTrayPlacement(
	value: unknown,
	storage: Pick<Storage, "setItem" | "removeItem">
): TrayPlacement {
	if (value === null || value === undefined) storage.removeItem(TRAY_PLACEMENT_KEY);
	else storage.setItem(TRAY_PLACEMENT_KEY, String(value));
	return trayPlacement(value);
}

/** Keep the strip's viewport within the compact dock's configured dimensions. */
export function trayStripSize(width: number, height: number, scale: number) {
	const safeScale = Number.isFinite(scale) && scale > 0 ? scale : 1;
	return {
		width: Math.max(28, Math.min(width / safeScale || 140, 248)),
		height: Math.max(28, Math.min(height / safeScale || 36, 44))
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
