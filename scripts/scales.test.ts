import { describe, expect, test } from "bun:test";
import { effectiveScale, readScales } from "../src/scales";
describe("island display scales", () => {
	test("starts at 100% and scales text independently", () => {
		expect(readScales({})).toEqual({ notch: 1, font: 1 });
		const scales = readScales({ "bloom-notch-scale": "1.2", "bloom-font-scale": "0.9" });
		expect(effectiveScale(scales)).toBe(1.2);
		expect(scales.font).toBe(0.9);
	});
	test("rejects damaged settings and bounds the controls", () => {
		expect(readScales({ "bloom-notch-scale": "NaN", "bloom-font-scale": true })).toEqual({
			notch: 1,
			font: 1
		});
		expect(readScales({ "bloom-notch-scale": "0x11", "bloom-font-scale": "Infinity" })).toEqual({
			notch: 1,
			font: 1
		});
		expect(readScales({ "bloom-notch-scale": "4", "bloom-font-scale": "-1" })).toEqual({
			notch: 1.3,
			font: 0.8
		});
	});
});
