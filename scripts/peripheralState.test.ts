import { describe, expect, test } from "bun:test";
import { peripheralNotices, type PeripheralReading } from "../src/peripheralState";
const device = (
	percent: number | null,
	connected: boolean | null = true,
	id = "ghub-mouse"
): PeripheralReading => ({
	id,
	name: "Device",
	kind: "mouse",
	connected,
	percent,
	charging: false,
	approximate: false
});
describe("peripheral transitions", () => {
	test("startup and unchanged battery never display unsolicited alerts", () => {
		expect(peripheralNotices(null, [device(15)])).toEqual([]);
		expect(peripheralNotices([device(15)], [device(15)])).toEqual([]);
		expect(peripheralNotices([device(75)], [device(75)])).toEqual([]);
	});
	test("known connection changes are immediate and do not repeat", () => {
		expect(peripheralNotices([device(null, false)], [device(60)]).map((n) => n.kind)).toEqual([
			"connected"
		]);
		expect(peripheralNotices([device(60)], [device(null, false)]).map((n) => n.kind)).toEqual([
			"disconnected"
		]);
		expect(peripheralNotices([device(60)], [device(60)])).toEqual([]);
	});
	test("unknown is not a proven Logitech connection change", () => {
		expect(peripheralNotices([device(null, null)], [device(60)])).toEqual([]);
		expect(peripheralNotices([device(60)], [device(null, null)])).toEqual([]);
	});
	test("Razer successful response after silence can show reconnection", () => {
		expect(
			peripheralNotices(
				[device(null, null, "razer-headset")],
				[device(60, true, "razer-headset")]
			).map((n) => n.kind)
		).toEqual(["connected"]);
	});
	test("battery notices only on crossing below 20, zero or a reliable 100", () => {
		const latches = new Map();
		expect(peripheralNotices([device(21)], [device(20)], latches)).toEqual([]);
		expect(peripheralNotices([device(20)], [device(19)], latches).map((n) => n.kind)).toEqual([
			"low"
		]);
		expect(peripheralNotices([device(19)], [device(20)], latches)).toEqual([]);
		expect(peripheralNotices([device(20)], [device(19)], latches)).toEqual([]);
		expect(peripheralNotices([device(1)], [device(0)], latches).map((n) => n.kind)).toEqual([
			"empty"
		]);
		expect(peripheralNotices([device(99)], [device(100)], latches).map((n) => n.kind)).toEqual([
			"full"
		]);
		expect(
			peripheralNotices([device(99)], [{ ...device(100), approximate: true }], new Map())
		).toEqual([]);
	});
});
