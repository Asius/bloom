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
import type { TrayIcon, TraySnapshot } from "../tray";
import "./SystemTray.css";

interface Props {
	scale: number;
	height: number;
	onGeometry?: () => void;
}

/** Visible icons beside the dock chevron, hidden icons in a bounded flyout. */
export function SystemTray({ scale, height, onGeometry }: Props) {
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
	const geometry = useRef(onGeometry);
	geometry.current = onGeometry;
	const report = useCallback(() => {
		const rects = [host.current, panel.current].filter(Boolean).map((el) => {
			const r = el!.getBoundingClientRect();
			return {
				x: Math.floor(r.x),
				y: Math.floor(r.y),
				width: Math.ceil(r.right) - Math.floor(r.x),
				height: Math.ceil(r.bottom) - Math.floor(r.y)
			};
		});
		void invoke("update_tray_surface", {
			rects,
			ready: state.current.healthy,
			open: state.current.open
		}).catch(console.error);
		geometry.current?.();
	}, []);
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
			window.removeEventListener("blur", blur);
			document.removeEventListener("pointerdown", outside, true);
			window.removeEventListener("resize", report);
			void invoke("update_tray_surface", { rects: [], ready: false, open: false });
			geometry.current?.();
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
	const action = async (icon?: TrayIcon, contextMenu = false) => {
		if (busyRef.current) return;
		busyRef.current = true;
		setBusy(true);
		setOpen(false);
		try {
			if (icon) {
				await invoke(contextMenu ? "show_tray_context_menu" : "activate_tray_icon", {
					trayId: icon.id
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
	const icons = liveIcons.filter((icon) => !icon.is_promoted);
	const promoted = liveIcons.filter((icon) => icon.is_promoted);
	const iconButton = (icon: TrayIcon, index: number, group: TrayIcon[]) => (
		<button
			key={icon.id}
			type="button"
			disabled={busy}
			className="bloom-tray-icon"
			aria-label={`${icon.name}${group.filter((i) => i.path === icon.path).length > 1 ? ` (${index + 1})` : ""}`}
			title={`${icon.name} — right-click for its native menu`}
			onClick={(e) => {
				e.stopPropagation();
				void action(icon);
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
								? 4
								: e.key === "ArrowUp"
									? -4
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
			className={`bloom-tray bloom-tray-right ${open ? "is-open" : ""}`}
			style={{ zoom: scale, height } as CSSProperties}
			onClick={(e) => e.stopPropagation()}
			onContextMenu={(e) => {
				e.preventDefault();
				e.stopPropagation();
			}}
		>
			{open && list}
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
				<ChevronUp size={16} style={{ transform: open ? "rotate(180deg)" : undefined }} />
			</button>
			<div className="bloom-tray-promoted" role="toolbar" aria-label="Visible notification icons">
				{promoted.map((icon, index) => iconButton(icon, index, promoted))}
			</div>
		</span>
	);
}
