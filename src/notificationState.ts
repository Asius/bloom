import type { NotificationSnapshot, WindowsNotice } from "./hooks/useNotifications";

/** A first successful read establishes a baseline; only later additions alert. */
export function reconcileNotifications(
	previous: Set<number> | null,
	snapshot: NotificationSnapshot
): {
	ids: Set<number> | null;
	arrival: WindowsNotice | null;
	snapshot: NotificationSnapshot;
} {
	if (snapshot.status !== "allowed") {
		return { ids: null, arrival: null, snapshot: { status: snapshot.status, items: [] } };
	}
	return {
		ids: new Set(snapshot.items.map((item) => item.id)),
		arrival: previous ? (snapshot.items.find((item) => !previous.has(item.id)) ?? null) : null,
		snapshot
	};
}
