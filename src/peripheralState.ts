export type PeripheralKind = "mouse" | "keyboard" | "headphones" | "other";

/** A receiver's presence alone never implies that its peripheral is online. */
export interface PeripheralReading {
	id: string;
	name: string;
	kind: PeripheralKind;
	connected: boolean | null;
	percent: number | null;
	approximate: boolean;
	charging: boolean | null;
}

export type PeripheralNotice =
	| { kind: "connected"; device: PeripheralReading }
	| { kind: "low"; device: PeripheralReading; threshold: 20 | 10 };

export interface PeripheralSnapshot {
	devices: PeripheralReading[];
	bluetoothAvailable: boolean;
}

/** Notice on a transition, not on every poll. */
export function peripheralNotices(
	previous: readonly PeripheralReading[] | null,
	current: readonly PeripheralReading[],
	latches: Map<string, 0 | 1 | 2> = new Map()
): PeripheralNotice[] {
	const before = new Map((previous ?? []).map((item) => [item.id, item]));
	const notices: PeripheralNotice[] = [];
	for (const item of current) {
		const old = before.get(item.id);
		if (!item.connected) continue;
		// A responding device after an unavailable reading may have reconnected.
		// The current response confirms it is online, even if the reason for
		// the preceding missing reading was unknown.
		if (previous && (!old || old.connected !== true))
			notices.push({ kind: "connected", device: item });
		if (item.charging) {
			latches.set(item.id, 0);
			continue;
		}
		if (item.percent === null || !Number.isFinite(item.percent)) continue;
		let level = latches.get(item.id) ?? 0;
		if (item.percent > 25) level = 0;
		else if (item.percent > 15 && level === 2) level = 1;
		if (item.percent <= 10 && level < 2) {
			notices.push({ kind: "low", device: item, threshold: 10 });
			level = 2;
		} else if (item.percent <= 20 && level === 0) {
			notices.push({ kind: "low", device: item, threshold: 20 });
			level = 1;
		}
		latches.set(item.id, level);
	}
	return notices;
}
