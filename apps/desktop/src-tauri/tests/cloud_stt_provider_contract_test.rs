use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::protocol::Message;
use voiceflow_lib::commands::settings::CloudSttConfig;
use voiceflow_lib::stt_engine::cloud::StreamingSttClient;
use voiceflow_lib::stt_engine::traits::{PartialResult, SttContext};

fn aliyun_config(base_url: String) -> CloudSttConfig {
    CloudSttConfig {
        enabled: true,
        provider_type: "aliyun-stream".to_string(),
        api_key: "aliyun-test-key".to_string(),
        app_id: String::new(),
        base_url,
        model: "qwen3-asr-flash-realtime".to_string(),
        language: "fr".to_string(),
    }
}

fn elevenlabs_config(base_url: String) -> CloudSttConfig {
    CloudSttConfig {
        enabled: true,
        provider_type: "elevenlabs".to_string(),
        api_key: "elevenlabs-test-key".to_string(),
        app_id: String::new(),
        base_url,
        model: "scribe_v2_realtime".to_string(),
        language: "fr".to_string(),
    }
}

#[tokio::test]
async fn aliyun_realtime_contract_covers_handshake_audio_and_final_result() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let base_url = format!("ws://{address}/api-ws/v1/realtime");

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut websocket = accept_hdr_async(stream, |request: &Request, response: Response| {
            assert_eq!(request.uri().path(), "/api-ws/v1/realtime");
            assert_eq!(
                request.uri().query(),
                Some("model=qwen3-asr-flash-realtime")
            );
            assert_eq!(
                request.headers().get("authorization").unwrap(),
                "Bearer aliyun-test-key"
            );
            assert_eq!(request.headers().get("openai-beta").unwrap(), "realtime=v1");
            Ok(response)
        })
        .await
        .unwrap();

        let session_update = websocket.next().await.unwrap().unwrap();
        let Message::Text(session_update) = session_update else {
            panic!("Aliyun session update must be a text frame");
        };
        let session_update: serde_json::Value = serde_json::from_str(&session_update).unwrap();
        assert_eq!(session_update["type"], "session.update");
        assert_eq!(session_update["session"]["input_audio_format"], "pcm");
        assert_eq!(session_update["session"]["sample_rate"], 16000);
        assert_eq!(
            session_update["session"]["input_audio_transcription"]["language"],
            "fr"
        );
        assert!(session_update["session"]["turn_detection"].is_null());

        websocket
            .send(Message::Text(
                serde_json::json!({"type": "session.updated"})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();

        let mut received_audio = false;
        let mut received_commit = false;
        while let Some(message) = websocket.next().await {
            let Message::Text(message) = message.unwrap() else {
                continue;
            };
            let message: serde_json::Value = serde_json::from_str(&message).unwrap();
            match message["type"].as_str().unwrap() {
                "input_audio_buffer.append" => {
                    received_audio = true;
                    assert!(!message["audio"].as_str().unwrap().is_empty());
                }
                "input_audio_buffer.commit" => received_commit = true,
                "session.finish" => {
                    assert!(received_audio, "audio must precede session.finish");
                    assert!(received_commit, "commit must precede session.finish");
                    websocket
                        .send(Message::Text(
                            serde_json::json!({
                                "type": "conversation.item.input_audio_transcription.completed",
                                "transcript": "Aliyun contract result"
                            })
                            .to_string()
                            .into(),
                        ))
                        .await
                        .unwrap();
                    websocket
                        .send(Message::Text(
                            serde_json::json!({"type": "session.finished"})
                                .to_string()
                                .into(),
                        ))
                        .await
                        .unwrap();
                    return;
                }
                other => panic!("unexpected Aliyun message: {other}"),
            }
        }

        panic!("Aliyun client closed before session.finish");
    });

    let callbacks = Arc::new(Mutex::new(Vec::<PartialResult>::new()));
    let callback_results = Arc::clone(&callbacks);
    let mut client = StreamingSttClient::new(
        aliyun_config(base_url),
        Some("fr-FR"),
        SttContext::default(),
    )
    .unwrap();
    client.set_partial_callback(Arc::new(move |result| {
        callback_results.lock().unwrap().push(result);
    }));

    client.connect().await.unwrap();
    let audio_sender = client.get_audio_sender().await.unwrap();
    audio_sender.send(vec![1, -2, 3, -4]).await.unwrap();
    drop(audio_sender);

    let result = tokio::time::timeout(std::time::Duration::from_secs(3), client.finish())
        .await
        .expect("Aliyun contract test timed out")
        .unwrap();
    assert_eq!(result, "Aliyun contract result");
    server.await.unwrap();

    let callbacks = callbacks.lock().unwrap();
    assert!(callbacks
        .iter()
        .any(|result| result.text == "Aliyun contract result" && result.is_definite));
    assert!(callbacks.iter().any(|result| result.is_final));
}

