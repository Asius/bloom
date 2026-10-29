use crate::state::*;
use crate::types::*;
use crate::utils::*;
use std::sync::mpsc::{channel, sync_channel, Sender, SyncSender};
use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU32, Ordering},
    Mutex, OnceLock,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use windows::core::BOOL;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Dwm::DwmGetWindowAttribute;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetWindowLongW, GetWindowThreadProcessId, IsWindowVisible, SetWindowsHookExW,
    GWL_EXSTYLE, MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_MOUSEMOVE,
};

fn overlay_enabled(app: &AppHandle, key: &str) -> bool {
    crate::utils::get_setting_str(app, key)
        .map(|v| v != "false")
        .unwrap_or(true)
}

pub fn setup_audio_visualization(app_handle: AppHandle) {
    std::thread::spawn(move || {
        use windows::Win32::Media::Audio::{
            eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDevice, IMMDeviceEnumerator,
            AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
        };
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
        };
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            const NUM_BANDS: usize = 5;
            let mut max_band_energies = [0.01f32; NUM_BANDS];
            let mut prev_values = [0.1f32; NUM_BANDS];

            loop {
                let _: Result<(), String> = (|| {
                    let enumerator: IMMDeviceEnumerator = CoCreateInstance(
                        &windows::Win32::Media::Audio::MMDeviceEnumerator,
                        None,
                        CLSCTX_ALL,
                    )
                    .map_err(|e| format!("CoCreateInstance failed: {:?}", e))?;

                    let device: IMMDevice =
                        enumerator
                            .GetDefaultAudioEndpoint(eRender, eConsole)
                            .map_err(|e| format!("GetDefaultAudioEndpoint failed: {:?}", e))?;

                    let current_id_str = if let Ok(id) = device.GetId() {
                        let id_str = windows::core::PCWSTR::from_raw(id.0)
                            .to_string()
                            .unwrap_or_default();
                        CoTaskMemFree(Some(id.0 as *const _));
                        id_str
                    } else {
                        String::new()
                    };

                    let audio_client: IAudioClient = device
                        .Activate(CLSCTX_ALL, None)
                        .map_err(|e| format!("Activate failed: {:?}", e))?;

                    let format_ptr = audio_client
                        .GetMixFormat()
                        .map_err(|e| format!("GetMixFormat failed: {:?}", e))?;

                    let channels = (*format_ptr).nChannels as usize;
                    let bits_per_sample = (*format_ptr).wBitsPerSample;
                    let bytes_per_sample = (bits_per_sample / 8) as usize;

                    if bytes_per_sample == 0 || channels == 0 || bytes_per_sample > 4 {
                        CoTaskMemFree(Some(format_ptr as *const _));
                        return Err(format!(
                            "Invalid audio format: channels={}, bits={}",
                            channels, bits_per_sample
                        ));
                    }

                    let buffer_duration = 10_000_000i64;
                    audio_client
                        .Initialize(
                            AUDCLNT_SHAREMODE_SHARED,
                            AUDCLNT_STREAMFLAGS_LOOPBACK,
                            buffer_duration,
                            0,
                            format_ptr,
                            Some(std::ptr::null()),
                        )
                        .map_err(|e| format!("Initialize failed: {:?}", e))?;

                    CoTaskMemFree(Some(format_ptr as *const _));

                    let capture_client: IAudioCaptureClient = audio_client
                        .GetService()
                        .map_err(|e| format!("GetService failed: {:?}", e))?;

                    audio_client
                        .Start()
                        .map_err(|e| format!("Start failed: {:?}", e))?;

                    const FFT_SIZE: usize = 512;
                    let mut fft_buffer = vec![0.0f32; FFT_SIZE];
                    let mut fft_input: Vec<rustfft::num_complex::Complex<f32>> =
                        vec![rustfft::num_complex::Complex::new(0.0, 0.0); FFT_SIZE];
                    let mut buffer_pos = 0;

                    let mut planner = rustfft::FftPlanner::<f32>::new();
                    let fft = planner.plan_fft_forward(FFT_SIZE);

                    let mut last_device_check = std::time::Instant::now();
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(32));

                        if last_device_check.elapsed().as_secs() >= 2 {
                            last_device_check = std::time::Instant::now();
                            if let Ok(new_device) =
                                enumerator.GetDefaultAudioEndpoint(eRender, eConsole)
                            {
                                if let Ok(new_id) = new_device.GetId() {
                                    let new_str = windows::core::PCWSTR::from_raw(new_id.0)
                                        .to_string()
                                        .unwrap_or_default();
                                    CoTaskMemFree(Some(new_id.0 as *const _));
                                    if current_id_str != new_str {
                                        return Err("Default device changed".into());
                                    }
                                }
                            }
                        }

                        // Skip all heavy processing if nothing is playing
                        if !ANY_MEDIA_PLAYING.load(Ordering::Relaxed) {
                            // Still gotta clear the buffer to avoid lag when it starts
                            loop {
                                let len = match capture_client.GetNextPacketSize() {
                                    Ok(l) => l,
                                    Err(_) => {
                                        return Err("Device invalidated".into());
                                    }
                                };
                                if len == 0 {
                                    break;
                                }
                                let mut data_ptr: *mut u8 = std::ptr::null_mut();
                                let mut num_frames = 0u32;
                                let mut flags = 0u32;
                                if capture_client
                                    .GetBuffer(
                                        &mut data_ptr,
                                        &mut num_frames,
                                        &mut flags,
                                        None,
                                        None,
                                    )
                                    .is_err()
                                {
                                    return Err("Device invalidated".into());
                                }
                                let _ = capture_client.ReleaseBuffer(num_frames);
                            }
                            continue;
                        }

                        let mut device_invalidated = false;
                        loop {
                            let packet_length = match capture_client.GetNextPacketSize() {
                                Ok(len) => len,
                                Err(_) => {
                                    device_invalidated = true;
                                    break;
                                }
                            };
                            if packet_length == 0 {
                                break;
                            }
                            let mut data_ptr: *mut u8 = std::ptr::null_mut();
                            let mut num_frames = 0u32;
                            let mut flags = 0u32;

                            if capture_client
                                .GetBuffer(&mut data_ptr, &mut num_frames, &mut flags, None, None)
                                .is_err()
                            {
                                device_invalidated = true;
                                break;
                            }

                            if !data_ptr.is_null()
                                && num_frames > 0
                                && bytes_per_sample > 0
                                && channels > 0
                            {
                                let stride = channels * bytes_per_sample;
                                for frame in 0..num_frames as usize {
                                    let frame_offset = frame * stride;
                                    let mut sample_val: i64 = 0;
                                    for ch in 0..channels {
                                        let sample_offset = frame_offset + ch * bytes_per_sample;
                                        let sample: i64 = match bytes_per_sample {
                                            2 => {
                                                *(data_ptr.add(sample_offset) as *const i16) as i64
                                            }
                                            4 => {
                                                *(data_ptr.add(sample_offset) as *const i32) as i64
                                            }
                                            _ => 0,
                                        };
                                        sample_val = sample_val.wrapping_add(sample);
                                    }
                                    sample_val /= channels as i64;
                                    let normalized = match bytes_per_sample {
                                        2 => (sample_val as f32) / 32768.0,
                                        4 => (sample_val as f32) / 2147483648.0,
                                        _ => 0.0,
                                    };
                                    if buffer_pos < FFT_SIZE {
                                        let window = 0.5
                                            * (1.0
                                                - (2.0 * std::f32::consts::PI * buffer_pos as f32
                                                    / FFT_SIZE as f32)
                                                    .cos());
                                        fft_buffer[buffer_pos] = normalized * window;
                                        buffer_pos += 1;
                                    }
                                    if buffer_pos >= FFT_SIZE {
                                        // Run FFT on the windowed buffer
                                        for (i, &sample) in fft_buffer.iter().enumerate() {
                                            fft_input[i] =
                                                rustfft::num_complex::Complex::new(sample, 0.0);
                                        }
                                        fft.process(&mut fft_input);

                                        let band_ranges =
                                            [(1, 2), (2, 6), (6, 18), (18, 60), (60, 200)];
                                        let mut output = [0.0f32; NUM_BANDS];
                                        for (band_idx, &(bin_start, bin_end)) in
                                            band_ranges.iter().enumerate()
                                        {
                                            let mut total_mag = 0.0f32;
                                            let mut count = 0u32;
                                            for sample in fft_input
                                                .iter()
                                                .take(bin_end.min(FFT_SIZE / 2))
                                                .skip(bin_start)
                                            {
                                                let mag = sample.norm();
                                                total_mag += mag;
                                                count += 1;
                                            }
                                            let avg_mag = total_mag / count.max(1) as f32;
                                            let mut scaled_mag = avg_mag;
                                            let weighting = [1.2, 1.2, 1.5, 2.8, 5.0];
                                            scaled_mag *= weighting[band_idx];
                                            if scaled_mag > max_band_energies[band_idx] {
                                                max_band_energies[band_idx] = scaled_mag;
                                            } else {
                                                max_band_energies[band_idx] *= 0.99;
                                            }
                                            let target = (scaled_mag
                                                / max_band_energies[band_idx].max(0.12))
                                            .min(1.0)
                                            .powf(0.75);
                                            let is_rising = target > prev_values[band_idx];
                                            let smooth_factor = if is_rising { 0.10 } else { 0.20 };
                                            output[band_idx] = prev_values[band_idx]
                                                * smooth_factor
                                                + target * (1.0 - smooth_factor);
                                            output[band_idx] = output[band_idx].clamp(0.18, 1.0);
                                            prev_values[band_idx] = output[band_idx];
                                        }
                                        let _ = app_handle.emit(
                                            "audio-visualization",
                                            AudioVisualizationData {
                                                frequencies: output.to_vec(),
                                            },
                                        );
                                        buffer_pos = 0;
                                    }
                                }
                                let _ = capture_client.ReleaseBuffer(num_frames);
                            }
                        }

                        if device_invalidated {
                            return Err("Device invalidated".into());
                        }
                    }
                })();

                // Wait before retrying (increased to 2500ms to allow Windows to fully update default endpoint)
                std::thread::sleep(std::time::Duration::from_millis(2500));
            }
            // if com_initialized { CoUninitialize(); }
        }
    });
}

