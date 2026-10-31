import { Bluetooth, Headphones, Keyboard, Mouse } from "lucide-react";
import { X } from "lucide-react";
import type { PeripheralNotice, PeripheralReading } from "../peripheralState";
import { BloomAvatar } from "./BloomAvatar";
import "./IslandPeripherals.css";

export function PeripheralNoticeView({
	notice,
	onOpen,
	onDismiss
}: {
	notice: PeripheralNotice;
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
			<BloomAvatar active={false} playing={false} />
			<div className="peripheral-notice-text">
				<strong>
					{notice.kind === "low"
						? device.id === "logitech-keyboard"
							? "Batería baja · Cambiar pilas"
							: "Batería baja"
						: device.name}
				</strong>
				<span>
					{notice.kind === "low" ? device.name : "Conectado"}
					{level}
				</span>
			</div>
			<div
				className={`peripheral-notice-indicator${notice.kind === "low" ? " is-low" : ""}`}
				aria-hidden="true"
			>
				<Icon size={13} />
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
			<p className="peripheral-caption">Batería de tus periféricos inalámbricos</p>
			{error && <p role="status">{error}</p>}
			{!error && !bluetoothAvailable && (
				<p role="status">Bluetooth no está disponible en este momento.</p>
			)}
			{!error && devices.length === 0 && <p role="status">Consultando dispositivos…</p>}
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
						? "Receptor desconectado"
						: device.connected === null
							? "Estado no disponible"
							: "Conectado";
				if (device.connected && device.charging) detail = "Cargando";
				const level =
					device.percent === null ? "—" : `${device.approximate ? "≈" : ""}${device.percent}%`;
				return (
					<div className="peripheral-row" key={device.id}>
						<div className="peripheral-icon">
							<Icon size={20} aria-hidden="true" />
						</div>
						<div className="peripheral-details">
							<strong>{device.name}</strong>
							<span>{detail}</span>
						</div>
						<strong
							className={device.percent !== null && device.percent <= 20 ? "peripheral-low" : ""}
						>
							{level}
						</strong>
					</div>
				);
			})}
		</section>
	);
}
