import { expect, test } from "bun:test";
import { reconcileNotifications } from "../src/notificationState";
import type { NotificationSnapshot, WindowsNotice } from "../src/hooks/useNotifications";

const item = (id: number): WindowsNotice => ({
	id,
	app: "App",
	title: "Aviso",
	body: "",
	createdMs: id
});
const allowed = (...items: WindowsNotice[]): NotificationSnapshot => ({ status: "allowed", items });

test("la lectura inicial no presenta avisos anteriores como nuevos", () => {
	const initial = reconcileNotifications(null, allowed(item(1)));
	expect(initial.arrival).toBeNull();
	const updated = reconcileNotifications(initial.ids, allowed(item(2), item(1)));
	expect(updated.arrival?.id).toBe(2);
	expect(reconcileNotifications(updated.ids, allowed(item(2), item(1))).arrival).toBeNull();
});

test("revocar el permiso elimina datos anteriores y restablece la primera lectura", () => {
	const initial = reconcileNotifications(null, allowed(item(1)));
	const revoked = reconcileNotifications(initial.ids, { status: "denied", items: [item(1)] });
	expect(revoked.snapshot.items).toEqual([]);
	expect(revoked.ids).toBeNull();
	expect(reconcileNotifications(revoked.ids, allowed(item(2))).arrival).toBeNull();
});
