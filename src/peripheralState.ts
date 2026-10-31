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
	| { kind: "disconnected"; device: PeripheralReading }
	| { kind: "low"; device: PeripheralReading; threshold: 20 | 10 };

export interface PeripheralSnapshot {
	devices: PeripheralReading[];
	bluetoothAvailable: boolean;
	ghubAvailable?: boolean;
}

export interface ConfirmedState { value: boolean | null; candidate: boolean | null; count: number }

/** Notice on a transition, not on every poll. */
export function peripheralNotices(
	previous: readonly PeripheralReading[] | null,
	current: readonly PeripheralReading[],
	latches: Map<string, 0 | 1 | 2> = new Map(),
	states: Map<string, ConfirmedState> = new Map()
): PeripheralNotice[] {
	const before = new Map((previous ?? []).map((item) => [item.id, item]));
	const notices: PeripheralNotice[] = [];
	for (const item of current) {
		const old = before.get(item.id);
		const state = states.get(item.id) ?? { value: null, candidate: null, count: 0 };
		if (item.connected === null) {
			state.candidate = null;
			state.count = 0;
		} else {
			if (state.candidate === item.connected) state.count++;
			else { state.candidate = item.connected; state.count = 1; }
			if (state.count >= 2 && state.value !== item.connected) {
				if (state.value !== null && previous && old)
					notices.push({ kind: item.connected ? "connected" : "disconnected", device: item });
				state.value = item.connected;
			}
		}
		states.set(item.id, state);
		if (!item.connected) continue;
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
