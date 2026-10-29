export type IslandMode = "music" | "calendar" | "command-center" | "status" | "shelf";
export type IslandView = Exclude<IslandMode, "status"> | "collapsed";

/** Geometry and content must select the same view, including the frame before
 * a disabled/ended activity has been reconciled by its effect. */
export function islandView({
	open,
	mode,
	calendarEnabled,
	musicEnabled,
	hasMedia
}: {
	open: boolean;
	mode: IslandMode;
	calendarEnabled: boolean;
	musicEnabled: boolean;
	hasMedia: boolean;
}): IslandView {
	if (!open) return "collapsed";
	if (mode === "shelf") return "shelf";
	if (mode === "music" && musicEnabled && hasMedia) return "music";
	if (mode === "calendar" && calendarEnabled) return "calendar";
	return "command-center";
}
