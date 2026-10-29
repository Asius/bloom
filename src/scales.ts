export interface ScaleSettings {
	notch: number;
	font: number;
}
export const SCALE_KEYS = ["bloom-notch-scale", "bloom-font-scale"] as const;
export type ScaleKey = (typeof SCALE_KEYS)[number];
function readNumber(value: unknown): number {
	if (typeof value !== "number" && typeof value !== "string") return 1;
	const text = String(value).trim();
	if (!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?$/i.test(text)) return 1;
	const parsed = Number(text);
	return Number.isFinite(parsed) ? Math.max(0.8, Math.min(1.3, parsed)) : 1;
}
export function readScales(values: Record<string, unknown>): ScaleSettings {
	return {
		notch: readNumber(values["bloom-notch-scale"]),
		font: readNumber(values["bloom-font-scale"])
	};
}
export function effectiveScale(settings: ScaleSettings): number {
	return settings.notch;
}