pub fn setup_system_worker(app_handle: AppHandle) -> Sender<SystemCommand> {
    let (tx, rx) = channel::<SystemCommand>();
    let handle_system = app_handle.clone();
    std::thread::spawn(move || {
        use base64::{engine::general_purpose, Engine as _};
        use windows::Media::Control::{
            GlobalSystemMediaTransportControlsSessionManager,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus,
        };
        use windows::Storage::Streams::DataReader;
        use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
        use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator};
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
        };

        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator = CoCreateInstance::<_, IMMDeviceEnumerator>(
                &windows::Win32::Media::Audio::MMDeviceEnumerator,
                None,
                CLSCTX_ALL,
            )
            .ok();
            let mut device = enumerator
                .as_ref()
                .and_then(|e| e.GetDefaultAudioEndpoint(eRender, eConsole).ok());
            let mut audio_endpoint_volume: Option<IAudioEndpointVolume> = device
                .as_ref()
                .and_then(|d| d.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None).ok());

            let mut current_device_id = String::new();
            if let Some(ref d) = device {
                if let Ok(id) = d.GetId() {
                    current_device_id = windows::core::PCWSTR::from_raw(id.0)
                        .to_string()
                        .unwrap_or_default();
                    CoTaskMemFree(Some(id.0 as *const _));
                }
            }

            let mut manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
                .and_then(|op| op.get())
                .ok();
            let mut last_processed_media = std::time::Instant::now();
            let mut last_device_check = std::time::Instant::now();
            #[allow(clippy::type_complexity)]
            let mut last_emitted_info: Option<(
                String,
                String,
                bool,
                bool,
                Option<String>,
                i64,
                i64,
            )> = None;
            let mut last_volume: f32 = -1.0;
            let mut last_muted: bool = false;

            // Only hide the native flyout when Bloom's matching overlay replaces it.
            let hide_osd = |overlay_key: &str| {
                if !overlay_enabled(&handle_system, overlay_key) {
                    return;
                }
                use windows::Win32::UI::WindowsAndMessaging::{FindWindowA, ShowWindow, SW_HIDE};
                let class1 = windows::core::PCSTR(c"NativeHWNDHost".as_ptr() as *const u8);
                if let Ok(hwnd1) = FindWindowA(class1, windows::core::PCSTR::null()) {
                    let _ = ShowWindow(hwnd1, SW_HIDE);
                }
            };

            loop {
                // Check for device change
                if last_device_check.elapsed().as_secs() >= 2 {
                    last_device_check = std::time::Instant::now();
                    if let Some(ref enum_ref) = enumerator {
                        if let Ok(new_device) = enum_ref.GetDefaultAudioEndpoint(eRender, eConsole)
                        {
                            if let Ok(id) = new_device.GetId() {
                                let new_id = windows::core::PCWSTR::from_raw(id.0)
                                    .to_string()
                                    .unwrap_or_default();
                                CoTaskMemFree(Some(id.0 as *const _));
                                if new_id != current_device_id || audio_endpoint_volume.is_none() {
                                    current_device_id = new_id;
                                    device = Some(new_device);
                                    audio_endpoint_volume = device.as_ref().and_then(|d| {
                                        d.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None).ok()
                                    });
                                    // Reset last_volume to force an update event
                                    last_volume = -1.0;
                                }
                            }
                        }
                    }
                }

                if manager.is_none() {
                    manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
                        .and_then(|op| op.get())
                        .ok();
                }
                while let Ok(cmd) = rx.try_recv() {
                    // Only the volume commands need the audio endpoint. Media, brightness and
                    // visibility commands must keep working when there is no output device.
                    match (cmd, audio_endpoint_volume.as_ref()) {
                        (SystemCommand::SetVolume(volume), Some(aev)) => {
                            let _ = aev.SetMasterVolumeLevelScalar(
                                volume.clamp(0.0, 1.0),
                                std::ptr::null(),
                            );
                            if volume > 0.0 {
                                let _ = aev.SetMute(false, std::ptr::null());
                            }
                            hide_osd("bloom-volume-overlay-enabled");
                        }
                        (SystemCommand::SetVolume(_), None) => {}
                        (SystemCommand::MediaPlayPause, _) => {
                            if let Some(ref mgr) = manager {
                                if let Ok(session) = mgr.GetCurrentSession() {
                                    let _ = session.TryTogglePlayPauseAsync();
                                }
                            }
                        }
                        (SystemCommand::MediaNext, _) => {
                            if let Some(ref mgr) = manager {
                                if let Ok(session) = mgr.GetCurrentSession() {
                                    let _ = session.TrySkipNextAsync();
                                }
                            }
                        }
                        (SystemCommand::MediaPrevious, _) => {
                            if let Some(ref mgr) = manager {
                                if let Ok(session) = mgr.GetCurrentSession() {
                                    let _ = session.TrySkipPreviousAsync();
                                }
                            }
                        }
                        (SystemCommand::MediaSeek(position_ms), _) => {
                            if let Some(ref mgr) = manager {
                                if let Ok(session) = mgr.GetCurrentSession() {
                                    let ticks = position_ms * 10_000;
                                    let _ = session.TryChangePlaybackPositionAsync(ticks);
                                }
                            }
                        }
                        (SystemCommand::ToggleVisibility(visible), _) => {
                            let _ = handle_system.emit("visibility-change", visible);
                        }
                    }
                }
                if let Some(ref aev) = audio_endpoint_volume {
                    if let (Ok(vol), Ok(muted)) = (aev.GetMasterVolumeLevelScalar(), aev.GetMute())
                    {
                        let is_muted: bool = muted.into();
                        if (vol - last_volume).abs() > 0.001 || is_muted != last_muted {
                            last_volume = vol;
                            last_muted = is_muted;
                            crate::state::CURRENT_VOLUME
                                .store((vol * 100.0) as u32, Ordering::Relaxed);
                            let _ = handle_system.emit(
                                "volume-change",
                                VolumeChangeEvent {
                                    volume: vol,
                                    is_muted,
                                },
                            );
                            hide_osd("bloom-volume-overlay-enabled");
                        }
                    }
                }
                if last_processed_media.elapsed().as_millis() >= 2000 {
                    last_processed_media = std::time::Instant::now();
                    let mut best_info: Option<MediaInfo> = None;
                    if let Some(ref mgr) = manager {
                        if let Ok(sessions) = mgr.GetSessions() {
                            for i in 0..sessions.Size().unwrap_or(0) {
                                if let Ok(session) = sessions.GetAt(i) {
                                    let is_playing = session
										.GetPlaybackInfo()
										.ok()
										.and_then(|p| p.PlaybackStatus().ok())
										== Some(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing);
                                    if let Ok(props) =
                                        session.TryGetMediaPropertiesAsync().and_then(|op| op.get())
                                    {
                                        let title = props.Title().unwrap_or_default().to_string();
                                        if !title.is_empty() {
                                            let artist =
                                                props.Artist().unwrap_or_default().to_string();
                                            let mut artwork = None;
                                            let mut got_art_from_cache = false;
                                            if let Some((
                                                ref last_title,
                                                ref last_artist,
                                                _,
                                                _,
                                                ref last_art,
                                                _,
                                                _,
                                            )) = last_emitted_info
                                            {
                                                if last_title == &title && last_artist == &artist {
                                                    artwork = last_art
                                                        .as_ref()
                                                        .map(|art| vec![art.clone()]);
                                                    got_art_from_cache = true;
                                                }
                                            }

                                            if !got_art_from_cache {
                                                artwork = (|| -> Option<Vec<String>> {
                                                    let stream = props
                                                        .Thumbnail()
                                                        .ok()?
                                                        .OpenReadAsync()
                                                        .ok()?
                                                        .get()
                                                        .ok()?;
                                                    let content_type = stream
                                                        .ContentType()
                                                        .ok()?
                                                        .to_string()
                                                        .split(',')
                                                        .next()
                                                        .unwrap_or("image/jpeg")
                                                        .trim()
                                                        .to_string();
                                                    let reader =
                                                        DataReader::CreateDataReader(&stream)
                                                            .ok()?;
                                                    let mut all_bytes = Vec::new();
                                                    let chunk_size = 65536u32;
                                                    loop {
                                                        let loaded = reader
                                                            .LoadAsync(chunk_size)
                                                            .ok()?
                                                            .get()
                                                            .ok()?;
                                                        if loaded == 0 {
                                                            break;
                                                        }
                                                        let mut chunk = vec![0u8; loaded as usize];
                                                        reader.ReadBytes(&mut chunk).ok()?;
                                                        all_bytes.extend_from_slice(&chunk);
                                                    }
                                                    if all_bytes.is_empty() {
                                                        return None;
                                                    }
                                                    Some(vec![format!(
                                                        "data:{};base64,{}",
                                                        content_type,
                                                        general_purpose::STANDARD.encode(all_bytes)
                                                    )])
                                                })(
                                                );
                                            }

                                            // Extract timeline properties for progress bar
                                            let (position_ms, duration_ms, seek_enabled) = {
                                                let defaults = (0i64, 0i64, false);
                                                match session.GetTimelineProperties() {
                                                    Ok(timeline) => {
                                                        let start = timeline
                                                            .StartTime()
                                                            .map(|t| t.Duration / 10_000)
                                                            .unwrap_or(0);
                                                        let end = timeline
                                                            .EndTime()
                                                            .map(|t| t.Duration / 10_000)
                                                            .unwrap_or(0);
                                                        let pos = timeline
                                                            .Position()
                                                            .map(|t| t.Duration / 10_000)
                                                            .unwrap_or(0);
                                                        let dur = (end - start).max(0);
                                                        // If duration is 0 but position > 0, some players don't report start/end
                                                        // but still track position — use position as fallback duration indicator
                                                        let effective_dur =
                                                            if dur > 0 { dur } else { 0 };
                                                        // Check if seeking is supported
                                                        let seek = session
                                                            .GetPlaybackInfo()
                                                            .ok()
                                                            .and_then(|pi| pi.Controls().ok())
                                                            .map(|c| {
                                                                c.IsPlaybackPositionEnabled()
                                                                    .unwrap_or(false)
                                                            })
                                                            .unwrap_or(false);
                                                        (pos, effective_dur, seek)
                                                    }
                                                    Err(_) => defaults,
                                                }
                                            };

                                            let now_ms = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .map(|d| d.as_millis() as u64)
                                                .unwrap_or(0);
                                            let info = MediaInfo {
                                                title,
                                                artist,
                                                is_playing,
                                                has_media: true,
                                                artwork,
                                                position_ms,
                                                duration_ms,
                                                seek_enabled,
                                                position_updated_at: now_ms,
                                            };
                                            if is_playing {
                                                best_info = Some(info);
                                                break;
                                            } else if best_info.is_none() {
                                                best_info = Some(info);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let current = best_info.unwrap_or(MediaInfo {
                        title: "".into(),
                        artist: "".into(),
                        is_playing: false,
                        has_media: false,
                        artwork: None,
                        position_ms: 0,
                        duration_ms: 0,
                        seek_enabled: false,
                        position_updated_at: 0,
                    });
                    let art_str = current.artwork.as_ref().and_then(|a| a.first()).cloned();
                    if last_emitted_info.as_ref().is_none_or(|(t, a, p, h, art, pos, _dur)| {
                        t != &current.title || a != &current.artist || p != &current.is_playing || h != &current.has_media || art != &art_str ||
                        // For position: emit if >1s difference (avoid spamming on every poll)
                        (*pos - current.position_ms).abs() > 1000
                    }) {
                        let _ = handle_system.emit("media-update", current.clone());
                        ANY_MEDIA_PLAYING.store(current.is_playing, Ordering::Relaxed);
                        last_emitted_info = Some((current.title, current.artist, current.is_playing, current.has_media, art_str, current.position_ms, current.duration_ms));
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(64));
            }
        }
    });

    setup_island_visibility(app_handle.clone(), tx.clone());

    tx
}

/// Explorer-owned windows that host the Windows 11 volume/brightness flyout.
/// The flyout lives in a small, non-activating `XamlExplorerHostIslandWindow`;
/// the same class also hosts full-screen surfaces (Alt+Tab, Task View), which
/// the size check excludes.
fn native_osd_windows() -> Vec<HWND> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowA, FindWindowExW, GetSystemMetrics, GetWindowRect, SM_CXSCREEN, SM_CYSCREEN,
        WS_EX_NOACTIVATE,
    };
    let mut found = Vec::new();
    unsafe {
        let tray_class = windows::core::PCSTR(c"Shell_TrayWnd".as_ptr() as *const u8);
        let Ok(tray) = FindWindowA(tray_class, windows::core::PCSTR::null()) else {
            return found;
        };
        let mut explorer_pid = 0u32;
        GetWindowThreadProcessId(tray, Some(&mut explorer_pid));
        let (screen_w, screen_h) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));

        let mut after: Option<HWND> = None;
        while let Ok(hwnd) = FindWindowExW(
            None,
            after,
            windows::core::w!("XamlExplorerHostIslandWindow"),
            windows::core::PCWSTR::null(),
        ) {
            after = Some(hwnd);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            let mut rect = RECT::default();
            let small = GetWindowRect(hwnd, &mut rect).is_ok()
                && (rect.right - rect.left) < screen_w / 2
                && (rect.bottom - rect.top) < screen_h / 2;
            if pid == explorer_pid && (ex_style & WS_EX_NOACTIVATE.0) != 0 && small {
                found.push(hwnd);
            }
        }
    }
    found
}

