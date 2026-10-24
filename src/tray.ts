/** Shared contract for the two approved tray placements. Action IDs are transient. */
export interface TrayIcon {
	id: string;
	name: string;
	path: string;
	is_promoted: boolean;
	is_shell_icon: boolean;
}

export interface TraySnapshot {
	shell_id: string;
	icons: TrayIcon[];
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
