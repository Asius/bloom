import { describe, expect, test } from "bun:test";
import { effectiveScale, readScales } from "../src/scales";

describe("desktop scales", () => {
	test("preserves an existing shared scale for both surfaces", () => {
		const settings = readScales({ "bloom-scale": "1.2" });
		expect(settings.linked).toBe(true);
		expect(effectiveScale(settings, "dock")).toBe(1.2);
		expect(effectiveScale(settings, "notch")).toBe(1.2);
		expect(settings.font).toBe(1);
	});
	test("separate surfaces fall back to the shared value until configured", () => {
		const settings = readScales({ "bloom-scale": "0.9", "bloom-scale-linked": "false", "bloom-dock-scale": "1.3", "bloom-font-scale": "1.2" });
		expect(effectiveScale(settings, "dock")).toBe(1.3);
		expect(effectiveScale(settings, "notch")).toBe(0.9);
		expect(settings.font).toBe(1.2);
	});
	test("rejects non-finite values and bounds valid scales", () => {
		const settings = readScales({ "bloom-scale": "NaN", "bloom-scale-linked": "false", "bloom-dock-scale": "4", "bloom-notch-scale": "-1", "bloom-font-scale": "Infinity" });
		expect(settings.shared).toBe(1);
		expect(effectiveScale(settings, "dock")).toBe(1.3);
		expect(effectiveScale(settings, "notch")).toBe(0.8);
		expect(settings.font).toBe(1);
	});
	test("does not interpret boolean or hexadecimal imports as a scale", () => {
		const settings = readScales({ "bloom-scale": "1.1", "bloom-scale-linked": false, "bloom-dock-scale": true, "bloom-notch-scale": "0x11" });
		expect(effectiveScale(settings, "dock")).toBe(1.1);
		expect(effectiveScale(settings, "notch")).toBe(1.1);
	});
});
