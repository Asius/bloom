import { invoke } from "@tauri-apps/api/core";

export type GithubDeviceFlowState = {
	pending: boolean;
	userCode: string;
	message: string;
	error: string;
	login: string;
	completedAt: number;
};

const emptyState: GithubDeviceFlowState = {
	pending: false,
	userCode: "",
	message: "",
	error: "",
	login: "",
	completedAt: 0
};

let state = emptyState;
let timer: ReturnType<typeof setTimeout> | null = null;
let generation = 0;
const subscribers = new Set<(value: GithubDeviceFlowState) => void>();

function publish(next: GithubDeviceFlowState) {
	state = next;
	for (const subscriber of subscribers) subscriber(state);
}

export function resetGithubDeviceFlow() {
	generation++;
	if (timer) clearTimeout(timer);
	timer = null;
	publish(emptyState);
}

export function getGithubDeviceFlowState() {
	return state;
}

export function subscribeGithubDeviceFlow(subscriber: (value: GithubDeviceFlowState) => void) {
	subscribers.add(subscriber);
	subscriber(state);
	return () => {
		subscribers.delete(subscriber);
	};
}

function finalAuthorizationError(error: string) {
	return /cancelad|expir|código.*(inválido|expir)|rechazó validar|no entregó.*token/i.test(error);
}

export async function startGithubDeviceFlow() {
	if (state.pending) return;
	const request = ++generation;
	publish({
		...emptyState,
		pending: true,
		message:
			"Abre GitHub y escribe el código. Bloom seguirá comprobando la autorización en segundo plano."
	});
	try {
		const start = await invoke<{ userCode: string; interval: number }>("github_device_start");
		if (request !== generation) return;
		let interval = Math.max(start.interval, 5);
		let retryDelay = interval;
		publish({
			pending: true,
			userCode: start.userCode,
			message: "El panel permanecerá abierto hasta confirmar tu sesión.",
			error: "",
			login: "",
			completedAt: 0
		});
		const poll = async () => {
			if (request !== generation) return;
			try {
				const result = await invoke<{
					state: string;
					waitSeconds?: number;
					login?: string;
				}>("github_device_poll");
				if (request !== generation) return;
				if (result.state === "connected") {
					if (timer) clearTimeout(timer);
					publish({
						pending: false,
						userCode: "",
						message: "Sesión de GitHub confirmada.",
						error: "",
						login: result.login ?? "",
						completedAt: Date.now()
					});
					return;
				}
				if (result.state === "slow_down") {
					interval += 5;
					retryDelay = interval;
				}
				if (result.waitSeconds) retryDelay = Math.max(retryDelay, result.waitSeconds);
				publish({ ...state, pending: true, error: "" });
				timer = setTimeout(() => void poll(), retryDelay * 1000);
			} catch (reason) {
				if (request !== generation) return;
				const error = String(reason);
				if (finalAuthorizationError(error)) {
					publish({ ...state, pending: false, userCode: "", error });
					return;
				}
				retryDelay = Math.min(Math.max(interval, retryDelay) + 5, 60);
				publish({
					...state,
					pending: true,
					error: "La conexión se interrumpió. Bloom volverá a comprobarlo automáticamente."
				});
				timer = setTimeout(() => void poll(), retryDelay * 1000);
			}
		};
		timer = setTimeout(() => void poll(), interval * 1000);
	} catch (reason) {
		if (request !== generation) return;
		publish({ ...emptyState, error: String(reason) });
	}
}
