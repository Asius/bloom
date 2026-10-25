import React from "react";
import ReactDOM from "react-dom/client";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";
import { SystemTray } from "./components/SystemTray";
import { useSettingsSync } from "./hooks/useSettingsSync";
import { trayHeight } from "./tray";
import "./tray-window.css";

/** Own HWND: input is bounded by its native region, never by dock hover. */
function TrayWindow() {
	const [scale, setScale] = useState(1);
	const [height, setHeight] = useState(36);
	const [loaded, setLoaded] = useState(false);
	useEffect(() => {
		let cancelled = false;
		void invoke<Record<string, unknown>>("load_settings").then(settings => {
			if (cancelled) return;
			const value = Number(settings["bloom-scale"] ?? 1);
			setScale(Number.isFinite(value) && value > 0 ? value : 1);
			setLoaded(true);
		}).catch(console.error);
		const size = listen<number>("dock-tray-height", event => setHeight(trayHeight(event.payload, 1)));
		void size.then(() => { if (!cancelled) void emit("tray-height-request"); });
		return () => { cancelled = true; void size.then(unlisten => unlisten()); };
	}, []);
	useSettingsSync({ "bloom-scale": value => {
		const next = Number(value);
		setScale(Number.isFinite(next) && next > 0 ? next : 1);
	} });
	return loaded ? <SystemTray scale={scale} height={height} /> : null;
}

document.addEventListener("contextmenu", e => e.preventDefault());
ReactDOM.createRoot(document.getElementById("root")!).render(
	<React.StrictMode><TrayWindow /></React.StrictMode>
);
