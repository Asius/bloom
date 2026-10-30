/** Civil dates have no timezone. Use UTC only for arithmetic, never parse a
 * YYYY-MM-DD holiday as a local instant (which would shift it in Colombia).
 * National rules: Ley 51/1983. No municipal civic days or working-day advice.
 */
export interface CalendarEntry {
	id: string;
	date: string;
	title: string;
	kind: "holiday" | "observance";
	description?: string;
	originalDate?: string;
	source: string;
}

export const CALENDAR_MIN_YEAR = 2002;
export const CALENDAR_MAX_YEAR = 2099;
const LAW = "https://www.funcionpublica.gov.co/eva/gestornormativo/norma_pdf.php?i=4954";
const UN = "https://www.un.org/en/observances/list-days-weeks";
const CARTAGENA =
	"https://cartagena.gov.co/noticias/cartagena-conmemora-211-anos-historia-heroismo-cultura-libertad";

function checkYear(year: number) {
	if (!Number.isInteger(year) || year < CALENDAR_MIN_YEAR || year > CALENDAR_MAX_YEAR)
		throw new RangeError(`Calendar year must be ${CALENDAR_MIN_YEAR}–${CALENDAR_MAX_YEAR}`);
}

export const dateKey = (date: Date) => date.toISOString().slice(0, 10);
export const civilDate = (year: number, month: number, day: number) =>
	new Date(Date.UTC(year, month - 1, day));
export const addDays = (date: Date, days: number) =>
	civilDate(date.getUTCFullYear(), date.getUTCMonth() + 1, date.getUTCDate() + days);
export const sameMonth = (a: Date, b: Date) =>
	a.getUTCFullYear() === b.getUTCFullYear() && a.getUTCMonth() === b.getUTCMonth();

/** Clamp Jan 31 -> Feb 28/29; navigation never skips a month. */
export function addMonths(date: Date, count: number) {
	const first = civilDate(date.getUTCFullYear(), date.getUTCMonth() + 1 + count, 1);
	const last = civilDate(first.getUTCFullYear(), first.getUTCMonth() + 2, 0).getUTCDate();
	return civilDate(
		first.getUTCFullYear(),
		first.getUTCMonth() + 1,
		Math.min(last, date.getUTCDate())
	);
}

export function todayCivil(now = new Date()) {
	return civilDate(now.getFullYear(), now.getMonth() + 1, now.getDate());
}

/** Gregorian computus (Meeus/Jones/Butcher), valid throughout our UI range. */
function easterSunday(year: number) {
	const a = year % 19,
		b = Math.floor(year / 100),
		c = year % 100;
	const d = Math.floor(b / 4),
		e = b % 4,
		f = Math.floor((b + 8) / 25);
	const g = Math.floor((b - f + 1) / 3);
	const h = (19 * a + b - d - g + 15) % 30;
	const i = Math.floor(c / 4),
		k = c % 4;
	const l = (32 + 2 * e + 2 * i - h - k) % 7;
	const m = Math.floor((a + 11 * h + 22 * l) / 451);
	const value = h + l - 7 * m + 114;
	return civilDate(year, Math.floor(value / 31), (value % 31) + 1);
}

export function colombianHolidays(year: number): CalendarEntry[] {
	checkYear(year);
	const entries: CalendarEntry[] = [];
	const add = (id: string, title: string, original: Date, monday = false) => {
		const observed = monday ? addDays(original, (8 - original.getUTCDay()) % 7) : original;
		entries.push({
			id: `co-${year}-${id}`,
			title,
			kind: "holiday",
			date: dateKey(observed),
			originalDate: dateKey(original),
			source: LAW
		});
	};
	for (const [id, title, month, day] of [
		["new-year", "Año Nuevo", 1, 1],
		["labour", "Día del Trabajo", 5, 1],
		["independence", "Independencia de Colombia", 7, 20],
		["boyaca", "Batalla de Boyacá", 8, 7],
		["immaculate", "Inmaculada Concepción", 12, 8],
		["christmas", "Navidad", 12, 25]
	] as const)
		add(id, title, civilDate(year, month, day));
	for (const [id, title, month, day] of [
		["epiphany", "Día de los Reyes Magos", 1, 6],
		["joseph", "Día de San José", 3, 19],
		["peter-paul", "San Pedro y San Pablo", 6, 29],
		["assumption", "Asunción de la Virgen", 8, 15],
		["diversity", "Día de la Diversidad Étnica y Cultural", 10, 12],
		["saints", "Todos los Santos", 11, 1],
		["cartagena", "Independencia de Cartagena", 11, 11]
	] as const)
		add(id, title, civilDate(year, month, day), true);
	const easter = easterSunday(year);
	add("holy-thursday", "Jueves Santo", addDays(easter, -3));
	add("good-friday", "Viernes Santo", addDays(easter, -2));
	add("ascension", "Ascensión del Señor", addDays(easter, 39), true);
	add("corpus", "Corpus Christi", addDays(easter, 60), true);
	add("sacred-heart", "Sagrado Corazón de Jesús", addDays(easter, 68), true);
	// Preserve distinct celebrations that coincide, e.g. 30 June 2025.
	return entries.sort((a, b) => a.date.localeCompare(b.date) || a.id.localeCompare(b.id));
}

/** A small, sourced selection; deliberately not a comprehensive Google feed. */
export function colombianObservances(year: number): CalendarEntry[] {
	checkYear(year);
	return (
		[
			["women", 3, 8, "Día Internacional de la Mujer", UN],
			[
				"victims",
				4,
				9,
				"Día Nacional de la Memoria y Solidaridad con las Víctimas",
				"https://www.unidadvictimas.gov.co/wp-content/uploads/Documentos/ley/Ley_1448_2011.pdf"
			],
			["earth", 4, 22, "Día Internacional de la Madre Tierra", UN],
			[
				"afro",
				5,
				21,
				"Día Nacional de la Afrocolombianidad",
				"https://www1.funcionpublica.gov.co/eva/gestornormativo/norma.php?i=4598"
			],
			["environment", 6, 5, "Día Mundial del Medio Ambiente", UN],
			["peace", 9, 21, "Día Internacional de la Paz", UN],
			["cartagena-history", 11, 11, "Independencia de Cartagena · 1811", CARTAGENA],
			["human-rights", 12, 10, "Día de los Derechos Humanos", UN]
		] as const
	)
		.filter(([id]) => id !== "victims" || year >= 2011)
		.map(([id, month, day, title, source]) => ({
			id: `observance-${year}-${id}`,
			date: dateKey(civilDate(year, month, day)),
			title,
			source,
			kind: "observance",
			description: "Conmemoración; no constituye un festivo nacional."
		}));
}

export function calendarEntries(year: number) {
	return [...colombianHolidays(year), ...colombianObservances(year)].sort(
		(a, b) => a.date.localeCompare(b.date) || a.kind.localeCompare(b.kind)
	);
}

/** Monday-first, six stable rows so month changes cannot resize the window. */
export function monthCells(month: Date): (Date | null)[] {
	const year = month.getUTCFullYear(),
		index = month.getUTCMonth();
	const offset = (civilDate(year, index + 1, 1).getUTCDay() + 6) % 7;
	const count = civilDate(year, index + 2, 0).getUTCDate();
	return Array.from({ length: 42 }, (_, i) => {
		const day = i - offset + 1;
		return day < 1 || day > count ? null : civilDate(year, index + 1, day);
	});
}
