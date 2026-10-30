import { useEffect, useState } from "react";
import "./BloomAvatar.css";

/** Local reactions only: no global pointer listener, network or background loop. */
export function BloomAvatar({ active, playing }: { active: boolean; playing: boolean }) {
	const [hovered, setHovered] = useState(false);
	const [focused, setFocused] = useState(false);
	const curious = hovered || focused;
	const [greeting, setGreeting] = useState(0);
	const [asleep, setAsleep] = useState(false);
	const [music, setMusic] = useState(false);
	useEffect(() => {
		if (!active) {
			setHovered(false);
			setFocused(false);
			setAsleep(false);
			return;
		}
		setAsleep(false);
		if (curious || playing) return;
		const timer = window.setTimeout(() => setAsleep(true), 30000);
		return () => window.clearTimeout(timer);
	}, [active, curious, playing, greeting]);
	useEffect(() => {
		if (!greeting) return;
		const timer = window.setTimeout(() => setGreeting(0), 900);
		return () => window.clearTimeout(timer);
	}, [greeting]);
	useEffect(() => {
		setMusic(active && playing);
		if (!active || !playing) return;
		const timer = window.setTimeout(() => setMusic(false), 1800);
		return () => window.clearTimeout(timer);
	}, [active, playing]);
	const state = greeting
		? "greeting"
		: curious
			? "curious"
			: asleep
				? "sleep"
				: music
					? "music"
					: "idle";
	return (
		<button
			type="button"
			className="bloom-avatar"
			data-state={state}
			data-active={active}
			tabIndex={active ? 0 : -1}
			aria-label="Saludar a Bloom"
			title="Saludar a Bloom"
			onPointerEnter={() => setHovered(true)}
			onPointerLeave={() => setHovered(false)}
			onPointerMove={(event) => {
				const bounds = event.currentTarget.getBoundingClientRect();
				const x = Math.max(-2, Math.min(2, ((event.clientX - bounds.left) / bounds.width) * 4 - 2));
				event.currentTarget.style.setProperty("--avatar-gaze", `${x}px`);
			}}
			onFocus={() => setFocused(true)}
			onBlur={() => setFocused(false)}
			onClick={(event) => {
				event.stopPropagation();
				setGreeting((n) => n + 1);
			}}
		>
			<svg viewBox="0 0 64 64" aria-hidden="true">
				<path
					className="avatar-body"
					d="M32 3 C37 3 37 10 42 9 C48 6 51 10 51 16 C51 21 59 20 59 26 C59 31 54 33 58 38 C62 44 56 47 51 48 C47 49 49 57 43 58 C38 59 36 54 32 59 C28 64 24 57 20 57 C14 59 12 53 12 49 C12 45 4 47 4 40 C4 35 9 33 5 28 C1 22 8 20 12 18 C15 15 12 8 19 8 C24 9 26 3 32 3 Z"
				/>
				<g className="avatar-eyes">
					{state === "sleep" ? (
						<path d="M19 30 Q24 37 29 30 M36 30 Q41 37 46 30" />
					) : state === "greeting" || state === "music" ? (
						<path d="M19 32 Q24 24 29 32 M36 32 Q41 24 46 32" />
					) : (
						<>
							<ellipse cx="24" cy="30" rx="5" ry="6" />
							<ellipse cx="41" cy="30" rx="5" ry="6" />
							<circle className="avatar-highlight" cx="25" cy="28" r="1.6" />
							<circle className="avatar-highlight" cx="42" cy="28" r="1.6" />
						</>
					)}
				</g>
			</svg>
		</button>
	);
}
