import { expect, test } from "bun:test";
import { accentForeground } from "../src/themeColors";

test("accent foreground stays readable for light, dark and malformed accents", () => {
	expect(accentForeground("#0067c0")).toBe("#ffffff");
	expect(accentForeground("#60cdff")).toBe("#000000");
	expect(accentForeground("#ffffff")).toBe("#000000");
	expect(accentForeground("#000000")).toBe("#ffffff");
	expect(accentForeground("invalid")).toBe("#ffffff");
});
