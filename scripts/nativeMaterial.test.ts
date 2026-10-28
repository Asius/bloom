import { expect, test } from "bun:test";
import { createMaterialSync, type MaterialStatus } from "../src/nativeMaterial";

test("material failure is reported and repeated frames do not retry", async () => {
    const results: MaterialStatus[] = [];
    let calls = 0;
    const sync = createMaterialSync(async () => { calls++; throw new Error("DWM"); }, s => results.push(s));
    await sync(true);
    await sync(true);
    expect(calls).toBe(1);
    expect(results).toEqual(["unavailable"]);
});

test("obsolete material response cannot overwrite newer preference", async () => {
    const results: MaterialStatus[] = [];
    const calls: boolean[] = [];
    let complete!: (value: MaterialStatus) => void;
    const sync = createMaterialSync(enabled => {
        calls.push(enabled);
        return enabled ? new Promise(resolve => { complete = resolve; }) : Promise.resolve("disabled");
    }, s => results.push(s));
    const first = sync(true);
    await Promise.resolve();
    const latest = sync(false);
    complete("accepted");
    await first;
    await latest;
    expect(calls).toEqual([true, false]);
    expect(results).toEqual(["disabled"]);
});

test("superseded queued requests are skipped and next change can recover", async () => {
    const calls: boolean[] = [];
    const sync = createMaterialSync(async enabled => {
        calls.push(enabled);
        return enabled ? "accepted" : "disabled";
    }, () => {});
    void sync(true);
    await sync(false);
    await sync(true);
    expect(calls).toEqual([false, true]);
});
