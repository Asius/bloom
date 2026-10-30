// Adapted from EasyUI Calendar, Copyright (c) 2026 Suraj Maurya (MIT).
// License and upstream reference: THIRD_PARTY_NOTICES.md.
import { invoke } from "@tauri-apps/api/core";
import { motion, useReducedMotion } from "framer-motion";
import { ChevronLeft, ChevronRight, ExternalLink } from "lucide-react";
import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import {
	addDays,
	addMonths,
	calendarEntries,
	CALENDAR_MAX_YEAR,
	CALENDAR_MIN_YEAR,
	civilDate,
	dateKey,
	monthCells,
	sameMonth,
	todayCivil,
	type CalendarEntry
} from "../calendar/colombia";
import "./IslandCalendar.css";

const monthFormat = new Intl.DateTimeFormat("es-CO", {
	month: "long",
	year: "numeric",
	timeZone: "UTC"
});
const fullFormat = new Intl.DateTimeFormat("es-CO", { dateStyle: "full", timeZone: "UTC" });
const dayFormat = new Intl.DateTimeFormat("es-CO", {
	weekday: "long",
	day: "numeric",
	month: "long",
	timeZone: "UTC"
});
const shortFormat = new Intl.DateTimeFormat("es-CO", {
	day: "numeric",
	month: "long",
	timeZone: "UTC"
});
const minDate = civilDate(CALENDAR_MIN_YEAR, 1, 1);
const maxDate = civilDate(CALENDAR_MAX_YEAR, 12, 31);
const clamp = (date: Date) => (date < minDate ? minDate : date > maxDate ? maxDate : date);

function Entry({ entry }: { entry: CalendarEntry }) {
	const transferred = entry.kind === "holiday" && entry.originalDate !== entry.date;
	return (
		<li className={`island-calendar-event ${entry.kind}`}>
			<span className="island-calendar-category">
				{entry.kind === "holiday" ? "Festivo nacional" : "Conmemoración"} · Todo el día
			</span>
			<strong>{entry.title}</strong>
			{transferred && entry.originalDate && (
				<small>
					Descanso trasladado del {shortFormat.format(new Date(`${entry.originalDate}T00:00:00Z`))}.
				</small>
			)}
			{entry.description && <small>{entry.description}</small>}
		</li>
	);
}

