// Changes are compared with the revision at which each snapshot was requested.
let revision = 0;
const recentSettings = new Map<string, { value: string | null; revision: number }>();
export function settingsRevision(): number { return revision; }
export function noteSettingChange(key: string, value: unknown) {
    recentSettings.set(key, { value: value == null ? null : String(value), revision: ++revision });
}
export function initialSettingValue(key: string, saved: unknown, fallback: string | null, since = 0): string | null {
    const recent = recentSettings.get(key);
    if (recent && recent.revision > since) return recent.value;
    return saved == null ? fallback : String(saved);
}
export function hasRecentSetting(key: string, since = 0): boolean {
    return (recentSettings.get(key)?.revision ?? 0) > since;
}
