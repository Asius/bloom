import { describe, expect, test } from "bun:test";
import { createNativeTrayAction, createTrayActionGate, nativeTrayLayout, trayHeight, type TrayIcon, type TraySnapshot } from "../src/tray";
test("the tray follows dock height and Bloom scale", () => {
	expect(trayHeight(54, 1.5)).toBe(36);
	expect(trayHeight(32, 1)).toBe(32);
	expect(trayHeight(52, 1)).toBe(52);
	expect(trayHeight(100, 1)).toBe(96);
});

describe("native tray placement", () => {
	const icon: TrayIcon = { id: "1", name: "Drive", path: "drive.exe", is_promoted: true,
		is_shell_icon: false, icon: null, native_rect: { x: 2380, y: 1408, width: 24, height: 24 } };
	const snapshot: TraySnapshot = { shell_id: "shell", icons: [icon],
		native_chevron: { x: 2340, y: 1408, width: 24, height: 24 } };
	const metrics = { x: 0, y: 990, width: 2560, height: 450, dpi: 1 };
	test("keeps the real icon position within a full-width tray window", () => {
		expect(nativeTrayLayout(snapshot, metrics)).toEqual({ anchor: snapshot.native_chevron, right: 2404, precise: true });
	});
	test("keeps a usable anchored layout when one icon has no native rectangle", () => {
		const missing = { ...snapshot, icons: [{ ...icon, native_rect: null }] };
		expect(nativeTrayLayout(missing, metrics)).toEqual({ anchor: snapshot.native_chevron, right: 2364, precise: false });
	});
	test("rejects coordinates left behind by another screen configuration", () => {
		expect(nativeTrayLayout(snapshot, { ...metrics, width: 1920 })).toBeNull();
	});
	test("uses the safe flow layout when native rectangles overlap", () => {
		const duplicate = { ...snapshot, icons: [icon, { ...icon, id: "2" }] };
		expect(nativeTrayLayout(duplicate, metrics)?.precise).toBe(false);
	});
	test("handles a 4K monitor at 200% DPI with a non-zero origin", () => {
		const rect = { x: 720, y: 2100, width: 48, height: 48 };
		const chevron = { x: 640, y: 2100, width: 48, height: 48 };
		const scaled = { ...snapshot, icons: [{ ...icon, native_rect: rect }], native_chevron: chevron };
		expect(nativeTrayLayout(scaled, { x: -3000, y: 1440, width: 3840, height: 720, dpi: 2 })?.precise).toBe(true);
	});
});

describe("tray action ordering", () => {
	test("delivers a double click after the first click without activating a different icon", () => {
		const gate = createTrayActionGate<{ id: string }>();
		const first = { id: "one" };
		expect(gate.begin(first)).toBe(true);
		expect(gate.begin({ id: "two" }, false, true)).toBe(false);
		expect(gate.begin(first, false, true)).toBe(false);
		expect(gate.finish(true)).toEqual(first);
		expect(gate.isBusy()).toBe(false);
	});
	test("drops a queued double click after failure and permits retry", () => {
		const gate = createTrayActionGate<{ id: string }>();
		const icon = { id: "one" };
		gate.begin(icon);
		gate.begin(icon, false, true);
		expect(gate.finish(false)).toBeNull();
		expect(gate.begin(icon)).toBe(true);
	});
});

describe("native tray control", () => {
	test("repeated clicks do not issue overlapping native requests", async () => {
		let complete!: (result: boolean) => void;
		const pending = new Promise<boolean>((resolve) => {
			complete = resolve;
		});
		let calls = 0;
		const busy: boolean[] = [];
		const open = createNativeTrayAction(
			() => {
				calls++;
				return pending;
			},
			async () => {},
			(value) => busy.push(value)
		);
		const first = open();
		await open();
		expect(calls).toBe(1);
		expect(busy).toEqual([true]);
		complete(true);
		await first;
		expect(busy).toEqual([true, false]);
	});
	test("a missing chevron is reported without trying another native action", async () => {
		const messages: string[] = [];
		let calls = 0;
		await createNativeTrayAction(
			async () => {
				calls++;
				return false;
			},
			async (text, kind) => {
				messages.push(kind + ":" + text);
			},
			() => {}
		)();
		expect(calls).toBe(1);
		expect(messages).toEqual(["info:No hidden-icons button is currently available in Windows."]);
	});
	test("a native failure remains an error and can be retried", async () => {
		let calls = 0;
		const messages: string[] = [];
		const busy: boolean[] = [];
		const open = createNativeTrayAction(
			async () => {
				if (++calls === 1) throw "Explorer restarted";
				return true;
			},
			async (text, kind) => {
				messages.push(kind + ":" + text);
			},
			(value) => busy.push(value)
		);
		await open();
		await open();
		expect(calls).toBe(2);
		expect(messages).toEqual(["error:Explorer restarted"]);
		expect(busy).toEqual([true, false, true, false]);
	});
	test("a failed notification does not leave the control busy or display twice", async () => {
		let notices = 0;
		let calls = 0;
		const busy: boolean[] = [];
		const open = createNativeTrayAction(
			async () => {
				calls++;
				return false;
			},
			async () => {
				notices++;
				throw new Error("dialog unavailable");
			},
			(value) => busy.push(value)
		);
		await expect(open()).rejects.toThrow("dialog unavailable");
		await expect(open()).rejects.toThrow("dialog unavailable");
		expect(calls).toBe(2);
		expect(notices).toBe(2);
		expect(busy).toEqual([true, false, true, false]);
	});
});
