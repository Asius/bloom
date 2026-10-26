import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { readScales, SCALE_KEYS, type ScaleKey, type ScaleSettings } from "../scales";

function localValues(): Record<string, unknown> {
	return Object.fromEntries(SCALE_KEYS.map((key) => [key, localStorage.getItem(key)]));
}

/** Read the same settings in each webview and apply live changes and resets. */
export function useDesktopScales(): ScaleSettings {
	const [values, setValues] = useState<Record<string, unknown>>(localValues);
	useEffect(() => {
		let cancelled = false;
		const changes: Record<string, unknown> = {};
		const onChange = ({ payload }: { payload: { key: string; value: unknown } }) => {
			if (!SCALE_KEYS.includes(payload.key as ScaleKey)) return;
			changes[payload.key] = payload.value;
			if (payload.value == null) localStorage.removeItem(payload.key);
			else localStorage.setItem(payload.key, String(payload.value));
			setValues((current) => ({ ...current, [payload.key]: payload.value }));
		};
		const listeners = Promise.all([
			listen<{ key: string; value: unknown }>("settings-changed", onChange),
			listen<{ key: string; value: unknown }>("settings-external-changed", onChange)
		]);
		void listeners.then(async () => {
			if (cancelled) return;
			try {
				const saved = await invoke<Record<string, unknown>>("load_settings");
				const readable = Object.keys(saved).length > 0 || await invoke<boolean>("settings_file_readable").catch(() => false);
				const initial = readable
					? Object.fromEntries(SCALE_KEYS.map((key) => [key, saved[key] ?? null]))
					: { ...localValues(), ...saved };
				if (!cancelled) setValues({ ...initial, ...changes });
			} catch (error) {
				console.error("Could not load display scales:", error);
			}
		});
		return () => {
			cancelled = true;
			void listeners.then((unlisten) => unlisten.forEach((stop) => stop()));
		};
	}, []);
	return readScales(values);
}

export function useBloomFontScale(scale: number) {
	useEffect(() => {
		document.documentElement.style.setProperty("--bloom-font-scale", String(scale));
	}, [scale]);
}
