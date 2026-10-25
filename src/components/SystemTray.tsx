import {
	useCallback,
	useEffect,
	useLayoutEffect,
	useRef,
	useState,
	type CSSProperties,
	type RefObject
} from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { message } from "@tauri-apps/plugin-dialog";
import { ChevronLeft, LayoutGrid } from "lucide-react";
import type { TrayIcon, TraySnapshot } from "../tray";
import "./SystemTray.css";

interface Props {
	placement: "notch" | "right";
	scale: number;
	width?: number;
	height?: number;
	onGeometry?: () => void;
	anchorRef?: RefObject<HTMLElement | null>;
	visible?: boolean;
	onOpenChange?: (open: boolean) => void;
}

/** Both approved placements share catalog, focus rules, native actions and cleanup. */
export function SystemTray({
	placement,
	scale,
	width = 180,
	height = 36,
	onGeometry,
	anchorRef,
	visible = true,
	onOpenChange
}: Props) {
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
	const showRef = useRef<() => Promise<void>>(async () => {});
	const mounted = useRef(true);
	const state = useRef({ healthy, open });
	state.current = { healthy, open };
	const geometry = useRef(onGeometry);
	geometry.current = onGeometry;
	const positioning = useRef({ anchorRef, placement, scale, visible });
	positioning.current = { anchorRef, placement, scale, visible };
	const report = useCallback(() => {
		const current = positioning.current;
		if (current.placement === "notch" && host.current) {
			const clock = (
				current.anchorRef?.current?.querySelector(".time-flip-container") ??
				current.anchorRef?.current
			)?.getBoundingClientRect();
			host.current.style.left = `${(clock ? clock.right : window.innerWidth / 2 + 36 * current.scale) / current.scale + 7}px`;
			host.current.style.top = `${clock ? (clock.top + clock.height / 2) / current.scale - 13 : 5}px`;
		}
		const rects = (current.visible ? [host.current, panel.current] : [])
			.filter(Boolean)
			.map((el) => {
				const r = el!.getBoundingClientRect();
				return {
					x: Math.floor(r.x),
					y: Math.floor(r.y),
					width: Math.ceil(r.width),
					height: Math.ceil(r.height)
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
					if (!next.icons.length) setOpen(false);
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
		const toggleFromCommandCenter = listen("tray-toggle", () => {
			void showRef.current();
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
			toggleFromCommandCenter.then((f) => f());
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
		onOpenChange?.(open);
		return () => onOpenChange?.(false);
	}, [open, onOpenChange]);
	useEffect(() => {
		if (!visible && !busyRef.current) setOpen(false);
	}, [visible]);
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
		const next = await refresh();
		if (!mounted.current) return;
		if (next?.icons.length) setOpen(true);
		else
			await message(
				next
					? "No live notification icons found. Right-click the tray button to access Windows tray."
					: "Windows tray discovery is unavailable. Right-click the tray button to access Windows tray.",
				{ title: "System Tray", kind: next ? "info" : "error" }
			).catch(console.error);
	};
	showRef.current = show;
	const icons = snapshot?.icons ?? [];
	const list = (
		<div
			ref={panel}
			className={`bloom-tray-icons ${placement === "notch" ? "bloom-tray-popup" : ""}`}
			role="toolbar"
			aria-label="Background applications"
			aria-orientation={placement === "right" ? "horizontal" : "vertical"}
			onWheel={(e) => {
				e.stopPropagation();
				if (placement === "right") {
					e.currentTarget.scrollLeft += e.deltaY;
				}
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
							: placement === "notch" && e.key === "ArrowDown"
								? 3
								: placement === "notch" && e.key === "ArrowUp"
									? -3
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
			{icons.map((icon, index) => (
				<button
					key={icon.id}
					type="button"
					disabled={busy}
					className="bloom-tray-icon"
					aria-label={`${icon.name}${icons.filter((i) => i.path === icon.path).length > 1 ? ` (${index + 1})` : ""}`}
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
			))}
		</div>
	);
	return (
		<span
			ref={host}
			className={`bloom-tray bloom-tray-${placement} ${open ? "is-open" : ""}`}
			style={
				placement === "right"
					? ({ zoom: scale, width: open ? width : height, height } as CSSProperties)
					: { zoom: scale, visibility: visible ? "visible" : "hidden" }
			}
			onClick={(e) => e.stopPropagation()}
			onContextMenu={(e) => {
				e.preventDefault();
				e.stopPropagation();
				void action();
			}}
		>
			{placement === "right" && open && list}
			<button
				ref={toggle}
				className="bloom-tray-toggle"
				type="button"
				aria-label="Background applications"
				aria-expanded={open}
				aria-haspopup="true"
				disabled={busy}
				title={failure || "Background applications — right-click for Windows tray"}
				onClick={(e) => {
					e.stopPropagation();
					void show();
				}}
				onKeyDown={(e) => {
					if (e.key === "Escape") {
						e.stopPropagation();
						setOpen(false);
					}
				}}
			>
				{placement === "notch" ? (
					<LayoutGrid size={16} fill="currentColor" strokeWidth={0} />
				) : (
					<ChevronLeft size={16} style={{ transform: open ? "rotate(180deg)" : undefined }} />
				)}
			</button>
			{placement === "notch" &&
				visible &&
				open &&
				createPortal(
					<div className="bloom-tray-notch-panel" style={{ zoom: scale }}>
						{list}
					</div>,
					document.body
				)}
		</span>
	);
}
