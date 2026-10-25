export interface WidgetConfig {
	left: string[];
	right: string[];
}

const supported = new Set(["weather", "cpu", "ram", "disk", "net"]);
export const DEFAULT_STATUS_WIDGETS: WidgetConfig = { left: ["weather"], right: [] };

/** Old desktop settings can still contain a battery widget or malformed IDs. */
export function normalizeStatusWidgets(value: unknown): WidgetConfig {
	if (!value || typeof value !== "object") return DEFAULT_STATUS_WIDGETS;
	const input = value as Partial<WidgetConfig>;
	if (!Array.isArray(input.left) || !Array.isArray(input.right)) return DEFAULT_STATUS_WIDGETS;
	const seen = new Set<string>();
	const zone = (items: unknown[]) => {
		const result: string[] = [];
		for (const id of items) {
			if (typeof id !== "string" || !supported.has(id) || seen.has(id)) continue;
			seen.add(id);
			result.push(id);
			if (result.length === 2) break;
		}
		return result;
	};
	return { left: zone(input.left), right: zone(input.right) };
}
