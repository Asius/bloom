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
			<div
				className={`peripheral-notice-indicator${notice.kind === "low" || notice.kind === "empty" ? " is-low" : ""}`}
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
				const level =
					device.percent === null ? "—" : `${device.approximate ? "≈" : ""}${device.percent}%`;
				return (
					<div className="peripheral-row" key={device.id}>
						<div className="peripheral-icon">
							<Icon size={20} aria-hidden="true" />
						</div>
						<div className="peripheral-details">
							<strong>{device.name}</strong>
							<span>
								<i
									className={`peripheral-status${device.connected === true ? " is-connected" : ""}`}
									aria-hidden="true"
								/>
								{detail} · {level}
							</span>
						</div>
					</div>
				);
			})}
		</section>
	);
}