/// Laptop brightness keys bypass input hooks (the firmware changes brightness
/// and notifies the display driver directly), so unlike the volume keys they
/// cannot be swallowed to keep Windows from drawing its own flyout. While
/// Bloom's overlay replaces it, the flyout host is kept fully transparent and
/// click-through instead; Explorer may still show it, but nothing is visible.
pub fn set_native_osd_suppressed(suppress: bool) {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetLayeredWindowAttributes, SetLayeredWindowAttributes, SetWindowLongW, LWA_ALPHA,
        WS_EX_LAYERED, WS_EX_TRANSPARENT,
    };
    for hwnd in native_osd_windows() {
        unsafe {
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            let mut alpha = 255u8;
            // Explorer never makes this window layered itself, so a layered
            // window at alpha 0 is Bloom's doing, including from a crashed run.
            let suppressed = (ex_style & WS_EX_LAYERED.0) != 0
                && GetLayeredWindowAttributes(hwnd, None, Some(&mut alpha), None).is_ok()
                && alpha == 0;
            if suppress && !suppressed {
                SetWindowLongW(
                    hwnd,
                    GWL_EXSTYLE,
                    (ex_style | WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0) as i32,
                );
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);
            } else if !suppress && suppressed {
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
                SetWindowLongW(
                    hwnd,
                    GWL_EXSTYLE,
                    (ex_style & !(WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0)) as i32,
                );
            }
        }
    }
}

/// This desktop build replaces the volume HUD. Restore the shared Windows
/// flyout host whenever that replacement is disabled.
fn sync_native_osd(app_handle: &AppHandle) {
    set_native_osd_suppressed(
        get_setting_str(app_handle, "bloom-volume-overlay-enabled").as_deref() != Some("false"),
    );
}

static MOUSE_HOOK_UPDATES: OnceLock<SyncSender<()>> = OnceLock::new();
static MOUSE_HOOK_CURSOR: AtomicI64 = AtomicI64::new(0);
static MOUSE_HOOK_LAST_EVENT_MS: AtomicI64 = AtomicI64::new(0);
static MH_LAST_MAIN_IGNORE: AtomicI32 = AtomicI32::new(-1);
static MH_LAST_OV_IGNORE: AtomicI32 = AtomicI32::new(-1);
static MH_LAST_TOP_EDGE_HOVER: AtomicI32 = AtomicI32::new(-1);
static MH_LAST_LEFT_EDGE_HOVER: AtomicI32 = AtomicI32::new(-1);
static MH_TOPBAR_EXPIRY_MS: AtomicI64 = AtomicI64::new(0);
static MH_LEFT_EXPIRY_MS: AtomicI64 = AtomicI64::new(0);
static MH_LAST_MONITOR_UPDATE_MS: AtomicI64 = AtomicI64::new(0);
static MH_CACHED_MON_POS: Mutex<Option<(i32, i32)>> = Mutex::new(None);
static MH_CACHED_MON_SIZE: Mutex<Option<(u32, u32)>> = Mutex::new(None);
static CAPTURE_UI_ACTIVE: AtomicBool = AtomicBool::new(false);

fn is_primary_monitor(flags: u32) -> bool {
    const PRIMARY_MONITOR_FLAG: u32 = 1;
    flags & PRIMARY_MONITOR_FLAG != 0
}

/// Hover-intent ("pressure") gate for the top edge. Contact with the top band
/// only starts a dwell clock; the edge is armed — and only then peeks the notch
/// or claims cursor input — after the cursor has rested in the band for the
/// configured delay. A pass-through to a maximized window's tab/title-bar
/// controls never arms it. The delay is user-tunable via
/// `bloom-notch-edge-delay` (ms); `0` restores the old instant behavior.
const TOP_EDGE_BAND_PX: f64 = 8.0;
const TOP_EDGE_DWELL_MS: i64 = 200;
/// The mouse hook only fires on WM_MOUSEMOVE, so a pending dwell is completed
/// by a watchdog that re-checks at this cadence.
const TOP_EDGE_POLL_MS: u64 = 40;
static MH_TOP_EDGE_ENTER_MS: AtomicI64 = AtomicI64::new(0);
static MH_TOP_EDGE_ARMED: AtomicBool = AtomicBool::new(false);

/// Physical bounds `(x, y, width, height)` of the expanded per-app volume
/// mixer (notch + panel), if it is open. The frontend reports the card in
/// overlay CSS pixels; scale and monitor offset convert it to virtual-desktop
/// coordinates.
fn volume_mixer_physical_rect_for(app: &AppHandle) -> Option<(i32, i32, i32, i32)> {
    let (x, y, width, height) = (*crate::state::VOLUME_MIXER_RECT.lock().ok()?)?;
    let ov_win = app.get_webview_window("overlay")?;
    let monitor = ov_win.primary_monitor().ok().flatten()?;
    let scale = monitor.scale_factor();
    let pos = monitor.position();
    Some((
        pos.x + (x * scale) as i32,
        pos.y + (y * scale) as i32,
        (width * scale) as i32,
        (height * scale) as i32,
    ))
}

