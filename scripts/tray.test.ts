import { describe, expect, test } from "bun:test";
import {
	createNativeTrayAction,
	createTrayActionGate,
	nativeTrayLayout,
	trayBarGeometry,
	nativeTrayCellWidth,
	trayHeight,
	traySurfaceHeight,
	createTrayPlacement,
	DEFAULT_TRAY_HEIGHT,
	type TrayIcon,
	type TraySnapshot
} from "../src/tray";
test("the tray follows dock height and Bloom scale", () => {
	expect(trayHeight(54, 1.5)).toBe(36);
	expect(trayHeight(32, 1)).toBe(32);
	expect(trayHeight(52, 1)).toBe(52);
	expect(trayHeight(100, 1)).toBe(96);
});

test("the tray surface stays slightly shorter than the dock and aligns to its base", () => {
	expect(traySurfaceHeight(48, 1)).toBe(42);
	expect(traySurfaceHeight(72, 1.5)).toBe(42);
	expect(traySurfaceHeight(32, 1)).toBe(32);
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
	test("uses compact cells, reserves network and audio, and keeps a stable right edge", () => {
		const normal = trayBarGeometry(3, 1, metrics, 2500);
		expect(normal.width).toBe(192);
		expect(normal.left + normal.width).toBe(2500);
		const fewer = trayBarGeometry(1, 1, metrics, 2500);
		expect(fewer.left + fewer.width).toBe(2500);
		const large = trayBarGeometry(3, 1.3, metrics, 2500);
		expect(large.left + large.width * 1.3).toBeLessThanOrEqual(2556);
		const crowded = trayBarGeometry(50, 1, metrics, 2500);
		expect(crowded.width).toBe(1152);
		expect(crowded.left + crowded.width).toBeLessThanOrEqual(2556);
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

test("native tray spacing ignores rows, invalid gaps and physical DPI", () => {
 const metrics = { x:0, y:0, width:3840, height:2160, dpi:2 };
 const icon = (x:number,y=2100): TrayIcon => ({ id:String(x),name:"fixture",path:"fixture.exe",is_promoted:true,is_shell_icon:false,icon:null,native_rect:{x,y,width:32,height:32} });
 expect(nativeTrayCellWidth({shell_id:"fixture",icons:[icon(100),icon(164),icon(228)]},metrics)).toBe(32);
 expect(nativeTrayCellWidth({shell_id:"fixture",icons:[icon(100),icon(164,2000)]},metrics)).toBe(28);
 expect(nativeTrayCellWidth(null,metrics)).toBe(28);
});


describe("stable tray metrics", () => {
 const metrics = { x:0, y:1000, width:2560, height:440, dpi:1 };
 const icon = (x:number): TrayIcon => ({id:String(x),name:"fixture",path:"fixture.exe",is_promoted:true,is_shell_icon:false,icon:null,native_rect:{x,y:1400,width:20,height:20}});
 const initial: TraySnapshot = {shell_id:"session-1",icons:[icon(2360),icon(2388),icon(2416)],native_chevron:{x:2320,y:1400,width:24,height:24}};
 test("dock and tray share the fallback surface height", () => {
  expect(DEFAULT_TRAY_HEIGHT).toBe(traySurfaceHeight(48,1));
  expect(traySurfaceHeight(48,1)).toBe(traySurfaceHeight(72,1.5));
 });
 test("late native spacing and transient anchors cannot move the visible tray", () => {
  const resolve = createTrayPlacement();
  const first = resolve({...initial,icons:initial.icons.map(i=>({...i,native_rect:null}))}, metrics,1,3);
  const next = resolve({...initial,icons:[icon(2360),icon(2392),icon(2424)],native_chevron:{x:2280,y:1400,width:24,height:24}},metrics,1,3);
  expect(next).toEqual(first);
 });
 test("late native anchors do not move the safe fallback", () => {
  const resolve = createTrayPlacement();
  const first = resolve({...initial,icons:[],native_chevron:null},metrics,1,0);
  expect(resolve({...initial,icons:[]},metrics,1,0)).toEqual(first);
 });
 test("icon removal keeps the right edge and cell spacing", () => {
  const resolve = createTrayPlacement();
  const first = resolve(initial,metrics,1,3);
  const fewer = resolve({...initial,icons:[icon(2360)]},metrics,1,1);
  expect(fewer.left + fewer.width).toBe(first.left + first.width);
  expect(fewer.cellWidth).toBe(first.cellWidth);
 });
 test("a new Explorer session or explicit scale change recalibrates placement", () => {
  const resolve = createTrayPlacement();
  resolve(initial,metrics,1,3);
  const updated = {...initial,shell_id:"session-2",icons:[icon(2360),icon(2392),icon(2424)]};
  expect(resolve(updated,metrics,1,3).cellWidth).toBe(32);
  const scaled=resolve(updated,metrics,1.2,3);
  expect(scaled.left + scaled.width * 1.2).toBeLessThanOrEqual(2556);
 });
 test("monitor and DPI changes discard old native coordinates", () => {
  const resolve = createTrayPlacement();
  resolve(initial,metrics,1,3);
  const changed=resolve(initial,{x:-3840,y:1440,width:3840,height:720,dpi:2},1,3);
  expect(changed.left).toBeGreaterThanOrEqual(0);
  expect(changed.left+changed.width).toBeLessThanOrEqual(1916);
 });
});
