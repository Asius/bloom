import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { TRAY_PLACEMENT_KEY, trayPlacement, type TrayPlacement } from "../tray";

/** This setting also handles deletion: null/invalid values restore the default. */
export function useTrayPlacement() {
	const [placement, setPlacement] = useState(() =>
		trayPlacement(localStorage.getItem(TRAY_PLACEMENT_KEY))
	);
	const [ready, setReady] = useState(false);
	const [saving, setSaving] = useState(false);
	const savePending = useRef(false);
	const revision = useRef(0);
	useEffect(() => {
		let disposed = false;
		const apply = (value: unknown) => {
			const normalized = trayPlacement(value);
			localStorage.setItem(TRAY_PLACEMENT_KEY, normalized);
			setPlacement(normalized);
		};
		const changed = (event: { payload: { key: string; value: unknown } }) => {
			if (!disposed && event.payload.key === TRAY_PLACEMENT_KEY) {
				revision.current++;
				apply(event.payload.value);
			}
		};
		const listeners = [
			listen("settings-changed", changed),
			listen("settings-external-changed", changed)
		];
		Promise.all(listeners)
			.then(async () => {
				const current = revision.current;
				const settings = await invoke<Record<string, unknown>>("load_settings");
				if (!disposed && revision.current === current) apply(settings[TRAY_PLACEMENT_KEY]);
				if (!disposed) setReady(true);
			})
			.catch(console.error);
		return () => {
			disposed = true;
			listeners.forEach((p) => p.then((f) => f()));
		};
	}, []);
	const save = async (value: TrayPlacement) => {
		if (savePending.current) return;
		savePending.current = true;
		setSaving(true);
		const current = revision.current;
		const normalized = trayPlacement(value);
		try {
			await invoke("save_setting", { key: TRAY_PLACEMENT_KEY, value: normalized });
			if (revision.current === current) {
				localStorage.setItem(TRAY_PLACEMENT_KEY, normalized);
				setPlacement(normalized);
			}
		} finally {
			savePending.current = false;
			setSaving(false);
		}
	};
	return { placement, ready, saving, save };
}
