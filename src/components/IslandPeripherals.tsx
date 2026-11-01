import { Bluetooth, Headphones, Keyboard, Mouse } from "lucide-react";
import { X } from "lucide-react";
import type { PeripheralNotice, PeripheralReading } from "../peripheralState";
import { BloomAvatar } from "./BloomAvatar";
import type { CSSProperties } from "react";
import "./IslandPeripherals.css";

export function PeripheralNoticeView({
	notice,
	time,
	onOpen,
	onDismiss
}: {
	notice: PeripheralNotice;
	time: string;
	onOpen: () => void;
	onDismiss: () => void;
}) {
	const { device } = notice;
	const Icon =
		device.kind === "mouse"
			? Mouse
			: device.kind === "keyboard"
				? Keyboard
				: device.kind === "headphones"
					? Headphones
					: Bluetooth;
	const level =
		device.percent === null ? "" : ` · ${device.approximate ? "≈" : ""}${device.percent}%`;
	return (
		<div
			className="main-row-inner peripheral-notice"
			role="status"
			onClick={(event) => {
				event.stopPropagation();
				onOpen();
			}}
		>
			<div className="peripheral-notice-avatar"><BloomAvatar active playing={false} reaction={notice.kind} /></div>
			<div className="peripheral-notice-text">
				<strong>
					{notice.kind === "full"
						? "Batería completa"
						: notice.kind === "empty"
							? "Batería agotada"
							: notice.kind === "low"
								? device.id === "logitech-keyboard"
									? "Batería baja · Cambiar pilas"
									: "Batería baja"
								: device.name}
				</strong>
				<span>
					{notice.kind === "low" || notice.kind === "empty" || notice.kind === "full"
						? device.name
						: notice.kind === "disconnected"
							? "Desconectado"
							: "Conectado"}
					{level}
				</span>
			</div>
			<time className="peripheral-notice-clock">{time}</time>
			<div
				className={`peripheral-notice-indicator is-${notice.kind}`}
				aria-hidden="true"
			>
				<Icon size={16} />
			</div>
			<button
				type="button"
				aria-label="Cerrar aviso"
				onClick={(event) => {
					event.stopPropagation();
					onDismiss();
				}}
			>
				<X size={14} />
			</button>
		</div>
	);
}

export function IslandPeripherals({
	devices,
	error,
	loading,
	bluetoothAvailable,
	refresh
}: {
	devices: PeripheralReading[];
	error: string | null;
	loading: boolean;
	bluetoothAvailable: boolean;
	refresh: () => void;
}) {
	return (
		<section className="peripheral-panel" aria-label="Dispositivos inalámbricos">
			<div className="peripheral-heading">
				<h2>Dispositivos</h2>
				<button type="button" onClick={refresh} disabled={loading}>
					{loading ? "Actualizando…" : "Actualizar"}
				</button>
			</div>
			{error && <p role="status">{error}</p>}
			{!error && !bluetoothAvailable && (
				<p role="status">Bluetooth no está disponible en este momento.</p>
			)}
			{!error && devices.length === 0 && (
				<p role="status">
					{loading ? "Consultando dispositivos…" : "No se encontraron dispositivos disponibles."}
				</p>
			)}
			{devices.map((device) => {
				const Icon =
					device.kind === "mouse"
						? Mouse
						: device.kind === "keyboard"
							? Keyboard
							: device.kind === "headphones"
								? Headphones
								: Bluetooth;
				let detail =
					device.connected === false
						? "Desconectado"
						: device.connected === null
							? "Estado no disponible"
							: "Conectado";
				if (device.connected && device.charging) detail = "Cargando";
				const percent = device.connected !== true || device.percent === null || !Number.isFinite(device.percent) ? null : Math.max(0, Math.min(100, device.percent));
				const level = percent === null ? "—" : `${device.approximate ? "≈" : ""}${percent}%`;
				const chargeClass = percent === null ? "is-unknown" : percent < 20 ? "is-low" : "is-normal";
				return (
					<div
						className="peripheral-row"
						key={device.id}
						role="group"
						aria-label={`${device.name}: ${detail}, batería ${level}`}
					>
						<div className="peripheral-icon">
							<Icon size={20} aria-hidden="true" />
							<i className={`peripheral-status ${device.connected === true ? "is-connected" : device.connected === false ? "is-disconnected" : "is-unknown"}`} aria-hidden="true" />
						</div>
						<div className="peripheral-details">
							<strong>{device.name}</strong>
						</div>
						<div className={`peripheral-charge ${chargeClass}`} style={{ "--charge": `${percent ?? 0}%` } as CSSProperties} aria-hidden="true"><span>{level}</span></div>
					</div>
				);
			})}
		</section>
	);
}
