import { MotionConfig } from "framer-motion";
import React from "react";
import ReactDOM from "react-dom/client";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { SystemTray } from "./components/SystemTray";
import { useBloomFontScale, useDesktopScales } from "./hooks/useDesktopScales";
import { effectiveScale } from "./scales";
import { trayHeight, DEFAULT_TRAY_HEIGHT, type TrayWindowMetrics } from "./tray";
import { initTheme } from "./theme";
import "./tray-window.css";

/** Own HWND: input is bounded by its native region, never by dock hover. */
function TrayWindow() {
	const scales = useDesktopScales();
	const scale = effectiveScale(scales, "dock");
	useBloomFontScale(scales.font);
	const [height, setHeight] = useState(DEFAULT_TRAY_HEIGHT);
	const [metrics, setMetrics] = useState<TrayWindowMetrics | null>(null);
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
		void invoke("load_settings").catch(console.error).finally(() => {
			if (!cancelled) setLoaded(true);
		});
		const size = listen<number>("dock-tray-height", event => {
            if (!cancelled && Number.isFinite(event.payload) && event.payload >= 32 && event.payload <= 96) {
                setHeight(trayHeight(event.payload, 1));
            }
        });
		void size.then(() => { if (!cancelled) void emit("tray-height-request"); });
		return () => {
			cancelled = true;
			cleanupTheme();
			void size.then(unlisten => unlisten());
			void moved.then(unlisten => unlisten());
			void resized.then(unlisten => unlisten());
		};
	}, []);
	return loaded ? <SystemTray scale={scale} height={height} metrics={metrics} /> : null;
}

document.addEventListener("contextmenu", e => e.preventDefault());
ReactDOM.createRoot(document.getElementById("root")!).render(
	<React.StrictMode><MotionConfig reducedMotion="user"><TrayWindow /></MotionConfig></React.StrictMode>
);
