export interface MediaInfo {
	title: string;
	artist: string;
	is_playing: boolean;
	has_media: boolean;
	playback_type?: "music" | "video" | "unknown";
	artwork?: string[];
	position_ms?: number;
	duration_ms?: number;
	seek_enabled?: boolean;
	position_updated_at?: number;
}

/** A seek permission or clock refresh is meaningful even if the title and
 * position did not change. The island only displays the first artwork. */
export function sameMediaInfo(a: MediaInfo, b: MediaInfo): boolean {
	return (
		a.title === b.title &&
		a.artist === b.artist &&
		a.is_playing === b.is_playing &&
		a.has_media === b.has_media &&
		a.playback_type === b.playback_type &&
		a.artwork?.[0] === b.artwork?.[0] &&
		a.position_ms === b.position_ms &&
		a.duration_ms === b.duration_ms &&
		a.seek_enabled === b.seek_enabled &&
		a.position_updated_at === b.position_updated_at
	);
}
