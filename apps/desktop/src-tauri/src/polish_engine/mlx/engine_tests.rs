use super::*;
use std::sync::Arc;

const READY: &str =
    r#"{"protocol":1,"kind":"ready","mlx_lm_version":"0.32.2","mlx_version":"0.32.4"}"#;

async fn fake_worker(handshake: &str, body: &str) -> (tempfile::TempDir, Result<Worker, String>) {
    let root = tempfile::tempdir().unwrap();
    let models = root.path().join("models");
    std::fs::create_dir_all(runtime::runtime_dir(&models)).unwrap();
    let script = root.path().join("fake.py");
    std::fs::write(
        &script,
        format!("import json, os, sys, time\nprint({handshake:?}, flush=True)\n{body}\n"),
    )
    .unwrap();
    let result = Worker::start(
        Path::new("/usr/bin/python3"),
        &script,
        &models,
        WORKER_START_TIMEOUT,
    )
    .await;
    (root, result)
}

async fn assert_stopped(process_id: u32) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let status = tokio::process::Command::new("/bin/kill")
                .args(["-0", &process_id.to_string()])
                .stderr(std::process::Stdio::null())
                .status()
                .await
                .unwrap();
            if !status.success() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("owned worker should be reaped promptly");
}

async fn waiting_engine() -> (tempfile::TempDir, Arc<MlxPolishEngine>, u32) {
    let (root, worker) = fake_worker(READY, "for line in sys.stdin:\n    time.sleep(60)").await;
    let worker = worker.unwrap();
    let process_id = worker.process.child.id().unwrap();
    let engine = Arc::new(MlxPolishEngine::new(root.path().join("models")));
    *engine.worker.lock().await = Some(worker);
    (root, engine, process_id)
}

async fn wait_for_request(engine: &MlxPolishEngine) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if engine.selected.lock().unwrap().is_some() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn wrong_readiness_version_and_malformed_protocol_are_rejected() {
    for handshake in [
        "not json".to_string(),
        READY.replace("0.32.2", "0.0.0"),
        READY.replace("\"protocol\":1", "\"protocol\":2"),
    ] {
        let (_root, result) = fake_worker(&handshake, "time.sleep(60)").await;
        assert!(result.is_err());
    }
}

#[tokio::test]
async fn startup_deadline_is_enforced_and_owned_process_is_reaped() {
    let root = tempfile::tempdir().unwrap();
    let models = root.path().join("models");
    std::fs::create_dir_all(runtime::runtime_dir(&models)).unwrap();
    let script = root.path().join("slow.py");
    let marker = root.path().join("pid");
    std::fs::write(
        &script,
        format!(
            "import os, time\nopen({:?}, 'w').write(str(os.getpid()))\ntime.sleep(60)\n",
            marker
        ),
    )
    .unwrap();
    let result = Worker::start(
        Path::new("/usr/bin/python3"),
        &script,
        &models,
        Duration::from_millis(300),
    )
    .await;
    assert!(result.is_err());
    let process_id = std::fs::read_to_string(marker).unwrap().parse().unwrap();
    assert_stopped(process_id).await;
}

#[tokio::test]
async fn mismatched_and_missing_success_responses_fail_closed() {
    for response in [r#"{"id":99,"ok":true}"#, r#"{"id":1}"#, "invalid"] {
        let body = format!("sys.stdin.readline()\nprint({response:?}, flush=True)\ntime.sleep(60)");
        let (_root, worker) = fake_worker(READY, &body).await;
        let mut worker = worker.unwrap();
        assert!(worker
            .request(json!({"action":"load", "model":"mlx-qwen3-0.6b"}))
            .await
            .is_err());
    }
}

#[tokio::test]
async fn timeout_kills_the_owned_worker_without_leaking_a_response() {
    let (_root, engine, process_id) = waiting_engine().await;
    let result = engine
        .execute_prepared(
            json!({"action":"load", "model":"mlx-qwen3-0.6b"}),
            Duration::from_millis(100),
            std::future::ready(Ok(())),
        )
        .await;
    assert!(result.unwrap_err().contains("timed out"));
    assert!(!engine.is_loaded("mlx-qwen3-0.6b"));
    assert_stopped(process_id).await;
}

#[tokio::test]
async fn preparation_does_not_consume_the_request_deadline() {
    let (root, worker) = fake_worker(READY, "for line in sys.stdin:\n    request = json.loads(line)\n    print(json.dumps({'id': request['id'], 'ok': True}), flush=True)").await;
    let engine = MlxPolishEngine::new(root.path().join("models"));
    *engine.worker.lock().await = Some(worker.unwrap());
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        engine.execute_prepared(
            json!({"action":"load", "model":"mlx-qwen3-0.6b"}),
            Duration::from_millis(100),
            async {
                tokio::time::sleep(Duration::from_millis(250)).await;
                Ok(())
            },
        ),
    )
    .await
    .unwrap();
    assert!(result.is_ok(), "{result:?}");
    assert!(engine.is_loaded("mlx-qwen3-0.6b"));
    engine.stop();
}

