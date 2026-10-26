import {
	useCallback,
	useEffect,
	useLayoutEffect,
	useRef,
	useState,
	type CSSProperties
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { message } from "@tauri-apps/plugin-dialog";
import { ChevronUp } from "lucide-react";
import { EdgeSurface } from "./EdgeSurface";
import type { TrayIcon, TrayRect, TraySnapshot } from "../tray";
import "./SystemTray.css";

interface Props {
	scale: number;
	height: number;
	metrics: { x: number; y: number; width: number; height: number; dpi: number } | null;
}

/** Visible icons beside the dock chevron, hidden icons in a bounded flyout. */
export function SystemTray({ scale, height, metrics }: Props) {
	const [snapshot, setSnapshot] = useState<TraySnapshot | null>(null);
	const [open, setOpen] = useState(false);
	const [busy, setBusy] = useState(false);
	const [healthy, setHealthy] = useState(false);
	const [failure, setFailure] = useState("");
	const toggle = useRef<HTMLButtonElement>(null);
	const host = useRef<HTMLSpanElement>(null);
	const panel = useRef<HTMLDivElement>(null);
	const request = useRef<Promise<TraySnapshot | null> | null>(null);
	const lastRefresh = useRef(0);
	const busyRef = useRef(false);
	const mounted = useRef(true);
	const state = useRef({ healthy, open });
	state.current = { healthy, open };
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
		if (busyRef.current) return null;
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
		const timer = setInterval(() => {
			if (Date.now() - lastRefresh.current >= (state.current.open ? 2000 : 10000)) void refresh();
			report();
		}, 2000);
		const dismiss = listen("tray-dismiss", () => {
			if (!busyRef.current) setOpen(false);
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
			if (!busyRef.current) setOpen(false);
		};
		const outside = (event: PointerEvent) => {
			const target = event.target as Node;
			if (!busyRef.current && !host.current?.contains(target) && !panel.current?.contains(target))
				setOpen(false);
		};
		document.addEventListener("pointerdown", outside, true);
		window.addEventListener("blur", blur);
		window.addEventListener("resize", report);
		return () => {
			mounted.current = false;
			clearInterval(timer);
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
		if (busyRef.current) return;
		void invoke("report_tray_input", { control: contextMenu ? "context" : "activate" });
		busyRef.current = true;
		setBusy(true);
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
		} catch (error) {
			await message(String(error), { title: "System Tray", kind: "error" }).catch(console.error);
		} finally {
			busyRef.current = false;
			if (mounted.current) {
				setBusy(false);
				void refresh();
			}
		}
	};
	const show = async () => {
		if (busyRef.current) return;
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
	const promotedRects = candidatesPromoted.flatMap((icon) => icon.native_rect ? [icon.native_rect] : []);
	const preciseLayout =
		!!metrics &&
		!!snapshot &&
		(!!snapshot.native_chevron || promotedRects.length > 0) &&
		promotedRects.length === candidatesPromoted.length;
	const promoted = candidatesPromoted;
	const icons = liveIcons.filter((icon) => !icon.is_promoted);
	const firstIconLeft = promotedRects.length ? Math.min(...promotedRects.map((rect) => rect.x)) : 0;
	const nativeAnchor = snapshot?.native_chevron ?? (promotedRects.length && metrics
		? { x: firstIconLeft - 28 * metrics.dpi, y: 0, width: 25 * metrics.dpi, height: 0 }
		: null);
	const nativeRight = Math.max(
		snapshot?.native_chevron ? snapshot.native_chevron.x + snapshot.native_chevron.width : 0,
		...promotedRects.map((rect) => rect.x + rect.width)
	);
	const dpi = metrics?.dpi ?? 1;
	const visualHeight = height * scale;
	const viewportHeight = metrics ? metrics.height / dpi : window.innerHeight;
	const barTop = viewportHeight - visualHeight;
	const nativeBarStyle: CSSProperties = preciseLayout && metrics && nativeAnchor ? {
		left: (nativeAnchor.x - metrics.x) / dpi,
		top: barTop,
		right: "auto",
		bottom: "auto",
		width: Math.max(25, (nativeRight - nativeAnchor.x) / dpi),
		height: visualHeight,
		zoom: 1
	} : { height, zoom: scale };
	const nativeIconStyle = (rect: TrayRect | null | undefined): CSSProperties | undefined => {
		if (!preciseLayout || !metrics || !nativeAnchor || !rect) return undefined;
		return {
			position: "absolute",
			left: (rect.x - nativeAnchor.x) / dpi,
			top: (rect.y - (metrics.y + barTop * dpi)) / dpi,
			width: rect.width / dpi,
			height: rect.height / dpi
		};
	};
	const toggleRect = snapshot?.native_chevron;
	const toggleStyle: CSSProperties | undefined = preciseLayout && metrics && nativeAnchor
		? toggleRect
			? nativeIconStyle(toggleRect)
			: { position: "absolute", left: 0, top: 0, width: 25, height: "100%" }
		: undefined;
	const columns = Math.min(5, Math.max(1, icons.length));
	const panelRight = preciseLayout && metrics && nativeAnchor && toggleRect
		? Math.max(0, (nativeRight - (toggleRect.x + toggleRect.width)) / dpi)
		: undefined;
	const iconButton = (icon: TrayIcon, index: number, group: TrayIcon[], style?: CSSProperties) => (
		<button
			key={icon.id}
			type="button"
			disabled={busy}
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
		<div
			ref={panel}
			className="bloom-tray-icons"
			role="toolbar"
			aria-label="Hidden notification icons"
			aria-orientation="vertical"
			style={{
				"--tray-columns": columns,
				...(panelRight === undefined ? {} : { right: `${panelRight}px`, left: "auto" })
			} as CSSProperties}
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
		</div>
	);
	return (
		<span
			ref={host}
			className={`bloom-tray bloom-tray-right ${preciseLayout ? "is-native-aligned" : ""} ${open ? "is-open" : ""}`}
			style={nativeBarStyle}
			onClick={(e) => e.stopPropagation()}
			onContextMenu={(e) => {
				e.preventDefault();
				e.stopPropagation();
			}}
		>
			<EdgeSurface edge="bottom" />
			{open && list}
			<button
				ref={toggle}
				className="bloom-tray-toggle"
				type="button"
				aria-label="Show hidden icons"
				aria-expanded={open}
				aria-haspopup="true"
				style={toggleStyle}
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
				<ChevronUp size={16} style={{ transform: open ? "rotate(180deg)" : undefined }} />
			</button>
			<div className="bloom-tray-promoted" role="toolbar" aria-label="Visible notification icons">
				{promoted.map((icon, index) =>
					iconButton(icon, index, promoted, nativeIconStyle(icon.native_rect))
				)}
			</div>
		</span>
	);
}