#[tokio::test]
async fn elevenlabs_realtime_contract_covers_handshake_context_and_final_result() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let base_url = format!("ws://{address}/v1/speech-to-text/realtime");

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut websocket = accept_hdr_async(stream, |request: &Request, response: Response| {
            assert_eq!(request.uri().path(), "/v1/speech-to-text/realtime");
            let query = request.uri().query().unwrap();
            assert!(query.contains("audio_format=pcm_16000"));
            assert!(query.contains("language_code=fr"));
            assert!(query.contains("model_id=scribe_v2_realtime"));
            assert_eq!(
                request.headers().get("xi-api-key").unwrap(),
                "elevenlabs-test-key"
            );
            Ok(response)
        })
        .await
        .unwrap();

        let first_audio = websocket.next().await.unwrap().unwrap();
        let Message::Text(first_audio) = first_audio else {
            panic!("ElevenLabs audio must be a text frame");
        };
        let first_audio: serde_json::Value = serde_json::from_str(&first_audio).unwrap();
        assert_eq!(first_audio["message_type"], "input_audio_chunk");
        assert_eq!(first_audio["sample_rate"], 16000);
        assert_eq!(first_audio["commit"], false);
        assert!(!first_audio["audio_base_64"].as_str().unwrap().is_empty());
        assert_eq!(
            first_audio["previous_text"],
            "Terminology: Voice Flow. Domain: software (editor). Keep command names"
        );

        let commit = websocket.next().await.unwrap().unwrap();
        let Message::Text(commit) = commit else {
            panic!("ElevenLabs commit must be a text frame");
        };
        let commit: serde_json::Value = serde_json::from_str(&commit).unwrap();
        assert_eq!(commit["message_type"], "input_audio_chunk");
        assert_eq!(commit["commit"], true);
        assert_eq!(commit["audio_base_64"], "");

        websocket
            .send(Message::Text(
                serde_json::json!({
                    "message_type": "partial_transcript",
                    "text": "ElevenLabs contract"
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
        websocket
            .send(Message::Text(
                serde_json::json!({
                    "message_type": "committed_transcript",
                    "text": "ElevenLabs contract result"
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
    });

    let context = SttContext {
        glossary: Some("Voice Flow".to_string()),
        domain: Some("software".to_string()),
        subdomain: Some("editor".to_string()),
        initial_prompt: Some("Keep command names".to_string()),
    };
    let callbacks = Arc::new(Mutex::new(Vec::<PartialResult>::new()));
    let callback_results = Arc::clone(&callbacks);
    let mut client =
        StreamingSttClient::new(elevenlabs_config(base_url), Some("fr-FR"), context).unwrap();
    client.set_partial_callback(Arc::new(move |result| {
        callback_results.lock().unwrap().push(result);
    }));

    client.connect().await.unwrap();
    let audio_sender = client.get_audio_sender().await.unwrap();
    audio_sender.send(vec![7, -8, 9, -10]).await.unwrap();
    drop(audio_sender);

    let result = tokio::time::timeout(std::time::Duration::from_secs(3), client.finish())
        .await
        .expect("ElevenLabs contract test timed out")
        .unwrap();
    assert_eq!(result, "ElevenLabs contract result");
    server.await.unwrap();

    let callbacks = callbacks.lock().unwrap();
    assert!(callbacks.iter().any(|result| {
        result.text == "ElevenLabs contract" && !result.is_definite && !result.is_final
    }));
    assert!(callbacks.iter().any(|result| {
        result.text == "ElevenLabs contract result" && result.is_definite && !result.is_final
    }));
}
