import type { ReactNode } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { BloomAvatar } from "./BloomAvatar";
import "./PanelHeader.css";

export function PanelHeader({
	title,
	subtitle,
	actions,
	icon,
	previous,
	next
}: {
	title: string;
	subtitle?: string;
	actions?: ReactNode;
	icon?: ReactNode;
	previous: () => void;
	next: () => void;
}) {
	return (
		<header className="panel-header" onClick={(e) => e.stopPropagation()}>
			<BloomAvatar active playing={false} />
			<span className="panel-header-divider" />
			{icon}
			<div className="panel-header-title">
				<h2>{title}</h2>
				{subtitle && <span>{subtitle}</span>}
			</div>
			<div className="panel-header-actions">{actions}</div>
			<nav aria-label="Paneles">
				<button onClick={previous} aria-label="Panel anterior">
					<ChevronLeft size={16} />
				</button>
				<button onClick={next} aria-label="Panel siguiente">
					<ChevronRight size={16} />
				</button>
			</nav>
		</header>
	);
}
