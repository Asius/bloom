import { describe, expect, test } from "bun:test";
import { isUpdateActive, isUpdateTerminal, updateErrorMessage, updateLabel } from "../src/updater";

describe("update failure recovery", () => {
	test("check and install failures never claim the app is up to date", () => {
		expect(updateLabel("check-error", "3.10.3")).toBe("Could not check for updates");
		expect(updateLabel("install-error", "3.10.3")).toBe("Could not install update");
		expect(updateLabel("uptodate", "3.10.3")).toBe("Bloom is up to date");
	});
	test("both success and failure dismiss the update overlay", () => {
		for (const status of ["done", "error"]) {
			expect(isUpdateTerminal(status)).toBe(true);
			expect(isUpdateActive(status)).toBe(false);
		}
		for (const status of ["checking", "downloading", "installing"]) {
			expect(isUpdateActive(status)).toBe(true);
			expect(isUpdateTerminal(status)).toBe(false);
		}
	});
	test("a completed attempt leaves the update action available for retry", () => {
		for (const status of ["idle", "available", "uptodate", "check-error", "install-error"]) {
			expect(isUpdateActive(status)).toBe(false);
		}
	});
	test("keeps meaningful native error messages and provides a safe fallback", () => {
		expect(updateErrorMessage("network disconnected")).toBe("network disconnected");
		expect(updateErrorMessage(new Error("invalid signature"))).toBe("invalid signature");
		expect(updateErrorMessage(undefined)).toBe("Please try again.");
		expect(updateErrorMessage({ private: "details" })).toBe("Please try again.");
	});
});
