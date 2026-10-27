import { describe, expect, test } from "bun:test";
import { createVisibilityPublisher, dockShouldHide } from "../src/dockVisibility";

describe("dock and tray visibility", () => {
	test("Smart hides for overlap and shows during tray or dock interaction", () => {
		expect(dockShouldHide(false, "smart", true, "none")).toBe(true);
		expect(dockShouldHide(false, "smart", true, "active")).toBe(false);
		expect(dockShouldHide(false, "smart", true, "grace")).toBe(false);
		expect(dockShouldHide(false, "smart", false, "none")).toBe(false);
		expect(dockShouldHide(false, "smart", false, "none", true)).toBe(true);
		expect(dockShouldHide(false, "smart", false, "active", true)).toBe(false);
	});
	test("fixed stays visible, Peek hides without interaction and startup can finish", () => {
		expect(dockShouldHide(false, "fixed", true, "none")).toBe(false);
		expect(dockShouldHide(false, "fixed", false, "none", true)).toBe(false);
		expect(dockShouldHide(false, "peek", false, "none")).toBe(true);
		expect(dockShouldHide(false, "peek", false, "active")).toBe(false);
		expect(dockShouldHide(true, "smart", true, "none")).toBe(false);
	});
	test("a delayed hide cannot arrive after a newer reveal", async () => {
		let release!: () => void;
		const first = new Promise<void>(resolve => { release = resolve; });
		const calls: boolean[] = [];
		const publisher = createVisibilityPublisher(async visible => {
			calls.push(visible);
			if (calls.length === 1) await first;
		});
		publisher.set(false);
		publisher.set(true);
		expect(calls).toEqual([false]);
		release();
		await new Promise(resolve => setTimeout(resolve, 0));
		expect(calls).toEqual([false, true]);
	});
});
