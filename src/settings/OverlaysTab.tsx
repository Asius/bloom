import { ArrowLeftToLine, Volume2 } from "lucide-react";
import { SettingRow } from "./SettingRow";

interface OverlaysTabProps {
	volumeOverlayEnabled: boolean;
	toggleVolumeOverlay: () => void;
	volumeEdgeEnabled: boolean;
	toggleVolumeEdge: () => void;
}

export function OverlaysTab({
	volumeOverlayEnabled,
	toggleVolumeOverlay,
	volumeEdgeEnabled,
	toggleVolumeEdge
}: OverlaysTabProps) {
	return (
		<>
			<div className="setting-group-label">Overlays</div>
			<div className="setting-group">
				<SettingRow icon={Volume2} label="Volume HUD" desc="Bloom volume overlay">
					<label className="toggle-switch">
						<input type="checkbox" checked={volumeOverlayEnabled} onChange={toggleVolumeOverlay} />
						<span className="slider"></span>
					</label>
				</SettingRow>

				{volumeOverlayEnabled && (
					<SettingRow
						icon={ArrowLeftToLine}
						label="Show on Edge Hover"
						desc="Slide in from left edge"
					>
						<label className="toggle-switch">
							<input type="checkbox" checked={volumeEdgeEnabled} onChange={toggleVolumeEdge} />
							<span className="slider"></span>
						</label>
					</SettingRow>
				)}
			</div>
		</>
	);
}
