import { describe, expect, test } from "bun:test";
import { peripheralNotices, type PeripheralReading } from "../src/peripheralState";

const mouse = (
	percent: number | null,
	connected = true,
	charging: boolean | null = false
): PeripheralReading => ({
	id: "mouse",
	name: "G PRO",
	kind: "mouse",
	connected,
	percent,
	charging,
	approximate: false
});

describe("peripheral notices", () => {
	test("initial detection does not produce a burst of connection notices", () => {
		expect(peripheralNotices(null, [mouse(50)])).toEqual([]);
	});
	test("reports connection and each low threshold once", () => {
		const latches = new Map<string, 0 | 1 | 2>();
		const states = new Map();
        peripheralNotices(null, [mouse(null, false)], latches, states);
        peripheralNotices([mouse(null, false)], [mouse(null, false)], latches, states);
        expect(peripheralNotices([mouse(null, false)], [mouse(60)], latches, states)).toEqual([]);
        expect(peripheralNotices([mouse(60)], [mouse(60)], latches, states).map(x => x.kind)).toEqual(["connected"]);
		expect(peripheralNotices([mouse(21)], [mouse(20)], latches)[0]?.kind).toBe("low");
		expect(peripheralNotices([mouse(20)], [mouse(19)], latches)).toEqual([]);
		expect(peripheralNotices([mouse(11)], [mouse(9)], latches)[0]?.kind).toBe("low");
		expect(peripheralNotices([mouse(9)], [mouse(11)], latches)).toEqual([]);
		expect(peripheralNotices([mouse(11)], [mouse(9)], latches)).toEqual([]);
	});
	test("reports low battery at startup and rearms after recovery", () => {
		const latches = new Map<string, 0 | 1 | 2>();
		expect(peripheralNotices(null, [mouse(15)], latches)[0]?.kind).toBe("low");
		expect(peripheralNotices([mouse(15)], [mouse(26)], latches)).toEqual([]);
		expect(peripheralNotices([mouse(26)], [mouse(20)], latches)[0]?.kind).toBe("low");
	});
	test("a device becomes available after an unknown status", () => {
		const unknown = { ...mouse(null), connected: null };
		const states = new Map();
        peripheralNotices(null, [unknown], new Map(), states);
        expect(peripheralNotices([unknown], [mouse(55)], new Map(), states)).toEqual([]);
        expect(peripheralNotices([mouse(55)], [mouse(55)], new Map(), states)).toEqual([]);
	});
	test("transient unknown is silent; a confirmed disconnect is emitted once", () => {
        const states = new Map(); const latch = new Map(); const active = mouse(50);
        const unknown = {...active, connected: null}; const off = mouse(null, false);
        peripheralNotices(null, [active], latch, states);
        peripheralNotices([active], [active], latch, states);
        expect(peripheralNotices([active], [unknown], latch, states)).toEqual([]);
        expect(peripheralNotices([unknown], [active], latch, states)).toEqual([]);
        expect(peripheralNotices([active], [off], latch, states)).toEqual([]);
        expect(peripheralNotices([off], [off], latch, states).map(x => x.kind)).toEqual(["disconnected"]);
        expect(peripheralNotices([off], [off], latch, states)).toEqual([]);
    });
    test("unknown, disconnected and charging values never produce a low warning", () => {
		expect(peripheralNotices([mouse(30)], [mouse(null)])).toEqual([]);
		expect(peripheralNotices([mouse(30)], [mouse(5, false)])).toEqual([]);
		expect(peripheralNotices([mouse(30)], [mouse(5, true, true)])).toEqual([]);
	});
});
