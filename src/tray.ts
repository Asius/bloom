/** Windows notification icons are distinct from open dock windows. IDs are transient. */
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

/** Keep the tray at the dock's configured height across Bloom scales. */
export function trayHeight(height: number, scale: number) {
	const safeScale = Number.isFinite(scale) && scale > 0 ? scale : 1;
	return Math.max(28, Math.min(height / safeScale || 36, 44));
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
