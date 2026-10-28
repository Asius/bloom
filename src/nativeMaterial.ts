export type MaterialStatus = "accepted" | "disabled" | "unsupported-surface" | "unavailable";

/** Serialize UI-thread requests; a stale completion must not change the fallback. */
export function createMaterialSync(
    configure: (enabled: boolean) => Promise<MaterialStatus>,
    publish: (status: MaterialStatus) => void
) {
    let desired: boolean | undefined;
    let revision = 0;
    let pending = Promise.resolve();
    return (enabled: boolean) => {
        if (desired === enabled) return pending;
        desired = enabled;
        const request = ++revision;
        pending = pending.then(async () => {
            if (request !== revision) return;
            let status: MaterialStatus;
            try { status = await configure(enabled); }
            catch { status = "unavailable"; }
            if (request === revision) publish(status);
        });
        return pending;
    };
}
