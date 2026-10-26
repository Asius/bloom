export interface ScaleSettings {
	shared: number;
	linked: boolean;
	dock: number | null;
	notch: number | null;
	font: number;
}

export const SCALE_KEYS = ["bloom-scale", "bloom-scale-linked", "bloom-dock-scale", "bloom-notch-scale", "bloom-font-scale"] as const;
export type ScaleKey = typeof SCALE_KEYS[number];

function readNumber(value: unknown, fallback: number): number {
	if (typeof value !== "number" && typeof value !== "string") return fallback;
	const text = String(value).trim();
	if (!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?$/i.test(text)) return fallback;
	const parsed = Number(text);
	return Number.isFinite(parsed)
		? Math.max(0.8, Math.min(1.3, parsed))
		: fallback;
}

export function readScales(values: Record<string, unknown>): ScaleSettings {
	const shared = readNumber(values["bloom-scale"], 1);
	return {
		shared,
		linked: String(values["bloom-scale-linked"] ?? "true") !== "false",
		dock: values["bloom-dock-scale"] == null ? null : readNumber(values["bloom-dock-scale"], shared),
		notch: values["bloom-notch-scale"] == null ? null : readNumber(values["bloom-notch-scale"], shared),
		font: readNumber(values["bloom-font-scale"], 1)
	};
}

export function effectiveScale(settings: ScaleSettings, surface: "dock" | "notch"): number {
	return settings.linked ? settings.shared : settings[surface] ?? settings.shared;
}
