import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { reconcileNotifications } from "../notificationState";

export type WindowsNotice = {
	id: number;
	app: string;
	title: string;
	body: string;
	createdMs: number;
	appId?: string | null;
};
export type NotificationStatus =
	| "loading"
	| "disabled"
	| "allowed"
	| "permission"
	| "denied"
	| "unavailable"
	| "error";
export type NotificationSnapshot = { status: NotificationStatus; items: WindowsNotice[] };

export function useNotifications(enabled: boolean) {
	const [snapshot, setSnapshot] = useState<NotificationSnapshot>({ status: "loading", items: [] });
	const [consent, setConsent] = useState<boolean | null>(null);
	const consentRef = useRef<boolean | null>(null);
	const preferenceRevision = useRef(0);
	const [preferenceBusy, setPreferenceBusy] = useState(false);
	const [preferenceError, setPreferenceError] = useState<{
		message: string;
		retryValue: boolean;
	} | null>(null);
	const [actionError, setActionError] = useState<string | null>(null);
	const [notice, setNotice] = useState<WindowsNotice | null>(null);
	const ids = useRef<Map<number, string> | null>(null);
	const busy = useRef(false);
	const saving = useRef(false);
	const active = useRef(false);
	const generation = useRef(0);
	const mutationRevision = useRef(0);
	const dismissTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
	const dismiss = useCallback(() => {
		if (dismissTimer.current) clearTimeout(dismissTimer.current);
		setNotice(null);
	}, []);
	const applyPreference = useCallback(
		(value: unknown) => {
			const optedIn = value === "true";
			if (!optedIn) {
				active.current = false;
				generation.current++;
				ids.current = null;
				dismiss();
				setSnapshot({ status: "disabled", items: [] });
			} else if (consentRef.current !== true) {
				setSnapshot({ status: "loading", items: [] });
			}
			consentRef.current = optedIn;
			setConsent(optedIn);
		},
		[dismiss]
	);
	const refresh = useCallback(async () => {
		if (busy.current || !active.current) return;
		busy.current = true;
		const current = generation.current;
		const mutationAtStart = mutationRevision.current;
		try {
			const next = await invoke<NotificationSnapshot>("notification_snapshot");
			if (
				!active.current ||
				current !== generation.current ||
				mutationAtStart !== mutationRevision.current
			)
				return;
			const reconciled = reconcileNotifications(ids.current, next);
			ids.current = reconciled.ids;
			if (next.status === "allowed") {
				const newItem = reconciled.arrival;
				if (newItem) {
					setNotice(newItem);
					if (dismissTimer.current) clearTimeout(dismissTimer.current);
					dismissTimer.current = setTimeout(() => setNotice(null), 4000);
				} else {
					setNotice((old) => (old && next.items.some((item) => item.id === old.id) ? old : null));
				}
			} else {
				dismiss();
			}
			setSnapshot(reconciled.snapshot);
		} catch {
			if (
				!active.current ||
				current !== generation.current ||
				mutationAtStart !== mutationRevision.current
			)
				return;
			ids.current = null;
			dismiss();
			setSnapshot({ status: "error", items: [] });
		} finally {
			busy.current = false;
		}
	}, [dismiss]);
	useEffect(() => {
		if (!enabled) return;
		let cancelled = false;
		const changed = (event: { payload: { key: string; value: unknown } }) => {
			if (cancelled || event.payload.key !== "bloom-notifications-enabled") return;
			preferenceRevision.current++;
			applyPreference(event.payload.value);
		};
		const listeners = [
			listen<{ key: string; value: unknown }>("settings-changed", changed),
			listen<{ key: string; value: unknown }>("settings-external-changed", changed)
		];
		Promise.all(listeners)
			.then(async () => {
				const startedAt = preferenceRevision.current;
				const settings = await invoke<Record<string, unknown>>("load_settings");
				if (!cancelled && preferenceRevision.current === startedAt)
					applyPreference(settings["bloom-notifications-enabled"]);
			})
			.catch(() => {
				if (!cancelled) {
					applyPreference(false);
					setPreferenceError({
						message: "No se pudo cargar la preferencia de notificaciones.",
						retryValue: false
					});
				}
			});
		return () => {
			cancelled = true;
			listeners.forEach((listener) => {
				void listener.then((unlisten) => unlisten()).catch(() => {});
			});
		};
	}, [enabled, applyPreference]);
	useEffect(() => {
		if (!enabled || consent !== true) return;
		active.current = true;
		generation.current++;
		void refresh();
		const timer = setInterval(() => void refresh(), 5000);
		return () => {
			active.current = false;
			generation.current++;
			clearInterval(timer);
			dismiss();
		};
	}, [enabled, consent, refresh, dismiss]);
	const setActive = useCallback(
		async (value: boolean) => {
			if (saving.current) return;
			saving.current = true;
			setPreferenceBusy(true);
			if (!value) {
				applyPreference(false);
			}
			const startedAt = preferenceRevision.current;
			try {
				await invoke("save_setting", { key: "bloom-notifications-enabled", value: String(value) });
				setPreferenceError(null);
				if (preferenceRevision.current === startedAt) applyPreference(String(value));
			} catch {
				setPreferenceError({
					message: value
						? "No se pudo activar."
						: "La pausa no se guardó; vuelve a intentarlo antes de reiniciar Bloom.",
					retryValue: value
				});
			} finally {
				saving.current = false;
				setPreferenceBusy(false);
			}
		},
		[applyPreference]
	);
	const requestAccess = useCallback(async () => {
		try {
			await invoke("request_notification_access");
		} catch {
			/* Refresh displays the actual state. */
		}
		await refresh();
	}, [refresh]);
	const removeNotifications = useCallback(async (notificationIds: number[]) => {
		if (notificationIds.length === 0) return;
		mutationRevision.current++;
		try {
			await invoke("remove_notifications", { ids: notificationIds });
			const removed = new Set(notificationIds);
			ids.current = ids.current
				? new Map([...ids.current].filter(([id]) => !removed.has(id)))
				: null;
			setSnapshot((current) => ({
				...current,
				items: current.items.filter((item) => !removed.has(item.id))
			}));
			setNotice((current) => (current && removed.has(current.id) ? null : current));
			setActionError(null);
		} catch {
			setActionError("No se pudo quitar la notificación de Windows.");
		}
	}, []);
	const clearAll = useCallback(async () => {
		mutationRevision.current++;
		try {
			await invoke("clear_notifications");
			ids.current = new Map();
			setSnapshot((current) => ({ ...current, items: [] }));
			dismiss();
			setActionError(null);
		} catch {
			setActionError("No se pudieron borrar las notificaciones de Windows.");
		}
	}, [dismiss]);
	const openApp = useCallback(async (notificationId: number) => {
		try {
			await invoke("open_notification_app", { id: notificationId });
			setActionError(null);
		} catch {
			setActionError("Windows no pudo abrir esta aplicación desde la notificación.");
		}
	}, []);
	return {
		...snapshot,
		notice,
		dismiss,
		refresh,
		requestAccess,
		preferenceError,
		preferenceBusy,
		setActive,
		removeNotifications,
		clearAll,
		openApp,
		actionError
	};
}
