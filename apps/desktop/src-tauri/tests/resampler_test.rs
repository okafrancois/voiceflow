#[test]
fn test_resample_same_rate() {
    let input: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.01).sin()).collect();
    let result = voiceflow_lib::audio::resampler::resample(&input, 48000, 48000);

    assert!(result.is_ok());
    let output = result.unwrap();
    assert_eq!(
        output.len(),
        input.len(),
        "Same rate should return same length"
    );
}

#[test]
fn resample_preserves_duration_and_final_audio() {
    // Speech at the very end must survive the resampler's filter delay.
    let mut input = vec![0.0; 9_600];
    input[9_504..].fill(0.5);
    let output = voiceflow_lib::audio::resampler::resample_to_16khz(&input, 48_000)
        .expect("resampling should succeed");

    assert_eq!(output.len(), 3_200, "the complete 200 ms must be retained");
    let tail_energy: f32 = output[output.len() - 24..]
        .iter()
        .map(|sample| sample.abs())
        .sum::<f32>()
        / 24.0;
    assert!(
        tail_energy > 0.3,
        "the final speech fragment was lost: {tail_energy}"
    );
}

#[test]
fn resample_preserves_fragments_shorter_than_the_filter() {
    for (from_hz, to_hz) in [(48_000, 16_000), (44_100, 16_000), (16_000, 48_000)] {
        let input = vec![0.5; 96];
        let output = voiceflow_lib::audio::resampler::resample(&input, from_hz, to_hz)
            .expect("short fragments should resample");
        let expected = (input.len() as f64 * to_hz as f64 / from_hz as f64).round() as usize;
        assert_eq!(output.len(), expected);
        assert!(output.iter().any(|sample| sample.abs() > 0.3));
    }
}

#[test]
fn resample_rejects_invalid_sample_rates() {
    for (from_hz, to_hz) in [(0, 16_000), (48_000, 0), (0, 0)] {
        assert!(voiceflow_lib::audio::resampler::resample(&[0.5], from_hz, to_hz).is_err());
    }
}

#[test]
fn resample_does_not_shift_audio_from_the_start() {
    let mut input = vec![0.0; 9_600];
    input[..96].fill(0.5);
    let output = voiceflow_lib::audio::resampler::resample_to_16khz(&input, 48_000).unwrap();
    assert!(output[..24].iter().all(|sample| *sample > 0.3));
    assert!(output[64..].iter().all(|sample| sample.abs() < 0.01));
}

#[test]
fn test_resample_downsample_48k_to_16k() {
    let input: Vec<f32> = (0..48000).map(|i| (i as f32 * 0.01).sin()).collect();
    let result = voiceflow_lib::audio::resampler::resample(&input, 48000, 16000);

    assert!(result.is_ok());
    let output = result.unwrap();

    let expected_ratio = 16000.0 / 48000.0;
    let expected_len = (input.len() as f32 * expected_ratio) as usize;

    let len_diff = (output.len() as i32 - expected_len as i32).abs();
    assert!(
        len_diff < 100,
        "Output length should be approximately 1/3 of input"
    );
}

#[test]
fn test_resample_upsample_16k_to_48k() {
    let input: Vec<f32> = (0..16000).map(|i| (i as f32 * 0.01).sin()).collect();
    let result = voiceflow_lib::audio::resampler::resample(&input, 16000, 48000);

    assert!(result.is_ok());
    let output = result.unwrap();

    let expected_ratio = 48000.0 / 16000.0;
    let expected_len = (input.len() as f32 * expected_ratio) as usize;

    let len_diff = (output.len() as i32 - expected_len as i32).abs();
    assert!(
        len_diff < 500,
        "Output length should be approximately 3x of input"
    );
}

#[test]
fn test_resample_preserves_signal_shape() {
    let freq = 440.0;
    let sample_rate = 48000u32;
    let duration = 0.1;
    let num_samples = (sample_rate as f32 * duration) as usize;

    let input: Vec<f32> = (0..num_samples)
        .map(|i| (2.0 * std::f32::consts::PI * freq * (i as f32 / sample_rate as f32)).sin())
        .collect();

    let result = voiceflow_lib::audio::resampler::resample(&input, sample_rate, 16000);

    assert!(result.is_ok());
    let output = result.unwrap();

    assert!(!output.is_empty(), "Output should not be empty");

    let max_val = output.iter().fold(0.0f32, |max, &v| max.max(v.abs()));
    assert!(max_val > 0.1, "Signal should be preserved after resampling");
}

#[test]
fn test_resample_to_16khz() {
    let input: Vec<f32> = (0..32000).map(|i| (i as f32 * 0.01).sin()).collect();
    let result = voiceflow_lib::audio::resampler::resample_to_16khz(&input, 32000);

    assert!(result.is_ok());
    let output = result.unwrap();

    assert!(!output.is_empty());
    let expected_ratio = 16000.0 / 32000.0;
    let expected_len = (input.len() as f32 * expected_ratio) as usize;
    let len_diff = (output.len() as i32 - expected_len as i32).abs();
    assert!(len_diff < 100);
}

#[test]
fn test_resample_empty_input() {
    let input: Vec<f32> = vec![];
    let result = voiceflow_lib::audio::resampler::resample(&input, 48000, 16000);

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.is_empty(), "Empty input should return empty output");
}