/// The mouse hook only runs on mouse movement, so a cursor that stops just
/// outside the mixer would leave it open indefinitely. This watchdog re-checks
/// the position and closes the card as soon as the cursor is off it.
fn setup_volume_mixer_watchdog(app_handle: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(200));
        if SHUTTING_DOWN.load(Ordering::Relaxed) {
            continue;
        }
        let Some((rx, ry, rw, rh)) = volume_mixer_physical_rect_for(&app_handle) else {
            continue;
        };
        let mut pt = windows::Win32::Foundation::POINT::default();
        if unsafe { windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt) }.is_err() {
            continue;
        }
        let inside = pt.x >= rx && pt.x <= rx + rw && pt.y >= ry && pt.y <= ry + rh;
        if inside {
            continue;
        }
        let at_left_edge = app_handle
            .primary_monitor()
            .ok()
            .flatten()
            .is_some_and(|m| {
                let pos = m.position();
                let size = m.size();
                pt.x <= pos.x + 8 && pt.y >= pos.y && pt.y <= pos.y + size.height as i32
            });
        if !at_left_edge && MH_LAST_LEFT_EDGE_HOVER.swap(0, Ordering::Relaxed) != 0 {
            MH_LEFT_EXPIRY_MS.store(0, Ordering::Relaxed);
            let _ = app_handle.emit("volume-edge-hover", false);
        }
    });
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Re-runs notch hit-testing immediately. Called when the frontend's hover
/// flag changes so edge-hover state cannot go stale while the cursor is
/// stationary — a native drag-out ends with the pointer outside the notch but
/// the webview never saw it leave.
pub fn refresh_main_interaction(app_handle: &AppHandle) {
    let mut pt = windows::Win32::Foundation::POINT::default();
    if unsafe { windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt) }.is_err() {
        return;
    }
    let fg_fs = CURRENT_FOREGROUND_FULLSCREEN.load(Ordering::Relaxed);
    update_main_interaction(app_handle, pt, now_ms(), fg_fs);
}

// The Snipping Tool editor is a normal desktop app. Only its selection overlay
fn capture_blocks_bloom(process: &str, has_caption: bool, covers_monitor: bool) -> bool {
    process == "screenclippinghost.exe"
        || (matches!(process, "snippingtool.exe" | "screensketch.exe")
            && !has_caption
            && covers_monitor)
}

unsafe extern "system" fn capture_ui_enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    use windows::Win32::{
        Foundation::RECT,
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        UI::WindowsAndMessaging::{
            GetWindowLongPtrW, GetWindowRect, IsIconic, GWL_STYLE, WS_CAPTION,
        },
    };
    let found = &mut *(lparam.0 as *mut bool);
    if *found {
        return BOOL(0);
    }
    if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
        return BOOL(1);
    }

    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid != 0 && pid != std::process::id() {
        if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut buf = [0u16; 512];
            let mut len = buf.len() as u32;
            if QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(buf.as_mut_ptr()),
                &mut len,
            )
            .is_ok()
            {
                let path = String::from_utf16_lossy(&buf[..len as usize]).to_lowercase();
                let name = path.rsplit('\\').next().unwrap_or("");
                if matches!(
                    name,
                    "snippingtool.exe" | "screenclippinghost.exe" | "screensketch.exe"
                ) {
                    let mut cloaked = 0u32;
                    let is_cloaked = DwmGetWindowAttribute(
                        hwnd,
                        windows::Win32::Graphics::Dwm::DWMWA_CLOAKED,
                        &mut cloaked as *mut _ as *mut _,
                        std::mem::size_of::<u32>() as u32,
                    )
                    .is_ok()
                        && cloaked != 0;
                    let mut rect = RECT::default();
                    let mut monitor = MONITORINFO {
                        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                        ..Default::default()
                    };
                    let covers_monitor = GetWindowRect(hwnd, &mut rect).is_ok()
                        && GetMonitorInfoW(
                            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
                            &mut monitor,
                        )
                        .as_bool()
                        && rect.left <= monitor.rcMonitor.left
                        && rect.top <= monitor.rcMonitor.top
                        && rect.right >= monitor.rcMonitor.right
                        && rect.bottom >= monitor.rcMonitor.bottom;
                    let has_caption = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_CAPTION.0 != 0;
                    *found = !is_cloaked && capture_blocks_bloom(name, has_caption, covers_monitor);
                }
            }
            let _ = CloseHandle(handle);
        }
    }

    BOOL(1)
}

fn is_capture_ui_present() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::EnumWindows;
    let mut found = false;
    unsafe {
        let _ = EnumWindows(
            Some(capture_ui_enum_proc),
            LPARAM(&mut found as *mut bool as isize),
        );
    }
    found
}

fn apply_capture_ui_state(app: &AppHandle, active: bool) {
    if SHUTTING_DOWN.load(Ordering::Relaxed) {
        return;
    }
    crate::diagnostics::record(
        "capture-selection",
        if active { "active" } else { "inactive" },
    );
    if let Some(main_win) = app.get_webview_window("main") {
        if active {
            if let Ok(hwnd) = main_win.hwnd() {
                set_click_through(hwnd, true);
            }
            let _ = main_win.hide();
        } else {
            let _ = main_win.show();
            reconcile_main_appbar(app);
        }
    }
    if active {
        if let Some(hwnd) = app
            .get_webview_window("overlay")
            .and_then(|w| w.hwnd().ok())
        {
            set_click_through(hwnd, true);
        }
    }
    // Force the mouse hook to re-evaluate hit-testing on the next move.
    MH_LAST_MAIN_IGNORE.store(-1, Ordering::Relaxed);
    MH_LAST_OV_IGNORE.store(-1, Ordering::Relaxed);
    // Cancel any pending top/bottom-edge dwell so they cannot arm while Bloom
    // is click-through for the capture tool.
    MH_TOP_EDGE_ENTER_MS.store(0, Ordering::Relaxed);
    MH_TOP_EDGE_ARMED.store(false, Ordering::Relaxed);
}

