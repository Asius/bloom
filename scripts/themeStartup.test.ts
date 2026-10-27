import { noteSettingChange, initialSettingValue, settingsRevision } from "../src/initialSettings";
import { expect, test } from "bun:test";
import { createThemeStartup } from "../src/themeStartup";

const keys = ["mode", "opacity"];
function fixture() {
	const values = new Map<string, string>([["mode", "dark"]]);
	let applications = 0;
	const startup = createThemeStartup(keys, {
		setItem: (key, value) => { values.set(key, value); },
		removeItem: key => { values.delete(key); }
	}, () => { applications++; });
	return { values, startup, applications: () => applications };
}

test("a late initial snapshot preserves a newer user theme and loads untouched keys", async () => {
	const f = fixture();
	f.startup.change("mode", "light");
	await f.startup.hydrate({ mode: "dark", opacity: "0.75" }, async () => false);
	expect(f.values.get("mode")).toBe("light");
	expect(f.values.get("opacity")).toBe("0.75");
});

test("external deletion during an awaited mirror check cannot resurrect the old value", async () => {
	const f = fixture();
	let release!: (value: boolean) => void;
	let preserved!: ReadonlySet<string>;
	const pending = f.startup.hydrate({ mode: "dark" }, async keys => {
		preserved = keys;
		return new Promise<boolean>(resolve => { release = resolve; });
	});
	f.startup.change("mode", null);
	expect(preserved.has("mode")).toBe(true);
	release(false);
	await pending;
	expect(f.values.has("mode")).toBe(false);
});

test("unmounted theme initialization cannot apply a delayed snapshot", async () => {
	const f = fixture();
	let release!: (value: boolean) => void;
	const pending = f.startup.hydrate({ mode: "light" }, () => new Promise(resolve => { release = resolve; }));
	f.startup.stop();
	release(false);
	await pending;
	f.startup.change("mode", "custom");
	expect(f.values.get("mode")).toBe("dark");
	expect(f.applications()).toBe(0);
});

test("a requested mirror reload prevents stale hydration", async () => {
	const f = fixture();
	await f.startup.hydrate({ mode: "light" }, async () => true);
	expect(f.values.get("mode")).toBe("dark");
	expect(f.applications()).toBe(0);
});


test("concurrent settings readers prefer recent saves and deletions to old snapshots", () => {
 noteSettingChange("test-mode", "light");
 expect(initialSettingValue("test-mode", "dark", "custom")).toBe("light");
 noteSettingChange("test-mode", null);
 expect(initialSettingValue("test-mode", "dark", "custom")).toBeNull();
 expect(initialSettingValue("untouched-opacity", "0.8", "0.92")).toBe("0.8");
 expect(initialSettingValue("missing-key", undefined, "fallback")).toBe("fallback");
});


test("a later explicit reload accepts imported settings instead of retaining old edits forever", () => {
 noteSettingChange("import-mode", "light");
 const revision = settingsRevision();
 expect(initialSettingValue("import-mode", "dark", "custom", revision)).toBe("dark");
 noteSettingChange("import-mode", "adaptive");
 expect(initialSettingValue("import-mode", "dark", "custom", revision)).toBe("adaptive");
});
