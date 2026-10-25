import { describe, expect, test } from "bun:test";
import { createNativeTrayAction, trayPlacement, trayStripSize } from "../src/tray";

test("missing, deleted and invalid placement values restore the notch", () => {
	for (const value of [null, undefined, "", "off", "bottom"])
		expect(trayPlacement(value)).toBe("notch");
	expect(trayPlacement("right")).toBe("right");
});
test("the strip follows dock dimensions and does not grow with icon count", () => {
	expect(trayStripSize(210, 54, 1.5)).toEqual({ width: 140, height: 36 });
	expect(trayStripSize(120, 32, 1)).toEqual({ width: 120, height: 32 });
	expect(trayStripSize(1600, 100, 1)).toEqual({ width: 248, height: 44 });
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