pub fn setup_mouse_hook(app_handle: AppHandle) {
    let (updates, receiver) = sync_channel::<()>(1);
    if MOUSE_HOOK_UPDATES.set(updates).is_err() {
        return;
    }
    setup_volume_mixer_watchdog(app_handle.clone());
    setup_top_edge_watchdog(app_handle.clone());
    std::thread::Builder::new()
        .name("bloom-cursor-worker".into())
        .spawn(move || {
            let frame = Duration::from_millis(32);
            let mut last_processed = Instant::now() - frame;
            while !SHUTTING_DOWN.load(Ordering::Relaxed) {
                match receiver.recv_timeout(Duration::from_millis(500)) {
                    Ok(()) => {
                        if let Some(remaining) = frame.checked_sub(last_processed.elapsed()) {
                            std::thread::sleep(remaining);
                        }
                        let cursor = unpack_cursor(MOUSE_HOOK_CURSOR.load(Ordering::Relaxed));
                        process_mouse_position(&app_handle, cursor);
                        last_processed = Instant::now();
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .expect("Failed to start cursor worker");
    std::thread::Builder::new()
        .name("bloom-mouse-hook".into())
        .spawn(|| {
            while !SHUTTING_DOWN.load(Ordering::Relaxed) {
                run_mouse_hook_thread();
                if !SHUTTING_DOWN.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        })
        .expect("Failed to start mouse hook thread");
}

fn pack_cursor(point: windows::Win32::Foundation::POINT) -> i64 {
    ((point.x as u32 as u64) << 32 | point.y as u32 as u64) as i64
}

fn unpack_cursor(packed: i64) -> windows::Win32::Foundation::POINT {
    windows::Win32::Foundation::POINT {
        x: (packed as u64 >> 32) as u32 as i32,
        y: packed as u32 as i32,
    }
}

fn queue_cursor(point: windows::Win32::Foundation::POINT) {
    MOUSE_HOOK_CURSOR.store(pack_cursor(point), Ordering::Relaxed);
    if let Some(updates) = MOUSE_HOOK_UPDATES.get() {
        let _ = updates.try_send(());
    }
}

fn hook_tick_ms() -> i64 {
    unsafe { windows::Win32::System::SystemInformation::GetTickCount64() as i64 }
}

fn run_mouse_hook_thread() {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetCursorPos, GetMessageW, KillTimer, PeekMessageW, SetTimer,
        TranslateMessage, UnhookWindowsHookEx, MSG, PM_NOREMOVE, WM_TIMER,
    };
    unsafe {
        let mut message = MSG::default();
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
        // A timer wakes GetMessageW even when the mouse is idle, so the thread
        // can shut down and detect a hook Windows silently removed on timeout.
        let timer = SetTimer(None, 1, 500, None);
        if timer == 0 {
            eprintln!("Bloom mouse hook: timer creation failed; retrying");
            return;
        }
        let mut hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), None, 0).ok();
        crate::diagnostics::record(
            "mouse_hook",
            if hook.is_some() {
                "installed"
            } else {
                "install failed; retrying"
            },
        );
        let mut point = windows::Win32::Foundation::POINT::default();
        if GetCursorPos(&mut point).is_ok() {
            queue_cursor(point);
            MOUSE_HOOK_LAST_EVENT_MS.store(hook_tick_ms(), Ordering::Relaxed);
        }
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            if message.message == WM_TIMER && message.wParam.0 == timer {
                if SHUTTING_DOWN.load(Ordering::Relaxed) {
                    break;
                }
                let mut current = windows::Win32::Foundation::POINT::default();
                let cursor_moved_without_hook = GetCursorPos(&mut current).is_ok()
                    && pack_cursor(current) != MOUSE_HOOK_CURSOR.load(Ordering::Relaxed)
                    && hook_tick_ms() - MOUSE_HOOK_LAST_EVENT_MS.load(Ordering::Relaxed) > 750;
                if hook.is_none() || cursor_moved_without_hook {
                    if let Some(old) = hook.take() {
                        let _ = UnhookWindowsHookEx(old);
                    }
                    hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), None, 0).ok();
                    crate::diagnostics::record(
                        "mouse_hook",
                        if hook.is_some() {
                            "reinstalled"
                        } else {
                            "reinstall failed"
                        },
                    );
                    if hook.is_some() {
                        queue_cursor(current);
                        MOUSE_HOOK_LAST_EVENT_MS.store(hook_tick_ms(), Ordering::Relaxed);
                    }
                }
            } else {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if timer != 0 {
            let _ = KillTimer(None, timer);
        }
        if let Some(hook) = hook {
            let _ = UnhookWindowsHookEx(hook);
        }
    }
}

/// Top-edge hover-intent delay in milliseconds. `0` disables the gate so the
/// notch reveals the moment the band is touched (the pre-fix behavior).
fn top_edge_dwell_ms(app_handle: &AppHandle) -> i64 {
    crate::utils::get_setting_str(app_handle, "bloom-notch-edge-delay")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(TOP_EDGE_DWELL_MS)
        .clamp(0, 2000)
}

/// True when the cursor sits inside the top-edge hot band on the primary
/// monitor. Shared by the dwell gate and top-edge hit-testing continuity.
fn in_top_edge_band(cursor: windows::Win32::Foundation::POINT, scale: f64) -> bool {
    let (mon_x, mon_y) = MH_CACHED_MON_POS
        .lock()
        .ok()
        .and_then(|g| *g)
        .unwrap_or((0, 0));
    let mon_w = MH_CACHED_MON_SIZE
        .lock()
        .ok()
        .and_then(|g| *g)
        .map(|s| s.0 as i32)
        .unwrap_or(1920);

    cursor.y <= (mon_y + (TOP_EDGE_BAND_PX * scale) as i32)
        && cursor.x >= mon_x
        && cursor.x <= (mon_x + mon_w)
}

/// True once the cursor has stayed inside the top band long enough to count as
/// a deliberate reveal. Mere contact does not arm: a pass through the top strip
/// on the way to a maximized window's tab/title-bar controls must not peek the
/// notch. Leaving the band disarms and restarts the clock.
///
/// The mouse hook only fires on movement, so `setup_top_edge_watchdog` also
/// calls this while a dwell is pending so it can complete on a stationary
/// cursor.
fn top_edge_armed_for(
    app_handle: &AppHandle,
    cursor: windows::Win32::Foundation::POINT,
    now: i64,
    scale: f64,
) -> bool {
    if !in_top_edge_band(cursor, scale) {
        MH_TOP_EDGE_ENTER_MS.store(0, Ordering::Relaxed);
        MH_TOP_EDGE_ARMED.store(false, Ordering::Relaxed);
        return false;
    }
    if MH_TOP_EDGE_ARMED.load(Ordering::Relaxed) {
        return true;
    }
    let dwell_ms = top_edge_dwell_ms(app_handle);
    if dwell_ms <= 0 {
        MH_TOP_EDGE_ARMED.store(true, Ordering::Relaxed);
        return true;
    }
    let enter = MH_TOP_EDGE_ENTER_MS.load(Ordering::Relaxed);
    if enter == 0 {
        MH_TOP_EDGE_ENTER_MS.store(now, Ordering::Relaxed);
        return false;
    }
    if now - enter >= dwell_ms {
        MH_TOP_EDGE_ARMED.store(true, Ordering::Relaxed);
        return true;
    }
    false
}

/// Hover + click-through state for the notch window. Shared by the mouse hook
/// (on movement) and the top-edge watchdog (to finish a stationary dwell).
fn update_main_interaction(
    app_handle: &AppHandle,
    cursor: windows::Win32::Foundation::POINT,
    now: i64,
    fg_fs: bool,
) {
    if fg_fs {
        // Fullscreen foreground app or capture overlay: Bloom steps aside and
        // any pending top-edge dwell is dropped.
        MH_TOP_EDGE_ENTER_MS.store(0, Ordering::Relaxed);
        MH_TOP_EDGE_ARMED.store(false, Ordering::Relaxed);
        if let Some(main_win) = app_handle.get_webview_window("main") {
            if let Ok(hwnd) = main_win.hwnd() {
                if MH_LAST_MAIN_IGNORE.load(Ordering::Relaxed) != 1 {
                    re_assert_topmost(hwnd);
                }
                set_click_through(hwnd, true);
            }
            MH_LAST_MAIN_IGNORE.store(1, Ordering::Relaxed);
        }
        if MH_LAST_TOP_EDGE_HOVER.swap(0, Ordering::Relaxed) != 0 {
            let _ = app_handle.emit("notch-edge-hover", false);
        }
        // Bloom is fully click-through here, so the webview cannot clear its
        // own hover state; do it for it.
        if NOTCH_IS_HOVERED.swap(false, Ordering::Relaxed) {
            let _ = app_handle.emit("notch-pointer-left", ());
        }
        return;
    }

    let Some(main_win) = app_handle.get_webview_window("main") else {
        return;
    };
    if !main_win.is_visible().unwrap_or(false) {
        return;
    }

    let in_notch_hover = NOTCH_IS_HOVERED.load(Ordering::Relaxed);
    let scale = main_win.scale_factor().unwrap_or(1.0);
    // The top-edge reveal exists only to bring a hidden notch on screen: it is
    // dwell-gated, and fixed mode (always visible) skips it entirely. This is
    // about showing the notch — the hit-testing continuity below is separate
    // and must stay immediate.
    let edge_armed = if notch_mode_reserves_work_area(
        get_setting_str(app_handle, "bloom-notch-mode").as_deref(),
    ) {
        MH_TOP_EDGE_ENTER_MS.store(0, Ordering::Relaxed);
        MH_TOP_EDGE_ARMED.store(false, Ordering::Relaxed);
        false
    } else {
        top_edge_armed_for(app_handle, cursor, now, scale)
    };

    let mut is_notch_hovered = false;
    if edge_armed || in_notch_hover {
        is_notch_hovered = true;
        MH_TOPBAR_EXPIRY_MS.store(now + 500, Ordering::Relaxed);
    }

    // An OS file drag can only reach the webview's drop target when the
    // window is not click-through: WS_EX_TRANSPARENT hides the entire window
    // (and its webview children) from OLE hit-testing, which shows the
    // no-drop cursor and never emits drag events. Detect a held left button
    // so a drag over the notch can force interactivity.
    let drag_held = unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
        (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0
    };

    let mut is_click_interactive = false;
    let main_rect_val = MAIN_WINDOW_RECT.lock().ok().and_then(|g| *g);

    if let Some((win_pos, _)) = main_rect_val {
        if let Ok(region) = NOTCH_RECT.try_lock() {
            if let Some(r) = *region {
                let pad_x = (20.0 * scale) as i32;
                let pad_y_bottom = (5.0 * scale) as i32;
                // Hysteresis keeps the notch interactive a little past its
                // bounds once grabbed, so removing the edge-forced
                // interactivity doesn't reintroduce boundary flicker.
                let hyst = if MH_LAST_MAIN_IGNORE.load(Ordering::Relaxed) == 0 {
                    (10.0 * scale) as i32
                } else {
                    0
                };
                let rx = win_pos.x + (r.x as f64 * scale) as i32 - pad_x - hyst;
                let rw = (r.width as f64 * scale) as i32 + (pad_x * 2) + (hyst * 2);
                let ry_top = win_pos.y;
                let ry_bottom = win_pos.y + (r.height as f64 * scale) as i32 + pad_y_bottom + hyst;

                let in_notch_rect = cursor.x >= rx
                    && cursor.x <= (rx + rw)
                    && cursor.y >= ry_top
                    && cursor.y <= ry_bottom;

                // Approaching along the top edge keeps the window interactive
                // while near the notch's horizontal span. This must stay
                // immediate (never dwell-gated): NOTCH_RECT lags the expand/
                // contract animation, and letting click-through flip mid-hover
                // makes the notch expand/contract repeatedly. It is gated on
                // the notch being on screen (or a live hover) so a hidden
                // notch still can't swallow pass-by clicks.
                let edge_pad = (60.0 * scale) as i32;
                let in_top_span = in_top_edge_band(cursor, scale)
                    && cursor.x >= rx - edge_pad
                    && cursor.x <= rx + rw + edge_pad;

                let notch_visible = NOTCH_IS_VISIBLE.load(Ordering::Relaxed);
                let hover_active =
                    is_notch_hovered || now < MH_TOPBAR_EXPIRY_MS.load(Ordering::Relaxed);
                if (notch_visible || hover_active) && (in_notch_rect || in_top_span) {
                    is_click_interactive = true;
                }

                // A held left button means a possible OS drag: OLE cannot
                // hit-test a click-through window, so force interactivity over
                // the notch or the drop target is unreachable.
                if drag_held && (in_notch_rect || in_top_span) {
                    is_click_interactive = true;
                }
            }
        }
    }

    let final_notch_hover = is_notch_hovered || now < MH_TOPBAR_EXPIRY_MS.load(Ordering::Relaxed);
    let prev = MH_LAST_TOP_EDGE_HOVER.load(Ordering::Relaxed);
    let new_val = if final_notch_hover { 1 } else { 0 };
    if prev != new_val {
        let _ = app_handle.emit("notch-edge-hover", final_notch_hover);
        MH_LAST_TOP_EDGE_HOVER.store(new_val, Ordering::Relaxed);
    }
    // the menu is caught by the mouse hook instead (see mouse_hook_proc).
    let final_ignore = !is_click_interactive;
    let prev_ignore = MH_LAST_MAIN_IGNORE.load(Ordering::Relaxed);
    let new_ignore = if final_ignore { 1 } else { 0 };
    if prev_ignore != new_ignore {
        // Z-order re-assertion is a synchronous window operation; skip it
        // while a drag may be active, where it can visibly hitch the drag.
        if !drag_held {
            if let Ok(hwnd) = main_win.hwnd() {
                re_assert_topmost(hwnd);
            }
        }
        MH_LAST_MAIN_IGNORE.store(new_ignore, Ordering::Relaxed);
        // Once the window is click-through the webview can never receive the
        // mouseleave that would clear its hover state (it may already have
        // missed it if the pointer left in the same frame). Tell it explicitly
        // or the notch stays expanded in peek/smart mode.
        if new_ignore == 1 && NOTCH_IS_HOVERED.swap(false, Ordering::Relaxed) {
            let _ = app_handle.emit("notch-pointer-left", ());
        }
    }
    // Applied on every update, not only on a change: it writes only when the
    // window's actual style differs, so it also repairs a style that tao
    // rewrote behind the cache's back.
    if let Ok(hwnd) = main_win.hwnd() {
        set_click_through(hwnd, final_ignore);
    }
}

/// Keeps notch hit-testing fresh while the cursor is stationary: completes a
/// pending top-edge dwell, forces interactivity during a held-button drag, and
/// catches boundary crossings the movement-only hook misses (so the click-
/// through flip — and its hover-clear — always happens).
fn setup_top_edge_watchdog(app_handle: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(TOP_EDGE_POLL_MS));
        if CAPTURE_UI_ACTIVE.load(Ordering::Relaxed) || SHUTTING_DOWN.load(Ordering::Relaxed) {
            continue;
        }
        let mut pt = windows::Win32::Foundation::POINT::default();
        if unsafe { windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt) }.is_err() {
            continue;
        }
        let fg_fs = CURRENT_FOREGROUND_FULLSCREEN.load(Ordering::Relaxed);
        update_main_interaction(&app_handle, pt, now_ms(), fg_fs);
    });
}

