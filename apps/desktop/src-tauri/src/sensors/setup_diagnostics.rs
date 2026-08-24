use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

#[derive(Debug, Clone, PartialEq)]
pub struct MicrophoneProbe {
    pub ready: bool,
    pub device_name: Option<String>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u16>,
    pub peak_level: Option<f32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareProbe {
    pub total_memory_mb: Option<u64>,
    pub logical_cpu_count: usize,
    pub architecture: String,
}

pub fn probe_hardware() -> HardwareProbe {
    HardwareProbe {
        total_memory_mb: total_memory_mb(),
        logical_cpu_count: std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1),
        architecture: std::env::consts::ARCH.to_string(),
    }
}

pub fn probe_microphone(sample_duration_ms: u64) -> MicrophoneProbe {
    match try_probe_microphone(sample_duration_ms.min(3_000)) {
        Ok(probe) => probe,
        Err(error) => MicrophoneProbe {
            ready: false,
            device_name: None,
            sample_rate_hz: None,
            channels: None,
            peak_level: None,
            error: Some(error),
        },
    }
}

fn try_probe_microphone(sample_duration_ms: u64) -> Result<MicrophoneProbe, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "No input device is available".to_string())?;
    let device_name = device.name().ok();
    let supported = device
        .default_input_config()
        .map_err(|error| format!("Failed to read microphone configuration: {error}"))?;
    let sample_rate_hz = supported.sample_rate().0;
    let channels = supported.channels();

    if sample_duration_ms == 0 {
        return Ok(MicrophoneProbe {
            ready: true,
            device_name,
            sample_rate_hz: Some(sample_rate_hz),
            channels: Some(channels),
            peak_level: None,
            error: None,
        });
    }

    let peak_bits = Arc::new(AtomicU32::new(0_f32.to_bits()));
    let error_message = Arc::new(parking_lot::Mutex::new(None::<String>));
    let error_target = error_message.clone();
    let error_callback = move |error: cpal::StreamError| {
        *error_target.lock() = Some(error.to_string());
    };
    let config = supported.config();
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            let peak_target = peak_bits.clone();
            device.build_input_stream(
                &config,
                move |samples: &[f32], _| update_peak(&peak_target, samples.iter().copied()),
                error_callback,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let peak_target = peak_bits.clone();
            device.build_input_stream(
                &config,
                move |samples: &[i16], _| {
                    update_peak(
                        &peak_target,
                        samples
                            .iter()
                            .map(|sample| *sample as f32 / i16::MAX as f32),
                    )
                },
                error_callback,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let peak_target = peak_bits.clone();
            device.build_input_stream(
                &config,
                move |samples: &[u16], _| {
                    update_peak(
                        &peak_target,
                        samples
                            .iter()
                            .map(|sample| *sample as f32 / u16::MAX as f32 * 2.0 - 1.0),
                    )
                },
                error_callback,
                None,
            )
        }
        format => return Err(format!("Unsupported microphone sample format: {format:?}")),
    }
    .map_err(|error| format!("Failed to create microphone sample stream: {error}"))?;

    stream
        .play()
        .map_err(|error| format!("Failed to start microphone sample stream: {error}"))?;
    std::thread::sleep(Duration::from_millis(sample_duration_ms));
    drop(stream);

    if let Some(error) = error_message.lock().take() {
        return Err(format!("Microphone sample failed: {error}"));
    }

    Ok(MicrophoneProbe {
        ready: true,
        device_name,
        sample_rate_hz: Some(sample_rate_hz),
        channels: Some(channels),
        peak_level: Some(f32::from_bits(peak_bits.load(Ordering::Relaxed))),
        error: None,
    })
}

fn update_peak(samples_peak: &AtomicU32, samples: impl Iterator<Item = f32>) {
    let peak = samples
        .map(f32::abs)
        .filter(|sample| sample.is_finite())
        .fold(0_f32, f32::max)
        .clamp(0.0, 1.0);
    let mut current = samples_peak.load(Ordering::Relaxed);
    while peak > f32::from_bits(current) {
        match samples_peak.compare_exchange_weak(
            current,
            peak.to_bits(),
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

#[cfg(target_os = "macos")]
fn total_memory_mb() -> Option<u64> {
    let output = std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(|bytes| bytes / 1_024 / 1_024)
}

#[cfg(target_os = "linux")]
fn total_memory_mb() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kilobytes = content
        .lines()
        .find(|line| line.starts_with("MemTotal:"))?
        .split_whitespace()
        .nth(1)?
        .parse::<u64>()
        .ok()?;
    Some(kilobytes / 1_024)
}

#[cfg(target_os = "windows")]
fn total_memory_mb() -> Option<u64> {
    use winapi::um::sysinfoapi::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        dwMemoryLoad: 0,
        ullTotalPhys: 0,
        ullAvailPhys: 0,
        ullTotalPageFile: 0,
        ullAvailPageFile: 0,
        ullTotalVirtual: 0,
        ullAvailVirtual: 0,
        ullAvailExtendedVirtual: 0,
    };
    let succeeded = unsafe { GlobalMemoryStatusEx(&mut status) };
    (succeeded != 0).then_some(status.ullTotalPhys / 1_024 / 1_024)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn total_memory_mb() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_measurement_ignores_invalid_samples_and_keeps_the_maximum() {
        let peak = AtomicU32::new(0_f32.to_bits());
        update_peak(&peak, [0.1, -0.8, f32::NAN, 2.0].into_iter());
        update_peak(&peak, [0.4].into_iter());
        assert_eq!(f32::from_bits(peak.load(Ordering::Relaxed)), 1.0);
    }

    #[test]
    fn hardware_probe_reports_at_least_one_cpu_and_an_architecture() {
        let probe = probe_hardware();
        assert!(probe.logical_cpu_count >= 1);
        assert!(!probe.architecture.is_empty());
    }
}
