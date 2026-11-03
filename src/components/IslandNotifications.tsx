import { invoke } from "@tauri-apps/api/core";
import {
	Bell,
	BellOff,
	ChevronDown,
	ChevronUp,
	ExternalLink,
	Settings,
	Trash2,
	X
} from "lucide-react";
import { useMemo, useState } from "react";
import { groupNotifications } from "../notificationState";
import type { NotificationStatus, WindowsNotice } from "../hooks/useNotifications";
import { PanelHeader } from "./PanelHeader";
import "./IslandNotifications.css";

export function IslandNotifications({
	status,
	items,
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
	return (
		<section
			className="notifications-panel"
			aria-label="Notificaciones"
			onClick={(event) => {
				event.stopPropagation();
			}}
			onKeyDown={(event) => {
				if (event.key === "Escape") (event.target as HTMLElement).blur();
			}}
		>
			<PanelHeader
				title="Notificaciones"
				previous={previous}
				next={next}
				actions={
					<>
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
					</>
				}
			/>
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
										<NoticeIcon notice={latest} />
									</span>
									<strong>{group.app}</strong>
									<time>{formatTime(latest.createdMs, timeFormat24h)}</time>
									<div className="notification-card-actions">
										{latest.appId && (
											<button
												type="button"
												aria-label="Abrir aplicación"
												title="Abrir aplicación"
												onClick={() => openApp(latest.id)}
											>
												<ExternalLink size={15} />
											</button>
										)}
										<button
											type="button"
											aria-label="Quitar notificación"
											title="Quitar notificación"
											onClick={() => removeNotifications([latest.id])}
										>
											<Trash2 size={15} />
										</button>
										{count > 1 && (
											<button
												type="button"
												aria-label="Quitar grupo completo"
												title="Quitar grupo"
												onClick={() => removeNotifications(group.items.map((item) => item.id))}
											>
												<X size={15} />
											</button>
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
				<NoticeIcon notice={notice} />
			</span>
			<button
				className="notification-toast-content"
				onClick={(event) => {
					event.stopPropagation();
					open();
				}}
				aria-label={`Abrir notificaciones: ${notice.app}`}
			>
				<strong>{notice.title || notice.app || "Notificación"}</strong>
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

function NoticeIcon({ notice }: { notice: WindowsNotice }) {
	const [failed, setFailed] = useState(false);
	return notice.icon && !failed ? (
		<img src={notice.icon} alt="" onError={() => setFailed(true)} />
	) : (
		<Bell size={20} aria-hidden="true" />
	);
}