unsafe extern "system" fn mouse_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    if code >= 0 && wparam.0 == WM_MOUSEMOVE as usize && !SHUTTING_DOWN.load(Ordering::Relaxed) {
        queue_cursor((*(lparam.0 as *const MSLLHOOKSTRUCT)).pt);
        MOUSE_HOOK_LAST_EVENT_MS.store(hook_tick_ms(), Ordering::Relaxed);
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn process_mouse_position(app_handle: &AppHandle, cursor: windows::Win32::Foundation::POINT) {
    if SHUTTING_DOWN.load(Ordering::Relaxed) {
        return;
    }
    let now = now_ms();
    {
        // While a capture UI (Snipping Tool) is up, Bloom is fully
        // click-through and skipped entirely so the tool owns the screen.
        if CAPTURE_UI_ACTIVE.load(Ordering::Relaxed) {
            if let Some(hwnd) = app_handle
                .get_webview_window("main")
                .and_then(|w| w.hwnd().ok())
            {
                set_click_through(hwnd, true);
            }
            MH_LAST_MAIN_IGNORE.store(1, Ordering::Relaxed);
            if MH_LAST_OV_IGNORE.load(Ordering::Relaxed) != 1 {
                if let Some(hwnd) = app_handle
                    .get_webview_window("overlay")
                    .and_then(|w| w.hwnd().ok())
                {
                    set_click_through(hwnd, true);
                }
                MH_LAST_OV_IGNORE.store(1, Ordering::Relaxed);
            }
            if MH_LAST_TOP_EDGE_HOVER.swap(0, Ordering::Relaxed) != 0 {
                let _ = app_handle.emit("notch-edge-hover", false);
            }
            return;
        }

        // Refresh cached monitor info every 1s
        if now - MH_LAST_MONITOR_UPDATE_MS.load(Ordering::Relaxed) > 1000 {
            if let Ok(Some(monitor)) = app_handle.primary_monitor() {
                let pos = *monitor.position();
                let size = *monitor.size();
                *MH_CACHED_MON_POS.lock().unwrap() = Some((pos.x, pos.y));
                *MH_CACHED_MON_SIZE.lock().unwrap() = Some((size.width, size.height));
                MH_LAST_MONITOR_UPDATE_MS.store(now, Ordering::Relaxed);
            }
        }

        let cached_pos = MH_CACHED_MON_POS.lock().unwrap().unwrap_or((0, 0));
        let cached_size = MH_CACHED_MON_SIZE.lock().unwrap().unwrap_or((1920, 1080));
        let mon_x = cached_pos.0;
        let mon_y = cached_pos.1;
        let mon_h = cached_size.1 as i32;

        let fg_fs = CURRENT_FOREGROUND_FULLSCREEN.load(Ordering::Relaxed);

        // --- Main (TopBar) Interaction ---
        update_main_interaction(app_handle, cursor, now, fg_fs);

        // Expanded per-app volume mixer (if open) — the reported rect covers
        // the notch plus panel, so it keeps the card alive and clickable.
        let mixer_rect = volume_mixer_physical_rect_for(app_handle);
        let in_mixer = mixer_rect.is_some_and(|(rx, ry, rw, rh)| {
            cursor.x >= rx && cursor.x <= rx + rw && cursor.y >= ry && cursor.y <= ry + rh
        });

        // --- Left Edge (Volume) ---
        if !fg_fs {
            let at_left_edge =
                cursor.x <= (mon_x + 8) && cursor.y >= mon_y && cursor.y <= (mon_y + mon_h);

            // The collapsed card extends past the 8px edge band, so once the
            // edge hover is established, keep it alive while the cursor is
            // over the card too (otherwise pausing on the mixer button lets
            // the hover expire and the HUD hides mid-click).
            let over_card = !at_left_edge
                && !in_mixer
                && MH_LAST_LEFT_EDGE_HOVER.load(Ordering::Relaxed) != 0
                && app_handle
                    .primary_monitor()
                    .ok()
                    .flatten()
                    .is_some_and(|m| {
                        let bloom_scale = crate::utils::get_notch_scale(app_handle);
                        let ms = m.size();
                        let mp = m.position();
                        let sc = m.scale_factor() * bloom_scale;
                        let nw = (48.0 * sc) as i32;
                        let nh = (196.0 * sc) as i32;
                        let nx = mp.x;
                        let ny = mp.y + (ms.height as i32 / 2) - (nh / 2);
                        cursor.x >= nx
                            && cursor.x <= nx + nw
                            && cursor.y >= ny
                            && cursor.y <= ny + nh
                    });

            if at_left_edge || in_mixer || over_card {
                MH_LEFT_EXPIRY_MS.store(now + 500, Ordering::Relaxed);
            }

            let final_left_hover = now < MH_LEFT_EXPIRY_MS.load(Ordering::Relaxed);
            let prev = MH_LAST_LEFT_EDGE_HOVER.load(Ordering::Relaxed);
            let new_val = if final_left_hover { 1 } else { 0 };
            if prev != new_val {
                let _ = app_handle.emit("volume-edge-hover", final_left_hover);
                MH_LAST_LEFT_EDGE_HOVER.store(new_val, Ordering::Relaxed);
            }
        } else {
            let prev = MH_LAST_LEFT_EDGE_HOVER.load(Ordering::Relaxed);
            if prev != 0 {
                let _ = app_handle.emit("volume-edge-hover", false);
                MH_LAST_LEFT_EDGE_HOVER.store(0, Ordering::Relaxed);
            }
        }

        // --- Overlay cursor passthrough ---
        if let Some(ov_win) = app_handle.get_webview_window("overlay") {
            if fg_fs {
                let prev = MH_LAST_OV_IGNORE.load(Ordering::Relaxed);
                if let Ok(hwnd) = ov_win.hwnd() {
                    if prev != 1 {
                        re_assert_topmost(hwnd);
                    }
                    set_click_through(hwnd, true);
                }
                MH_LAST_OV_IGNORE.store(1, Ordering::Relaxed);
            } else {
                // The side volume card follows the island scale.
                let bloom_scale = crate::utils::get_notch_scale(app_handle);
                let over_left = in_mixer
                    || if let Ok(Some(m)) = ov_win.primary_monitor() {
                        let ms = m.size();
                        let mp = m.position();
                        let sc = m.scale_factor() * bloom_scale;
                        let nw = (48.0 * sc) as i32;
                        let nh = (196.0 * sc) as i32;
                        let nx = mp.x;
                        let ny = mp.y + (ms.height as i32 / 2) - (nh / 2);
                        cursor.x >= nx
                            && cursor.x <= nx + nw
                            && cursor.y >= ny
                            && cursor.y <= ny + nh
                    } else {
                        false
                    };

                let should_ignore = !over_left;
                let prev = MH_LAST_OV_IGNORE.load(Ordering::Relaxed);
                let new_val = if should_ignore { 1 } else { 0 };
                if prev != new_val {
                    if let Ok(hwnd) = ov_win.hwnd() {
                        re_assert_topmost(hwnd);
                    }
                    MH_LAST_OV_IGNORE.store(new_val, Ordering::Relaxed);
                }
                // Repair input styles after a WebView style change.
                if let Ok(hwnd) = ov_win.hwnd() {
                    set_click_through(hwnd, should_ignore);
                }
            }
        }
    }
}

pub fn sync_overlays(app: &AppHandle) {
    // Don't reposition while splash is playing
    if crate::state::OVERLAY_IN_SPLASH.load(Ordering::Relaxed) {
        return;
    }
    // Full-screen overlay — notches render at left/right edges via CSS
    // The window covers the entire primary monitor so it never needs repositioning
    if let Some(ov_win) = app.get_webview_window("overlay") {
        if let Ok(Some(monitor)) = ov_win.primary_monitor() {
            let size = monitor.size();
            let pos = monitor.position();
            let _ = ov_win.set_position(tauri::PhysicalPosition::new(pos.x, pos.y));
            let _ = ov_win.set_size(tauri::PhysicalSize::new(size.width, size.height));
        }
        if let Ok(hwnd) = ov_win.hwnd() {
            re_assert_topmost(hwnd);
        }
    }
}

pub fn register_appbar(window: tauri::WebviewWindow) {
    if SHUTTING_DOWN.load(Ordering::Relaxed) {
        return;
    }
    if let Ok(Some(monitor)) = window.app_handle().primary_monitor() {
        let m_size = monitor.size();
        let m_pos = monitor.position();
        let hwnd = window.hwnd().unwrap();
        let scale = monitor.scale_factor();
        let bloom_scale = crate::utils::get_notch_scale(window.app_handle());
        let ph = ((480.0 * bloom_scale) * scale) as i32;
        let pr = ((48.0 * bloom_scale) * scale) as i32;

        unsafe {
            use windows::Win32::Foundation::RECT;
            use windows::Win32::UI::Shell::{
                SHAppBarMessage, ABE_TOP, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, APPBARDATA,
            };
            use windows::Win32::UI::WindowsAndMessaging::{
                GetWindowLongPtrW, GetWindowRect, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE,
                SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOZORDER, WS_EX_NOACTIVATE as WS_EX_NA,
                WS_EX_TOOLWINDOW,
            };

            // Set styles first
            let mut ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as usize;
            ex_style |= (WS_EX_TOOLWINDOW.0 | WS_EX_NA.0) as usize;
            let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style as isize);

            let mut abd = APPBARDATA {
                cbSize: std::mem::size_of::<APPBARDATA>() as u32,
                hWnd: hwnd,
                uCallbackMessage: 0x8041, // WM_APP + 0x41, reserved for the notch AppBar
                ..Default::default()
            };

            if !MAIN_APPBAR_REGISTERED.load(Ordering::Relaxed) {
                let _ = SHAppBarMessage(ABM_REMOVE, &mut abd);
                let registered = SHAppBarMessage(ABM_NEW, &mut abd);
                crate::diagnostics::record(
                    "main_appbar",
                    &format!("registration_result={registered}"),
                );
                if registered == 0 {
                    eprintln!("Bloom: notch AppBar registration failed");
                    return;
                }
                MAIN_APPBAR_REGISTERED.store(true, Ordering::Relaxed);
            }

            abd.uEdge = ABE_TOP;
            abd.rc = RECT {
                left: m_pos.x,
                top: m_pos.y,
                right: m_pos.x + m_size.width as i32,
                bottom: m_pos.y + pr,
            };

            SHAppBarMessage(ABM_QUERYPOS, &mut abd);
            SHAppBarMessage(ABM_SETPOS, &mut abd);

            // Use the shell-approved rect for the final position, but keep our ph height for the window
            let final_width = abd.rc.right - abd.rc.left;

            let mut current_rect = RECT::default();
            let mut already_positioned = false;
            if GetWindowRect(hwnd, &mut current_rect).is_ok() {
                let current_width = current_rect.right - current_rect.left;
                let current_height = current_rect.bottom - current_rect.top;
                if current_rect.left == abd.rc.left
                    && current_rect.top == abd.rc.top
                    && current_width == final_width
                    && current_height == ph
                {
                    already_positioned = true;
                }
            }

            if !already_positioned {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    abd.rc.left,
                    abd.rc.top,
                    final_width,
                    ph,
                    SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }

            // Re-assert topmost after repositioning — use re_assert_topmost instead of
            // set_always_on_top(true) to include SWP_NOACTIVATE and re-stamp WS_EX_NOACTIVATE.
            // This prevents WM_ACTIVATE from reaching WebView2, which caused bloom windows
            // to blank/hide when other windows were minimized or closed.
            re_assert_topmost(hwnd);

            if !window.is_visible().unwrap_or(false) {
                let _ = window.show();
            }
        }
    } else {
        let w = window.clone();
        tauri::async_runtime::spawn(async move {
            for _ in 0..10 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if let Ok(Some(_monitor)) = w.app_handle().primary_monitor() {
                    // Reconcile rather than register directly: the notch mode
                    // may have changed while waiting for the monitor.
                    reconcile_main_appbar(w.app_handle());
                    break;
                }
            }
        });
    }
}

