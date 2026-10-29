import { describe, expect, test } from "bun:test";
import { islandView, type IslandMode } from "../src/islandView";
import { sameMediaInfo, type MediaInfo } from "../src/mediaInfo";

describe("exclusive island views", () => {
	const enabled = { calendarEnabled: true, musicEnabled: true, hasMedia: true };
	test("closing during a panel or activity switch always returns to a single collapsed view", () => {
		for (const mode of ["music", "calendar", "command-center", "status", "shelf"] as IslandMode[]) {
			expect(islandView({ ...enabled, mode, open: false })).toBe("collapsed");
		}
	});
	test("rapid music, controls, collapse and reopen requests select the latest view", () => {
		const frames = [
			{ mode: "music", open: true },
			{ mode: "command-center", open: true },
			{ mode: "music", open: false },
			{ mode: "music", open: true },
			{ mode: "calendar", open: true }
		] as const;
		expect(frames.map((frame) => islandView({ ...enabled, ...frame }))).toEqual([
			"music",
			"command-center",
			"collapsed",
			"music",
			"calendar"
		]);
	});
	test("an ended or disabled activity cannot retain music geometry or an invisible calendar", () => {
		expect(islandView({ ...enabled, mode: "music", open: true, hasMedia: false })).toBe(
			"command-center"
		);
		expect(islandView({ ...enabled, mode: "music", open: true, musicEnabled: false })).toBe(
			"command-center"
		);
		expect(islandView({ ...enabled, mode: "calendar", open: true, calendarEnabled: false })).toBe(
			"command-center"
		);
	});
});

test("media capability and clock updates are not dropped as duplicate titles", () => {
	const previous: MediaInfo = {
		title: "Track",
		artist: "Player",
		has_media: true,
		is_playing: true,
		position_ms: 1000,
		duration_ms: 60000,
		seek_enabled: false,
		position_updated_at: 10000
	};
	expect(sameMediaInfo(previous, { ...previous })).toBe(true);
	expect(sameMediaInfo(previous, { ...previous, seek_enabled: true })).toBe(false);
	expect(sameMediaInfo(previous, { ...previous, position_updated_at: 11000 })).toBe(false);
	expect(sameMediaInfo(previous, { ...previous, has_media: false })).toBe(false);
});
