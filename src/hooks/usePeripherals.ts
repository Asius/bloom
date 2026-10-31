import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import {
	peripheralNotices,
	type PeripheralNotice,
	type PeripheralReading,
	type PeripheralSnapshot
} from "../peripheralState";

export function usePeripherals(enabled: boolean, canPresent: boolean) {
	const [devices, setDevices] = useState<PeripheralReading[]>([]);
	const [error, setError] = useState<string | null>(null);
	const [notice, setNotice] = useState<PeripheralNotice | null>(null);
	const [loading, setLoading] = useState(false);
	const [bluetoothAvailable, setBluetoothAvailable] = useState(true);
	const refreshRef = useRef<() => void>(() => {});
	const latches = useRef(new Map<string, 0 | 1 | 2>());
	const previous = useRef<PeripheralReading[] | null>(null);
	const pending = useRef<PeripheralNotice[]>([]);
	useEffect(() => {
		if (!enabled) return;
		let cancelled = false;
		let polling = false;
		let timer: ReturnType<typeof setTimeout>;
		const poll = async () => {
			if (polling) return;
			polling = true;
			clearTimeout(timer);
			setLoading(true);
			try {
				const snapshot = await invoke<PeripheralSnapshot>("peripheral_batteries");
				const next = snapshot.devices;
				if (cancelled) return;
				setDevices(next);
				setBluetoothAvailable(snapshot.bluetoothAvailable);
				setError(null);
				const stillApplies = (item: PeripheralNotice) => {
					const fresh = next.find((device) => device.id === item.device.id);
					return (
						!!fresh?.connected &&
						(item.kind === "connected" ||
							(!fresh.charging && fresh.percent !== null && fresh.percent <= item.threshold))
					);
				};
				pending.current = pending.current.filter(stillApplies);
				pending.current.push(...peripheralNotices(previous.current, next, latches.current));
				const retained = !snapshot.bluetoothAvailable
					? (previous.current ?? []).filter((item) => item.id.startsWith("bluetooth-"))
					: [];
				previous.current = [...next, ...retained];
				setNotice((current) =>
					current && stillApplies(current) ? current : (pending.current.shift() ?? null)
				);
			} catch {
				if (!cancelled) {
					pending.current = [];
					setNotice(null);
					setDevices([]);
					setError("No se pudieron consultar los dispositivos.");
				}
			} finally {
				polling = false;
				if (!cancelled) {
					setLoading(false);
					timer = setTimeout(poll, 30_000);
				}
			}
		};
		refreshRef.current = () => {
			void poll();
		};
		void poll();
		return () => {
			cancelled = true;
			refreshRef.current = () => {};
			clearTimeout(timer);
		};
	}, [enabled]);
	useEffect(() => {
		if (!notice || !canPresent) return;
		const timer = setTimeout(() => setNotice(pending.current.shift() ?? null), 6000);
		return () => clearTimeout(timer);
	}, [notice, canPresent]);
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
