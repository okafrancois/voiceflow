use super::integrity::ModelIntegrity;
use super::*;
use std::sync::atomic::Ordering;

const FILES: &[models::ModelFile] = &[
    models::ModelFile {
        name: "config.json",
        bytes: 3,
        sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    },
    models::ModelFile {
        name: "model.safetensors",
        bytes: 3,
        sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    },
];
const MODEL: models::Model = models::Model {
    id: "integrity-fixture",
    name: "Integrity fixture",
    size: "6 B",
    repository: "fixture",
    revision: "immutable",
    files: FILES,
};

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join(MODEL.id)).unwrap();
    for file in MODEL.files {
        std::fs::write(root.path().join(MODEL.id).join(file.name), b"abc").unwrap();
    }
    std::fs::write(
        root.path().join(MODEL.id).join(".model-ready"),
        models::signature(&MODEL),
    )
    .unwrap();
    let runtime = runtime::runtime_dir(root.path());
    std::fs::create_dir_all(&runtime).unwrap();
    std::fs::write(runtime.join(".runtime-ready"), b"healthy runtime sentinel").unwrap();
    root
}

fn assert_runtime_preserved(root: &Path) {
    assert_eq!(
        std::fs::read(runtime::runtime_dir(root).join(".runtime-ready")).unwrap(),
        b"healthy runtime sentinel"
    );
}

async fn corrupt_and_repair(file_name: &str, warm_cache: bool) {
    let root = fixture();
    let integrity = ModelIntegrity::default();
    if warm_cache {
        integrity
            .verify(
                root.path(),
                &MODEL,
                Arc::new(AtomicBool::new(false)),
                Duration::from_secs(2),
            )
            .await
            .unwrap();
    }
    let file = root.path().join(MODEL.id).join(file_name);
    std::fs::write(&file, b"abd").unwrap();
    assert!(models::files_complete_for(root.path(), &MODEL));
    let error = integrity
        .verify(
            root.path(),
            &MODEL,
            Arc::new(AtomicBool::new(false)),
            Duration::from_secs(2),
        )
        .await
        .unwrap_err();
    assert!(error.contains("checksum mismatch"), "{error}");
    assert!(!models::files_complete_for(root.path(), &MODEL));
    assert!(!file.exists());
    assert_runtime_preserved(root.path());
    let (url, _started, server) = super::download_tests::server(None).await;
    download_file(
        DownloadOptions::new(url, &file).with_minimum_bytes(3),
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    server.await.unwrap();
    std::fs::write(
        root.path().join(MODEL.id).join(".model-ready"),
        models::signature(&MODEL),
    )
    .unwrap();
    integrity
        .verify(
            root.path(),
            &MODEL,
            Arc::new(AtomicBool::new(false)),
            Duration::from_secs(2),
        )
        .await
        .unwrap();
    assert!(models::files_complete_for(root.path(), &MODEL));
    assert_runtime_preserved(root.path());
}

#[tokio::test]
async fn same_size_config_corruption_is_rejected_before_first_use_and_repairs() {
    corrupt_and_repair("config.json", false).await;
}

#[tokio::test]
async fn same_size_weights_corruption_is_rejected_before_first_use_and_repairs() {
    corrupt_and_repair("model.safetensors", false).await;
}

#[tokio::test]
async fn changed_same_size_files_cannot_reuse_cached_hashes() {
    corrupt_and_repair("config.json", true).await;
    corrupt_and_repair("model.safetensors", true).await;
}

#[tokio::test]
async fn cancelled_or_expired_integrity_checks_preserve_healthy_readiness() {
    let root = fixture();
    let integrity = ModelIntegrity::default();
    for (cancelled, timeout) in [(true, Duration::from_secs(2)), (false, Duration::ZERO)] {
        let result = integrity
            .verify(
                root.path(),
                &MODEL,
                Arc::new(AtomicBool::new(cancelled)),
                timeout,
            )
            .await;
        let error = result.unwrap_err();
        assert!(
            if cancelled {
                error == "cancelled"
            } else {
                error.contains("verification timed out")
            },
            "{error}"
        );
        assert!(models::files_complete_for(root.path(), &MODEL));
        assert_runtime_preserved(root.path());
    }
}

#[tokio::test]
async fn cancellation_and_deadline_interrupt_hashing_without_removing_checkpoint() {
    const LARGE: models::Model = models::Model {
        files: &[models::ModelFile {
            name: "model.safetensors",
            bytes: 128 * 1024 * 1024,
            sha256: FILES[0].sha256,
        }],
        ..MODEL
    };
    let root = fixture();
    std::fs::File::create(root.path().join(MODEL.id).join("model.safetensors"))
        .unwrap()
        .set_len(LARGE.files[0].bytes)
        .unwrap();
    std::fs::write(
        root.path().join(MODEL.id).join(".model-ready"),
        models::signature(&LARGE),
    )
    .unwrap();
    for cancel_request in [true, false] {
        let flag = Arc::new(AtomicBool::new(false));
        let cancel = flag.clone();
        let integrity = ModelIntegrity::default();
        let verification = integrity.verify(
            root.path(),
            &LARGE,
            flag,
            if cancel_request {
                Duration::from_secs(2)
            } else {
                Duration::from_millis(5)
            },
        );
        let cancellation = async move {
            if cancel_request {
                tokio::time::sleep(Duration::from_millis(5)).await;
                cancel.store(true, Ordering::Relaxed);
            }
        };
        let (result, ()) = tokio::time::timeout(Duration::from_secs(2), async {
            tokio::join!(verification, cancellation)
        })
        .await
        .unwrap();
        let error = result.unwrap_err();
        assert!(
            if cancel_request {
                error == "cancelled"
            } else {
                error.contains("verification timed out")
            },
            "{error}"
        );
        assert!(models::files_complete_for(root.path(), &LARGE));
        assert_runtime_preserved(root.path());
    }
}
