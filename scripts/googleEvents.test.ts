import { expect, test } from "bun:test";
import { eventsForDay, type GoogleEvent } from "../src/calendar/googleEvents";

test("all-day event end is exclusive and cancelled events are omitted", () => {
	const entries: GoogleEvent[] = [
		{ id: "trip", start: { date: "2026-10-30" }, end: { date: "2026-11-02" } },
		{
			id: "cancelled",
			status: "cancelled",
			start: { date: "2026-10-31" },
			end: { date: "2026-11-01" }
		}
	];
	const day = (s: string) =>
		eventsForDay(
			entries,
			s,
			new Date(`${s}T05:00:00Z`),
			new Date(Date.parse(`${s}T05:00:00Z`) + 86400000)
		);
	expect(day("2026-10-31").map((x) => x.id)).toEqual(["trip"]);
	expect(day("2026-11-02")).toEqual([]);
});

test("timed events use real instants and exclusive day boundaries", () => {
	const entries: GoogleEvent[] = [
		{
			id: "overnight",
			start: { dateTime: "2026-10-30T23:00:00-05:00" },
			end: { dateTime: "2026-10-31T01:00:00-05:00" }
		},
		{
			id: "midnight",
			start: { dateTime: "2026-10-30T20:00:00-05:00" },
			end: { dateTime: "2026-10-31T00:00:00-05:00" }
		},
		{
			id: "bad",
			start: { dateTime: "2026-10-31T01:00:00" },
			end: { dateTime: "2026-10-31T02:00:00" }
		}
	];
	const matches = eventsForDay(
		entries,
		"2026-10-31",
		new Date("2026-10-31T05:00:00Z"),
		new Date("2026-11-01T05:00:00Z")
	);
	expect(matches.map((x) => x.id)).toEqual(["overnight"]);
});
