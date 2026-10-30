import { islandView, type IslandMode } from "./islandView";
import { sameMediaInfo, type MediaInfo } from "./mediaInfo";
import { Visualizer } from "./components/Visualizer";
import { BloomAvatar } from "./components/BloomAvatar";
import { IslandCalendar } from "./components/IslandCalendar";
import { normalizeStatusWidgets } from "./statusWidgets";
import { startDrag } from "@crabnebula/tauri-plugin-drag";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { AnimatePresence, motion, useAnimation } from "framer-motion";
import {
	ArrowUpDown,
	ChevronLeft,
	ChevronRight,
	Cpu,
	HardDrive,
	Inbox,
	MemoryStick,
	Type,
	X
} from "lucide-react";
import {
	useCallback,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
	type CSSProperties
} from "react";
import "./App.css";
import { CompactMediaPlayer } from "./CompactMediaPlayer";
import { EdgeSurface } from "./components/EdgeSurface";
import { ShelfTypeIcon } from "./components/ShelfTypeIcon";
import type { WidgetConfig } from "./components/StatusWidgetConfig";
import { reloadIfMirrorWasStale } from "./hooks/settingsMirror";
import { useBloomFontScale, useIslandScales } from "./hooks/useIslandScales";
import { useSettingsSync } from "./hooks/useSettingsSync";
import { useTrailingThrottle } from "./hooks/useTrailingThrottle";
import { useWeather } from "./hooks/useWeather";
import {
	HeadphonesIcon,
	MusicNoteIcon,
	PauseIcon,
	PlayIcon,
	SkipBackIcon,
	SkipForwardIcon,
	VolumeHighIcon,
	VolumeLowIcon
} from "./icons";
import { initTheme } from "./theme";
import { effectiveScale } from "./scales";
import type { UpdateCheckResult } from "./updater";
import { createRegionSender, measureElements, notchPillBox, type RegionRect } from "./windowRegion";

/**
 * The notch has two kinds of content:
 * - Panels are tools the user chooses. A click opens the last-used panel by
 *   default, or a selected fixed panel when panel memory is turned off.
 * - Live activities (music and a filled shelf) show a compact form in
 *   the pill or a bubble beside it, and opening one shows its own view, which
 *   stands alone (no panel arrows) and is never remembered as the last panel.
 *   Dragging files onto the notch also opens the shelf. Wheel/swipe switches
 *   between tool panels, including when a live activity occupies the pill.
 */
type NotchPanel = "calendar" | "command-center";
const NOTCH_PANELS: readonly NotchPanel[] = ["calendar", "command-center"];
const asNotchPanel = (value: unknown): NotchPanel | null =>
	NOTCH_PANELS.includes(value as NotchPanel) ? (value as NotchPanel) : null;

function GreenDownArrowIcon() {
	return (
		<div
			style={{
				display: "inline-flex",
				alignItems: "center",
				justifyContent: "center",
				width: "18px",
				height: "18px",
				borderRadius: "50%",
				background: "rgba(50, 215, 75, 0.15)",
				border: "1px solid rgba(50, 215, 75, 0.35)",
				boxShadow: "0 0 8px rgba(50, 215, 75, 0.25)",
				flexShrink: 0,
				verticalAlign: "middle"
			}}
		>
			<svg
				width="10"
				height="10"
				viewBox="0 0 24 24"
				fill="none"
				stroke="#32D74B"
				strokeWidth="3"
				strokeLinecap="round"
				strokeLinejoin="round"
			>
				<line x1="12" y1="4" x2="12" y2="16"></line>
				<polyline points="18 10 12 16 6 10"></polyline>
				<line x1="6" y1="20" x2="18" y2="20"></line>
			</svg>
		</div>
	);
}

// A shelf file's preview from the backend: a picture when one can be made,
// else the opening text of a document or text file, and whether it's a folder
// (for the placeholder icon).
interface ShelfPreview {
	thumb: string | null;
	text: string | null;
	folder: boolean;
}

interface ShelfItem {
	id: string;
	type: "file" | "text";
	name: string;
	path?: string;
	text?: string;
	timestamp: number;
}

type BloomMode = IslandMode;

// The shelf is a short-lived stash: one row, newest first, oldest evicted.
const SHELF_MAX_ITEMS = 10;

const MARQUEE_SPEED = 30; // px/s — constant for all titles
const MARQUEE_MIN_DURATION = 5; // floor so short titles don't flicker

const TitleMarquee = ({ title }: { title: string }) => {
	const textRef = useRef<HTMLSpanElement>(null);
	const containerRef = useRef<HTMLDivElement>(null);
	const [isOverflowing, setIsOverflowing] = useState(false);
	const [scrollDistance, setScrollDistance] = useState(0);
	const [scrollDuration, setScrollDuration] = useState(8);

	useEffect(() => {
		const check = () => {
			if (textRef.current && containerRef.current) {
				const textWidth = textRef.current.scrollWidth;
				const containerWidth = containerRef.current.clientWidth;
				const overflow = textWidth > containerWidth;
				setIsOverflowing(overflow);
				if (overflow) {
					const distance = textWidth - containerWidth;
					setScrollDistance(distance);
					setScrollDuration(Math.max(distance / MARQUEE_SPEED, MARQUEE_MIN_DURATION));
				}
			}
		};
		check();
		const observer = new ResizeObserver(check);
		if (containerRef.current) observer.observe(containerRef.current);
		return () => observer.disconnect();
	}, [title]);

	return (
		<div
			ref={containerRef}
			className="premium-title-wrap"
			style={{ width: "100%", overflow: "hidden" }}
		>
			<span
				ref={textRef}
				className={`premium-title ${isOverflowing ? "marquee" : ""}`}
				style={
					{
						"--scroll-distance": isOverflowing ? `-${scrollDistance}px` : undefined,
						"--scroll-duration": `${scrollDuration}s`
					} as React.CSSProperties
				}
			>
				{title}
			</span>
		</div>
	);
};

