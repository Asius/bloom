import { invoke } from "@tauri-apps/api/core";
import { Bell, BellOff, ChevronLeft, ChevronRight, Settings } from "lucide-react";
import { BloomAvatar } from "./BloomAvatar";
import type { NotificationStatus, WindowsNotice } from "../hooks/useNotifications";
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
	previous: () => void;
	next: () => void;
}) {
	return (
		<section className="notifications-panel" aria-label="Notificaciones">
			<header className="notifications-header">
				<BloomAvatar active playing={false} />
				<span className="notifications-divider" />
				<h2>Notificaciones</h2>
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
			{status === "allowed" && items.length > 0 ? (
				<div
					className="notifications-list"
					tabIndex={0}
					onWheel={(event) => event.stopPropagation()}
				>
					{items.map((item) => (
						<article className="notification-row" key={item.id}>
							<span className="notification-icon">
								<Bell size={19} aria-hidden="true" />
							</span>
							<div className="notification-copy">
								<strong>{item.app || "Aplicación"}</strong>
								<span>{[item.title, item.body].filter(Boolean).join(" · ") || "Notificación"}</span>
							</div>
							<time>
								{new Date(item.createdMs).toLocaleTimeString([], {
									hour: "2-digit",
									minute: "2-digit",
									hour12: !timeFormat24h
								})}
							</time>
						</article>
					))}
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
	time,
	open
}: {
	notice: WindowsNotice;
	time: string;
	open: () => void;
}) {
	return (
		<div className="notification-brief">
			<BloomAvatar active playing={false} />
			<span className="notifications-divider" />
			<button
				className="notification-brief-action"
				onClick={(event) => {
					event.stopPropagation();
					open();
				}}
				aria-label={`Abrir notificaciones: ${notice.app}`}
			>
				<span className="notification-icon">
					<Bell size={18} />
				</span>
				<span className="notification-copy">
					<strong>{notice.app}</strong>
					<span>{[notice.title, notice.body].filter(Boolean).join(" · ") || "Notificación"}</span>
				</span>
				<time>{time}</time>
			</button>
		</div>
	);
}