#[tokio::test]
async fn generation_deadline_starts_after_preparation_and_stops_the_worker() {
    let (_root, engine, process_id) = waiting_engine().await;
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        engine.execute_prepared(
            json!({"action":"load", "model":"mlx-qwen3-0.6b"}),
            Duration::from_millis(100),
            async {
                tokio::time::sleep(Duration::from_millis(250)).await;
                Ok(())
            },
        ),
    )
    .await
    .unwrap();
    assert!(result.unwrap_err().contains("timed out"));
    assert!(started.elapsed() >= Duration::from_millis(350));
    assert!(!engine.is_loaded("mlx-qwen3-0.6b"));
    assert_stopped(process_id).await;
}

#[tokio::test]
async fn dropping_an_active_request_kills_its_worker() {
    let (_root, engine, process_id) = waiting_engine().await;
    let cloned = engine.clone();
    let request = tokio::spawn(async move {
        cloned
            .execute_prepared(
                json!({"action":"load", "model":"mlx-qwen3-0.6b"}),
                Duration::from_secs(60),
                std::future::ready(Ok(())),
            )
            .await
    });
    wait_for_request(&engine).await;
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    assert_stopped(process_id).await;
    assert!(!engine.is_loaded("mlx-qwen3-0.6b"));
}

#[tokio::test]
async fn unload_cancels_active_clones_and_later_requests_can_restart() {
    let (_root, engine, process_id) = waiting_engine().await;
    let cloned = engine.clone();
    let request = tokio::spawn(async move {
        cloned
            .execute_prepared(
                json!({"action":"load", "model":"mlx-qwen3-0.6b"}),
                Duration::from_secs(60),
                std::future::ready(Ok(())),
            )
            .await
    });
    wait_for_request(&engine).await;
    engine.stop_model("mlx-qwen3-0.6b");
    assert!(request.await.unwrap().is_err());
    assert_stopped(process_id).await;
    let (_fresh_root, worker) = fake_worker(READY, "for line in sys.stdin:\n    request = json.loads(line)\n    print(json.dumps({'id': request['id'], 'ok': True}), flush=True)").await;
    *engine.worker.lock().await = Some(worker.unwrap());
    engine
        .execute_prepared(
            json!({"action":"load", "model":"mlx-qwen3-0.6b"}),
            Duration::from_secs(2),
            std::future::ready(Ok(())),
        )
        .await
        .unwrap();
    assert!(engine.is_loaded("mlx-qwen3-0.6b"));
    engine.stop();
}

#[tokio::test]
async fn idle_cleanup_does_not_interrupt_an_active_request() {
    let (_root, engine, process_id) = waiting_engine().await;
    let cloned = engine.clone();
    let request = tokio::spawn(async move {
        cloned
            .execute_prepared(
                json!({"action":"load", "model":"mlx-qwen3-0.6b"}),
                Duration::from_secs(60),
                std::future::ready(Ok(())),
            )
            .await
    });
    wait_for_request(&engine).await;
    assert!(!engine.stop_if_idle(Duration::ZERO));
    engine.stop();
    assert!(request.await.unwrap().is_err());
    assert_stopped(process_id).await;
}

#[tokio::test]
async fn first_load_integrity_rejection_never_sends_corrupt_checkpoint_to_worker() {
    let (root, worker) = fake_worker(READY, "for line in sys.stdin:\n    open('received', 'w').write(line)\n    request = json.loads(line)\n    print(json.dumps({'id': request['id'], 'ok': True}), flush=True)").await;
    let worker = worker.unwrap();
    let process_id = worker.process.child.id().unwrap();
    let models_dir = root.path().join("models");
    let model = &models::MODELS[0];
    let directory = models_dir.join(model.id);
    std::fs::create_dir_all(&directory).unwrap();
    for file in model.files {
        std::fs::File::create(directory.join(file.name))
            .unwrap()
            .set_len(file.bytes)
            .unwrap();
    }
    std::fs::write(directory.join(".model-ready"), models::signature(model)).unwrap();
    let marker = runtime::runtime_dir(&models_dir).join(".runtime-ready");
    std::fs::write(&marker, b"healthy runtime sentinel").unwrap();
    let engine = MlxPolishEngine::new(models_dir.clone());
    *engine.worker.lock().await = Some(worker);
    let error = engine
        .execute(
            json!({"action":"load", "model":model.id}),
            Duration::from_secs(2),
        )
        .await
        .unwrap_err();
    assert!(error.contains("checksum mismatch"), "{error}");
    assert!(!models::files_complete(&models_dir, model.id));
    assert!(!runtime::runtime_dir(&models_dir)
        .parent()
        .unwrap()
        .join("received")
        .exists());
    assert_eq!(std::fs::read(marker).unwrap(), b"healthy runtime sentinel");
    assert_stopped(process_id).await;
}
