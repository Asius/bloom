/** Google Calendar represents all-day end dates as exclusive civil dates.
 * Timed events are instants and must be compared to the local day's boundaries.
 * This module never reads credentials, contacts, descriptions, or location.
 */
export interface GoogleEvent {
	id: string;
	summary?: string;
	status?: string;
	start: { date?: string; dateTime?: string };
	end: { date?: string; dateTime?: string };
}

function isCivilDate(value: string | undefined): value is string {
	if (!value || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
	const parsed = new Date(`${value}T00:00:00Z`);
	return Number.isFinite(parsed.getTime()) && parsed.toISOString().slice(0, 10) === value;
}

/** Filter a bounded page of expanded instances; cancelled and malformed items are ignored. */
export function eventsForDay(
	events: GoogleEvent[],
	civilDay: string,
	dayStart: Date,
	nextDayStart: Date
): GoogleEvent[] {
	if (
		!isCivilDate(civilDay) ||
		!Number.isFinite(dayStart.getTime()) ||
		!Number.isFinite(nextDayStart.getTime()) ||
		dayStart >= nextDayStart
	)
		return [];
	return events.filter((event) => {
		if (!event || event.status === "cancelled" || !event.id || !event.start || !event.end)
			return false;
		const start = event.start.date,
			end = event.end.date;
		if (isCivilDate(start) && isCivilDate(end))
			return start < end && start <= civilDay && civilDay < end;
		const startTime = event.start.dateTime,
			endTime = event.end.dateTime;
		// RFC3339 offset is required. Never interpret an offsetless time in the PC's zone.
		if (
			!startTime ||
			!endTime ||
			!/(?:Z|[+-]\d\d:\d\d)$/i.test(startTime) ||
			!/(?:Z|[+-]\d\d:\d\d)$/i.test(endTime)
		)
			return false;
		const a = Date.parse(startTime),
			b = Date.parse(endTime);
		return (
			Number.isFinite(a) &&
			Number.isFinite(b) &&
			a < b &&
			a < nextDayStart.getTime() &&
			b > dayStart.getTime()
		);
	});
}
