import { expect, test } from "bun:test";
import { DEFAULT_STATUS_WIDGETS, normalizeStatusWidgets } from "../src/statusWidgets";

test("upgrading desktop settings removes retired and unknown widgets before rendering", () => {
	expect(
		normalizeStatusWidgets({ left: ["weather", "battery"], right: ["battery", "cpu", "unknown"] })
	).toEqual({ left: ["weather"], right: ["cpu"] });
});
test("malformed and duplicate imported widgets cannot crash the configuration view", () => {
	expect(normalizeStatusWidgets({ left: null, right: [] })).toEqual(DEFAULT_STATUS_WIDGETS);
	expect(
		normalizeStatusWidgets({ left: ["cpu", "cpu", "ram", "disk"], right: ["cpu", "net", 7] })
	).toEqual({ left: ["cpu", "ram"], right: ["net"] });
});