/// Serializes main AppBar register/unregister decisions. Mode switches and
/// display/power events can otherwise interleave between reading the notch mode
/// and applying it, leaving the reservation in the wrong state until the next
/// reconcile.
static MAIN_APPBAR_LOCK: Mutex<()> = Mutex::new(());

fn lock_main_appbar() -> std::sync::MutexGuard<'static, ()> {
    MAIN_APPBAR_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Apply a reservation decision to the main window. Callers must hold
/// `MAIN_APPBAR_LOCK`.
fn set_main_appbar_reservation(main_win: tauri::WebviewWindow, reserve: bool) {
    if reserve {
        register_appbar(main_win);
    } else if let Ok(hwnd) = main_win.hwnd() {
        // Ask the shell to remove the reservation unconditionally: the flag
        // may look clean while the shell still holds the strip.
        unregister_appbar_native(hwnd);
        MAIN_APPBAR_REGISTERED.store(false, Ordering::Relaxed);
        re_assert_topmost(hwnd);
    }
}

/// Whether the notch's top strip should currently be reserved as an AppBar.
/// Only fixed mode reserves screen space; smart and peek hide the notch freely.
/// An unset mode defaults to fixed, matching SETTINGS.md and `re_register_appbars`.
fn notch_mode_reserves_work_area(mode: Option<&str>) -> bool {
    mode.map(|m| m == "fixed").unwrap_or(true)
}

/// Bring the main AppBar registration in line with the configured notch mode.
///
/// `MAIN_APPBAR_REGISTERED` can drift from the setting: a DPI/scale change used
/// to register the window unconditionally, and subsequent power/display events
/// kept re-registering it from the stale flag, leaving the work-area gap in
/// smart/peek mode until restart (issue #109). Deciding from the mode makes
/// every re-registration path self-healing.
pub fn reconcile_main_appbar(app: &AppHandle) {
    let _guard = lock_main_appbar();
    let mode = get_setting_str(app, "bloom-notch-mode");
    if let Some(main_win) = app.get_webview_window("main") {
        set_main_appbar_reservation(main_win, notch_mode_reserves_work_area(mode.as_deref()));
    }
}

/// Apply an explicit mode to the main AppBar. `change_notch_mode` knows the
/// target mode even when the settings cache has not caught up yet, so it cannot
/// rely on a reconcile read.
pub fn apply_main_appbar_mode(app: &AppHandle, reserve: bool) {
    let _guard = lock_main_appbar();
    if let Some(main_win) = app.get_webview_window("main") {
        set_main_appbar_reservation(main_win, reserve);
    }
}

pub fn unregister_appbar_native(hwnd: HWND) {
    unsafe {
        use windows::Win32::UI::Shell::{SHAppBarMessage, ABM_REMOVE, APPBARDATA};
        let mut abd = APPBARDATA {
            cbSize: std::mem::size_of::<APPBARDATA>() as u32,
            hWnd: hwnd,
            ..Default::default()
        };
        SHAppBarMessage(ABM_REMOVE, &mut abd);
    }
}

fn reposition_all_windows(app: &AppHandle) {
    if !SHUTTING_DOWN.load(Ordering::Relaxed) {
        reposition_island(app);
    }
}

static TASKBAR_CREATED_MESSAGE: AtomicU32 = AtomicU32::new(0);

pub fn setup_display_change_monitor(app_handle: AppHandle) {
    let _ = crate::state::DISPLAY_MONITOR_HANDLE.set(app_handle);

    std::thread::spawn(|| unsafe {
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::Win32::UI::WindowsAndMessaging::*;

        let taskbar_created =
            RegisterWindowMessageA(windows::core::PCSTR(c"TaskbarCreated".as_ptr() as *const u8));
        TASKBAR_CREATED_MESSAGE.store(taskbar_created, Ordering::Relaxed);

        let class_name = windows::core::PCSTR(c"BloomDisplayMonitor".as_ptr() as *const u8);
        let h_inst = GetModuleHandleW(None).unwrap_or_default().into();

        let wnd_class = WNDCLASSEXA {
            cbSize: std::mem::size_of::<WNDCLASSEXA>() as u32,
            lpfnWndProc: Some(display_monitor_proc),
            hInstance: h_inst,
            lpszClassName: class_name,
            ..Default::default()
        };

        if RegisterClassExA(&wnd_class) == 0 {
            return;
        }

        let hwnd = CreateWindowExA(
            WINDOW_EX_STYLE(WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0),
            class_name,
            windows::core::PCSTR::null(),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(h_inst),
            None,
        );

        let hwnd = match hwnd {
            Ok(h) => h,
            Err(_) => return,
        };

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, Some(hwnd), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

unsafe extern "system" fn display_monitor_proc(
    hwnd: HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    use windows::Win32::UI::WindowsAndMessaging::*;

    const WM_DISPLAYCHANGE: u32 = 0x007E;
    const WM_POWERBROADCAST: u32 = 0x0218;
    const PBT_APMPOWERSTATUSCHANGE: u32 = 0x000A;
    const PBT_APMRESUMESUSPEND: u32 = 0x0007;
    const WM_DWMCOLORIZATIONCOLORCHANGED: u32 = 0x0320;
    const WM_SETTINGCHANGE: u32 = 0x001A;

    if msg != 0 && msg == TASKBAR_CREATED_MESSAGE.load(Ordering::Relaxed) {
        // Explorer has rebuilt its AppBar list. A primary-monitor DPI change
        // can send the same broadcast, so re-register in either case.
        MAIN_APPBAR_REGISTERED.store(false, Ordering::Relaxed);
        if let Some(app_handle) = DISPLAY_MONITOR_HANDLE.get() {
            let ah = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                for delay in [300, 1200] {
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    if SHUTTING_DOWN.load(Ordering::Relaxed) {
                        break;
                    }
                    reposition_all_windows(&ah);
                }
            });
        }
        return LRESULT(0);
    }

    match msg {
        WM_DISPLAYCHANGE => {
            let now = get_now_ms();
            let last = LAST_DISPLAY_CHANGE_MS.load(Ordering::Relaxed);
            if now - last > 500 {
                LAST_DISPLAY_CHANGE_MS.store(now, Ordering::Relaxed);
                if let Some(app_handle) = DISPLAY_MONITOR_HANDLE.get() {
                    let ah = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        reposition_all_windows(&ah);
                    });
                }
            }
            LRESULT(0)
        }
        WM_POWERBROADCAST => {
            let pw = wparam.0 as u32;
            if pw == PBT_APMPOWERSTATUSCHANGE || pw == PBT_APMRESUMESUSPEND {
                let now = get_now_ms();
                let last = LAST_DISPLAY_CHANGE_MS.load(Ordering::Relaxed);
                if now - last > 1000 {
                    LAST_DISPLAY_CHANGE_MS.store(now, Ordering::Relaxed);
                    if let Some(app_handle) = DISPLAY_MONITOR_HANDLE.get() {
                        let ah = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            // Longer delay for wake from sleep to allow display to fully initialize
                            let delay = if pw == PBT_APMRESUMESUSPEND { 500 } else { 300 };
                            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                            reposition_all_windows(&ah);
                        });
                    }
                }
            }
            LRESULT(0)
        }
        WM_DWMCOLORIZATIONCOLORCHANGED | WM_SETTINGCHANGE => {
            if let Some(app_handle) = DISPLAY_MONITOR_HANDLE.get() {
                let _ = app_handle.emit(
                    "system-transparency-changed",
                    crate::commands::get_system_transparency(),
                );
                let color_hex = if let Some(c) = crate::commands::get_windows_accent_color() {
                    c
                } else {
                    let mut color = 0u32;
                    let mut opaque = windows::core::BOOL(0);
                    if unsafe {
                        windows::Win32::Graphics::Dwm::DwmGetColorizationColor(
                            &mut color,
                            &mut opaque,
                        )
                        .is_ok()
                    } {
                        let r = ((color >> 16) & 0xff) as u8;
                        let g = ((color >> 8) & 0xff) as u8;
                        let b = (color & 0xff) as u8;
                        format!("#{:02x}{:02x}{:02x}", r, g, b)
                    } else {
                        "#0078d4".to_string()
                    }
                };
                let _ = app_handle.emit("system-accent-changed", color_hex);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

static WEBVIEWS_WATCHED_AT: OnceLock<Instant> = OnceLock::new();
static RECOVERING_FROM_WEBVIEW_FAILURE: AtomicBool = AtomicBool::new(false);

/// If a webview's browser or renderer process exits (crash, or killed), the
/// island windows stay on screen with dead content. They also stop
/// switching click-through, so they keep swallowing clicks meant for the
/// windows under them. Bloom restarts itself when that happens.
pub fn watch_webview_processes(app: &AppHandle) {
    let _ = WEBVIEWS_WATCHED_AT.set(Instant::now());
    for (_, window) in app.webview_windows() {
        let handle = app.clone();
        let _ = window.with_webview(move |webview| unsafe {
            use webview2_com::Microsoft::Web::WebView2::Win32::{
                COREWEBVIEW2_PROCESS_FAILED_KIND,
                COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED,
                COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED,
            };
            use webview2_com::ProcessFailedEventHandler;

            let Ok(core) = webview.controller().CoreWebView2() else {
                return;
            };
            let mut token = 0i64;
            let _ = core.add_ProcessFailed(
                &ProcessFailedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
                    args.ProcessFailedKind(&mut kind)?;
                    if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED
                        || kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED
                    {
                        crate::diagnostics::record(
                            "webview_failure",
                            &format!("process_kind={kind:?}"),
                        );
                        recover_from_webview_failure(&handle);
                    }
                    Ok(())
                })),
                &mut token,
            );
        });
    }
}

