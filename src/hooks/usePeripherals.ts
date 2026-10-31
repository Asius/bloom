import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import {
	peripheralNotices,
	type PeripheralNotice,
	type PeripheralReading,
	type PeripheralSnapshot
} from "../peripheralState";

export function usePeripherals(enabled: boolean) {
	const [devices, setDevices] = useState<PeripheralReading[]>([]);
	const [error, setError] = useState<string | null>(null);
	const [notice, setNotice] = useState<PeripheralNotice | null>(null);
	const [loading, setLoading] = useState(false);
	const [bluetoothAvailable, setBluetoothAvailable] = useState(true);
	const refreshRef = useRef<() => void>(() => {});
	const previous = useRef<PeripheralReading[] | null>(null);
	const latches = useRef(new Map<string, "low" | "empty" | "full" | null>());
	const pending = useRef<PeripheralNotice[]>([]);
	useEffect(() => {
		if (!enabled) return;
		let cancelled = false;
		let slowBusy = false;
		let fastBusy = false;
		let slowTimer: ReturnType<typeof setTimeout>;
		let fastTimer: ReturnType<typeof setTimeout>;
		let otherDevices: PeripheralReading[] = [];
		let slowGhub: PeripheralReading[] = [];
		let fastGhub: PeripheralReading[] | null | undefined;
		const apply = () => {
			const ghub = fastGhub === undefined ? slowGhub : fastGhub;
			const next = [
				...(ghub ?? []),
				...otherDevices.filter((d) => ghub === null || !d.id.startsWith("logitech-"))
			];
			const applies = (item: PeripheralNotice) => {
				const fresh = next.find((d) => d.id === item.device.id);
				if (!fresh) return false;
				if (item.kind === "disconnected") return fresh.connected === false;
				if (!fresh.connected) return false;
				if (item.kind === "connected") return true;
				if (fresh.percent === null) return false;
				if (item.kind === "low") return fresh.percent < 20 && !fresh.charging;
				if (item.kind === "empty") return fresh.percent === 0 && !fresh.approximate;
				return fresh.percent === 100 && !fresh.approximate;
			};
			pending.current = pending.current.filter(applies);
			pending.current.push(...peripheralNotices(previous.current, next, latches.current));
			previous.current = next;
			setDevices(next);
			setNotice((current) =>
				current && applies(current) ? current : (pending.current.shift() ?? null)
			);
		};
		const pollFast = async () => {
			if (fastBusy) return;
			fastBusy = true;
			clearTimeout(fastTimer);
			try {
				const next = await invoke<PeripheralReading[] | null>("ghub_batteries");
				if (!cancelled) {
					fastGhub = next;
					apply();
				}
			} catch {
				if (!cancelled) {
					fastGhub = null;
					apply();
				}
			} finally {
				fastBusy = false;
				if (!cancelled) fastTimer = setTimeout(pollFast, 2000);
			}
		};
		const pollSlow = async () => {
			if (slowBusy) return;
			slowBusy = true;
			clearTimeout(slowTimer);
			setLoading(true);
			try {
				const snapshot = await invoke<PeripheralSnapshot>("peripheral_batteries");
				if (cancelled) return;
				slowGhub = snapshot.devices.filter((d) => d.id.startsWith("ghub-"));
				otherDevices = snapshot.devices.filter((d) => !d.id.startsWith("ghub-"));
				setBluetoothAvailable(snapshot.bluetoothAvailable);
				setError(null);
				apply();
			} catch {
				if (!cancelled) setError("No se pudieron consultar todos los dispositivos.");
			} finally {
				slowBusy = false;
				if (!cancelled) {
					setLoading(false);
					slowTimer = setTimeout(pollSlow, 8000);
				}
			}
		};
		refreshRef.current = () => {
			void pollFast();
			void pollSlow();
		};
		void pollFast();
		void pollSlow();
		return () => {
			cancelled = true;
			refreshRef.current = () => {};
			clearTimeout(slowTimer);
			clearTimeout(fastTimer);
		};
	}, [enabled]);
	useEffect(() => {
		if (!notice) return;
		const timer = setTimeout(() => setNotice(pending.current.shift() ?? null), 10_000);
		return () => clearTimeout(timer);
	}, [notice]);
	return {
		devices,
		error,
		notice,
		loading,
		bluetoothAvailable,
		refresh: () => refreshRef.current(),
		dismiss: () => setNotice(pending.current.shift() ?? null)
	};
}
