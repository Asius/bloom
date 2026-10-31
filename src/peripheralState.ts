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
	| { kind: "connected" | "disconnected"; device: PeripheralReading }
	| { kind: "low" | "empty" | "full"; device: PeripheralReading };

export interface PeripheralSnapshot {
	devices: PeripheralReading[];
	bluetoothAvailable: boolean;
	ghubAvailable?: boolean;
}

/** Only observable transitions produce notices; startup and unknown values are silent. */
export function peripheralNotices(
	previous: readonly PeripheralReading[] | null,
	current: readonly PeripheralReading[],
	latches: Map<string, "low" | "empty" | "full" | null> = new Map()
): PeripheralNotice[] {
	if (!previous) return [];
	const before = new Map(previous.map((item) => [item.id, item]));
	const notices: PeripheralNotice[] = [];
	for (const item of current) {
		const old = before.get(item.id);
		if (!old) continue;
		if (old.connected !== null && item.connected !== null && old.connected !== item.connected) {
			notices.push({ kind: item.connected ? "connected" : "disconnected", device: item });
		} else if (item.id === "razer-headset" && old.connected === null && item.connected === true) {
			// A Razer battery response proves the link is active. Silence does not prove it is off.
			notices.push({ kind: "connected", device: item });
		}
		if (
			old.connected !== true ||
			item.connected !== true ||
			old.percent === null ||
			item.percent === null
		)
			continue;
		let latch = latches.get(item.id) ?? null;
		if (item.percent > 25 && item.percent < 95) latch = null;
		if (item.percent === 0 && old.percent > 0 && !item.approximate && latch !== "empty") {
			notices.push({ kind: "empty", device: item });
			latch = "empty";
		} else if (
			item.percent < 20 &&
			old.percent >= 20 &&
			!item.charging &&
			latch !== "low" &&
			latch !== "empty"
		) {
			notices.push({ kind: "low", device: item });
			latch = "low";
		} else if (item.percent === 100 && old.percent < 100 && !item.approximate && latch !== "full") {
			notices.push({ kind: "full", device: item });
			latch = "full";
		}
		latches.set(item.id, latch);
	}
	return notices;
}
