import type { NotificationSnapshot, WindowsNotice } from "./hooks/useNotifications";

export type NotificationGroup = {
	key: string;
	app: string;
	sender: string;
	items: WindowsNotice[];
	latest: WindowsNotice;
};

/** Windows may update an existing notification ID when a conversation changes. */
export function reconcileNotifications(
	previous: Map<number, string> | null,
	snapshot: NotificationSnapshot
): {
	ids: Map<number, string> | null;
	arrival: WindowsNotice | null;
	snapshot: NotificationSnapshot;
} {
	if (snapshot.status !== "allowed") {
		return { ids: null, arrival: null, snapshot: { status: snapshot.status, items: [] } };
	}
	const ids = new Map(snapshot.items.map((item) => [item.id, notificationFingerprint(item)]));
	return {
		ids,
		arrival: previous
			? (snapshot.items.find((item) => previous.get(item.id) !== notificationFingerprint(item)) ??
				null)
			: null,
		snapshot
	};
}

function notificationFingerprint(item: WindowsNotice): string {
	return `${item.app}\u001f${item.title}\u001f${item.body}\u001f${item.createdMs}`;
}

/** Group only matching app/sender pairs; untitled alerts remain independent. */
export function groupNotifications(items: WindowsNotice[]): NotificationGroup[] {
	const groups = new Map<string, NotificationGroup>();
	const newestFirst = [...items].sort((a, b) => b.createdMs - a.createdMs || b.id - a.id);
	for (const item of newestFirst) {
		const app = item.app.trim() || "Aplicación";
		const sender = item.title.trim();
		const key = sender
			? `${app.toLowerCase()}\u001f${sender.toLowerCase()}`
			: `${app.toLowerCase()}\u001f#${item.id}`;
		const group = groups.get(key);
		if (group) group.items.push(item);
		else groups.set(key, { key, app, sender, items: [item], latest: item });
	}
	return [...groups.values()];
}