fn recover_from_webview_failure(handle: &AppHandle) {
    // Quitting destroys the webviews, which ends their processes too. And
    // every window reports the same browser exit, so act once.
    if SHUTTING_DOWN.load(Ordering::Relaxed)
        || RECOVERING_FROM_WEBVIEW_FAILURE.swap(true, Ordering::SeqCst)
    {
        return;
    }
    let started = WEBVIEWS_WATCHED_AT.get().map(Instant::elapsed);
    let handle = handle.clone();
    // Leave the WebView2 callback before touching windows.
    tauri::async_runtime::spawn(async move {
        // A webview that dies right after launch would die again after a
        // restart, so release the island and exit instead of looping.
        if started.is_some_and(|elapsed| elapsed < Duration::from_secs(30)) {
            crate::diagnostics::record("webview_recovery", "startup failure; exiting safely");
            crate::commands::quit_island(&handle);
        } else {
            crate::diagnostics::record("webview_recovery", "restarting Bloom");
            crate::commands::restart_bloom(handle).await;
        }
    });
}

pub fn reposition_island(app: &AppHandle) {
    if SHUTTING_DOWN.load(Ordering::Relaxed) {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(Some(monitor)) = window.primary_monitor() {
            let height = (480.0 * crate::utils::get_notch_scale(app) * monitor.scale_factor())
                .round() as u32;
            let _ = window.set_position(*monitor.position());
            let _ = window.set_size(tauri::PhysicalSize::new(monitor.size().width, height));
        }
    }
    reconcile_main_appbar(app);
    sync_overlays(app);
}

/// Poll only foreground geometry and capture selection; no launcher catalog,
/// background-window inventory or replacement-taskbar enforcement.
fn setup_island_visibility(app: AppHandle, tx: Sender<SystemCommand>) {
    std::thread::spawn(move || unsafe {
        use windows::Win32::{
            Foundation::RECT,
            Graphics::Gdi::{
                GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
            },
            UI::WindowsAndMessaging::*,
        };
        let mut last = HWND::default();
        let mut previous = None;
        let mut tick = 0u32;
        use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent};
        unsafe extern "system" fn capture_changed(
            _: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
            _: u32,
            _: HWND,
            object: i32,
            child: i32,
            _: u32,
            _: u32,
        ) {
            if object == 0 && child == 0 {
                CAPTURE_DIRTY.store(true, Ordering::Relaxed);
            }
        }
        let hook = SetWinEventHook(
            EVENT_OBJECT_SHOW,
            EVENT_OBJECT_HIDE,
            None,
            Some(capture_changed),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );
        while !SHUTTING_DOWN.load(Ordering::Relaxed) {
            let mut foreground = GetForegroundWindow();
            let mut pid = 0;
            GetWindowThreadProcessId(foreground, Some(&mut pid));
            if pid == std::process::id() && IsWindow(Some(last)).as_bool() {
                foreground = last;
            } else {
                last = foreground;
            }
            let mut rectangle = RECT::default();
            let mut monitor = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let mut class = [0u16; 128];
            let len = GetClassNameW(foreground, &mut class);
            let excluded =
                excluded_foreground_class(&String::from_utf16_lossy(&class[..len as usize]));
            let meaningful = !excluded
                && !foreground.is_invalid()
                && IsWindowVisible(foreground).as_bool()
                && !IsIconic(foreground).as_bool()
                && GetWindowRect(foreground, &mut rectangle).is_ok()
                && GetMonitorInfoW(
                    MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST),
                    &mut monitor,
                )
                .as_bool()
                && is_primary_monitor(monitor.dwFlags);
            let full = meaningful
                && rectangle.left <= monitor.rcMonitor.left
                && rectangle.top <= monitor.rcMonitor.top
                && rectangle.right >= monitor.rcMonitor.right
                && rectangle.bottom >= monitor.rcMonitor.bottom
                && !(IsZoomed(foreground).as_bool()
                    && GetWindowLongW(foreground, GWL_STYLE) as u32 & WS_CAPTION.0 != 0);
            let overlap = meaningful
                && app.primary_monitor().ok().flatten().is_some_and(|display| {
                    NOTCH_RECT
                        .lock()
                        .ok()
                        .and_then(|rect| *rect)
                        .is_some_and(|notch| {
                            let scale = display.scale_factor();
                            let origin = display.position();
                            let left = origin.x + (notch.x as f64 * scale) as i32;
                            let right = left + (notch.width as f64 * scale) as i32;
                            rectangle.left < right
                                && rectangle.right > left
                                && rectangle.top
                                    < origin.y
                                        + (48.0 * crate::utils::get_notch_scale(&app) * scale)
                                            as i32
                        })
                });
            CURRENT_NOTCH_OVERLAP.store(if overlap { 1 } else { 0 }, Ordering::Relaxed);
            if previous != Some(overlap) {
                let _ = app.emit("notch-overlap", overlap);
                previous = Some(overlap);
            }
            if CURRENT_FOREGROUND_FULLSCREEN.swap(full, Ordering::Relaxed) != full {
                let _ = tx.send(SystemCommand::ToggleVisibility(!full));
            }
            // Capture needs prompt recovery even when no cursor movement occurs.
            if CAPTURE_DIRTY.swap(false, Ordering::Relaxed)
                || tick == 0
                || CAPTURE_UI_ACTIVE.load(Ordering::Relaxed)
            {
                let active = is_capture_ui_present();
                if CAPTURE_UI_ACTIVE.swap(active, Ordering::Relaxed) != active {
                    apply_capture_ui_state(&app, active);
                }
            }
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            tick += 1;
            if tick >= 6 {
                tick = 0;
                sync_native_osd(&app);
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        if !hook.is_invalid() {
            let _ = UnhookWinEvent(hook);
        }
    });
}

static CAPTURE_DIRTY: AtomicBool = AtomicBool::new(true);
fn excluded_foreground_class(class: &str) -> bool {
    matches!(
        class,
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
}
#[cfg(test)]
mod island_tests {
    use super::*;
    #[test]
    fn the_desktop_and_native_taskbar_do_not_hide_the_island_as_fullscreen_apps() {
        assert!(excluded_foreground_class("Progman"));
        assert!(excluded_foreground_class("WorkerW"));
        assert!(excluded_foreground_class("Shell_TrayWnd"));
        assert!(!excluded_foreground_class("Chrome_WidgetWin_1"));
    }
    #[test]
    fn capture_selection_yields_input_but_an_edited_screenshot_does_not() {
        assert!(capture_blocks_bloom("snippingtool.exe", false, true));
        assert!(!capture_blocks_bloom("snippingtool.exe", true, true));
        assert!(!capture_blocks_bloom("notepad.exe", false, true));
    }
    #[test]
    fn cursor_coordinates_preserve_negative_monitors() {
        let point = windows::Win32::Foundation::POINT { x: -1920, y: 1080 };
        let restored = unpack_cursor(pack_cursor(point));
        assert_eq!((restored.x, restored.y), (point.x, point.y));
    }
    #[test]
    fn only_fixed_island_mode_reserves_screen_space() {
        assert!(notch_mode_reserves_work_area(Some("fixed")));
        assert!(!notch_mode_reserves_work_area(Some("smart")));
        assert!(!notch_mode_reserves_work_area(Some("peek")));
    }
}
