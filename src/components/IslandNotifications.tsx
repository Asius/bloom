import { invoke } from "@tauri-apps/api/core";
import {
	Bell,
	BellOff,
	ChevronDown,
	ChevronLeft,
	ChevronRight,
	ChevronUp,
	ExternalLink,
	MoreHorizontal,
	Settings,
	Trash2,
	X
} from "lucide-react";
import { useMemo, useState } from "react";
import { groupNotifications } from "../notificationState";
import type { NotificationStatus, WindowsNotice } from "../hooks/useNotifications";
import { BloomAvatar } from "./BloomAvatar";
import "./IslandNotifications.css";

export function IslandNotifications({
	status,
	items,
	time,
	timeFormat24h,
	requestAccess,
	setActive,
	preferenceError,
	preferenceBusy,
	actionError,
	removeNotifications,
	clearAll,
	openApp,
	previous,
	next
}: {
	status: NotificationStatus;
	items: WindowsNotice[];
	time: string;
	timeFormat24h: boolean;
	requestAccess: () => void;
	setActive: (value: boolean) => void;
	preferenceError: { message: string; retryValue: boolean } | null;
	preferenceBusy: boolean;
	actionError: string | null;
	removeNotifications: (ids: number[]) => void;
	clearAll: () => void;
	openApp: (id: number) => void;
	previous: () => void;
	next: () => void;
}) {
	const groups = useMemo(() => groupNotifications(items), [items]);
	const [expandedGroups, setExpandedGroups] = useState<Set<string>>(() => new Set());
	const [openMenu, setOpenMenu] = useState<string | null>(null);
	return (
		<section
			className="notifications-panel"
			aria-label="Notificaciones"
			onClick={(event) => {
				if (
					event.target instanceof Element &&
					!event.target.closest(".notification-group-more-wrap")
				)
					setOpenMenu(null);
				event.stopPropagation();
			}}
			onKeyDown={(event) => {
				if (event.key === "Escape") setOpenMenu(null);
			}}
		>
			<header className="notifications-header">
				<BloomAvatar active playing={false} />
				<span className="notifications-divider" />
				<h2>Notificaciones</h2>
				{status === "allowed" && items.length > 0 && (
					<button
						className="notifications-clear-all"
						onClick={clearAll}
						aria-label="Borrar todas las notificaciones de Windows"
						title="Borrar todo"
					>
						<Trash2 size={14} />
						<span>Borrar todo</span>
					</button>
				)}
				{status !== "disabled" && status !== "loading" && (
					<button
						className="notifications-pause"
						disabled={preferenceBusy}
						onClick={() => setActive(false)}
						aria-label="Desactivar notificaciones de Bloom"
						title="Desactivar notificaciones de Bloom"
					>
						<BellOff size={15} />
					</button>
				)}
				<div className="notifications-nav">
					<button onClick={previous} aria-label="Panel anterior">
						<ChevronLeft size={15} />
					</button>
					<button onClick={next} aria-label="Panel siguiente">
						<ChevronRight size={15} />
					</button>
				</div>
				<time>{time}</time>
			</header>
			{actionError && (
				<p className="notifications-action-error" role="alert">
					{actionError}
				</p>
			)}
			{status === "allowed" && groups.length > 0 ? (
				<div
					className="notifications-list"
					tabIndex={0}
					onWheel={(event) => event.stopPropagation()}
				>
					{groups.map((group) => {
						const expanded = expandedGroups.has(group.key);
						const latest = group.latest;
						const label = group.sender || group.app;
						const count = group.items.length;
						return (
							<article className="notification-group" key={group.key}>
								<div className="notification-group-header">
									<span className="notification-icon">
										<Bell size={17} aria-hidden="true" />
									</span>
									<strong>{group.app}</strong>
									<time>{formatTime(latest.createdMs, timeFormat24h)}</time>
									<div className="notification-group-more-wrap">
										<button
											className="notification-group-more"
											type="button"
											aria-label={`Más opciones: ${label}`}
											aria-expanded={openMenu === group.key}
											onClick={() =>
												setOpenMenu((current) => (current === group.key ? null : group.key))
											}
										>
											<MoreHorizontal size={18} />
										</button>
										{openMenu === group.key && (
											<div className="notification-group-menu" role="menu">
												{latest.appId && (
													<button
														role="menuitem"
														onClick={() => {
															openApp(latest.id);
															setOpenMenu(null);
														}}
													>
														<ExternalLink size={14} /> Abrir aplicación
													</button>
												)}
												<button
													role="menuitem"
													onClick={() => {
														removeNotifications([latest.id]);
														setOpenMenu(null);
													}}
												>
													<Trash2 size={14} /> Quitar notificación
												</button>
												{count > 1 && (
													<button
														role="menuitem"
														onClick={() => {
															removeNotifications(group.items.map((item) => item.id));
															setOpenMenu(null);
														}}
													>
														<Trash2 size={14} /> Quitar grupo completo
													</button>
												)}
											</div>
										)}
									</div>
								</div>
								<button
									className="notification-group-summary"
									type="button"
									disabled={count < 2}
									aria-expanded={expanded}
									onClick={() =>
										setExpandedGroups((current) => {
											const next = new Set(current);
											if (next.has(group.key)) next.delete(group.key);
											else next.add(group.key);
											return next;
										})
									}
								>
									<span className="notification-group-copy">
										<strong>{label}</strong>
										<span>
											{[latest.title && !group.sender ? latest.title : "", latest.body]
												.filter(Boolean)
												.join(" · ") || "Notificación"}
										</span>
										{count > 1 && <small>+{count - 1} notificaciones</small>}
									</span>
									{count > 1 && (expanded ? <ChevronUp size={17} /> : <ChevronDown size={17} />)}
								</button>
								{expanded && count > 1 && (
									<div className="notification-group-messages">
										{group.items.slice(1).map((item) => (
											<div className="notification-message" key={item.id}>
												<span>{item.body || item.title || "Notificación"}</span>
												<time>{formatTime(item.createdMs, timeFormat24h)}</time>
												<button
													type="button"
													aria-label="Quitar esta notificación"
													title="Quitar esta notificación"
													onClick={() => removeNotifications([item.id])}
												>
													<X size={13} />
												</button>
											</div>
										))}
									</div>
								)}
							</article>
						);
					})}
				</div>
			) : (
				<div className="notifications-empty" role="status">
					{status === "disabled" ? (
						<>
							<Bell size={24} />
							<span>
								Activa las notificaciones para ver los avisos disponibles en Windows. Bloom no
								guarda una copia.
							</span>
							{preferenceError ? (
								<>
									<span>{preferenceError.message}</span>
									<button
										disabled={preferenceBusy}
										onClick={() => setActive(preferenceError.retryValue)}
									>
										Reintentar
									</button>
								</>
							) : (
								<button disabled={preferenceBusy} onClick={() => setActive(true)}>
									Activar
								</button>
							)}
						</>
					) : status === "allowed" ? (
						<>
							<Bell size={24} />
							<span>Sin notificaciones</span>
						</>
					) : status === "loading" ? (
						<span>Consultando notificaciones…</span>
					) : status === "permission" ? (
						<>
							<Settings size={24} />
							<span>Permite el acceso a las notificaciones de Windows</span>
							<button onClick={requestAccess}>Permitir acceso</button>
						</>
					) : status === "denied" ? (
						<>
							<Settings size={24} />
							<span>El acceso está desactivado en Windows</span>
							<button onClick={() => void invoke("open_notification_privacy_settings")}>
								Configurar
							</button>
						</>
					) : status === "unavailable" ? (
						<>
							<Settings size={24} />
							<span>
								El acceso a notificaciones no está disponible con la configuración actual de
								Windows.
							</span>
							<button onClick={() => void invoke("open_notification_privacy_settings")}>
								Revisar ajustes de Windows
							</button>
						</>
					) : (
						<span>No se pudieron consultar las notificaciones.</span>
					)}
				</div>
			)}
		</section>
	);
}

