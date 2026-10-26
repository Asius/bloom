import React from "react";
import ReactDOM from "react-dom/client";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { SystemTray } from "./components/SystemTray";
import { useSettingsSync } from "./hooks/useSettingsSync";
import { trayHeight } from "./tray";
import { initTheme } from "./theme";
import "./tray-window.css";

interface WindowMetrics {
	x: number;
	y: number;
	width: number;
	height: number;
	dpi: number;
}

/** Own HWND: input is bounded by its native region, never by dock hover. */
function TrayWindow() {
	const [scale, setScale] = useState(1);
	const [height, setHeight] = useState(36);
	const [metrics, setMetrics] = useState<WindowMetrics | null>(null);
	const [loaded, setLoaded] = useState(false);
	useEffect(() => {
		let cancelled = false;
		const cleanupTheme = initTheme();
		const trayWindow = getCurrentWebviewWindow();
		const refreshMetrics = async () => {
			try {
				const [position, size, dpi] = await Promise.all([
					trayWindow.innerPosition(),
					trayWindow.innerSize(),
					trayWindow.scaleFactor()
				]);
				if (!cancelled && dpi > 0) {
					setMetrics({ x: position.x, y: position.y, width: size.width, height: size.height, dpi });
				}
			} catch (error) {
				console.error("Could not read tray window geometry:", error);
			}
		};
		void refreshMetrics();
		const moved = trayWindow.onMoved(() => void refreshMetrics());
		const resized = trayWindow.onResized(() => void refreshMetrics());
		void invoke<Record<string, unknown>>("load_settings").then(settings => {
			if (cancelled) return;
			const value = Number(settings["bloom-scale"] ?? 1);
			setScale(Number.isFinite(value) && value > 0 ? value : 1);
			setLoaded(true);
		}).catch(console.error);
		const size = listen<number>("dock-tray-height", event => setHeight(trayHeight(event.payload, 1)));
		void size.then(() => { if (!cancelled) void emit("tray-height-request"); });
		return () => {
			cancelled = true;
			cleanupTheme();
			void size.then(unlisten => unlisten());
			void moved.then(unlisten => unlisten());
			void resized.then(unlisten => unlisten());
		};
	}, []);
	useSettingsSync({ "bloom-scale": value => {
		const next = Number(value);
		setScale(Number.isFinite(next) && next > 0 ? next : 1);
	} });
	return loaded ? <SystemTray scale={scale} height={height} metrics={metrics} /> : null;
}

document.addEventListener("contextmenu", e => e.preventDefault());
ReactDOM.createRoot(document.getElementById("root")!).render(
	<React.StrictMode><TrayWindow /></React.StrictMode>
);
