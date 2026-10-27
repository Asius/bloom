/** Hydrate unchanged keys only: events received during a slow load win. */
export function createThemeStartup(
	keys: readonly string[],
	storage: Pick<Storage, "setItem" | "removeItem">,
	apply: () => void
) {
	let active = true;
	const changed = new Set<string>();
	return {
		change(key: string, value: unknown) {
			if (!active || !keys.includes(key)) return;
			changed.add(key);
			if (value == null) storage.removeItem(key);
			else storage.setItem(key, String(value));
			apply();
		},
		async hydrate(settings: Record<string, unknown>, reload: (preserved: ReadonlySet<string>) => Promise<boolean>) {
			if (!active || await reload(changed) || !active) return;
			for (const key of keys) {
				if (!changed.has(key) && settings[key] != null) storage.setItem(key, String(settings[key]));
			}
			apply();
		},
		stop() { active = false; }
	};
}
