/** Browser-only regression fixture. Never inject IPC mocks into dist or the app.
 * Build first, run this script, then:
 * node node_modules/vite/bin/vite.js preview --outDir src-tauri/target/ui-preview --host 127.0.0.1 --port 4321
 */
import { cpSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const source = resolve(root, "dist");
const destination = resolve(root, "src-tauri/target/ui-preview");
const { version } = JSON.parse(readFileSync(resolve(root, "package.json"), "utf8"));
if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error("Invalid fixture version");
for (const name of ["index.html", "settings.html", "overlay.html"]) {
	if (readFileSync(resolve(source, name), "utf8").includes("IPC simulado")) {
		throw new Error("Rebuild dist before creating the isolated preview");
	}
}
cpSync(source, destination, { recursive: true });
const fixture = `<script>(${simulateIPC.toString()})(${JSON.stringify(version)})</script>`;
for (const name of ["index.html", "settings.html", "overlay.html"]) {
	const path = resolve(destination, name);
	writeFileSync(path, readFileSync(path, "utf8").replace("<head>", "<head>" + fixture));
}
console.log(`Browser fixture ready: ${destination} (production dist untouched)`);

function simulateIPC(version) {
	const query = new URLSearchParams(location.search);
	const label = location.pathname.includes("settings")
		? "settings"
		: location.pathname.includes("overlay")
			? "overlay"
			: "main";
	const callbacks = new Map(),
		events = new Map();
	let counter = 0;
	let mockGoogleConnected = false;
	const values = {
		"bloom-first-run": "done",
		"bloom-app-version": version,
		"bloom-weather-enabled": "false",
		"bloom-notch-mode": "fixed",
		"bloom-notch-scale": query.get("scale") || "1",
		"bloom-font-scale": query.get("font") || "1",
		"bloom-theme-mode": query.get("theme") || "light",
		"bloom-media-layout": query.get("layout") || "classic",
		"bloom-notch-remember-panel": "false",
		"bloom-notch-default-panel": "command-center",
		"bloom-status-widgets": JSON.stringify({ left: [], right: [] })
	};
	// Fixture data belongs only to this localhost origin, never Tauri's origin.
	for (const key of Object.keys(localStorage))
		if (key.startsWith("bloom-")) localStorage.removeItem(key);
	Object.entries(values).forEach(([key, value]) => localStorage.setItem(key, value));
	function dispatch(event, payload) {
		for (const [id, item] of events)
			if (item.event === event) callbacks.get(item.handler)?.({ event, id, payload });
	}
	window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: (_, id) => events.delete(id) };
	window.__TAURI_INTERNALS__ = {
		metadata: { currentWindow: { label }, currentWebview: { label } },
		transformCallback: (fn) => {
			callbacks.set(++counter, fn);
			return counter;
		},
		unregisterCallback: (id) => callbacks.delete(id),
		convertFileSrc: (path) => path,
		invoke: async (cmd, args = {}) => {
			if (cmd === "google_calendar_connected") return mockGoogleConnected;
			if (cmd === "google_calendar_connect") { mockGoogleConnected = true; dispatch("google-calendar-state", null); return true; }
			if (cmd === "google_calendar_disconnect") { mockGoogleConnected = false; dispatch("google-calendar-state", null); return; }
			if (cmd === "load_settings") return { ...values };
			if (cmd === "settings_file_readable" || cmd === "plugin:window|is_visible") return true;
			if (cmd === "save_setting") {
				values[args.key] = String(args.value);
				dispatch("settings-changed", args);
				return;
			}
			if (cmd === "get_update_state" || cmd === "check_for_updates")
				return { available: false, checking: false, installing: false };
			if (cmd === "get_system_transparency") return true;
			if (cmd === "get_system_accent_color") return "#60cdff";
			if (cmd === "configure_native_material") return args.enabled ? "accepted" : "disabled";
			if (cmd === "get_volume_state") return { volume: 0.5, is_muted: false };
			if (cmd === "get_volume") return 0.5;
			if (cmd === "get_audio_sessions") return [];
			if (cmd === "shelf_prune") return args.paths || [];
			if (cmd === "plugin:event|listen") {
				const id = ++counter;
				events.set(id, { event: args.event, handler: args.handler });
				return id;
			}
			if (cmd === "plugin:event|unlisten") {
				events.delete(args.eventId);
				return;
			}
			if (cmd === "plugin:event|emit" || cmd === "plugin:event|emit_to") {
				dispatch(args.event, args.payload);
				return;
			}
			if (cmd === "plugin:app|version") return version;
			if (cmd === "plugin:autostart|is_enabled") return false;
			if (cmd === "plugin:window|scale_factor") return 1;
			if (/plugin:window.*(?:inner_size|outer_size)/.test(cmd))
				return { width: innerWidth, height: innerHeight };
			if (/plugin:window.*(?:inner_position|outer_position)/.test(cmd)) return { x: 0, y: 0 };
			return null;
		}
	};
	document.addEventListener("DOMContentLoaded", () => {
		document.body.style.background = query.get("theme") === "dark" ? "#282b31" : "#c7cbd1";
		const toolbar = document.createElement("div");
		toolbar.style = "position:fixed;bottom:35px;left:10px;z-index:999999;display:flex;gap:8px";
		const media = {
			title: "Netflix",
			artist: "Test session",
			is_playing: true,
			has_media: true,
			position_ms: 0,
			duration_ms: 0,
			seek_enabled: false
		};
		for (const [label, action] of [
			["Play music", () => dispatch("media-update", { ...media, playback_type: "music", position_updated_at: Date.now() })],
			["Play video", () => dispatch("media-update", { ...media, playback_type: "video", position_updated_at: Date.now() })],
			[
				"Start media",
				() => dispatch("media-update", { ...media, position_updated_at: Date.now() })
			],
			[
				"Pause media",
				() =>
					dispatch("media-update", { ...media, is_playing: false, position_updated_at: Date.now() })
			],
			[
				"End media",
				() =>
					dispatch("media-update", { title: "", artist: "", is_playing: false, has_media: false })
			],
			["Pointer left", () => dispatch("notch-pointer-left", null)],
			[
				"Media seekable",
				() =>
					dispatch("media-update", {
						...media,
						position_ms: 10000,
						duration_ms: 100000,
						seek_enabled: true,
						position_updated_at: Date.now()
					})
			]
		]) {
			const button = document.createElement("button");
			button.textContent = label;
			button.onclick = action;
			toolbar.append(button);
		}
		document.body.append(toolbar);
		const note = document.createElement("div");
		note.textContent = "Prueba de interfaz · IPC simulado · Windows pendiente";
		note.style =
			"position:fixed;bottom:8px;left:10px;z-index:999999;color:#777;font:12px Segoe UI;pointer-events:none";
		document.body.append(note);
	});
}
