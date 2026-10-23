use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use tracing::debug;

pub fn resample(input: &[f32], from_hz: u32, to_hz: u32) -> Result<Vec<f32>, String> {
    if from_hz == 0 || to_hz == 0 {
        return Err("Sample rates must be greater than zero".to_string());
    }
    if input.is_empty() {
        return Ok(Vec::new());
    }
    if from_hz == to_hz {
        return Ok(input.to_vec());
    }

    debug!(from_hz, to_hz, "resampling_started");

    let ratio = to_hz as f64 / from_hz as f64;

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    // A finite buffer must supply the sinc filter's trailing lookahead. Rubato
    // 0.15's SincFixedIn starts at the first input frame: output_delay describes
    // lookahead, not leading silent frames to discard. Trim only the padding.
    let mut padded_input = input.to_vec();
    padded_input.resize(input.len() + 2 * params.sinc_len, 0.0);
    let expected_frames = (input.len() as f64 * ratio).round() as usize;
    let mut resampler = SincFixedIn::<f32>::new(ratio, 2.0, params, padded_input.len(), 1)
        .map_err(|e| format!("Failed to create resampler: {:?}", e))?;

    let output_frames = resampler.output_frames_max();
    let mut output = vec![0.0f32; output_frames];

    let (_, written) = resampler
        .process_into_buffer(&[padded_input], &mut [&mut output], None)
        .map_err(|e| format!("Resampling failed: {:?}", e))?;

    if written < expected_frames {
        return Err(format!(
            "Incomplete resampling: produced {written} frames, expected at least {expected_frames}"
        ));
    }
    output.truncate(expected_frames);

    debug!(
        input_samples = input.len(),
        output_samples = output.len(),
        "resampling_completed"
    );

    Ok(output)
}

pub fn resample_to_16khz(input: &[f32], input_sample_rate: u32) -> Result<Vec<f32>, String> {
    resample(input, input_sample_rate, 16_000)
}
