import { describe, expect, test } from "bun:test";
import {
	addMonths,
	calendarEntries,
	civilDate,
	colombianHolidays,
	colombianObservances,
	dateKey,
	monthCells
} from "../src/calendar/colombia";

describe("Colombian national calendar", () => {
	test("2026: fixed dates, Easter and Monday transfers match the national calendar", () => {
		expect(colombianHolidays(2026).map((x) => x.date)).toEqual([
			"2026-01-01",
			"2026-01-12",
			"2026-03-23",
			"2026-04-02",
			"2026-04-03",
			"2026-05-01",
			"2026-05-18",
			"2026-06-08",
			"2026-06-15",
			"2026-06-29",
			"2026-07-20",
			"2026-08-07",
			"2026-08-17",
			"2026-10-12",
			"2026-11-02",
			"2026-11-16",
			"2026-12-08",
			"2026-12-25"
		]);
	});
	test("coincident holidays are both retained (June 30, 2025)", () => {
		expect(
			colombianHolidays(2025)
				.filter((x) => x.date === "2025-06-30")
				.map((x) => x.title)
				.sort()
		).toEqual(["Sagrado Corazón de Jesús", "San Pedro y San Pablo"]);
	});
	test("historical date and observed day remain separate, even when coincident", () => {
		const november = calendarEntries(2026);
		expect(november.filter((x) => x.date === "2026-11-11").map((x) => x.kind)).toEqual([
			"observance"
		]);
		expect(november.find((x) => x.date === "2026-11-16")?.originalDate).toBe("2026-11-11");
		expect(
			calendarEntries(2024)
				.filter((x) => x.date === "2024-11-11")
				.map((x) => x.kind)
		).toEqual(["holiday", "observance"]);
	});
	test("every supported year preserves all 18 celebrations and valid civil dates", () => {
		for (let year = 2002; year <= 2099; year++) {
			const entries = colombianHolidays(year);
			expect(entries).toHaveLength(18);
			expect(new Set(entries.map((x) => x.id)).size).toBe(18);
			expect(
				entries.every(
					(x) =>
						x.date.startsWith(`${year}-`) && dateKey(new Date(`${x.date}T00:00:00Z`)) === x.date
				)
			).toBe(true);
		}
		expect(() => colombianHolidays(NaN)).toThrow(RangeError);
		expect(() => colombianHolidays(2100)).toThrow(RangeError);
		expect(colombianObservances(2010).some((x) => x.id.includes("victims"))).toBe(false);
	});
});

test("civil month navigation handles leap years, year boundaries and six-row months", () => {
	expect(dateKey(addMonths(civilDate(2024, 1, 31), 1))).toBe("2024-02-29");
	expect(dateKey(addMonths(civilDate(2026, 1, 31), 1))).toBe("2026-02-28");
	expect(dateKey(addMonths(civilDate(2026, 1, 31), -1))).toBe("2025-12-31");
	const november = monthCells(civilDate(2026, 11, 1));
	expect(november).toHaveLength(42);
	expect(november.slice(0, 6)).toEqual(Array(6).fill(null));
	expect(dateKey(november[6]!)).toBe("2026-11-01");
	expect(november.filter(Boolean)).toHaveLength(30);
	expect(monthCells(civilDate(2024, 2, 1)).filter(Boolean)).toHaveLength(29);
});
