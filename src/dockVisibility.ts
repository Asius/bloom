export type DockInteraction = "active" | "grace" | "none";

/** Both desktop surfaces follow the dock's existing Smart/Peek decision. */
export function dockShouldHide(
	startup: boolean,
	mode: string,
	overlapped: boolean,
	interaction: DockInteraction,
	maximized = false
): boolean {
	return !startup && ((mode === "smart" && (overlapped || maximized) && interaction === "none") ||
		(mode === "peek" && interaction === "none"));
}

/** Serialize IPC writes so a slow hide cannot arrive after a newer reveal. */
export function createVisibilityPublisher(send: (visible: boolean) => Promise<unknown>) {
	let desired: boolean | null = null;
	let running = false;
	return {
		set(visible: boolean) {
			desired = visible;
			if (running) return;
			running = true;
			void (async () => {
				try {
					while (desired !== null) {
						const next = desired;
						desired = null;
						try { await send(next); } catch (error) { console.error("Could not synchronize tray visibility:", error); }
					}
				} finally { running = false; }
			})();
		}
	};
}
