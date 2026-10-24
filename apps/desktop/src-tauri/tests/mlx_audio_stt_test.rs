use std::io::Cursor;
use std::sync::{Arc, Mutex};
use voiceflow_lib::commands::settings::CloudSttConfig;
use voiceflow_lib::stt_engine::cloud::{StreamingConsumer, StreamingSttClient};
use voiceflow_lib::stt_engine::traits::{PartialResult, RecordingConsumer, SttContext};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MODEL: &str = "mlx-community/whisper-large-v3-turbo-asr-fp16";

fn config(base_url: String) -> CloudSttConfig {
    CloudSttConfig {
        enabled: true,
        provider_type: "mlx-audio".to_string(),
        base_url,
        ..Default::default()
    }
}

async fn server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "list", "data": []
        })))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn mlx_audio_preserves_all_chunks_and_uses_local_multipart_contract() {
    let server = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "text": " Bonjour, voici la fin de ma phrase. "
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = StreamingSttClient::new(
        config(format!("{}/v1/", server.uri())),
        Some("fr-FR"),
        SttContext::default(),
    )
    .expect("MLX Audio should be a supported provider without credentials");
    assert_eq!(client.provider_name(), "MLX Audio");
    let partials = Arc::new(Mutex::new(Vec::<PartialResult>::new()));
    let observed = partials.clone();
    let consumer = StreamingConsumer::new(
        client,
        Arc::new(move |result| observed.lock().unwrap().push(result)),
    )
    .await
    .unwrap();
    let first = vec![i16::MIN, -1, 0, 1, i16::MAX];
    let tail = vec![125, -273, 44];
    consumer.send_chunk(first.clone()).await.unwrap();
    consumer.send_chunk(tail.clone()).await.unwrap();
    let text = consumer.finish().await.unwrap();
    assert_eq!(text, "Bonjour, voici la fin de ma phrase.");

    let requests = server.received_requests().await.unwrap();
    let upload = requests
        .iter()
        .find(|request| request.method == "POST")
        .unwrap();
    assert!(upload.headers.get("authorization").is_none());
    assert!(upload.headers["content-type"]
        .to_str()
        .unwrap()
        .starts_with("multipart/form-data;"));
    let body = String::from_utf8_lossy(&upload.body);
    assert!(body.contains(&format!("name=\"model\"\r\n\r\n{MODEL}\r\n")));
    assert!(body.contains("name=\"language\"\r\n\r\nfr\r\n"));
    assert!(body.contains("name=\"response_format\"\r\n\r\njson\r\n"));
    assert!(body.contains("filename=\"recording.wav\""));
    let start = upload
        .body
        .windows(4)
        .position(|bytes| bytes == b"RIFF")
        .unwrap();
    let reader = hound::WavReader::new(Cursor::new(&upload.body[start..])).unwrap();
    assert_eq!(reader.spec().sample_rate, 16_000);
    assert_eq!(reader.spec().channels, 1);
    assert_eq!(reader.spec().bits_per_sample, 16);
    let samples = reader
        .into_samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(samples, [first, tail].concat());
    assert!(partials
        .lock()
        .unwrap()
        .iter()
        .all(|result| result.is_final));
}

#[tokio::test]
async fn mlx_audio_empty_session_does_not_submit_audio() {
    let server = server().await;
    let mut client = StreamingSttClient::new(
        config(format!("{}/v1", server.uri())),
        Some("auto"),
        SttContext::default(),
    )
    .unwrap();
    client.connect().await.unwrap();
    assert_eq!(client.finish().await.unwrap(), "");
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn mlx_audio_rejects_the_apps_text_only_llama_server_before_recording() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "list", "data": [{"id":"qwen3-4b", "owned_by":"llamacpp"}]
        })))
        .mount(&server)
        .await;
    let mut client = StreamingSttClient::new(
        config(format!("{}/v1", server.uri())),
        None,
        SttContext::default(),
    )
    .unwrap();
    let error = client.connect().await.unwrap_err();
    assert!(error.contains("llama-server"), "{error}");
    assert!(client.get_audio_sender().await.is_none());
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn mlx_audio_close_discards_audio_and_closes_the_sender() {
    let server = server().await;
    let mut client = StreamingSttClient::new(
        config(format!("{}/v1", server.uri())),
        None,
        SttContext::default(),
    )
    .unwrap();
    client.connect().await.unwrap();
    let sender = client.get_audio_sender().await.unwrap();
    sender.send(vec![1; 16_000]).await.unwrap();
    client.close().await;
    assert!(sender.send(vec![2]).await.is_err());
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn mlx_audio_reports_server_failures_without_silent_fallback() {
    let server = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"detail":"Model could not load"})),
        )
        .mount(&server)
        .await;
    let client = StreamingSttClient::new(
        config(format!("{}/v1", server.uri())),
        None,
        SttContext::default(),
    )
    .unwrap();
    let consumer = StreamingConsumer::new(client, Arc::new(|_| {}))
        .await
        .unwrap();
    consumer.send_chunk(vec![1; 16_000]).await.unwrap();
    let error = consumer.finish().await.unwrap_err();
    assert!(error.contains("500"), "{error}");
    assert!(error.contains("Model could not load"), "{error}");
}

#[tokio::test]
async fn mlx_audio_rejects_malformed_transcription_results() {
    let server = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"wrong":"shape"})),
        )
        .mount(&server)
        .await;
    let client = StreamingSttClient::new(
        config(format!("{}/v1", server.uri())),
        None,
        SttContext::default(),
    )
    .unwrap();
    let consumer = StreamingConsumer::new(client, Arc::new(|_| {}))
        .await
        .unwrap();
    consumer.send_chunk(vec![1; 16_000]).await.unwrap();
    assert!(consumer.finish().await.unwrap_err().contains("response"));
}

#[test]
fn mlx_audio_rejects_non_local_and_non_http_endpoints() {
    for endpoint in [
        "https://example.com/v1",
        "ws://127.0.0.1:8000/v1",
        "http://localhost.example.com/v1",
    ] {
        let result =
            StreamingSttClient::new(config(endpoint.to_string()), None, SttContext::default());
        assert!(result.is_err(), "unexpectedly accepted {endpoint}");
    }
}

#[tokio::test]
#[ignore = "requires the local MLX Audio server and VOICEFLOW_MLX_TEST_WAV"]
async fn mlx_audio_transcribes_a_real_local_french_recording() {
    let path = std::env::var("VOICEFLOW_MLX_TEST_WAV").expect("set VOICEFLOW_MLX_TEST_WAV");
    let decoded = voiceflow_lib::services::transcription_workbench::decode_media_to_mono_16k(
        std::path::Path::new(&path),
    )
    .unwrap();
    let client = StreamingSttClient::new(
        config("http://127.0.0.1:8000/v1".to_string()),
        Some("fr"),
        SttContext::default(),
    )
    .unwrap();
    let consumer = StreamingConsumer::new(client, Arc::new(|_| {}))
        .await
        .unwrap();
    let samples: Vec<i16> = decoded
        .samples
        .iter()
        .map(|sample| (sample * 32768.0).clamp(-32768.0, 32767.0) as i16)
        .collect();
    for chunk in samples.chunks(1600) {
        consumer.send_chunk(chunk.to_vec()).await.unwrap();
    }
    let text = consumer.finish().await.unwrap();
    println!("MLX Audio transcript: {text}");
    assert!(text.to_lowercase().contains("voix française"), "{text}");
    assert!(text.to_lowercase().contains("mac"), "{text}");
}
