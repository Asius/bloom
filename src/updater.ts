export interface UpdateCheckResult {
	available: boolean;
	version?: string;
	date?: string;
	body?: string;
}

export type UpdateStatus =
	| "idle"
	| "checking"
	| "available"
	| "uptodate"
	| "downloading"
	| "installing"
	| "check-error"
	| "install-error";

export interface AutoUpdateEvent {
	status: "checking" | "downloading" | "installing" | "done" | "error";
	progress?: number;
	operation?: "check" | "install";
	message?: string;
}

export const isUpdateActive = (status: string) =>
	status === "checking" || status === "downloading" || status === "installing";

export const isUpdateTerminal = (status: string) => status === "done" || status === "error";

export function updateErrorMessage(error: unknown): string {
	if (error instanceof Error) return error.message;
	return typeof error === "string" && error.trim() ? error : "Please try again.";
}

export function updateLabel(status: UpdateStatus, version: string): string {
	switch (status) {
		case "checking":
			return "Checking...";
		case "available":
			return `Update Available (v${version})`;
		case "uptodate":
			return "Bloom is up to date";
		case "downloading":
			return "Downloading Update...";
		case "installing":
			return "Installing...";
		case "check-error":
			return "Could not check for updates";
		case "install-error":
			return "Could not install update";
		default:
			return "Check for Updates";
	}
}
