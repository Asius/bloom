import { Palette, Droplet, Contrast, Droplets, Sun, Square, Maximize2 } from "lucide-react";
import { SettingRow } from "./SettingRow";

interface AppearanceTabProps {
	themeMode: string;
	handleThemeModeChange: (mode: string) => void;
	themeColor: string;
	handleThemeColorChange: (color: string) => void;
	themeOpacity: number;
	handleOpacityChange: (val: number) => void;
	themeSaturation: number;
	handleSaturationChange: (val: number) => void;
	themeBrightness: number;
	handleBrightnessChange: (val: number) => void;
	cornersEnabled: boolean;
	toggleCorners: () => void;
	scale: number;
	handleScaleChange: (val: number) => void;
	scaleLinked: boolean;
	handleScaleLinkedChange: (linked: boolean) => void;
	dockScale: number;
	handleDockScaleChange: (val: number) => void;
	notchScale: number;
	handleNotchScaleChange: (val: number) => void;
	fontScale: number;
	handleFontScaleChange: (val: number) => void;
}

function ScaleButtons({ label, value, onChange }: { label: string; value: number; onChange: (value: number) => void }) {
	return (
		<div className="scale-button-container">
			<button type="button" onClick={() => onChange(Math.max(0.8, Number((value - 0.1).toFixed(1))))}
				disabled={value <= 0.8} className="scale-adjust-btn" aria-label={`Decrease ${label}`}>
				—
			</button>
			<span className="scale-display-value">{Math.round(value * 100)}%</span>
			<button type="button" onClick={() => onChange(Math.min(1.3, Number((value + 0.1).toFixed(1))))}
				disabled={value >= 1.3} className="scale-adjust-btn" aria-label={`Increase ${label}`}>
				+
			</button>
		</div>
	);
}

export function AppearanceTab({
	themeMode,
	handleThemeModeChange,
	themeColor,
	handleThemeColorChange,
	themeOpacity,
	handleOpacityChange,
	themeSaturation,
	handleSaturationChange,
	themeBrightness,
	handleBrightnessChange,
	cornersEnabled,
	toggleCorners,
	scale,
	handleScaleChange,
	scaleLinked,
	handleScaleLinkedChange,
	dockScale,
	handleDockScaleChange,
	notchScale,
	handleNotchScaleChange,
	fontScale,
	handleFontScaleChange
}: AppearanceTabProps) {
	const showCustomColor = themeMode === "custom";
	const showAdvancedSliders = themeMode === "custom" || themeMode === "adaptive";

	return (
		<>
			<div className="setting-group-label">Theme</div>
			<div className="setting-group">
				<SettingRow icon={Palette} label="Theme Mode" desc="Configure visual styling">
					<select
						className="settings-select"
						value={themeMode}
						onChange={(e) => handleThemeModeChange(e.target.value)}
					>
						<option value="system">System (Windows)</option>
						<option value="dark">Dark (Translucent)</option>
						<option value="light">Light (Translucent)</option>
						<option value="custom">Custom Color</option>
						<option value="adaptive">Adaptive Accent</option>
					</select>
				</SettingRow>

				{showCustomColor && (
					<SettingRow
						icon={Droplet}
						label="Custom Theme Color"
						desc="Choose layout background color"
					>
						<div className="color-picker-row">
							<input
								type="color"
								value={themeColor}
								onChange={(e) => handleThemeColorChange(e.target.value)}
								className="color-picker-input"
							/>
							<span className="color-picker-label">{themeColor.toUpperCase()}</span>
						</div>
					</SettingRow>
				)}

				<SettingRow
					icon={Contrast}
					label="Background Opacity"
					desc={`Adjust theme transparency (${Math.round(themeOpacity * 100)}%)`}
					divider={showAdvancedSliders}
				>
					<input
						type="range"
						min="0.1"
						max="1.0"
						step="0.05"
						value={themeOpacity}
						onChange={(e) => handleOpacityChange(parseFloat(e.target.value))}
						className="settings-slider"
					/>
				</SettingRow>

				{showAdvancedSliders && (
					<>
						<SettingRow
							icon={Droplets}
							label="Color Saturation"
							desc={`Adjust theme color vibrancy (${Math.round(themeSaturation * 100)}%)`}
						>
							<input
								type="range"
								min="0.0"
								max="1.0"
								step="0.02"
								value={themeSaturation}
								onChange={(e) => handleSaturationChange(parseFloat(e.target.value))}
								className="settings-slider"
							/>
						</SettingRow>

						<SettingRow
							icon={Sun}
							label="Background Brightness"
							desc={`Adjust background lightness (${Math.round(themeBrightness * 100)}%)`}
							divider={false}
						>
							<input
								type="range"
								min="0.0"
								max="1.0"
								step="0.02"
								value={themeBrightness}
								onChange={(e) => handleBrightnessChange(parseFloat(e.target.value))}
								className="settings-slider"
							/>
						</SettingRow>
					</>
				)}
			</div>

			<div className="setting-group-label">Display</div>
			<div className="setting-group">
				<SettingRow icon={Square} label="Screen Corners" desc="Rounded top edges">
					<label className="toggle-switch">
						<input type="checkbox" checked={cornersEnabled} onChange={toggleCorners} />
						<span className="slider"></span>
					</label>
				</SettingRow>

				<SettingRow icon={Maximize2} label="Scale Dock and Notch Together" desc="Use one size for both surfaces">
					<label className="toggle-switch">
						<input type="checkbox" aria-label="Scale dock and notch together" checked={scaleLinked} onChange={(e) => handleScaleLinkedChange(e.target.checked)} />
						<span className="slider"></span>
					</label>
				</SettingRow>
				{scaleLinked ? (
					<SettingRow icon={Maximize2} label="Dock & Notch Scale" desc="Resize both surfaces together">
						<ScaleButtons label="dock and notch scale" value={scale} onChange={handleScaleChange} />
					</SettingRow>
				) : (
					<>
						<SettingRow icon={Maximize2} label="Dock & Tray Scale" desc="Resize the dock and system tray">
							<ScaleButtons label="dock scale" value={dockScale} onChange={handleDockScaleChange} />
						</SettingRow>
						<SettingRow icon={Maximize2} label="Notch Scale" desc="Resize the notch independently">
							<ScaleButtons label="notch scale" value={notchScale} onChange={handleNotchScaleChange} />
						</SettingRow>
					</>
				)}
				<SettingRow icon={Maximize2} label="Text Scale" desc="Change text size without resizing icons" divider={false}>
					<ScaleButtons label="text scale" value={fontScale} onChange={handleFontScaleChange} />
				</SettingRow>
			</div>
		</>
	);
}
