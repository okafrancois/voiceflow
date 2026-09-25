fn main() {
    println!("cargo::rustc-check-cfg=cfg(apple_speech)");
    let target = std::env::var("TARGET").unwrap_or_default();
    let mut attributes = tauri_build::Attributes::new();

    if target.contains("windows") {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        embed_windows_manifest();
    }

    if target.contains("apple-darwin") {
        println!("cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=12.0");

        // ggml-metal Objective-C code uses @available which emits calls to
        // ___isPlatformVersionAtLeast.  That symbol lives in libclang_rt.osx.a
        // which Rust's linker skips because it passes -nodefaultlibs.
        // Ask clang where its runtime dir is and link it explicitly.
        if let Ok(out) = std::process::Command::new("clang")
            .arg("--print-runtime-dir")
            .output()
        {
            let dir = String::from_utf8_lossy(&out.stdout);
            let dir = dir.trim();
            if !dir.is_empty() {
                println!("cargo:rustc-link-search={dir}");
                println!("cargo:rustc-link-lib=static=clang_rt.osx");
            }
        }

        build_apple_speech(&target);
    }

    tauri_build::try_build(attributes).expect("failed to build Tauri application resources");
}

const APPLE_SPEECH_PACKAGE: &str = "swift/AppleSpeech";
const APPLE_SPEECH_MIN_SDK_MAJOR: u32 = 26;

/// Compiles the Swift bridge to SpeechAnalyzer and links it statically.
///
/// The `apple_speech` cfg is only set when the bridge is linked; without it the
/// Rust side compiles a stub that reports the engine as unsupported. The
/// bridge needs a macOS 26 SDK because SpeechAnalyzer first ships there.
fn build_apple_speech(target: &str) {
    println!("cargo:rerun-if-changed={APPLE_SPEECH_PACKAGE}/Package.swift");
    println!("cargo:rerun-if-changed={APPLE_SPEECH_PACKAGE}/Sources");
    println!("cargo:rerun-if-env-changed=VOICEFLOW_DISABLE_APPLE_SPEECH");

    if std::env::var_os("VOICEFLOW_DISABLE_APPLE_SPEECH").is_some() {
        println!("cargo:warning=Apple speech engine disabled by VOICEFLOW_DISABLE_APPLE_SPEECH");
        return;
    }

    let sdk_major = command_output("xcrun", &["--sdk", "macosx", "--show-sdk-version"])
        .and_then(|version| version.split('.').next()?.parse::<u32>().ok());
    if sdk_major.is_none_or(|major| major < APPLE_SPEECH_MIN_SDK_MAJOR) {
        println!(
            "cargo:warning=Apple speech engine disabled: macOS SDK {APPLE_SPEECH_MIN_SDK_MAJOR} or later is required (found {sdk_major:?})"
        );
        return;
    }

    let arch = if target.starts_with("aarch64") {
        "arm64"
    } else {
        "x86_64"
    };
    let triple = format!("{arch}-apple-macosx12.0");
    let build_path = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set"))
        .join("apple-speech");
    let build_path = build_path.to_str().expect("OUT_DIR is valid UTF-8");
    let swift_args = [
        "build",
        "-c",
        "release",
        "--package-path",
        APPLE_SPEECH_PACKAGE,
        "--triple",
        &triple,
        "--build-path",
        build_path,
    ];

    // Captured rather than inherited: cargo parses build script stdout.
    let build = std::process::Command::new("swift")
        .args(swift_args)
        .output()
        .expect("failed to run `swift build` for the Apple speech bridge");
    assert!(
        build.status.success(),
        "`swift build` failed for the Apple speech bridge:\n{}\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let mut bin_path_args = swift_args.to_vec();
    bin_path_args.push("--show-bin-path");
    let bin_path = command_output("swift", &bin_path_args)
        .expect("failed to locate the Apple speech bridge build output");
    println!("cargo:rustc-link-search=native={bin_path}");
    println!("cargo:rustc-link-lib=static=AppleSpeech");

    let target_info = command_output("swift", &["-print-target-info", "-target", &triple])
        .expect("failed to read the Swift runtime paths");
    for path in swift_runtime_library_paths(&target_info) {
        println!("cargo:rustc-link-search=native={path}");
    }

    println!("cargo:rustc-cfg=apple_speech");
}

/// Extracts `paths.runtimeLibraryPaths` from `swift -print-target-info`.
fn swift_runtime_library_paths(target_info: &str) -> Vec<String> {
    let info: serde_json::Value =
        serde_json::from_str(target_info).expect("`swift -print-target-info` returns JSON");
    info["paths"]["runtimeLibraryPaths"]
        .as_array()
        .map(|paths| {
            paths
                .iter()
                .filter_map(|path| path.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn embed_windows_manifest() {
    let manifest = std::env::current_dir()
        .expect("failed to resolve the Tauri source directory")
        .join("windows-app-manifest.xml");

    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}