export function NotificationNotice({
	notice,
	timeFormat24h,
	open,
	dismiss
}: {
	notice: WindowsNotice;
	timeFormat24h: boolean;
	open: () => void;
	dismiss: () => void;
}) {
	return (
		<div className="notification-toast" role="status" aria-live="polite">
			<span className="notification-icon">
				<Bell size={18} aria-hidden="true" />
			</span>
			<button
				className="notification-toast-content"
				onClick={(event) => {
					event.stopPropagation();
					open();
				}}
				aria-label={`Abrir notificaciones: ${notice.app}`}
			>
				<strong>{notice.app || "Aplicación"}</strong>
				<span className="notification-toast-title">{notice.title || "Notificación"}</span>
				{notice.body && <span className="notification-toast-body">{notice.body}</span>}
			</button>
			<time>{formatTime(notice.createdMs, timeFormat24h)}</time>
			<button
				className="notification-toast-dismiss"
				type="button"
				onClick={(event) => {
					event.stopPropagation();
					dismiss();
				}}
				aria-label="Cerrar aviso"
				title="Cerrar aviso"
			>
				<X size={16} />
			</button>
		</div>
	);
}

function formatTime(timestamp: number, timeFormat24h: boolean): string {
	return new Date(timestamp).toLocaleTimeString([], {
		hour: "2-digit",
		minute: "2-digit",
		hour12: !timeFormat24h
	});
}