export function IslandCalendar() {
	const [today, setToday] = useState(() => clamp(todayCivil()));
	const [selected, setSelected] = useState(today);
	const [view, setView] = useState(() =>
		civilDate(today.getUTCFullYear(), today.getUTCMonth() + 1, 1)
	);
	const [focused, setFocused] = useState(today);
	const [direction, setDirection] = useState(1);
	const [helpError, setHelpError] = useState("");
	const reducedMotion = useReducedMotion();
	const selectionId = useId();
	const headingId = useId();
	const gridRef = useRef<HTMLDivElement>(null);
	const shouldFocus = useRef(false);
	const entries = useMemo(() => calendarEntries(view.getUTCFullYear()), [view]);
	const cells = useMemo(() => monthCells(view), [view]);
	const byDate = useMemo(() => {
		const map = new Map<string, CalendarEntry[]>();
		for (const entry of entries) map.set(entry.date, [...(map.get(entry.date) ?? []), entry]);
		return map;
	}, [entries]);
	const selectedKey = dateKey(selected);
	// Selection can stay in the previous year while the user browses another month.
	const selectedEntries = useMemo(
		() => calendarEntries(selected.getUTCFullYear()).filter((entry) => entry.date === selectedKey),
		[selected, selectedKey]
	);
	const tabDate = sameMonth(focused, view) ? focused : view;
	const canPrevious = view > minDate;
	const canNext = view.getUTCFullYear() < CALENDAR_MAX_YEAR || view.getUTCMonth() < 11;

	useEffect(() => {
		// Only mounted while the calendar is visible. Refresh after midnight/resume.
		const timer = window.setInterval(
			() =>
				setToday((previous) => {
					const next = clamp(todayCivil());
					return dateKey(previous) === dateKey(next) ? previous : next;
				}),
			60_000
		);
		return () => window.clearInterval(timer);
	}, []);
	useEffect(() => {
		if (!shouldFocus.current) return;
		shouldFocus.current = false;
		gridRef.current?.querySelector<HTMLButtonElement>(`[data-date="${dateKey(focused)}"]`)?.focus();
	}, [focused, view]);

	const go = (target: Date) => {
		const bounded = clamp(target);
		setDirection(bounded > view ? 1 : -1);
		setView(civilDate(bounded.getUTCFullYear(), bounded.getUTCMonth() + 1, 1));
	};
	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		const button = (event.target as HTMLElement).closest<HTMLButtonElement>("[data-date]");
		if (!button?.dataset.date) return;
		const date = new Date(`${button.dataset.date}T00:00:00Z`);
		const column = (date.getUTCDay() + 6) % 7;
		let next: Date;
		switch (event.key) {
			case "ArrowLeft":
				next = addDays(date, -1);
				break;
			case "ArrowRight":
				next = addDays(date, 1);
				break;
			case "ArrowUp":
				next = addDays(date, -7);
				break;
			case "ArrowDown":
				next = addDays(date, 7);
				break;
			case "Home":
				next = addDays(date, -column);
				break;
			case "End":
				next = addDays(date, 6 - column);
				break;
			case "PageUp":
				next = addMonths(date, event.shiftKey ? -12 : -1);
				break;
			case "PageDown":
				next = addMonths(date, event.shiftKey ? 12 : 1);
				break;
			default:
				return;
		}
		event.preventDefault();
		next = clamp(next);
		shouldFocus.current = true;
		setFocused(next);
		if (!sameMonth(next, view)) go(next);
	};

	return (
		<section
			className="island-calendar"
			aria-label="Calendario de Colombia"
			onWheel={(event) => event.stopPropagation()}
		>
			<div className="island-calendar-month">
				<div className="island-calendar-navigation">
					<button
						type="button"
						aria-label="Mes anterior"
						disabled={!canPrevious}
						onClick={() => go(addMonths(view, -1))}
					>
						<ChevronLeft size={17} />
					</button>
					<h2 id={headingId} aria-live="polite">
						{monthFormat.format(view)}
					</h2>
					<button
						type="button"
						aria-label="Mes siguiente"
						disabled={!canNext}
						onClick={() => go(addMonths(view, 1))}
					>
						<ChevronRight size={17} />
					</button>
				</div>
				<div className="island-calendar-weekdays" aria-hidden="true">
					{["L", "M", "X", "J", "V", "S", "D"].map((day, i) => (
						<span key={i}>{day}</span>
					))}
				</div>
				<div ref={gridRef} role="grid" aria-labelledby={headingId} onKeyDown={onKeyDown}>
					{/* Immediate month replacement avoids stale interactive cells during navigation. */}
					<motion.div
						key={dateKey(view)}
						initial={reducedMotion ? false : { opacity: 0, x: direction * 14 }}
						animate={{ opacity: 1, x: 0 }}
						transition={{ duration: reducedMotion ? 0 : 0.18 }}
					>
						{Array.from({ length: 6 }, (_, row) => (
							<div role="row" className="island-calendar-week" key={row}>
								{cells.slice(row * 7, row * 7 + 7).map((date, column) => {
									if (!date) return <div role="gridcell" key={column} />;
									const key = dateKey(date),
										items = byDate.get(key) ?? [];
									const picked = key === selectedKey;
									const holiday = items.some((entry) => entry.kind === "holiday");
									const observance = items.some((entry) => entry.kind === "observance");
									return (
										<div role="gridcell" aria-selected={picked} key={column}>
											<button
												type="button"
												data-date={key}
												tabIndex={dateKey(tabDate) === key ? 0 : -1}
												aria-current={key === dateKey(today) ? "date" : undefined}
												aria-label={[
													fullFormat.format(date),
													...items.map(
														(entry) =>
															`${entry.kind === "holiday" ? "Festivo" : "Conmemoración"}: ${entry.title}`
													)
												].join(". ")}
												className={`island-calendar-day ${picked ? "selected" : ""} ${holiday ? "holiday" : ""}`}
												onFocus={() => setFocused(date)}
												onClick={() => {
													setSelected(date);
													setFocused(date);
												}}
											>
												{picked && (
													<motion.span
														className="island-calendar-selection"
														layoutId={`${selectionId}-selection`}
														transition={
															reducedMotion
																? { duration: 0 }
																: { type: "spring", stiffness: 460, damping: 38, mass: 0.7 }
														}
													/>
												)}
												<span className="island-calendar-number">{date.getUTCDate()}</span>
												<span className="island-calendar-dots" aria-hidden="true">
													{holiday && <i />}
													{observance && <i className="observance" />}
												</span>
											</button>
										</div>
									);
								})}
							</div>
						))}
					</motion.div>
				</div>
				<div className="island-calendar-footer">
					<div className="island-calendar-legend">
						<span>
							<i />
							Festivo
						</span>
						<span>
							<i className="observance" />
							Conmemoración
						</span>
					</div>
					<button
						type="button"
						className="island-calendar-today"
						onClick={() => {
							const now = clamp(todayCivil());
							setToday(now);
							setSelected(now);
							setFocused(now);
							go(now);
						}}
					>
						Hoy
					</button>
				</div>
			</div>
			<aside className="island-calendar-agenda" aria-label="Agenda del día" tabIndex={0}>
				<div className="island-calendar-eyebrow">AGENDA DEL DÍA</div>
				<h3>{dayFormat.format(selected)}</h3>
				<div aria-live="polite" aria-atomic="true">
					{selectedEntries.length ? (
						<ul>
							{selectedEntries.map((entry) => (
								<Entry entry={entry} key={entry.id} />
							))}
						</ul>
					) : (
						<p className="island-calendar-empty">Sin festivos ni conmemoraciones en esta fecha.</p>
					)}
				</div>
				<div className="island-calendar-google">
					<strong>Google Calendar</strong>
					<span className="island-calendar-status">Sin configurar</span>
					<p>Consulta aquí tus eventos cuando habilitemos la conexión con tu cuenta.</p>
					<button
						type="button"
						onClick={() => {
							setHelpError("");
							invoke("open_calendar_setup_guide").catch(() =>
								setHelpError(
									"No se pudo abrir la guía. Está en el repositorio: GOOGLE_CALENDAR.md."
								)
							);
						}}
					>
						Preparar conexión <ExternalLink size={13} />
					</button>
					{helpError && <p role="alert">{helpError}</p>}
					<small>
						Opcional · Primera conexión prevista: solo lectura. Las notificaciones llegarán después.
					</small>
				</div>
			</aside>
		</section>
	);
}