function App() {
	const scales = useIslandScales();
	const scale = effectiveScale(scales);
	const notchHeaderHeight = 48;
	useBloomFontScale(scales.font);
	useEffect(() => {
		return initTheme();
	}, []);

	const [time, setTime] = useState("");
	const [isHovered, setIsHovered] = useState(false);
	// Expanded into a panel by a click. Hover alone only peeks.
	const [isOpen, setIsOpen] = useState(false);
	const isOpenRef = useRef(false);
	isOpenRef.current = isOpen;
	const [isReady, setIsReady] = useState(false);
	const [timeFormat24h, setTimeFormat24h] = useState(
		() => localStorage.getItem("bloom-time-format-24h") === "true"
	);

	// The status widget whose tooltip is open, the side of the pill it hangs
	// from, and the distance from the widget to just past that edge.
	const [statusTooltip, setStatusTooltip] = useState<{
		id: string;
		side: "below" | "left" | "right";
		offset: number;
	} | null>(null);
	const statusTooltipTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	// Width the hovered status strip needs so neither side runs into the clock.
	const [statusFitWidth, setStatusFitWidth] = useState(0);

	const [eventPeek, setEventPeek] = useState(false);
	const eventPeekTimeoutRef = useRef<any>(null);
	const updatePulseTimerRef = useRef<any>(null);
	const triggerEventPeek = useCallback((duration = 3000) => {
		setEventPeek(true);
		if (eventPeekTimeoutRef.current) clearTimeout(eventPeekTimeoutRef.current);
		eventPeekTimeoutRef.current = setTimeout(() => setEventPeek(false), duration);
	}, []);

	const [notchMode, setNotchMode] = useState(() => {
		const raw = localStorage.getItem("bloom-notch-mode") || "fixed";
		if (raw === "auto-hide") return "smart";
		return raw;
	});

	// Weather state (managed by useWeather hook)

	// Media state
	const [isPlaying, setIsPlaying] = useState(false);
	const [mediaInfo, setMediaInfo] = useState<MediaInfo>({
		title: "",
		artist: "",
		is_playing: false,
		has_media: false
	});
	const albumArtUrl = mediaInfo.artwork?.[0] ?? null;
	const [volume, setVolume] = useState(0.5);
	const [isMuted, setIsMuted] = useState(false);
	// Sliders show 0 while muted, matching the overlay HUD.
	const displayVolume = isMuted ? 0 : volume;

	// System metrics for status widgets
	const [cpuUsage, setCpuUsage] = useState(0);
	const [ramUsage, setRamUsage] = useState(0);
	const [diskSpace, setDiskSpace] = useState(0);
	const [netUpSpeed, setNetUpSpeed] = useState(0);
	const [netDownSpeed, setNetDownSpeed] = useState(0);
	const [statusWidgets, setStatusWidgets] = useState<WidgetConfig>({
		left: ["weather"],
		right: []
	});

	const [windowLabel, setWindowLabel] = useState<string>("");
	useEffect(() => {
		setWindowLabel(getCurrentWebviewWindow().label);
	}, []);

	// Update state
	const [updateAvailable, setUpdateAvailable] = useState(false);
	const [showUpdateIndicator, setShowUpdateIndicator] = useState(
		() => localStorage.getItem("bloom-show-update-indicator") !== "false"
	);
	const [showUpdatePulse, setShowUpdatePulse] = useState(false);

	useEffect(() => {
		if (windowLabel !== "main") return;

		let unlisten: (() => void) | undefined;
		let disposed = false;

		listen<UpdateCheckResult>("update-available", (event) => {
			setUpdateAvailable(event.payload.available);
			if (!event.payload.available) {
				setShowUpdatePulse(false);
				if (updatePulseTimerRef.current) clearTimeout(updatePulseTimerRef.current);
				return;
			}
			setShowUpdatePulse(true);
			if (notchMode === "peek") triggerEventPeek(6000);
			if (updatePulseTimerRef.current) clearTimeout(updatePulseTimerRef.current);
			updatePulseTimerRef.current = setTimeout(() => {
				setShowUpdatePulse(false);
			}, 6000);
		}).then((fn) => {
			if (disposed) fn();
			else unlisten = fn;
		});

		invoke<UpdateCheckResult>("get_update_state")
			.then((state) => {
				// Only restore a badge; clearing is left to check results so a
				// late mount read cannot race an "update-available" event.
				if (state.available) setUpdateAvailable(true);
			})
			.catch((e) => console.error("Failed to read update state:", e));

		return () => {
			disposed = true;
			unlisten?.();
			if (updatePulseTimerRef.current) clearTimeout(updatePulseTimerRef.current);
		};
	}, [windowLabel, notchMode, triggerEventPeek]);

	const [isVisible, setIsVisible] = useState(true);
	const [isImpacted, setIsImpacted] = useState(false);
	const [isExpanded, setIsExpanded] = useState(false);
	const [startupAnimating, setStartupAnimating] = useState(false);
	const [isNotchHovered, setIsNotchHovered] = useState(false);

	const [isEdgeHovered, setIsEdgeHovered] = useState(false);
	const [isOverlapped, setIsOverlapped] = useState(false);
	const [interactionState, setInteractionState] = useState<"active" | "grace" | "none">("none");
	const bloomRef = useRef<HTMLDivElement>(null);
	// Measured to fit the hovered status strip; see statusFitWidth.
	const leftWidgetsRef = useRef<HTMLDivElement>(null);
	const rightWidgetsRef = useRef<HTMLDivElement>(null);
	const timeCenterRef = useRef<HTMLDivElement>(null);
	const shelfBubbleRef = useRef<HTMLButtonElement>(null);
	const reportNotchRect = useRef<() => void>(() => {});
	// The pill's target box in window CSS px, and the sender that applies the
	// notch window's region (holding earlier regions while it shrinks).
	const pillTargetRef = useRef<RegionRect | null>(null);
	const sendNotchRegion = useMemo(
		() =>
			createRegionSender((rects) => {
				invoke("set_window_region", { rects }).catch(() => {});
			}),
		[]
	);

	// Recomputes the notch window's region from the pill's target box and the
	// floating parts drawn around it, and sends it if it changed. Reads only
	// refs and the DOM, so it is safe to call from any callback.
	const updateNotchRegion = () => {
		const wanted: RegionRect[] = [];
		if (pillTargetRef.current) wanted.push(pillTargetRef.current);
		// Each floating part, padded to cover what it paints outside its box: a
		// tooltip card's shadow reaches 20px out plus a 6px drop, a bubble's
		// glow less, and the screen corners nothing.
		wanted.push(
			...measureElements([
				[".activity-bubble", 16],
				[".status-tooltip", 28],
				[".screen-corner", 0]
			])
		);
		sendNotchRegion(wanted);
	};
	const [systemSettingsReady, setSystemSettingsReady] = useState(false);
	const [waitForStartupSplash, setWaitForStartupSplash] = useState(false);
	const initialSystemSyncDone = useRef(false);
	const splashDone = useRef(false);
	const systemSyncQueue = useRef<Promise<void>>(Promise.resolve());
	const appliedNotchMode = useRef<string | null>(null);

	const isAnyInteraction = isHovered || isNotchHovered || isEdgeHovered;
	const isHidden =
		!startupAnimating &&
		((notchMode === "smart" && isOverlapped && interactionState === "none") ||
			(notchMode === "peek" && interactionState === "none" && !eventPeek));

	useEffect(() => {
		if (isAnyInteraction) {
			setInteractionState("active");
		} else if (interactionState !== "none") {
			setInteractionState("grace");
			const timer = setTimeout(() => setInteractionState("none"), 800);
			return () => clearTimeout(timer);
		}
	}, [isAnyInteraction]);

	useEffect(() => {
		if (windowLabel === "main") {
			invoke("set_notch_hovered", { hovered: isNotchHovered }).catch(() => {});
		}
	}, [isNotchHovered, windowLabel]);

	// Tell the backend whether the notch is actually on screen so a hidden
	// notch (smart/peek mode) stays click-through instead of swallowing clicks
	// in its footprint.
	useEffect(() => {
		if (windowLabel === "main") {
			invoke("set_notch_visible", { visible: isVisible && !isHidden }).catch(() => {});
		}
	}, [isVisible, isHidden, windowLabel]);

	useEffect(() => {
		const updateRect = () => {
			if (bloomRef.current && windowLabel === "main") {
				// Activity bubbles sit beside the pill and must stay clickable too.
				const rects = [bloomRef.current, shelfBubbleRef.current]
					.filter((el): el is NonNullable<typeof el> => el !== null)
					.map((el) => el.getBoundingClientRect());
				const left = Math.min(...rects.map((r) => r.left));
				const top = Math.min(...rects.map((r) => r.top));
				const right = Math.max(...rects.map((r) => r.right));
				const bottom = Math.max(...rects.map((r) => r.bottom));
				invoke("update_notch_rect", {
					rect: {
						x: Math.round(left),
						y: Math.round(top),
						width: Math.round(right - left),
						height: Math.round(bottom - top)
					}
				}).catch(() => {});
				// Bubbles finishing their entrance land here too.
				updateNotchRegion();
			}
		};
		reportNotchRect.current = updateRect;

		updateRect();
		window.addEventListener("resize", updateRect);
		const observer = new ResizeObserver(updateRect);
		if (bloomRef.current) observer.observe(bloomRef.current);

		return () => {
			window.removeEventListener("resize", updateRect);
			observer.disconnect();
		};
	}, [isExpanded, isHidden, windowLabel, scale]);

	useEffect(() => {
		if (!windowLabel) return;

		// Only animate the main top-bar
		if (windowLabel !== "main") {
			setIsReady(true);
			setIsImpacted(true);
			setIsExpanded(true);
			return;
		}

		const proceedWithStartup = () => {
			const checkVisibility = async () => {
				try {
					const win = getCurrentWebviewWindow();
					const visible = await win.isVisible();
					if (visible) {
						setStartupAnimating(true);
						setIsReady(true);
						// The window is shown here; WebView2 may only create its
						// render child now, so refresh the drop targets.
						invoke("setup_shelf_drop").catch(() => {});
						setTimeout(() => {
							setIsImpacted(true);
							setIsExpanded(true);
						}, 240);
						setTimeout(() => setStartupAnimating(false), 1500);
						return true;
					}
				} catch (e) {}
				return false;
			};

			const interval = setInterval(async () => {
				if (await checkVisibility()) clearInterval(interval);
			}, 100);

			checkVisibility();
			return interval;
		};

		// Mirror Overlay.tsx's splash decision so we only wait when a splash will actually fire.
		// Overlay always emits splash-done, but on a normal relaunch it emits it near-instantly
		// (after one async getVersion() call) — before this listener would be registered.
		// By making the same decision here we avoid a race and avoid any unnecessary delay.
		const firstRun = localStorage.getItem("bloom-first-run") === null;
		const storedVersion = localStorage.getItem("bloom-app-version");

		const waitForSplash = () => {
			// Splash is definitely coming — register listener now (2800ms animation gives us plenty of time)
			let started = false;
			let interval: any;
			const unlistenSplash = listen("splash-done", () => {
				if (started) return;
				started = true;
				interval = proceedWithStartup();
				unlistenSplash.then((fn) => fn());
			});
			const safetyTimer = setTimeout(() => {
				if (started) return;
				started = true;
				interval = proceedWithStartup();
				unlistenSplash.then((fn) => fn());
			}, 6000);
			return () => {
				clearTimeout(safetyTimer);
				if (interval) clearInterval(interval);
				unlistenSplash.then((fn) => fn());
			};
		};

		if (firstRun || storedVersion === null) {
			// Splash is definitely showing — wait for it
			return waitForSplash();
		}

		// Has a version key — need async check to know if version changed
		let interval: any;
		getVersion()
			.then((currentVersion) => {
				if (storedVersion !== currentVersion) {
					// Version changed — splash is coming, wait for it
					// (splash takes 2800ms so there's plenty of time to register the listener)
					waitForSplash();
				} else {
					// Same version — no splash, start immediately
					interval = proceedWithStartup();
				}
			})
			.catch(() => {
				interval = proceedWithStartup();
			});
		return () => {
			if (interval) clearInterval(interval);
		};
	}, [windowLabel]);

	// Settings state
	const [settingsWeatherEnabled, setSettingsWeatherEnabled] = useState(
		() => localStorage.getItem("bloom-weather-enabled") !== "false"
	);
	const [settingsCalendarEnabled, setSettingsCalendarEnabled] = useState(
		() => localStorage.getItem("bloom-calendar-enabled") !== "false"
	);

	const [settingsMusicModeEnabled, setSettingsMusicModeEnabled] = useState(
		() => localStorage.getItem("bloom-music-mode-enabled") !== "false"
	);
	const [rememberPanelEnabled, setRememberPanelEnabled] = useState(
		() =>
			(localStorage.getItem("bloom-notch-remember-panel") ??
				localStorage.getItem("bloom-notch-click-to-open")) !== "false"
	);
	const rememberPanelEnabledRef = useRef(rememberPanelEnabled);
	rememberPanelEnabledRef.current = rememberPanelEnabled;
	const [defaultPanel, setDefaultPanel] = useState<NotchPanel>(
		() => asNotchPanel(localStorage.getItem("bloom-notch-default-panel")) ?? "calendar"
	);
	// The panel the open notch was last left on. Opening the notch returns to it,
	// across restarts too (bloom-notch-last-panel).
	const [lastPanel, setLastPanel] = useState<NotchPanel | null>(() =>
		asNotchPanel(localStorage.getItem("bloom-notch-last-panel"))
	);
	const [settingsMusicCompactNotch, setSettingsMusicCompactNotch] = useState(
		() => localStorage.getItem("bloom-music-compact-notch") !== "false"
	);
	const [shelfPersist, setShelfPersist] = useState(
		() => localStorage.getItem("bloom-shelf-persist") === "true"
	);
	const [settingsVisualizerEnabled, setSettingsVisualizerEnabled] = useState(
		() => localStorage.getItem("bloom-visualizer-enabled") !== "false"
	);
	const [settingsAlbumArtEnabled, setSettingsAlbumArtEnabled] = useState(
		() => localStorage.getItem("bloom-media-album-art-enabled") !== "false"
	);
	const [settingsCornersEnabled, setSettingsCornersEnabled] = useState(
		() => localStorage.getItem("bloom-corners-enabled") === "true"
	);
	const [mediaLayout, setMediaLayout] = useState<"classic" | "compact">(
		() => (localStorage.getItem("bloom-media-layout") as "classic" | "compact") || "classic"
	);
	const [compactVolumeExpanded, setCompactVolumeExpanded] = useState(false);

	// Weather hook
	const {
		temperature,
		weatherCondition,
		weatherIcon: WeatherIcon,
		cityName,
		tempUnit
	} = useWeather(settingsWeatherEnabled);

	useEffect(() => {
		if (!windowLabel) return;
		let cancelled = false;

		invoke("load_settings")
			.then(async (settings: any) => {
				if (await reloadIfMirrorWasStale(settings)) return;
				if (cancelled) return;
				const getVal = (key: string, fallback: string | null = null) => {
					const val = settings[key];
					if (val !== undefined && val !== null) return String(val);
					const local = localStorage.getItem(key);
					if (local !== null) return local;
					return fallback;
				};

				setSettingsWeatherEnabled(getVal("bloom-weather-enabled", "true") !== "false");
				setSettingsCalendarEnabled(getVal("bloom-calendar-enabled", "true") !== "false");
				setSettingsMusicModeEnabled(getVal("bloom-music-mode-enabled", "true") !== "false");
				setRememberPanelEnabled(
					(getVal("bloom-notch-remember-panel") ?? getVal("bloom-notch-click-to-open", "true")) !==
						"false"
				);
				setDefaultPanel(asNotchPanel(getVal("bloom-notch-default-panel")) ?? "calendar");
				setShelfPersist(getVal("bloom-shelf-persist", "false") === "true");
				setSettingsMusicCompactNotch(getVal("bloom-music-compact-notch", "true") !== "false");
				setLastPanel(asNotchPanel(getVal("bloom-notch-last-panel")));
				const viz =
					getVal("bloom-media-visualizer-enabled") ?? getVal("bloom-visualizer-enabled", "true");
				setSettingsVisualizerEnabled(viz !== "false");
				setSettingsAlbumArtEnabled(getVal("bloom-media-album-art-enabled", "true") !== "false");
				setSettingsCornersEnabled(getVal("bloom-corners-enabled", "false") === "true");
				setTimeFormat24h(getVal("bloom-time-format-24h") === "true");

				const nMode = getVal("bloom-notch-mode", "fixed");
				if (nMode) {
					const mapped = nMode === "auto-hide" ? "smart" : nMode;
					setNotchMode(mapped);
				}

				if (windowLabel === "main") {
					const firstRun = localStorage.getItem("bloom-first-run") === null;
					if (firstRun) {
						import("@tauri-apps/plugin-autostart").then(({ enable, isEnabled }) => {
							isEnabled().then((enabled) => {
								if (!enabled) enable().catch(() => {});
							});
						});
						localStorage.setItem("bloom-first-run", "done");
					}
					const storedVersion = localStorage.getItem("bloom-app-version");
					let waitForSplash = firstRun || storedVersion === null;
					if (!waitForSplash) {
						try {
							waitForSplash = storedVersion !== (await getVersion());
						} catch {
							// Version lookup is optional; continue startup without a splash.
						}
					}
					if (cancelled) return;
					setWaitForStartupSplash(waitForSplash);
					setSystemSettingsReady(true);
				}

				const widgetsVal = getVal("bloom-status-widgets");
				if (widgetsVal) {
					try {
						const parsed = JSON.parse(widgetsVal);
						setStatusWidgets(normalizeStatusWidgets(parsed));
					} catch {}
				}
			})
			.catch((error) => {
				console.error(error);
				if (!cancelled && windowLabel === "main") setSystemSettingsReady(true);
			});
		return () => {
			cancelled = true;
		};
	}, [windowLabel]); // eslint-disable-line react-hooks/exhaustive-deps

	useEffect(() => {
		// Disable context menu globally
		const preventContext = (e: MouseEvent) => e.preventDefault();
		document.addEventListener("contextmenu", preventContext);

		const unlistenVisibility = listen<boolean>("visibility-change", (event) => {
			setIsVisible(event.payload);
		});

		const unlistenNotchOverlap = listen<boolean>("notch-overlap", (event) => {
			setIsOverlapped(event.payload);
		});

		const unlistenNotchEdgeHover = listen<boolean>("notch-edge-hover", (event) => {
			setIsEdgeHovered(event.payload);
		});

		// The backend emits this when the window flips to click-through while
		// the webview still thinks the pointer is inside the notch (it cannot
		// receive the mouseleave once the window is click-through).
		const unlistenNotchPointerLeft = listen("notch-pointer-left", () => {
			releaseNotchHover();
		});

		return () => {
			unlistenVisibility.then((f) => f());
			unlistenNotchOverlap.then((f) => f());
			unlistenNotchEdgeHover.then((f) => f());
			unlistenNotchPointerLeft.then((f) => f());
			document.removeEventListener("contextmenu", preventContext);
		};
	}, [windowLabel]);

	// Settings sync — consolidated dispatch for all settings events
	useSettingsSync(
		{
			"bloom-weather-enabled": setSettingsWeatherEnabled,
			"bloom-calendar-enabled": setSettingsCalendarEnabled,

			"bloom-music-mode-enabled": setSettingsMusicModeEnabled,
			"bloom-notch-remember-panel": setRememberPanelEnabled,
			"bloom-notch-default-panel": (v: unknown) => setDefaultPanel(asNotchPanel(v) ?? "calendar"),
			"bloom-shelf-persist": setShelfPersist,
			"bloom-music-compact-notch": setSettingsMusicCompactNotch,
			"bloom-notch-last-panel": (v: unknown) => setLastPanel(asNotchPanel(v)),
			"bloom-media-visualizer-enabled": setSettingsVisualizerEnabled,
			"bloom-visualizer-enabled": setSettingsVisualizerEnabled,
			"bloom-media-album-art-enabled": setSettingsAlbumArtEnabled,
			"bloom-media-layout": setMediaLayout,
			"bloom-corners-enabled": setSettingsCornersEnabled,
			"bloom-notch-mode": setNotchMode,
			"bloom-status-widgets": (value) => {
				try {
					const parsed = JSON.parse(value);
					setStatusWidgets(normalizeStatusWidgets(parsed));
				} catch {}
			},
			"bloom-show-update-indicator": (value) => setShowUpdateIndicator(String(value) === "true"),
			"bloom-time-format-24h": setTimeFormat24h
		},
		[windowLabel]
	);

	// One owner applies the latest window settings. Superseded splash timers and
	// retries are cancelled; an in-flight command finishes before the new state
	// is applied, so an old startup retry cannot overwrite a user change.
	useEffect(() => {
		if (windowLabel !== "main" || !systemSettingsReady) return;

		let cancelled = false;
		let started = false;
		let attempts = 0;
		let startTimer: number | undefined;
		let splashTimer: number | undefined;
		let retryTimer: number | undefined;
		let settleTimer: number | undefined;
		let unlistenSplash: (() => void) | undefined;

		const reconcile = () => {
			systemSyncQueue.current = systemSyncQueue.current
				.catch(() => {})
				.then(async () => {
					if (cancelled) return;
					try {
						if (cancelled) return;
						if (appliedNotchMode.current !== notchMode) {
							await invoke("change_notch_mode", { mode: notchMode });
							appliedNotchMode.current = notchMode;
						}
						if (cancelled) return;
						await invoke("sync_appbar");
						if (cancelled) return;
						const wasInitialSync = !initialSystemSyncDone.current;
						initialSystemSyncDone.current = true;
						if (wasInitialSync) {
							settleTimer = window.setTimeout(() => {
								if (!cancelled) {
									systemSyncQueue.current = systemSyncQueue.current
										.catch(() => {})
										.then(() => invoke<void>("sync_appbar"))
										.catch((error) => console.error("AppBar settle failed", error));
								}
							}, 1000);
						}
					} catch (error) {
						console.error("Window reconciliation failed", error);
						if (!cancelled && attempts++ < 1) {
							retryTimer = window.setTimeout(reconcile, 1000);
						}
					}
				});
		};

		const start = () => {
			if (cancelled || started) return;
			started = true;
			splashDone.current = true;
			if (splashTimer !== undefined) window.clearTimeout(splashTimer);
			unlistenSplash?.();
			startTimer = window.setTimeout(reconcile, initialSystemSyncDone.current ? 0 : 400);
		};

		if (waitForStartupSplash && !splashDone.current && !initialSystemSyncDone.current) {
			void listen("splash-done", start)
				.then((unlisten) => {
					if (cancelled || started) unlisten();
					else unlistenSplash = unlisten;
				})
				.catch(() => start());
			splashTimer = window.setTimeout(start, 6000);
		} else {
			start();
		}

		return () => {
			cancelled = true;
			for (const timer of [startTimer, splashTimer, retryTimer, settleTimer]) {
				if (timer !== undefined) window.clearTimeout(timer);
			}
			unlistenSplash?.();
		};
	}, [windowLabel, systemSettingsReady, waitForStartupSplash, notchMode]);

	// User panels and live activities share the notch's animation state.
	const [bloomMode, setBloomMode] = useState<BloomMode>("status");

	// Shelf: files and selected text stashed in the notch via drag-and-drop.
	// Files are stored as plain paths (references, never copies). Session-scoped
	// by default; the bloom-shelf-persist setting keeps items across restarts.
	const [shelfItems, setShelfItems] = useState<ShelfItem[]>(() => {
		if (localStorage.getItem("bloom-shelf-persist") !== "true") return [];
		try {
			const saved = localStorage.getItem("bloom-shelf-items");
			const parsed = saved ? JSON.parse(saved) : [];
			if (!Array.isArray(parsed)) return [];
			return parsed
				.filter((i): i is ShelfItem => {
					if (typeof i?.id !== "string" || typeof i?.name !== "string") return false;
					// Items stored before text support have no `type`; treat as files.
					if (i.type === "text") return typeof i.text === "string";
					return typeof i.path === "string";
				})
				.map((i) => ({ ...i, type: i.type === "text" ? "text" : "file" }));
		} catch {
			return [];
		}
	});
	useEffect(() => {
		try {
			if (shelfPersist) {
				localStorage.setItem("bloom-shelf-items", JSON.stringify(shelfItems));
			} else {
				localStorage.removeItem("bloom-shelf-items");
			}
		} catch {
			/* storage full or unavailable — shelf just won't persist */
		}
	}, [shelfItems, shelfPersist]);

	// Drop file references whose files are gone (moved/deleted/drive unplugged)
	// once at startup, so a persisted shelf does not accumulate dead entries.
	// Text items are always kept.
	const shelfPrunedRef = useRef(false);
	useEffect(() => {
		if (shelfPrunedRef.current) return;
		shelfPrunedRef.current = true;
		const filePaths = shelfItems
			.filter((i) => i.type === "file" && i.path)
			.map((i) => i.path as string);
		if (filePaths.length === 0) return;
		invoke<string[]>("shelf_prune", { paths: filePaths })
			.then((existing) => {
				const alive = new Set(existing);
				setShelfItems((prev) => {
					const kept = prev.filter(
						(i) => i.type !== "file" || (i.path !== undefined && alive.has(i.path))
					);
					return kept.length === prev.length ? prev : kept;
				});
			})
			.catch(() => {});
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	// Previews are generated lazily per path and cached. Images, PDFs, Office
	// files with a saved thumbnail, and anything Windows can thumbnail (videos,
	// folders...) show a picture; documents and text files show their opening
	// text; the rest fall back to an icon for their type. Text items render
	// their content.
	const [shelfThumbs, setShelfThumbs] = useState<Record<string, ShelfPreview>>({});
	const shelfThumbPending = useRef<Set<string>>(new Set());
	useEffect(() => {
		for (const item of shelfItems) {
			if (item.type !== "file" || !item.path) continue;
			const path = item.path;
			if (shelfThumbs[path] !== undefined || shelfThumbPending.current.has(path)) continue;
			shelfThumbPending.current.add(path);
			// The item may be removed while this is in flight; don't resurrect
			// a stale cache entry.
			const storeThumb = (preview: ShelfPreview) =>
				setShelfThumbs((prev) => {
					if (!shelfItemsRef.current.some((i) => i.path === path)) return prev;
					return { ...prev, [path]: preview };
				});
			invoke<ShelfPreview>("shelf_thumbnail", { path })
				.then(storeThumb)
				.catch(() => storeThumb({ thumb: null, text: null, folder: false }))
				.finally(() => shelfThumbPending.current.delete(path));
		}
	}, [shelfItems, shelfThumbs]);

	// Drop cached thumbnails for items that left the shelf so the cache cannot
	// grow without bound over a long session (each entry is a data URI).
	useEffect(() => {
		const current = new Set(shelfItems.filter((i) => i.path).map((i) => i.path as string));
		setShelfThumbs((prev) => {
			const keys = Object.keys(prev);
			if (keys.every((k) => current.has(k))) return prev;
			const next: Record<string, ShelfPreview> = {};
			for (const k of keys) {
				if (current.has(k)) next[k] = prev[k];
			}
			return next;
		});
	}, [shelfItems]);

	// Native drag preview for items without a thumbnail.
	const [shelfDragIcon, setShelfDragIcon] = useState<string | null>(null);
	useEffect(() => {
		invoke<string | null>("shelf_drag_icon")
			.then(setShelfDragIcon)
			.catch(() => {});
	}, []);

	// Picking a file back up is a drag-out gesture: press, move past a small
	// threshold, and the OS takes over the drag. A press without movement is a
	// plain click that opens the file (or copies text items).
	const shelfPointerRef = useRef<{ id: string; x: number; y: number; dragging: boolean } | null>(
		null
	);
	// Timestamp of the last drop onto the notch. A drop recorded during our
	// own drag-out means the item came back to the notch (pointer still over
	// it), which decides whether the drag cleanup clears the hover flags.
	const shelfSelfDropAtRef = useRef(0);
	// True while our own drag-out is running, so the drag-enter we receive when
	// it starts over the notch does not flash the hover outline or re-open it.
	const shelfSelfDragRef = useRef(false);
	const handleShelfPointerDown = (e: React.PointerEvent<HTMLDivElement>, item: ShelfItem) => {
		if (e.button !== 0) return;
		// Text items have no native drag-out; a click copies them.
		if (item.type !== "file" || !item.path) return;
		// The remove button must keep receiving its own click; capturing the
		// pointer on the tile would retarget it.
		if ((e.target as HTMLElement).closest(".shelf-tile-remove")) return;
		e.stopPropagation();
		e.currentTarget.setPointerCapture(e.pointerId);
		shelfPointerRef.current = { id: item.id, x: e.clientX, y: e.clientY, dragging: false };
	};
	const handleShelfPointerMove = (e: React.PointerEvent<HTMLDivElement>, item: ShelfItem) => {
		const st = shelfPointerRef.current;
		if (!st || st.id !== item.id || st.dragging) return;
		if (Math.hypot(e.clientX - st.x, e.clientY - st.y) < 6) return;
		if (!item.path) return;
		const path = item.path;
		const icon = shelfThumbs[path]?.thumb ?? shelfDragIcon;
		if (!icon) return;
		st.dragging = true;
		// Picking an item up is a deliberate act; don't let the post-drop
		// auto-close fire mid-drag.
		clearShelfCollapse();
		shelfSelfDragRef.current = true;
		const dragStartedAt = Date.now();
		startDrag({ item: [path], icon })
			.catch((err) => console.error("[shelf] drag out failed:", path, err))
			.finally(() => {
				shelfSelfDragRef.current = false;
				// A drop recorded during this drag means it landed back on the
				// notch, so the pointer is still over it. Otherwise the native
				// drag owned the pointer and the webview never saw it leave —
				// release the hover state (flags AND the resting mode) or the
				// shelf panel stays expanded. The command also makes the
				// backend re-evaluate its edge-hover state immediately.
				const droppedOnSelf = shelfSelfDropAtRef.current >= dragStartedAt;
				if (!droppedOnSelf) {
					releaseNotchHover();
					invoke("set_notch_hovered", { hovered: false }).catch(() => {});
				}
			});
	};
	const handleShelfTileClick = (e: React.MouseEvent<HTMLDivElement>, item: ShelfItem) => {
		e.stopPropagation();
		const st = shelfPointerRef.current;
		shelfPointerRef.current = null;
		// Only this tile's own drag suppresses the click; a stale flag from a
		// previous tile's drag-out must not swallow it.
		if (st && st.id === item.id && st.dragging) return;
		openShelfItem(item);
	};

	// Wheel scrolling for the shelf strip: ease towards a target instead of
	// jumping, so wheel and trackpad input both feel fluid.
	const shelfScrollTargetRef = useRef<number | null>(null);
	const shelfGridRef = useRef<HTMLDivElement>(null);
	// Whether the shelf row overflows, so it gets room for its scrollbar.
	const [shelfOverflowing, setShelfOverflowing] = useState(false);
	// Measured against the open shelf's width (380px minus its 14px side
	// padding) rather than the live one, which is still growing while the
	// notch animates open. A removed tile stays in the row until its exit
	// animation ends, so the row is measured again then.
	const measureShelfOverflow = useCallback(() => {
		const el = shelfGridRef.current;
		setShelfOverflowing(!!el && el.scrollWidth > 380 - 28 + 1);
	}, []);
	useLayoutEffect(measureShelfOverflow, [shelfItems, bloomMode, measureShelfOverflow]);
	const shelfScrollRafRef = useRef<number | null>(null);
	// The wheel scrolls the row from anywhere over the shelf view, not just
	// over the tiles. Mice that report lines instead of pixels are scaled up.
	const handleShelfWheel = (e: React.WheelEvent<HTMLDivElement>) => {
		const el = shelfGridRef.current;
		if (!el) return;
		const max = el.scrollWidth - el.clientWidth;
		if (max <= 0) return;
		const unit = e.deltaMode === 1 ? 40 : e.deltaMode === 2 ? el.clientWidth : 1;
		const base = shelfScrollTargetRef.current ?? el.scrollLeft;
		shelfScrollTargetRef.current = Math.max(0, Math.min(max, base + (e.deltaY + e.deltaX) * unit));
		if (shelfScrollRafRef.current !== null) return;
		const step = () => {
			const target = shelfScrollTargetRef.current;
			if (target === null) {
				shelfScrollRafRef.current = null;
				return;
			}
			const diff = target - el.scrollLeft;
			if (Math.abs(diff) < 0.5) {
				el.scrollLeft = target;
				shelfScrollTargetRef.current = null;
				shelfScrollRafRef.current = null;
				return;
			}
			const before = el.scrollLeft;
			el.scrollLeft += diff * 0.22;
			// At fractional display scales a small step can round back to the
			// same position; finish the move instead of stalling short of it.
			if (el.scrollLeft === before) {
				el.scrollLeft = target;
				shelfScrollTargetRef.current = null;
				shelfScrollRafRef.current = null;
				return;
			}
			shelfScrollRafRef.current = requestAnimationFrame(step);
		};
		shelfScrollRafRef.current = requestAnimationFrame(step);
	};

	// The shelf closes itself shortly after a drop so the notch does not stay
	// expanded. Hovering it cancels the timer; leaving collapses it.
	const shelfCollapseTimer = useRef<number | null>(null);
	// What the collapsed notch shows (see collapsedMode). Refs let the drop
	// listener and the backend pointer-left event, both mounted once, act on
	// the current state.
	const restingModeRef = useRef<BloomMode>("status");
	const bloomModeRef = useRef<BloomMode>("status");
	const rememberPanelRef = useRef<(panel: NotchPanel) => void>(() => {});

	// Shared hover-release, called on DOM hover end and when the backend
	// reports the pointer left the (now click-through) notch window. Closes an
	// open notch, remembering its panel, and returns to the resting mode.
	const releaseNotchHover = () => {
		setIsHovered(false);
		setIsNotchHovered(false);
		if (isOpenRef.current) {
			const panel = asNotchPanel(bloomModeRef.current);
			if (panel && rememberPanelEnabledRef.current) rememberPanelRef.current(panel);
			setIsOpen(false);
		}
		setBloomMode((prev) =>
			prev === "music" ||
			prev === "command-center" ||
			prev === "calendar" ||
			prev === "status" ||
			prev === "shelf"
				? restingModeRef.current
				: prev
		);
	};

	const clearShelfCollapse = () => {
		if (shelfCollapseTimer.current !== null) {
			window.clearTimeout(shelfCollapseTimer.current);
			shelfCollapseTimer.current = null;
		}
	};

	const collapseShelfSoon = (delay = 1800) => {
		clearShelfCollapse();
		shelfCollapseTimer.current = window.setTimeout(() => {
			shelfCollapseTimer.current = null;
			if (bloomModeRef.current !== "shelf") return;
			setIsOpen(false);
			setBloomMode(restingModeRef.current);
		}, delay);
	};

	const removeShelfItem = (id: string) => {
		const next = shelfItems.filter((i) => i.id !== id);
		setShelfItems(next);
		if (next.length === 0 && bloomMode === "shelf") {
			// Nothing left to show: an open notch moves to the remembered panel,
			// a closed one to its resting mode, instead of an empty shelf.
			clearShelfCollapse();
			setBloomMode(isOpen ? openingPanel() : restingModeRef.current);
		}
	};

	// A full shelf never evicts silently: whatever fits is added and the rest
	// is reported. Reads through a ref so the drop listener (mounted once) sees
	// the current list.
	const shelfItemsRef = useRef<ShelfItem[]>(shelfItems);
	shelfItemsRef.current = shelfItems;
	// Tiles that were just added get an entrance animation; ones restored when
	// the panel reopens do not (the panel itself already fades in).
	const shelfNewIdsRef = useRef<Set<string>>(new Set());
	const markShelfNew = (ids: string[]) => {
		for (const id of ids) shelfNewIdsRef.current.add(id);
		window.setTimeout(() => {
			for (const id of ids) shelfNewIdsRef.current.delete(id);
		}, 900);
	};
	const [shelfNotice, setShelfNotice] = useState<string | null>(null);
	const shelfNoticeTimer = useRef<number | null>(null);
	const showShelfNotice = (message: string) => {
		setShelfNotice(message);
		if (shelfNoticeTimer.current !== null) window.clearTimeout(shelfNoticeTimer.current);
		shelfNoticeTimer.current = window.setTimeout(() => {
			shelfNoticeTimer.current = null;
			setShelfNotice(null);
		}, 2600);
	};

	const addShelfItems = (paths: string[]) => {
		if (paths.length === 0) return;
		const prev = shelfItemsRef.current;
		const existing = new Set(prev.filter((i) => i.path).map((i) => i.path));
		const fresh: ShelfItem[] = [];
		for (const p of paths) {
			if (existing.has(p)) continue;
			existing.add(p);
			fresh.push({
				id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
				type: "file",
				name: p.split("\\").pop()?.split("/").pop() || p,
				path: p,
				timestamp: Date.now()
			});
		}
		if (fresh.length === 0) return;

		const room = Math.max(0, SHELF_MAX_ITEMS - prev.length);
		if (room === 0) {
			showShelfNotice(`Shelf is full (${SHELF_MAX_ITEMS}) — remove an item first`);
			return;
		}
		if (fresh.length > room) {
			showShelfNotice(`Shelf is full — added ${room} of ${fresh.length} items`);
		}
		const added = fresh.slice(0, room);
		markShelfNew(added.map((i) => i.id));
		setShelfItems([...added, ...prev]);
	};

	const addShelfText = (text: string) => {
		const prev = shelfItemsRef.current;
		if (prev.some((i) => i.type === "text" && i.text === text)) return;
		if (prev.length >= SHELF_MAX_ITEMS) {
			showShelfNotice(`Shelf is full (${SHELF_MAX_ITEMS}) — remove an item first`);
			return;
		}
		const item: ShelfItem = {
			id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
			type: "text",
			name: text.trim().replace(/\s+/g, " ").slice(0, 48),
			text,
			timestamp: Date.now()
		};
		markShelfNew([item.id]);
		setShelfItems([item, ...prev]);
	};

	// Files open in their default app; text items are copied to the clipboard
	// so they can be pasted anywhere. A file that no longer exists is reported
	// and dropped from the shelf.
	const openShelfItem = (item: ShelfItem) => {
		if (item.type === "text") {
			if (!item.text) return;
			writeText(item.text)
				.then(() => showShelfNotice("Copied to clipboard"))
				.catch((e) => console.error("[shelf] copy failed:", e));
			return;
		}
		if (item.path) {
			const path = item.path;
			invoke("shelf_open", { path }).catch(async (e) => {
				console.error("[shelf] open failed:", path, e);
				// Only a confirmed-missing file leaves the shelf; a transient
				// failure (e.g. no association) must not remove it.
				const alive = await invoke<string[]>("shelf_prune", { paths: [path] }).catch(() => null);
				if (alive && alive.length === 0) {
					showShelfNotice("File no longer exists — removed from shelf");
					removeShelfItem(item.id);
				} else {
					showShelfNotice("Couldn't open the file");
				}
			});
		}
	};

	// Shelf drag-and-drop plumbing. The backend registers its own OLE drop
	// target (src-tauri/src/shelf.rs) because wry's target can go stale when
	// WebView2 recreates its render child; it emits the shelf-drag-* events
	// below. Drag-enter switches to the shelf panel (restoring the previous
	// mode on cancel); drop stashes the real file paths.
	const [isDragHovering, setIsDragHovering] = useState(false);
	const shelfPrevModeRef = useRef<BloomMode>("status");
	const shelfPrevOpenRef = useRef(false);
	const shelfDragActiveRef = useRef(false);
	useEffect(() => {
		let unlisteners: (() => void)[] = [];
		let cancelled = false;

		const enter = (event: { payload: { paths?: string[]; text?: string } }) => {
			// Our own drag-out starts with the cursor over the notch; ignore
			// the enter it causes instead of flashing the hover state.
			if (shelfSelfDragRef.current) return;
			console.info("[shelf] drag-enter:", event.payload.paths ?? event.payload.text);
			setIsDragHovering(true);
			const prev = bloomModeRef.current;
			if (prev === "shelf") return;
			shelfPrevModeRef.current = prev;
			shelfPrevOpenRef.current = isOpenRef.current;
			shelfDragActiveRef.current = true;
			setIsOpen(true);
			setBloomMode("shelf");
		};
		const leave = () => {
			setIsDragHovering(false);
			if (shelfDragActiveRef.current) {
				shelfDragActiveRef.current = false;
				if (bloomModeRef.current === "shelf") {
					setIsOpen(shelfPrevOpenRef.current);
					setBloomMode(shelfPrevModeRef.current);
				}
			}
		};
		const drop = (event: { payload: { paths?: string[]; text?: string } }) => {
			setIsDragHovering(false);
			shelfDragActiveRef.current = false;
			shelfSelfDropAtRef.current = Date.now();
			const paths = event.payload.paths ?? [];
			const text = event.payload.text;
			console.info("[shelf] drop:", paths.length > 0 ? paths : text);
			if (paths.length === 0 && !text) return;
			if (paths.length > 0) addShelfItems(paths);
			if (text) addShelfText(text);
			setIsOpen(true);
			setBloomMode("shelf");
			collapseShelfSoon();
		};

		Promise.all([
			listen<{ paths?: string[]; text?: string }>("shelf-drag-enter", enter),
			listen("shelf-drag-leave", leave),
			listen<{ paths?: string[]; text?: string }>("shelf-drag-drop", drop)
		])
			.then((fns) => {
				if (cancelled) {
					fns.forEach((fn) => fn());
				} else {
					unlisteners = fns;
				}
			})
			.catch((e) => console.error("[shelf] drag-drop listener failed:", e));

		invoke("setup_shelf_drop").catch((e) => console.error("[shelf] drop target setup failed:", e));

		return () => {
			cancelled = true;
			unlisteners.forEach((fn) => fn());
		};
	}, []);

	// Window height is now kept constant to prevent rendering layout lag and sharp corners

	const lastScrollTime = useRef(0);
	const handleWheel = (e: React.WheelEvent) => {
		const target = e.target as HTMLElement;
		if (
			target.closest(".calendar-content") ||
			target.closest(".shelf-content") ||
			target.closest(".qp-list")
		) {
			return;
		}

		const now = Date.now();
		if (now - lastScrollTime.current < 250) return;

		// Use absolute values to detect horizontal swipe gestures on trackpad
		const delta = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
		if (Math.abs(delta) < 5) return; // Ignore tiny movements

		if (cycleBloomMode(delta > 0 ? 1 : -1)) lastScrollTime.current = now;
	};

	function panelAvailable(panel: NotchPanel) {
		if (panel === "calendar") return settingsCalendarEnabled;
		return true;
	}

	// Wheel and swipe navigate panels, even from a collapsed live activity.
	// Music and shelf remain entry points through their activity affordances.
	const cycleBloomMode = (direction: 1 | -1) => {
		const panels = NOTCH_PANELS.filter(panelAvailable);
		const currentIndex = panels.indexOf(bloomMode as NotchPanel);
		const nextMode =
			currentIndex === -1
				? openingPanel()
				: panels[(currentIndex + direction + panels.length) % panels.length];
		setIsOpen(true);
		setBloomMode(nextMode);
		return true;
	};

	// Arrows and wheel both stay within panels once one is open.
	const cyclePanel = (direction: 1 | -1) => {
		const panels = NOTCH_PANELS.filter(panelAvailable);
		const index = panels.indexOf(bloomMode as NotchPanel);
		if (index === -1 || panels.length < 2) return;
		setBloomMode(panels[(index + direction + panels.length) % panels.length]);
	};

	const lastTrackRef = useRef<string | null>(null);
	const lastPlayingRef = useRef<boolean>(false);
	const [musicActivityVisible, setMusicActivityVisible] = useState(false);

	// Keep a paused media session visible briefly so it can be resumed, then
	// clear its compact activity even if Windows retains the media session.
	useEffect(() => {
		if (!settingsMusicModeEnabled || !mediaInfo.has_media) {
			setMusicActivityVisible(false);
			return;
		}
		setMusicActivityVisible(true);
		if (isPlaying) return;
		const timeout = window.setTimeout(() => setMusicActivityVisible(false), 5000);
		return () => window.clearTimeout(timeout);
	}, [settingsMusicModeEnabled, mediaInfo.has_media, isPlaying]);

	// Like the Dynamic Island, the collapsed notch shows one live activity at a
	// time. Music fades a few seconds after playback stops.

	const musicActivity =
		settingsMusicModeEnabled &&
		settingsMusicCompactNotch &&
		mediaInfo.has_media &&
		musicActivityVisible;
	const collapsedMode = musicActivity ? "music" : "status";

	const showShelfBubble = shelfItems.length > 0 && !isOpen && isVisible && !isHidden;
	restingModeRef.current = collapsedMode;
	bloomModeRef.current = bloomMode;

	// A click opens the last-used panel when memory is enabled, otherwise the
	// selected default. The controls panel is always available as a fallback.
	const openingPanel = (): NotchPanel => {
		const preferred = rememberPanelEnabled ? (lastPanel ?? defaultPanel) : defaultPanel;
		return panelAvailable(preferred) ? preferred : "command-center";
	};

	// Expands the notch into a panel, or into an activity's own view.
	const openNotch = (view: NotchPanel | "music" | "shelf") => {
		setIsOpen(true);
		setBloomMode(view);
	};

	const rememberPanel = (panel: NotchPanel) => {
		if (panel === lastPanel) return;
		setLastPanel(panel);
		localStorage.setItem("bloom-notch-last-panel", panel);
		invoke("save_setting", { key: "bloom-notch-last-panel", value: panel }).catch(console.error);
	};
	rememberPanelRef.current = rememberPanel;

	// In peek mode, briefly show the notch when a new track starts or playback
	// resumes. The notch itself never switches views for media; the collapsed
	// pill follows the live-activity rule below.
	useEffect(() => {
		const isNewTrackWhilePlaying = mediaInfo.title !== lastTrackRef.current && isPlaying;
		const justStartedPlaying = isPlaying && !lastPlayingRef.current;

		// Peek notch on media events (play start or track change)
		if (notchMode === "peek" && (isNewTrackWhilePlaying || justStartedPlaying)) {
			triggerEventPeek(3000);
		}

		lastTrackRef.current = mediaInfo.title;
		lastPlayingRef.current = isPlaying;
	}, [isPlaying, mediaInfo.title, notchMode, triggerEventPeek]);

	// Reset bloom mode when calendar setting is disabled
	useEffect(() => {
		if (!settingsCalendarEnabled && bloomMode === "calendar") {
			setBloomMode(isOpen ? openingPanel() : collapsedMode);
		}
	}, [settingsCalendarEnabled, bloomMode, isOpen]);

	// Leave the music view when music is turned off or the player ends its media
	// session: an open notch falls back to the remembered panel.
	useEffect(() => {
		if (bloomMode === "music" && !(settingsMusicModeEnabled && mediaInfo.has_media)) {
			setBloomMode(isOpen ? openingPanel() : collapsedMode);
		}
	}, [settingsMusicModeEnabled, mediaInfo.has_media, bloomMode, isOpen]);

	// The collapsed notch always shows its current live activity: music while a
	// session exists, and the clock otherwise. An open notch is left alone.
	useEffect(() => {
		if (isOpen) return;
		if ((bloomMode === "status" || bloomMode === "music") && bloomMode !== collapsedMode) {
			setBloomMode(collapsedMode);
		}
	}, [collapsedMode, bloomMode, isOpen]);

	// Update time
	useEffect(() => {
		const updateTime = () => {
			const now = new Date();
			setTime(
				now.toLocaleTimeString([], {
					hour: "2-digit",
					minute: "2-digit",
					hour12: !timeFormat24h
				})
			);
		};

		updateTime();
		const interval = setInterval(updateTime, 1000);

		return () => {
			clearInterval(interval);
		};
	}, [timeFormat24h]);

	// Listen for Volume Changes
	useEffect(() => {
		let mounted = true;
		void invoke<{ volume: number; is_muted: boolean }>("get_volume_state")
			.then((state) => {
				if (!mounted) return;
				setVolume(state.volume);
				setIsMuted(state.is_muted);
			})
			.catch(console.error);
		const unlisten = listen<{ volume: number; is_muted: boolean }>("volume-change", (event) => {
			setVolume(event.payload.volume);
			setIsMuted(event.payload.is_muted);
		});
		return () => {
			mounted = false;
			unlisten.then((fn) => fn());
		};
	}, []);

	// Poll system metrics for status widgets
	useEffect(() => {
		const fetchMetrics = () => {
			invoke<number>("get_cpu_usage")
				.then(setCpuUsage)
				.catch((e) => console.warn("CPU:", e));
			invoke<number>("get_ram_usage")
				.then(setRamUsage)
				.catch((e) => console.warn("RAM:", e));
			invoke<number>("get_disk_space")
				.then(setDiskSpace)
				.catch((e) => console.warn("Disk:", e));
			invoke<[number, number]>("get_network_speed")
				.then(([up, down]) => {
					setNetUpSpeed(up);
					setNetDownSpeed(down);
				})
				.catch(() => {});
		};
		fetchMetrics();
		const interval = setInterval(fetchMetrics, 3000);
		return () => clearInterval(interval);
	}, []);

	// Native Windows Media Controls - Listen for updates from background worker
	useEffect(() => {
		const unlisten = listen<MediaInfo>("media-update", (event) => {
			const info = event.payload;
			if (!info) return;

			// Keep state updaters pure: playback/artwork no longer schedule other
			// state updates while React evaluates a previous snapshot.
			setIsPlaying(info.is_playing);
			setMediaInfo((previous) => (sameMediaInfo(previous, info) ? previous : info));
		});

		return () => {
			unlisten.then((fn) => fn());
		};
	}, []);

	// Media controls via Tauri commands
	const togglePlayPause = useCallback(async () => {
		try {
			await invoke("media_play_pause");
			setIsPlaying(!isPlaying);
		} catch (e) {
			console.error("Failed to toggle play/pause:", e);
		}
	}, [isPlaying]);

	const skipNext = useCallback(async () => {
		try {
			await invoke("media_next");
		} catch (e) {
			console.error("Failed to skip next:", e);
		}
	}, []);

	const skipPrevious = useCallback(async () => {
		try {
			await invoke("media_previous");
		} catch (e) {
			console.error("Failed to skip previous:", e);
		}
	}, []);

	// Slide-push animation for prev/next buttons
	const prevFront = useAnimation();
	const prevBack = useAnimation();
	const nextFront = useAnimation();
	const nextBack = useAnimation();

	const animatePrev = useCallback(async () => {
		prevFront.set({ x: 0 });
		prevBack.set({ x: 30 });
		prevFront.start({ x: -30, transition: { duration: 0.18, ease: [0.4, 0, 0.2, 1] } });
		await prevBack.start({ x: 0, transition: { duration: 0.18, ease: [0.4, 0, 0.2, 1] } });
		skipPrevious();
	}, [skipPrevious, prevFront, prevBack]);

	const animateNext = useCallback(async () => {
		nextFront.set({ x: 0 });
		nextBack.set({ x: -30 });
		nextFront.start({ x: 30, transition: { duration: 0.18, ease: [0.4, 0, 0.2, 1] } });
		await nextBack.start({ x: 0, transition: { duration: 0.18, ease: [0.4, 0, 0.2, 1] } });
		skipNext();
	}, [skipNext, nextFront, nextBack]);

	const sendVolume = useTrailingThrottle((newVol: number) => {
		invoke("set_volume", { volume: newVol }).catch(() => {});
	}, 50);

	const handleVolumeChange = useCallback(
		(newVol: number) => {
			setVolume(newVol);
			// set_volume unmutes for any non-zero level.
			setIsMuted(newVol === 0);
			sendVolume(newVol);
		},
		[sendVolume]
	);

	// The notch never takes focus, and native `title` tooltips only show for an
	// active window, so the status widgets draw their own. A tooltip opens
	// after a tooltip-like delay, by which time the battery details fetched on
	// enter are in. Under the status strip it hangs below the pill; once the
	// notch is open the pill is the tall panel, so it sits beside the pill on
	// the widget's side instead, level with the widget.
	const showStatusTooltip = useCallback(
		(id: string, widget: HTMLElement) => {
			if (statusTooltipTimerRef.current) clearTimeout(statusTooltipTimerRef.current);
			statusTooltipTimerRef.current = setTimeout(() => {
				const pill = bloomRef.current?.getBoundingClientRect();
				const own = widget.getBoundingClientRect();
				// Client rects are zoomed by the notch scale; offsets are not.
				if (!pill) {
					setStatusTooltip({ id, side: "below", offset: 8 });
				} else if (!isOpen) {
					const gap = (pill.bottom - own.bottom) / scale;
					setStatusTooltip({ id, side: "below", offset: Math.max(0, gap) + 8 });
				} else if (own.left + own.width / 2 >= pill.left + pill.width / 2) {
					const gap = (pill.right - own.right) / scale;
					setStatusTooltip({ id, side: "right", offset: Math.max(0, gap) + 10 });
				} else {
					const gap = (own.left - pill.left) / scale;
					setStatusTooltip({ id, side: "left", offset: Math.max(0, gap) + 10 });
				}
			}, 400);
		},
		[scale, isOpen]
	);

	const hideStatusTooltip = useCallback(() => {
		if (statusTooltipTimerRef.current) clearTimeout(statusTooltipTimerRef.current);
		statusTooltipTimerRef.current = null;
		setStatusTooltip(null);
	}, []);

	// Widgets unmount when the notch collapses, without a mouse leave, and an
	// open card is placed for the notch's shape when it opened.
	useEffect(() => {
		hideStatusTooltip();
	}, [isHovered, isOpen, hideStatusTooltip]);

	// Both sides of the status strip get equal width around the centered
	// clock, so the notch must fit the clock plus twice the wider side. Widget
	// widths vary (network speed especially), so they are measured rather than
	// counted. The width only grows during a hover, so a live label getting
	// shorter doesn't make the notch jitter. Layout widths are unaffected by
	// the notch scale.
	useLayoutEffect(() => {
		if (!isHovered || bloomMode !== "status") {
			if (statusFitWidth !== 0) setStatusFitWidth(0);
			return;
		}
		// offsetWidth, not scrollWidth: an open tooltip card overflows its
		// widget and must not widen the notch.
		const side = Math.max(
			leftWidgetsRef.current?.offsetWidth ?? 0,
			rightWidgetsRef.current?.offsetWidth ?? 0
		);
		const clock = timeCenterRef.current?.offsetWidth ?? 0;
		// The pill's 12px padding on each side, and a 10px gap to the clock.
		const needed = Math.ceil(clock + 2 * (side + 10) + 24);
		if (side > 0 && needed > statusFitWidth) setStatusFitWidth(needed);
	});

	const openSettingsWindow = useCallback(async () => {
		try {
			await invoke("open_settings_window");
		} catch (e) {
			console.error("Failed to open settings window:", e);
		}
	}, []);

	const toggleNotchModeSetting = useCallback(
		async (e: React.MouseEvent) => {
			e.stopPropagation();
			const nextMode = notchMode === "fixed" ? "smart" : notchMode === "smart" ? "peek" : "fixed";
			setNotchMode(nextMode);
			localStorage.setItem("bloom-notch-mode", nextMode);
			invoke("save_setting", { key: "bloom-notch-mode", value: nextMode }).catch(console.error);
			try {
				await invoke("change_notch_mode", { mode: nextMode });
			} catch (err) {
				console.error("Failed to change notch mode:", err);
			}
		},
		[notchMode]
	);

	const toggleCalendarMode = (e: React.MouseEvent) => {
		e.stopPropagation();
		// The time is part of the notch: a click opens it like anywhere else.
		// Once open, it is a shortcut to the calendar.
		if (!isOpen) {
			openNotch(openingPanel());
		} else if (settingsCalendarEnabled) {
			setBloomMode("calendar");
		}
	};

	// Where a status tooltip card sits and how it slides in, by placement.
	const statusTooltipMotion = (side: "below" | "left" | "right", offset: number) => {
		if (side === "below") {
			return {
				style: { top: `calc(100% + ${offset}px)`, left: "50%" },
				hidden: { opacity: 0, x: "-50%", y: -4 },
				shown: { opacity: 1, x: "-50%", y: 0 }
			};
		}
		// Top-aligned with the widget rather than centered on it: the widgets
		// sit near the top of the screen, which would cut a centered card off.
		const nudge = side === "right" ? -4 : 4;
		return {
			style: { top: -8, [side === "right" ? "left" : "right"]: `calc(100% + ${offset}px)` },
			hidden: { opacity: 0, x: nudge, y: 0 },
			shown: { opacity: 1, x: 0, y: 0 }
		};
	};

	// A status widget with its tooltip card, which hangs outside the pill.
	const statusWidget = (
		id: string,
		tooltip: string,
		content: React.ReactNode,
		onEnter?: () => void
	) => (
		<div
			className="passive-feature"
			key={id}
			onMouseEnter={(e) => {
				onEnter?.();
				showStatusTooltip(id, e.currentTarget);
			}}
			onMouseLeave={hideStatusTooltip}
		>
			{content}
			<AnimatePresence>
				{statusTooltip?.id === id &&
					(() => {
						const placed = statusTooltipMotion(statusTooltip.side, statusTooltip.offset);
						return (
							<motion.div
								className="status-tooltip"
								style={placed.style}
								initial={placed.hidden}
								animate={placed.shown}
								exit={placed.hidden}
								transition={{ duration: 0.15 }}
							>
								{tooltip}
							</motion.div>
						);
					})()}
			</AnimatePresence>
		</div>
	);

	// Render a status widget by ID
	const renderStatusWidget = (id: string) => {
		switch (id) {
			case "weather":
				if (!settingsWeatherEnabled || temperature === null) return null;
				return statusWidget(
					"weather",
					cityName ? `${weatherCondition} — ${cityName}` : weatherCondition,
					<>
						<WeatherIcon size={12} strokeWidth={2.2} />
						<span className="label">
							{temperature}°{tempUnit === "fahrenheit" ? "F" : "C"}
						</span>
					</>
				);

			case "cpu":
				return statusWidget(
					"cpu",
					"CPU Usage",
					<>
						<Cpu size={12} strokeWidth={2} />
						<span className="label">{cpuUsage}%</span>
					</>
				);
			case "ram":
				return statusWidget(
					"ram",
					"RAM Usage",
					<>
						<MemoryStick size={12} strokeWidth={2} />
						<span className="label">{Math.round(ramUsage)}%</span>
					</>
				);
			case "disk":
				return statusWidget(
					"disk",
					"Free Disk Space",
					<>
						<HardDrive size={12} strokeWidth={2} />
						<span className="label">{diskSpace}GB</span>
					</>
				);
			case "net":
				return statusWidget(
					"net",
					"Network Speed",
					<>
						<ArrowUpDown size={12} strokeWidth={2} />
						<span className="label">
							↑{formatBytes(netUpSpeed)} ↓{formatBytes(netDownSpeed)}
						</span>
					</>
				);
			default:
				return null;
		}
	};

	const formatBytes = (bytes: number) => {
		if (bytes < 1024) return `${bytes}B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}K`;
		return `${(bytes / (1024 * 1024)).toFixed(1)}M`;
	};

	const activeView = islandView({
		open: isOpen,
		mode: bloomMode,
		calendarEnabled: settingsCalendarEnabled,
		musicEnabled: settingsMusicModeEnabled,
		hasMedia: mediaInfo.has_media
	});

	// Music mode shows any time we have media info (playing or paused) and music mode setting is enabled
	const isMusicMode = mediaInfo.has_media && bloomMode === "music" && settingsMusicModeEnabled;
	// A media session remains reachable on hover after its activity fades, or
	// whenever the compact music display is disabled.
	const showMusicOnStatusHover =
		!isOpen &&
		isHovered &&
		bloomMode === "status" &&
		settingsMusicModeEnabled &&
		mediaInfo.has_media &&
		(!musicActivityVisible || !settingsMusicCompactNotch);
	const showCompactMusicIcons = isMusicMode || showMusicOnStatusHover;

	// The 24-hour clock is ~16px narrower than the 12-hour one.
	const clock24hTrim = timeFormat24h ? 16 : 0;

	// Calculate width dynamically based on enabled features
	const getDynamicWidth = () => {
		if (isCalendarMode) {
			return Math.min(620 + Math.max(0, scales.font - 1) * 140, window.innerWidth / scale - 48);
		}
		if (activeView === "shelf") return 380;
		if (activeView === "command-center") return 350;
		if (showMusicOnStatusHover) {
			return 206 + (settingsVisualizerEnabled ? 30 : 0);
		}
		if (bloomMode === "status" && isHovered) {
			const totalWidgets = statusWidgets.left.length + statusWidgets.right.length;
			return Math.max(Math.min(200 + totalWidgets * 50, 380) - clock24hTrim, statusFitWidth);
		}
		if (activeView === "music") return mediaLayout === "compact" ? 300 : 340;
		if (showUpdatePulse && !isHovered) return 200;

		// Reserve the avatar's 28px plus its 8px gap without shrinking clock/media.
		let w = 176 - clock24hTrim;
		if (isMusicMode) {
			if (settingsVisualizerEnabled && isPlaying) w += 30;
			if (settingsAlbumArtEnabled) w += 30;
		}

		return w;
	};

	const getDynamicHeight = () => {
		if (!isExpanded || !isVisible || isHidden) {
			return isImpacted ? 28.9 : 44.2;
		}
		// Six stable calendar rows; browsing months never changes the region.
		const headerExtra = notchHeaderHeight - 36;
		if (activeView === "calendar") {
			return Math.min(
				notchHeaderHeight + 380 + Math.max(0, scales.font - 1) * 100,
				window.innerHeight / scale - 24
			);
		}
		// Shelf expands on mode alone (no hover needed: there is no cursor
		// hover during an OS drag) and is sized to exactly one row of tiles,
		// plus room for the scrollbar once the tiles overflow it.
		if (activeView === "shelf") {
			if (shelfItems.length === 0) return 130 + headerExtra;
			return (shelfOverflowing ? 190 : 176) + headerExtra;
		}
		if (activeView === "command-center") return 190 + headerExtra;
		if (activeView === "collapsed") return notchHeaderHeight;
		if (activeView === "music") {
			const hasProgressBar = (mediaInfo.duration_ms ?? 0) > 0;
			let h = mediaLayout === "compact" ? (hasProgressBar ? 132 : 116) : 120;
			if (mediaLayout === "compact") {
				if (compactVolumeExpanded) h += 36;
			}
			return h;
		}
		return notchHeaderHeight;
	};

	const isCalendarMode = activeView === "calendar";
	// Arrows switch between the two tool panels; wheel/swipe can also open one
	// directly from the collapsed pill or a live activity.
	const showPanelArrows =
		isOpen && asNotchPanel(bloomMode) !== null && NOTCH_PANELS.filter(panelAvailable).length > 1;

	// Close compact media player expansions when notch is unhovered or mode changes
	useEffect(() => {
		if (mediaLayout === "compact") {
			setCompactVolumeExpanded(false);
		}
	}, [isHovered, mediaLayout, bloomMode]);

	// The pill's target geometry: the animation springs toward it, and the
	// window region below is cut from it, so the region is already at the
	// final size while the pill animates.
	const pillTargetY = !isReady ? 250 : isVisible ? (isHidden ? -100 : 0) : -150;
	const pillTargetWidth = !isReady
		? 34
		: isExpanded && isVisible && !isHidden
			? getDynamicWidth()
			: isImpacted
				? 39.1
				: 30.6;
	const pillTargetHeight = !isReady ? 34 : getDynamicHeight();
	// Shadow and the edge flares beside the pill.
	pillTargetRef.current = notchPillBox(
		window.innerWidth,
		scale,
		pillTargetY,
		pillTargetWidth,
		pillTargetHeight,
		24
	);

	// Clip the notch window to what it draws (see window_region.rs): outside
	// these rectangles the window takes no clicks, whatever its click-through
	// state. Sent after every render, but only when the region changes.
	useEffect(() => {
		if (windowLabel === "main") updateNotchRegion();
	});

	return (
		<div className="screen" style={{ overflow: "hidden" }}>
			{/* Screen Corners (Top) */}
			<AnimatePresence>
				{isVisible && settingsCornersEnabled && (
					<>
						<motion.div
							className="screen-corner top-left"
							initial={{ opacity: 0 }}
							animate={{ opacity: 1, filter: "blur(0px)" }}
							exit={{ opacity: 0, filter: "blur(10px)" }}
						/>
						<motion.div
							className="screen-corner top-right"
							initial={{ opacity: 0 }}
							animate={{ opacity: 1, filter: "blur(0px)" }}
							exit={{ opacity: 0, filter: "blur(10px)" }}
						/>
					</>
				)}
			</AnimatePresence>

			<div
				style={
					{
						zoom: scale,
						width: "100%",
						display: "flex",
						justifyContent: "center",
						position: "relative",
						"--notch-header-height": `${notchHeaderHeight}px`
					} as CSSProperties
				}
			>
				<motion.div
					ref={bloomRef}
					className={`bloom ${isHovered ? "expanded" : ""} ${isImpacted ? "is-impacted" : ""} ${isDragHovering ? "drag-hover" : ""}`}
					onMouseEnter={() => setIsNotchHovered(true)}
					onMouseLeave={() => setIsNotchHovered(false)}
					onWheel={handleWheel}
					initial={{
						y: 250,
						width: 30.6,
						height: 44.2,
						borderTopLeftRadius: 18,
						borderTopRightRadius: 18,
						borderBottomLeftRadius: 18,
						borderBottomRightRadius: 18,
						scaleX: 1,
						scaleY: 1,
						opacity: 0
					}}
					animate={{
						y: pillTargetY,
						width: pillTargetWidth,
						height: pillTargetHeight,
						opacity: isVisible ? 1 : 0,
						scaleX: 1,
						scaleY: 1,
						borderTopLeftRadius: isImpacted ? 0 : 18,
						borderTopRightRadius: isImpacted ? 0 : 18,
						borderBottomLeftRadius: isCalendarMode ? 28 : 18,
						borderBottomRightRadius: isCalendarMode ? 28 : 18,
						filter: isVisible ? "blur(0px)" : "blur(8px)",
						pointerEvents: isVisible ? "auto" : "none"
					}}
					onClick={(e) => {
						e.stopPropagation();
						if (!isOpen) openNotch(openingPanel());
					}}
					onHoverStart={() => {
						setIsHovered(true);
						// Hovering the shelf cancels its post-drop auto-close.
						if (bloomMode === "shelf") clearShelfCollapse();
					}}
					onHoverEnd={releaseNotchHover}
					style={{ originY: 0 }}
					transition={{
						width: { type: "spring", stiffness: 400, damping: 31 },
						height: { type: "spring", stiffness: 450, damping: 29 },
						y: { type: "spring", stiffness: 550, damping: 45, mass: 0.8, restDelta: 0.001 },
						opacity: { duration: 0.2 },
						borderTopLeftRadius: { type: "spring", stiffness: 1000, damping: 40 },
						borderTopRightRadius: { type: "spring", stiffness: 1000, damping: 40 },
						borderBottomLeftRadius: { type: "spring", stiffness: 1000, damping: 40 },
						borderBottomRightRadius: { type: "spring", stiffness: 1000, damping: 40 },
						default: { type: "spring", stiffness: 500, damping: 30, mass: 1 }
					}}
				>
					<EdgeSurface edge="top" flares={isImpacted} />

					{isExpanded && (
						<motion.div
							key={activeView}
							data-island-view={activeView}
							initial={{ opacity: 0 }}
							animate={{ opacity: 1 }}

							transition={{ duration: 0.1 }}
							style={{
								width: "100%",
								height: "100%",
								display: "flex",
								flexDirection: "column",
								alignItems: "center",
								position: "relative",
								borderRadius: "inherit"
							}}
						>
							{/* One view owns the panel area; only its entrance is animated. */}

							{activeView === "music" ? (
								<motion.div
									key="expanded-music"
									className="expanded-music-container"
									initial={{ opacity: 0, scale: 0.98 }}
									animate={{ opacity: 1, scale: 1 }}

									transition={{ type: "spring", stiffness: 500, damping: 30 }}
								>
									{mediaLayout === "compact" ? (
										<CompactMediaPlayer
											mediaInfo={mediaInfo}
											albumArtUrl={albumArtUrl}
											isPlaying={isPlaying}
											volume={displayVolume}
											volumeExpanded={compactVolumeExpanded}
											onVolumeExpandedChange={setCompactVolumeExpanded}
											onTogglePlayPause={togglePlayPause}
											onVolumeChange={handleVolumeChange}
											prevFront={prevFront}
											prevBack={prevBack}
											nextFront={nextFront}
											nextBack={nextBack}
											onAnimatePrev={animatePrev}
											onAnimateNext={animateNext}
											onLayoutChange={(layout) => {
												setMediaLayout(layout);
												localStorage.setItem("bloom-media-layout", layout);
												invoke("save_setting", {
													key: "bloom-media-layout",
													value: layout
												}).catch(console.error);
												window.dispatchEvent(
													new CustomEvent("settings-changed", {
														detail: { key: "media-layout", value: layout }
													})
												);
											}}
										/>
									) : (
										<div className="compact-premium-layout">
											<div className="album-art-section">
												<motion.div
													className="premium-album-art"
													whileHover={{ scale: 1.05 }}
													whileTap={{ scale: 0.95 }}
													onClick={(e) => {
														e.stopPropagation();
														setMediaLayout("compact");
														localStorage.setItem("bloom-media-layout", "compact");
														invoke("save_setting", {
															key: "bloom-media-layout",
															value: "compact"
														}).catch(console.error);
														window.dispatchEvent(
															new CustomEvent("settings-changed", {
																detail: { key: "media-layout", value: "compact" }
															})
														);
													}}
													style={{ cursor: "pointer" }}
												>
													<AnimatePresence mode="wait" initial={false}>
														{albumArtUrl ? (
															<motion.img
																key={albumArtUrl}
																src={albumArtUrl}
																alt="Art"
																initial={{ rotateY: 90, opacity: 0 }}
																animate={{ rotateY: 0, opacity: 1 }}
																exit={{ rotateY: -90, opacity: 0 }}
																transition={{ duration: 0.3, ease: "easeInOut" }}
																style={{ width: "100%", height: "100%", objectFit: "cover" }}
															/>
														) : (
															<motion.div
																key="placeholder"
																className="art-placeholder-mini"
																initial={{ rotateY: 90, opacity: 0 }}
																animate={{ rotateY: 0, opacity: 1 }}
																exit={{ rotateY: -90, opacity: 0 }}
																transition={{ duration: 0.3, ease: "easeInOut" }}
															>
																<MusicNoteIcon size={32} className="music-placeholder-svg" />
															</motion.div>
														)}
													</AnimatePresence>
												</motion.div>
											</div>

											<div className="metadata-controls-section-middle">
												<div className="track-header-row">
													<div className="track-info-middle">
														<TitleMarquee title={mediaInfo.title} />
														<span className="premium-artist">{mediaInfo.artist}</span>
													</div>
													<div className="header-visualizer-wrap">
														<div className="header-visualizer">
															<Visualizer isPlaying={isPlaying} bars={5} height={18} />
														</div>
														<motion.button
															className="classic-audio-output-btn"
															onClick={(e) => {
																e.stopPropagation();
																invoke("open_sound_settings").catch(() => {});
															}}
															whileTap={{ scale: 0.9 }}
															title="Audio Output"
														>
															<HeadphonesIcon size={20} style={{ opacity: 0.5 }} />
														</motion.button>
													</div>
												</div>

												<div className="controls-row-sleek">
													<motion.button
														aria-label="Previous track"
														className="sleek-btn previous-btn"
														onClick={(e) => {
															e.stopPropagation();
															animatePrev();
														}}
														whileHover={{ scale: 1.2 }}
														whileTap={{ scale: 0.9 }}
														transition={{ type: "spring", stiffness: 400, damping: 25 }}
													>
														<div
															style={{
																position: "relative",
																width: 32,
																height: 16,
																overflow: "hidden"
															}}
														>
															<motion.div
																animate={prevBack}
																style={{
																	position: "absolute",
																	inset: 0,
																	display: "flex",
																	alignItems: "center",
																	justifyContent: "center"
																}}
															>
																<SkipBackIcon size={32} />
															</motion.div>
															<motion.div
																animate={prevFront}
																style={{
																	position: "absolute",
																	inset: 0,
																	display: "flex",
																	alignItems: "center",
																	justifyContent: "center"
																}}
															>
																<SkipBackIcon size={32} />
															</motion.div>
														</div>
													</motion.button>

													<motion.button
														aria-label={isPlaying ? "Pause" : "Play"}
														className="sleek-btn play-pause-btn-floating"
														onClick={(e) => {
															e.stopPropagation();
															togglePlayPause();
														}}
														whileHover={{ scale: 1.2 }}
														whileTap={{ scale: 0.95 }}
														transition={{ type: "spring", stiffness: 400, damping: 25 }}
													>
														<AnimatePresence mode="wait" initial={false}>
															<motion.div
																key={isPlaying ? "pause" : "play"}
																initial={{ opacity: 0, scale: 0.8 }}
																animate={{ opacity: 1, scale: 1 }}
																exit={{ opacity: 0, scale: 0.8 }}
																transition={{ duration: 0.15 }}
																style={{
																	display: "flex",
																	alignItems: "center",
																	justifyContent: "center"
																}}
															>
																{isPlaying ? <PauseIcon size={26} /> : <PlayIcon size={26} />}
															</motion.div>
														</AnimatePresence>
													</motion.button>

													<motion.button
														aria-label="Next track"
														className="sleek-btn next-btn"
														onClick={(e) => {
															e.stopPropagation();
															animateNext();
														}}
														whileHover={{ scale: 1.2 }}
														whileTap={{ scale: 0.9 }}
														transition={{ type: "spring", stiffness: 400, damping: 25 }}
													>
														<div
															style={{
																position: "relative",
																width: 32,
																height: 16,
																overflow: "hidden"
															}}
														>
															<motion.div
																animate={nextBack}
																style={{
																	position: "absolute",
																	inset: 0,
																	display: "flex",
																	alignItems: "center",
																	justifyContent: "center"
																}}
															>
																<SkipForwardIcon size={32} />
															</motion.div>
															<motion.div
																animate={nextFront}
																style={{
																	position: "absolute",
																	inset: 0,
																	display: "flex",
																	alignItems: "center",
																	justifyContent: "center"
																}}
															>
																<SkipForwardIcon size={32} />
															</motion.div>
														</div>
													</motion.button>
												</div>

												<div className="volume-slider-container">
													<VolumeLowIcon size={12} style={{ opacity: 0.5 }} />
													<div className="slider-track-premium">
														<input
															aria-label="Media volume"
															type="range"
															min="0"
															max="1"
															step="0.01"
															value={displayVolume}
															onChange={(e) => handleVolumeChange(parseFloat(e.target.value))}
															onPointerDown={(e) => e.stopPropagation()}
															onClick={(e) => e.stopPropagation()}
															className="premium-slider"
														/>
														<div
															className="slider-progress-fill"
															style={{ width: `${displayVolume * 100}%` }}
														/>
													</div>
													<VolumeHighIcon size={14} style={{ opacity: 0.5 }} />
												</div>
											</div>
										</div>
									)}
								</motion.div>
							) : (
								<motion.div
									key="standard-view-group"
									initial={{ opacity: 0, y: -5 }}
									animate={{ opacity: 1, y: 0 }}

									transition={{ duration: 0.2 }}
									style={{ width: "100%" }}
								>
									<div className="main-row">
										<AnimatePresence mode="wait">
											{showUpdatePulse && !isHovered ? (
												<motion.div
													key="update-pulse-view"
													initial={{ opacity: 0, scale: 0.95, filter: "blur(4px)" }}
													animate={{ opacity: 1, scale: 1, filter: "blur(0px)" }}
													exit={{ opacity: 0, scale: 1.05, filter: "blur(4px)" }}
													className="power-pulse-content"
												>
													<GreenDownArrowIcon />
													<span className="label" style={{ color: "#32D74B" }}>
														Update Available
													</span>
												</motion.div>
											) : (
												<motion.div
													key="standard-view"
													className="main-row-inner"
													initial={{ opacity: 0 }}
													animate={{ opacity: 1 }}
													exit={{ opacity: 0 }}
												>
													{/* Left: music on hover while a paused session remains, otherwise status widgets */}
													<div className="side-content left">
														{showCompactMusicIcons && settingsVisualizerEnabled ? (
															<AnimatePresence>
																{settingsVisualizerEnabled && (
																	<motion.div
																		key="visualizer"
																		initial={{ scale: 0.8, opacity: 0 }}
																		animate={{ scale: 1, opacity: 1 }}
																		exit={{ scale: 0.8, opacity: 0 }}
																	>
																		<Visualizer isPlaying={isPlaying} />
																	</motion.div>
																)}
															</AnimatePresence>
														) : !showCompactMusicIcons &&
														  isHovered &&
														  statusWidgets.left.length > 0 ? (
															<motion.div
																key="left-widgets"
																ref={leftWidgetsRef}
																className="passive-features-group"
																initial={{ opacity: 0 }}
																animate={{ opacity: 1 }}
																exit={{ opacity: 0 }}
																transition={{ duration: 0.2 }}
															>
																{statusWidgets.left.map(renderStatusWidget)}
															</motion.div>
														) : null}
													</div>

													{/* Center - Time (always visible) */}
													<div className="time-center" ref={timeCenterRef}>
														<BloomAvatar active={isVisible && !isHidden && isExpanded} playing={isPlaying} />
														<AnimatePresence>
															{showPanelArrows && (
																<motion.button
																	key="panel-prev"
																	type="button"
																	className="panel-arrow prev"
																	aria-label="Previous panel"
																	initial={{ opacity: 0, x: 4 }}
																	animate={{ opacity: 1, x: 0 }}
																	exit={{ opacity: 0, x: 4 }}
																	transition={{ duration: 0.15 }}
																	onClick={(e) => {
																		e.stopPropagation();
																		cyclePanel(-1);
																	}}
																>
																	<ChevronLeft size={12} strokeWidth={2.5} />
																</motion.button>
															)}
														</AnimatePresence>
														<div className="time-flip-container" onClick={toggleCalendarMode}>
															<AnimatePresence initial={false}>
																{
																	<motion.span
																		key="clock"
																		className="time"
																		initial={{ rotateX: -90, opacity: 0 }}
																		animate={{ rotateX: 0, opacity: 1 }}
																		exit={{ rotateX: 90, opacity: 0 }}
																		transition={{ type: "spring", stiffness: 600, damping: 30 }}
																	>
																		{time}
																	</motion.span>
																}
															</AnimatePresence>
														</div>
														<AnimatePresence>
															{showPanelArrows && (
																<motion.button
																	key="panel-next"
																	type="button"
																	className="panel-arrow next"
																	aria-label="Next panel"
																	initial={{ opacity: 0, x: -4 }}
																	animate={{ opacity: 1, x: 0 }}
																	exit={{ opacity: 0, x: -4 }}
																	transition={{ duration: 0.15 }}
																	onClick={(e) => {
																		e.stopPropagation();
																		cyclePanel(1);
																	}}
																>
																	<ChevronRight size={12} strokeWidth={2.5} />
																</motion.button>
															)}
														</AnimatePresence>
														{updateAvailable && showUpdateIndicator && (
															<div className="update-dot" />
														)}
													</div>

													{/* Right: album art on a retained music session, otherwise status widgets */}
													<div className="side-content right">
														{showCompactMusicIcons &&
														(settingsAlbumArtEnabled || showMusicOnStatusHover) ? (
															<AnimatePresence mode="wait">
																<motion.div
																	key="album-art"
																	className="album-art-transition-wrapper"
																	initial={{ opacity: 0, scale: 0.8 }}
																	animate={{ opacity: 1, scale: 1 }}
																	exit={{ opacity: 0, y: -20, scale: 0.8, filter: "blur(8px)" }}
																	transition={{ duration: 0.12 }}
																>
																	<button
																		className={`album-art album-art-activity${!isPlaying ? " paused" : ""}`}
																		aria-label="Open music"
																		onClick={(e) => {
																			e.stopPropagation();
																			openNotch("music");
																		}}
																		onContextMenu={(e) => {
																			e.preventDefault();
																			e.stopPropagation();
																			skipPrevious();
																		}}
																	>
																		<div className="album-art-inner">
																			<AnimatePresence mode="wait" initial={false}>
																				{settingsAlbumArtEnabled && albumArtUrl ? (
																					<motion.img
																						key={albumArtUrl}
																						src={albumArtUrl}
																						alt="Art"
																						draggable={false}
																						initial={{ rotateY: 90, opacity: 0 }}
																						animate={{ rotateY: 0, opacity: 1 }}
																						exit={{ rotateY: -90, opacity: 0 }}
																						transition={{ duration: 0.25, ease: "easeInOut" }}
																						style={{
																							width: "100%",
																							height: "100%",
																							objectFit: "cover"
																						}}
																					/>
																				) : (
																					<motion.div
																						key="compact-placeholder"
																						className="album-art-placeholder"
																						initial={{ rotateY: 90, opacity: 0 }}
																						animate={{ rotateY: 0, opacity: 1 }}
																						exit={{ rotateY: -90, opacity: 0 }}
																						transition={{ duration: 0.25, ease: "easeInOut" }}
																					>
																						<MusicNoteIcon className="music-placeholder-svg-small" />
																					</motion.div>
																				)}
																			</AnimatePresence>
																		</div>
																	</button>
																</motion.div>
															</AnimatePresence>
														) : !showCompactMusicIcons &&
														  isHovered &&
														  statusWidgets.right.length > 0 ? (
															<motion.div
																key="right-widgets"
																ref={rightWidgetsRef}
																className="passive-features-group"
																initial={{ opacity: 0 }}
																animate={{ opacity: 1 }}
																exit={{ opacity: 0 }}
																transition={{ duration: 0.2 }}
															>
																{statusWidgets.right.map(renderStatusWidget)}
															</motion.div>
														) : null}
													</div>
												</motion.div>
											)}
										</AnimatePresence>
									</div>
								</motion.div>
							)}

							{/* Command Center Panel */}

							{activeView === "command-center" && (
								<motion.div
									className="command-center-content-minimal"
									onClick={(e) => e.stopPropagation()}
									initial={{ opacity: 0 }}
									animate={{ opacity: 1 }}

									transition={{ type: "spring", stiffness: 400, damping: 30 }}
								>
									{/* Pills Grid */}
									<div className="cc-pills-grid cc-pills-island">
										{/* Notch Mode Pill */}
										<button
											type="button"
											className={`cc-pill-tile ${notchMode === "fixed" ? "active" : ""}`}
											onClick={toggleNotchModeSetting}
											title="Cycle notch mode: Fixed / Smart / Peek"
										>
											<div className="cc-pill-icon-wrapper">
												<NotchIcon />
											</div>
											<div className="cc-pill-info">
												<span className="cc-pill-title">Notch Mode</span>
												<span className="cc-pill-status">
													{notchMode === "fixed"
														? "Fixed"
														: notchMode === "smart"
															? "Smart"
															: "Peek"}
												</span>
											</div>
										</button>
									</div>

									{/* Circular Actions Row */}
									<div className="cc-circular-actions-row">
										<button
											className="cc-circular-btn"
											onClick={(e) => {
												e.stopPropagation();
												openSettingsWindow();
											}}
											title="Bloom Settings"
										>
											<SettingsIcon />
										</button>
										<button
											className="cc-circular-btn"
											onClick={(e) => {
												e.stopPropagation();
												invoke("restart_bloom");
											}}
											title="Restart Bloom"
										>
											<ReloadIcon />
										</button>
									</div>

									{/* Classic Sliders Area */}
									<div className="cc-classic-sliders-area">
										{/* Volume Slider */}
										<div className="cc-classic-slider-row">
											<div className="cc-classic-slider-label">
												<VolumeLowIcon style={{ opacity: 0.5 }} />
												<span>Volume</span>
											</div>
											<div className="cc-classic-slider-track">
												<input
													type="range"
													min="0"
													max="1"
													step="0.01"
													value={displayVolume}
													onChange={(e) => handleVolumeChange(parseFloat(e.target.value))}
													onPointerDown={(e) => e.stopPropagation()}
													onClick={(e) => e.stopPropagation()}
													className="cc-classic-input"
												/>
												<div
													className="cc-classic-fill"
													style={{ width: `${displayVolume * 100}%` }}
												/>
											</div>
											<span className="cc-classic-percentage">
												{Math.round(displayVolume * 100)}%
											</span>
										</div>
									</div>
								</motion.div>
							)}

							{/* Calendar */}

							{activeView === "calendar" && (
								<motion.div
									className="calendar-content"
									onClick={(e) =>
										e.stopPropagation()
									} /* Block mode switches when clicking inside */
									initial={{ opacity: 0 }}
									animate={{ opacity: 1 }}

									transition={{ type: "spring", stiffness: 400, damping: 30 }}
								>
									<IslandCalendar />
								</motion.div>
							)}

							{/* Shelf Panel */}

							{activeView === "shelf" && (
								<motion.div
									className="shelf-content"
									onClick={(e) => e.stopPropagation()}
									onWheel={handleShelfWheel}
									initial={{ opacity: 0 }}
									animate={{ opacity: 1 }}

									transition={{ type: "spring", stiffness: 400, damping: 30 }}
								>
									{shelfNotice && <div className="shelf-notice">{shelfNotice}</div>}
									{shelfItems.length === 0 ? (
										<div className="shelf-empty">
											<span>Drop files or selected text here</span>
										</div>
									) : (
										<div
											className={`shelf-items-grid ${shelfOverflowing ? "overflowing" : ""}`}
											ref={shelfGridRef}
										>
											<AnimatePresence mode="popLayout" onExitComplete={measureShelfOverflow}>
												{shelfItems.map((item) => {
													const preview = item.path ? shelfThumbs[item.path] : undefined;
													const thumb = preview?.thumb;
													return (
														<motion.div
															key={item.id}
															layout
															initial={
																shelfNewIdsRef.current.has(item.id)
																	? { opacity: 0, scale: 0.82 }
																	: false
															}
															animate={{ opacity: 1, scale: 1 }}
															exit={{
																opacity: 0,
																scale: 0.82,
																transition: { duration: 0.16, ease: "easeOut" }
															}}
															transition={{
																type: "spring",
																stiffness: 520,
																damping: 38,
																mass: 0.7
															}}
															className={`shelf-tile ${item.type === "text" || (!thumb && preview?.text) ? "is-text" : ""}`}
															title={item.type === "text" ? item.text?.slice(0, 1000) : item.path}
															onPointerDown={(e) => handleShelfPointerDown(e, item)}
															onPointerMove={(e) => handleShelfPointerMove(e, item)}
															onClick={(e) => handleShelfTileClick(e, item)}
														>
															<div className="shelf-tile-preview">
																{item.type === "text" ? (
																	<div className="shelf-tile-text">
																		<Type
																			size={11}
																			strokeWidth={2}
																			className="shelf-tile-text-icon"
																		/>
																		<span className="shelf-tile-text-body">{item.text}</span>
																	</div>
																) : !thumb && preview?.text ? (
																	// A document or text file: its opening text,
																	// marked with its type's icon.
																	<div className="shelf-tile-text">
																		<ShelfTypeIcon
																			name={item.name}
																			folder={false}
																			size={11}
																			className="shelf-tile-text-icon"
																		/>
																		<span className="shelf-tile-text-body">{preview.text}</span>
																	</div>
																) : (
																	<>
																		<div className="shelf-tile-placeholder">
																			{preview && !thumb ? (
																				<ShelfTypeIcon name={item.name} folder={preview.folder} />
																			) : null}
																		</div>
																		{thumb ? <img src={thumb} alt="" draggable={false} /> : null}
																	</>
																)}
															</div>
															<span className="shelf-tile-name">
																{item.type === "text"
																	? `Text · ${item.text?.length ?? 0} chars`
																	: item.name}
															</span>
															<button
																className="shelf-tile-remove"
																title="Remove from shelf"
																onClick={(e) => {
																	e.stopPropagation();
																	removeShelfItem(item.id);
																}}
															>
																<X size={11} strokeWidth={2.5} />
															</button>
														</motion.div>
													);
												})}
											</AnimatePresence>
										</div>
									)}
								</motion.div>
							)}
						</motion.div>
					)}
				</motion.div>
				{/* The shelf's items, as a bubble on the left of the collapsed pill */}
				<AnimatePresence onExitComplete={() => reportNotchRect.current()}>
					{showShelfBubble && (
						<motion.button
							key="shelf-bubble"
							ref={shelfBubbleRef}
							type="button"
							className={`activity-bubble shelf-bubble ${isHovered ? "expanded" : ""}`}
							aria-label={`Open shelf: ${shelfItems.length} item${shelfItems.length === 1 ? "" : "s"}`}
							// Anchored at the pill's centre, offset past its left edge.
							style={{ right: "50%" }}
							initial={{ opacity: 0, scale: 0.4, x: 28, marginRight: getDynamicWidth() / 2 + 8 }}
							animate={{ opacity: 1, scale: 1, x: 0, marginRight: getDynamicWidth() / 2 + 8 }}
							exit={{ opacity: 0, scale: 0.4, x: 28 }}
							transition={{
								type: "spring",
								stiffness: 400,
								damping: 31,
								opacity: { duration: 0.15 }
							}}
							onAnimationComplete={() => reportNotchRect.current()}
							onClick={(e) => {
								e.stopPropagation();
								setIsHovered(true);
								openNotch("shelf");
							}}
						>
							<Inbox size={12} strokeWidth={2.4} />
							<span>{shelfItems.length}</span>
						</motion.button>
					)}
				</AnimatePresence>
			</div>
		</div>
	);
}

function SettingsIcon() {
	return (
		<svg
			width="14"
			height="14"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			strokeWidth="2.5"
			strokeLinecap="round"
			strokeLinejoin="round"
			style={{ opacity: 0.9 }}
		>
			<circle cx="12" cy="12" r="3" />
			<path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
		</svg>
	);
}

function NotchIcon() {
	return (
		<svg
			width="18"
			height="18"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			strokeWidth="2.5"
			strokeLinecap="round"
			strokeLinejoin="round"
		>
			<path d="M4 3h16a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" />
			<path d="M9 9v4a2 2 0 0 0 2 2h2a2 2 0 0 0 2-2V9" />
		</svg>
	);
}

function ReloadIcon() {
	return (
		<svg
			width="14"
			height="14"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			strokeWidth="2.5"
			strokeLinecap="round"
			strokeLinejoin="round"
		>
			<path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" />
		</svg>
	);
}

export default App;
