import { surfaceRevealMotion } from "../revealMotion";
import {
	useCallback,
	useEffect,
	useLayoutEffect,
	useRef,
	useState,
	type CSSProperties
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { message } from "@tauri-apps/plugin-dialog";
import { AnimatePresence, motion } from "framer-motion";
import { ChevronUp, Globe, Monitor, Wifi, Volume1, Volume2, VolumeX } from "lucide-react";
import { EdgeSurface } from "./EdgeSurface";
import {
	createTrayActionGate,
	createTrayPlacement,
	type TrayIcon,
	type TraySnapshot,
	type TrayWindowMetrics
} from "../tray";
import "./SystemTray.css";

interface Props {
	scale: number;
	height: number;
	metrics: TrayWindowMetrics | null;
}

/** Visible icons beside the dock chevron, hidden icons in a bounded flyout. */
export function SystemTray({ scale, height, metrics }: Props) {
	const [snapshot, setSnapshot] = useState<TraySnapshot | null>(null);
	const [open, setOpen] = useState(false);
	const [busy, setBusy] = useState(false);
	const [hovered, setHovered] = useState(false);
	const [surfaceVisible, setSurfaceVisible] = useState(false);
	const [healthy, setHealthy] = useState(false);
	const [failure, setFailure] = useState("");
	const [network, setNetwork] = useState<{
		connected: boolean;
		internet: boolean;
		wifi: boolean | null;
	} | null>(null);
	const [volume, setVolume] = useState<{ volume: number; is_muted: boolean } | null>(null);
	const placement = useRef(createTrayPlacement());
	const toggle = useRef<HTMLButtonElement>(null);
	const host = useRef<HTMLSpanElement>(null);
	const panel = useRef<HTMLDivElement>(null);
	const request = useRef<Promise<TraySnapshot | null> | null>(null);
	const lastRefresh = useRef(0);
	const actionGate = useRef(createTrayActionGate<TrayIcon>());
	const mounted = useRef(true);
	const state = useRef({ healthy, open });
	state.current = { healthy, open };
	const interaction = useRef(false);
	interaction.current = hovered || open || busy;
	useEffect(() => {
		void emit("tray-interaction", interaction.current);
	}, [hovered, open, busy]);
	useEffect(() => {
		const request = listen("tray-interaction-request", () => {
			void emit("tray-interaction", interaction.current);
		});
		const hide = listen("dock-tray-hidden", () => {
			setHovered(false);
			setOpen(false);
		});
		const visibility = listen<boolean>("tray-presentation-visibility", (event) =>
			setSurfaceVisible(event.payload)
		);
		void visibility.then(() => emit("dock-tray-visibility-request"));
		return () => {
			void request.then((unlisten) => unlisten());
			void hide.then((unlisten) => unlisten());
			void visibility.then((unlisten) => unlisten());
			void emit("tray-interaction", false);
		};
	}, []);
	const nativeHeight = useRef<number | null>(null);
	nativeHeight.current = snapshot?.native_taskbar_height ?? null;
	useEffect(() => {
		if (nativeHeight.current !== null) void emit("native-taskbar-height", nativeHeight.current);
	}, [snapshot?.native_taskbar_height]);
	useEffect(() => {
		let cancelled = false;
		let receivedEvent = false;
		const subscription = listen<{ volume: number; is_muted: boolean }>("volume-change", (event) => {
			receivedEvent = true;
			if (!cancelled) setVolume(event.payload);
		});
		void subscription
			.then(() => invoke<{ volume: number; is_muted: boolean }>("get_volume_state"))
			.then((value) => {
				if (!cancelled && !receivedEvent) setVolume(value);
			})
			.catch(console.error);
		return () => {
			cancelled = true;
			void subscription.then((unlisten) => unlisten());
		};
	}, []);
	useEffect(() => {
		const requestHeight = listen("native-taskbar-height-request", () => {
			if (nativeHeight.current !== null) void emit("native-taskbar-height", nativeHeight.current);
		});
		return () => {
			void requestHeight.then((unlisten) => unlisten());
		};
	}, []);
	const report = useCallback(() => {
		const rects = [host.current, panel.current].filter(Boolean).map((el) => {
			const r = el!.getBoundingClientRect();
			// Keep the bottom flare and flyout edge inside the interactive region.
			const pad = 13 * scale;
			return {
				x: Math.floor(r.x - pad),
				y: Math.floor(r.y - pad),
				width: Math.ceil(r.right + pad) - Math.floor(r.x - pad),
				height: Math.ceil(r.bottom + pad) - Math.floor(r.y - pad)
			};
		});
		void invoke("update_tray_surface", {
			rects,
			ready: state.current.healthy,
			open: state.current.open
		}).catch(console.error);
	}, [scale]);
	const refresh = useCallback(async () => {
		if (actionGate.current.isBusy()) return null;
		if (request.current) return request.current;
		lastRefresh.current = Date.now();
		const loading = (async () => {
			try {
				const next = await invoke<TraySnapshot>("get_tray_icons");
				if (mounted.current) {
					setSnapshot(next);
					setHealthy(true);
					setFailure("");
					if (!next.icons.some((icon) => !icon.is_promoted)) setOpen(false);
				}
				return next;
			} catch (error) {
				if (mounted.current) {
					setHealthy(false);
					setFailure(String(error));
					setOpen(false);
				}
				return null;
			}
		})();
		request.current = loading;
		try {
			return await loading;
		} finally {
			request.current = null;
		}
	}, []);
	useEffect(() => {
		mounted.current = true;
		void refresh();
		let checkingNetwork = false;
		let cancelled = false;
		const refreshNetwork = () => {
			if (checkingNetwork) return;
			checkingNetwork = true;
			void invoke<typeof network>("tray_network_status")
				.then((value) => {
					if (!cancelled) setNetwork(value);
				})
				.catch(() => {
					if (!cancelled) setNetwork(null);
				})
				.finally(() => {
					checkingNetwork = false;
				});
		};
		refreshNetwork();
		const networkTimer = setInterval(refreshNetwork, 5000);
		const timer = setInterval(() => {
			if (Date.now() - lastRefresh.current >= (state.current.open ? 2000 : 10000)) void refresh();
			if (interaction.current) void emit("tray-interaction", true);
			report();
		}, 2000);
		const dismiss = listen("tray-dismiss", () => {
			if (!actionGate.current.isBusy()) setOpen(false);
		});
		let changedAgain = false;
		let receivingChanges = false;
		const changes = listen("tray-icons-changed", async () => {
			changedAgain = true;
			if (receivingChanges) return;
			receivingChanges = true;
			try {
				do {
					changedAgain = false;
					// A change may arrive after an in-flight snapshot was read.
					// Wait for it, then request the updated state instead of losing it.
					if (request.current) await request.current;
					if (mounted.current) await refresh();
				} while (changedAgain && mounted.current);
			} finally {
				receivingChanges = false;
			}
		});
		const blur = () => {
			if (!actionGate.current.isBusy()) setOpen(false);
		};
		const outside = (event: PointerEvent) => {
			const target = event.target as Node;
			if (
				!actionGate.current.isBusy() &&
				!host.current?.contains(target) &&
				!panel.current?.contains(target)
			)
				setOpen(false);
		};
		document.addEventListener("pointerdown", outside, true);
		window.addEventListener("blur", blur);
		window.addEventListener("resize", report);
		return () => {
			cancelled = true;
			mounted.current = false;
			clearInterval(timer);
			clearInterval(networkTimer);
			dismiss.then((f) => f());
			changes.then((f) => f());
			window.removeEventListener("blur", blur);
			document.removeEventListener("pointerdown", outside, true);
			window.removeEventListener("resize", report);
			void invoke("update_tray_surface", { rects: [], ready: false, open: false });
		};
	}, [refresh, report]);
	useLayoutEffect(() => {
		report();
	});
	useEffect(() => {
		if (!open) return;
		const raf = requestAnimationFrame(() =>
			panel.current?.querySelector<HTMLButtonElement>("button")?.focus()
		);
		return () => cancelAnimationFrame(raf);
	}, [open]);
	const action = async (icon?: TrayIcon, contextMenu = false, doubleClick = false) => {
		if (!actionGate.current.begin(icon, contextMenu, doubleClick)) return;
		void invoke("report_tray_input", { control: contextMenu ? "context" : "activate" });
		setBusy(true);
		let succeeded = false;
		// Native left clicks may intentionally do nothing until a double click.
		// Keep the flyout alive; activation/menu focus will dismiss it on blur.
		if (contextMenu || !icon?.id.startsWith("live:")) setOpen(false);
		try {
			if (icon) {
				await invoke(contextMenu ? "show_tray_context_menu" : "activate_tray_icon", {
					trayId: icon.id,
					doubleClick
				});
			} else {
				const opened = await invoke<boolean>("open_system_tray");
				if (!opened)
					await message("Windows has no hidden-icons button available.", {
						title: "System Tray",
						kind: "info"
					}).catch(console.error);
			}
			succeeded = true;
		} catch (error) {
			await message(String(error), { title: "System Tray", kind: "error" }).catch(console.error);
		} finally {
			const double = actionGate.current.finish(succeeded);
			if (mounted.current) {
				setBusy(false);
				if (succeeded && double) void action(double, false, true);
				else void refresh();
			}
		}
	};
	const show = async () => {
		if (actionGate.current.isBusy()) return;
		void invoke("report_tray_input", { control: "toggle" });
		if (open) {
			setOpen(false);
			return;
		}
		// A known catalog can open immediately. Explorer refreshes in the
		// background rather than holding the chevron closed during discovery.
		if (healthy && snapshot?.icons.some((icon) => !icon.is_promoted)) {
			setOpen(true);
			void refresh();
			return;
		}
		const next = await refresh();
		if (!mounted.current) return;
		if (next?.icons.some((icon) => !icon.is_promoted)) setOpen(true);
		else
			await message(
				next
					? "No hidden notification icons found."
					: "Windows tray discovery is unavailable. Right-click the tray button to access Windows tray.",
				{ title: "System Tray", kind: next ? "info" : "error" }
			).catch(console.error);
	};
	const liveIcons = healthy ? (snapshot?.icons ?? []) : [];
	const candidatesPromoted = liveIcons.filter((icon) => icon.is_promoted);
	const promoted = candidatesPromoted;
    const icons = liveIcons.filter((icon) => !icon.is_promoted);
    const geometry = metrics && healthy && snapshot
        ? placement.current(snapshot, metrics, scale, promoted.length)
        : null;
    const cellWidth = geometry?.cellWidth ?? 28;
	const nativeBarStyle: CSSProperties & { "--tray-row-cell"?: string } = geometry
		? {
				"--tray-row-cell": `${cellWidth}px`,
				left: geometry.left,
				right: "auto",
				bottom: 0,
				width: geometry.width,
				height,
				transform: `scale(${scale})`,
				transformOrigin: "bottom left",
				maxWidth: "none"
			}
		: { height, zoom: scale, transformOrigin: "bottom left" };
	const columns = Math.min(5, Math.max(1, icons.length));
	const panelRight = geometry ? Math.max(0, geometry.width - 32) : undefined;
	const iconButton = (icon: TrayIcon, index: number, group: TrayIcon[], style?: CSSProperties) => (
		<button
			key={icon.id}
			type="button"
			className="bloom-tray-icon"
			style={style}
			aria-label={`${icon.name}${group.filter((i) => i.path === icon.path).length > 1 ? ` (${index + 1})` : ""}`}
			title={`${icon.tooltip || icon.name} — right-click for its native menu`}
			onClick={(e) => {
				e.stopPropagation();
				if (e.detail <= 1) void action(icon);
			}}
			onDoubleClick={(e) => {
				e.stopPropagation();
				void action(icon, false, true);
			}}
			onFocus={(e) => e.currentTarget.scrollIntoView({ block: "nearest", inline: "nearest" })}
			onContextMenu={(e) => {
				e.preventDefault();
				e.stopPropagation();
				void action(icon, true);
			}}
			onKeyDown={(e) => {
				if ((e.shiftKey && e.key === "F10") || e.key === "ContextMenu") {
					e.preventDefault();
					e.stopPropagation();
					void action(icon, true);
				}
			}}
		>
			{icon.icon ? (
				<img src={icon.icon} alt="" draggable={false} />
			) : (
				<span aria-hidden="true">{icon.name.slice(0, 1)}</span>
			)}
		</button>
	);
	const list = (
		<motion.div
			key="tray-flyout"
			ref={panel}
			initial={{ opacity: 0, y: 8, scale: 0.97 }}
			animate={{ opacity: 1, y: 0, scale: 1 }}
			exit={{ opacity: 0, y: 8, scale: 0.97 }}
			transition={{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }}
			className="bloom-tray-icons"
			role="toolbar"
			aria-label="Hidden notification icons"
			aria-orientation="vertical"
			style={
				{
					"--tray-columns": columns,
					...(panelRight === undefined ? {} : { right: `${panelRight}px`, left: "auto" })
				} as CSSProperties
			}
			onWheel={(e) => {
				e.stopPropagation();
				// The overflow scrolls vertically, like the Windows hidden-icons panel.
			}}
			onKeyDown={(e) => {
				if (e.key === "Escape") {
					e.stopPropagation();
					setOpen(false);
					toggle.current?.focus();
				}
				const step =
					e.key === "ArrowRight"
						? 1
						: e.key === "ArrowLeft"
							? -1
							: e.key === "ArrowDown"
								? columns
								: e.key === "ArrowUp"
									? -columns
									: 0;
				if (step) {
					e.preventDefault();
					e.stopPropagation();
					const buttons = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>("button"));
					const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
					buttons[(index + step + buttons.length) % buttons.length]?.focus();
				}
			}}
		>
			{icons.map((icon, index) => iconButton(icon, index, icons))}
		</motion.div>
	);
	return (
		<span
			ref={host}
			className={`bloom-tray bloom-tray-right ${open ? "is-open" : ""}`}
			aria-busy={busy}
			style={nativeBarStyle}
			onMouseEnter={() => setHovered(true)}
			onMouseLeave={() => setHovered(false)}
			onClick={(e) => e.stopPropagation()}
			onContextMenu={(e) => {
				e.preventDefault();
				e.stopPropagation();
			}}
		>
			<motion.span
				className="bloom-tray-content"
				initial={{ y: 100, opacity: 0 }}
				animate={{ y: surfaceVisible ? 0 : 100, opacity: surfaceVisible ? 1 : 0 }}
				transition={{
					y: surfaceRevealMotion,
					opacity: { type: "tween", duration: 0.2 }
				}}
			>
				<EdgeSurface edge="bottom" />
				<AnimatePresence initial={false} onExitComplete={report}>
					{open && list}
				</AnimatePresence>
				<button
					ref={toggle}
					className="bloom-tray-toggle"
					type="button"
					aria-label="Show hidden icons"
					aria-expanded={open}
					aria-haspopup="true"
					disabled={busy}
					title={failure || "Show hidden icons — right-click for Windows tray"}
					onClick={(e) => {
						e.stopPropagation();
						void show();
					}}
					onContextMenu={(e) => {
						e.preventDefault();
						e.stopPropagation();
						void action();
					}}
					onKeyDown={(e) => {
						if (e.key === "Escape") {
							e.stopPropagation();
							setOpen(false);
						}
					}}
				>
					<ChevronUp size={14} style={{ transform: open ? "rotate(180deg)" : undefined }} />
				</button>
				<div className="bloom-tray-promoted" role="toolbar" aria-label="Visible notification icons">
					{promoted.map((icon, index) => iconButton(icon, index, promoted))}
				</div>
				<button
					type="button"
					className="bloom-tray-icon bloom-tray-network"
					aria-label="Network and Quick Settings"
					title={
						network
							? network.internet
								? "Network: Internet connected"
								: network.connected
									? "Network: no Internet"
									: "Network: disconnected"
							: "Network status unavailable"
					}
					onClick={() => {
						setOpen(false);
						void invoke("open_tray_network", { settings: false }).catch((error) =>
							message(String(error), { title: "Network", kind: "error" })
						);
					}}
					onContextMenu={(e) => {
						e.preventDefault();
						e.stopPropagation();
						setOpen(false);
						void invoke("open_tray_network", { settings: true }).catch((error) =>
							message(String(error), { title: "Network", kind: "error" })
						);
					}}
				>
					{network?.internet && network.wifi !== null ? (
						network.wifi ? (
							<Wifi size={16} />
						) : (
							<Monitor size={16} />
						)
					) : (
						<Globe size={16} />
					)}
				</button>
				<button
					type="button"
					className="bloom-tray-icon bloom-tray-volume"
					aria-label="Sound and Quick Settings"
					title={
						volume
							? volume.is_muted
								? "Sound: muted"
								: `Sound: ${Math.round(volume.volume * 100)}%`
							: "Sound settings"
					}
					onClick={() => {
						setOpen(false);
						void invoke("open_tray_network", { settings: false }).catch((error) =>
							message(String(error), { title: "Sound", kind: "error" })
						);
					}}
					onContextMenu={(e) => {
						e.preventDefault();
						e.stopPropagation();
						setOpen(false);
						void invoke("open_sound_settings");
					}}
					onKeyDown={(e) => {
						if ((e.shiftKey && e.key === "F10") || e.key === "ContextMenu") {
							e.preventDefault();
							void invoke("open_sound_settings");
						}
					}}
				>
					{volume?.is_muted ? (
						<VolumeX size={16} />
					) : volume && volume.volume < 0.5 ? (
						<Volume1 size={16} />
					) : (
						<Volume2 size={16} />
					)}
				</button>
			</motion.span>
		</span>
	);
}
