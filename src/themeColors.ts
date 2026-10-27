/** Choose the higher-contrast text colour for a Windows/custom accent. */
export function accentForeground(hex: string): string {
	if (!/^#[0-9a-f]{6}$/i.test(hex)) return "#ffffff";
	const values = [1, 3, 5].map(offset => {
		const channel = parseInt(hex.slice(offset, offset + 2), 16) / 255;
		return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
	});
	const luminance = values[0] * 0.2126 + values[1] * 0.7152 + values[2] * 0.0722;
	return luminance > 0.179 ? "#000000" : "#ffffff";
}
