import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import {
	peripheralNotices,
	type PeripheralNotice,
	type PeripheralReading,
	type PeripheralSnapshot,
	type ConfirmedState
} from "../peripheralState";

export function usePeripherals(enabled: boolean) {
	const [devices, setDevices] = useState<PeripheralReading[]>([]);
	const [error, setError] = useState<string | null>(null);
	const [notice, setNotice] = useState<PeripheralNotice | null>(null);
	const [loading, setLoading] = useState(false);
	const [bluetoothAvailable, setBluetoothAvailable] = useState(true);
	const refreshRef = useRef<() => void>(() => {});
	const latches = useRef(new Map<string, 0 | 1 | 2>());
	const previous = useRef<PeripheralReading[] | null>(null);
	const states = useRef(new Map<string, ConfirmedState>());
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
				// A source disappearing is not evidence that a device disconnected.
				for (const [id, state] of states.current) {
					if (!next.some((device) => device.id === id)) {
						state.candidate = null;
						state.count = 0;
					}
				}
				setBluetoothAvailable(snapshot.bluetoothAvailable);
				setError(null);
				const stillApplies = (item: PeripheralNotice) => {
					const fresh = next.find((device) => device.id === item.device.id);
					return (
						(item.kind === "disconnected" ? fresh?.connected === false : !!fresh?.connected &&
						(item.kind === "connected" ||
							(!fresh.charging && fresh.percent !== null && fresh.percent <= item.threshold))
						)
					);
				};
				pending.current = pending.current.filter(stillApplies);
				pending.current.push(...peripheralNotices(previous.current, next, latches.current, states.current));
				setDevices(next.map((device) => device.connected === false && states.current.get(device.id)?.value !== false
					? { ...device, connected: null, percent: null }
					: device));
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
					timer = setTimeout(poll, 15_000);
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
