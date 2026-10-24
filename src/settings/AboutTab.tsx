import { Download, RefreshCw, FileDown, Upload } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { GithubIcon } from "../icons";
import { SettingRow } from "./SettingRow";
import { isUpdateActive, updateLabel, type UpdateStatus } from "../updater";

interface AboutTabProps {
	appVersion: string;
	autoUpdate: boolean;
	toggleAutoUpdate: () => void;
	updateStatus: UpdateStatus;
	updateError: string;
	updateVersion: string;
	checkForUpdates: () => void;
	installUpdate: () => void;
	exportStatus: string;
	importStatus: string;
	handleExportSettings: () => void;
	handleImportSettings: () => void;
}

export function AboutTab({
	appVersion,
	autoUpdate,
	toggleAutoUpdate,
	updateStatus,
	updateError,
	updateVersion,
	checkForUpdates,
	installUpdate,
	exportStatus,
	importStatus,
	handleExportSettings,
	handleImportSettings
}: AboutTabProps) {
	const getUpdateDesc = () =>
		updateStatus === "check-error" || updateStatus === "install-error"
			? `${updateError || "Please try again."} Click to check again.`
			: updateStatus === "available"
				? "Click to install and restart"
				: `Currently running v${appVersion}`;

	const getExportLabel = () => {
		if (exportStatus === "exporting") return "Exporting...";
		if (exportStatus === "success") return "Exported!";
		if (exportStatus === "error") return "Export failed. Try again.";
		return "Export Settings";
	};

	const getImportLabel = () => {
		if (importStatus === "importing") return "Importing...";
		if (importStatus === "success") return "Imported!";
		if (importStatus === "error") return "Import failed. Check the file and try again.";
		return "Import Settings";
	};

	return (
		<div className="about-tab-container">
			<div className="about-header">
				<img src="/bloom.png" className="about-logo" alt="Bloom Logo" draggable={false} />
				<h1 className="about-title">Bloom</h1>
				<p className="about-version">Version {appVersion}</p>
				<p className="about-credit">
					Made with <span className="about-heart">❤️</span> by{" "}
					<button
						className="about-author"
						onClick={() => openUrl("https://github.com/SehajveerSingh2005")}
						title="sehaz on GitHub"
					>
						<GithubIcon size={12} />
						<span>sehaz</span>
					</button>
				</p>
			</div>

			<div className="setting-group-label">Software Updates</div>
			<div className="setting-group">
				<SettingRow icon={Download} label="Auto Update" desc="Update automatically on startup">
					<label className="toggle-switch">
						<input type="checkbox" checked={autoUpdate} onChange={toggleAutoUpdate} />
						<span className="slider"></span>
					</label>
				</SettingRow>

				<SettingRow
					icon={RefreshCw}
					label={updateLabel(updateStatus, updateVersion)}
					desc={getUpdateDesc()}
					action={!isUpdateActive(updateStatus)}
					divider={false}
					onClick={
						isUpdateActive(updateStatus)
							? undefined
							: () => (updateStatus === "available" ? installUpdate() : checkForUpdates())
					}
				/>
			</div>

			<div className="setting-group-label setting-group-label--spaced">Data</div>
			<div className="setting-group">
				<SettingRow
					icon={FileDown}
					label={getExportLabel()}
					desc="Save settings to a file"
					action
					onClick={handleExportSettings}
				/>
				<SettingRow
					icon={Upload}
					label={getImportLabel()}
					desc="Load settings from a file"
					action
					divider={false}
					onClick={handleImportSettings}
				/>
			</div>
		</div>
	);
}
