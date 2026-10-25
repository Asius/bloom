import { describe, expect, test } from "bun:test";
import { createNativeTrayAction, trayHeight } from "../src/tray";
test("the tray follows dock height and Bloom scale", () => {
	expect(trayHeight(54, 1.5)).toBe(36);
	expect(trayHeight(32, 1)).toBe(32);
	expect(trayHeight(100, 1)).toBe(44);
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
