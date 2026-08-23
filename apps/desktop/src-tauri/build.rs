fn main() {
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
    }

    tauri_build::try_build(attributes).expect("failed to build Tauri application resources");
}

fn embed_windows_manifest() {
    let manifest = std::env::current_dir()
        .expect("failed to resolve the Tauri source directory")
        .join("windows-app-manifest.xml");

    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}
