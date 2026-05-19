use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Inject build date as environment variable
    let build_date = chrono::Local::now().format("%Y-%m-%d").to_string();
    println!("cargo:rustc-env=BUILD_DATE={}", build_date);

    tauri_build::build();

    // Compile native status overlay for Windows
    #[cfg(target_os = "windows")]
    {
        let mut build = cc::Build::new();
        build
            .cpp(true)
            // Keep the overlay CRT aligned with Sherpa static prebuilt libs on Windows.
            .static_crt(true)
            .include("native")
            .file("native/status_overlay_win.cpp");
        build.compile("status_overlay");

        println!("cargo:rustc-link-lib=user32");
        println!("cargo:rustc-link-lib=gdi32");
        println!("cargo:rustc-link-lib=gdiplus");
    }

    // Compile native status overlay for macOS
    #[cfg(target_os = "macos")]
    {
        compile_macos_swift_overlay();

        println!("cargo:rustc-link-lib=framework=ApplicationServices");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }

    // Compile native status overlay for Linux
    #[cfg(target_os = "linux")]
    {
        // Use pkg-config to find GTK3
        let gtk = pkg_config::Config::new()
            .atleast_version("3.0")
            .probe("gtk+-3.0")
            .expect("GTK3 is required. Please install libgtk-3-dev");

        let mut build = cc::Build::new();
        build
            .file("native/status_overlay_linux.c")
            .include("native");

        for path in &gtk.include_paths {
            build.include(path);
        }

        build.compile("status_overlay");

        // Link GTK libraries
        for lib in &gtk.libs {
            println!("cargo:rustc-link-lib={}", lib);
        }
    }
}

#[cfg(target_os = "macos")]
fn compile_macos_swift_overlay() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is not set"));
    let sdk_path = command_output(
        Command::new("xcrun").args(["--sdk", "macosx", "--show-sdk-path"]),
        "failed to locate the macOS SDK with xcrun",
    );
    let swiftc_path = command_output(
        Command::new("xcrun").args(["--find", "swiftc"]),
        "failed to locate swiftc with xcrun",
    );
    let target = env::var("TARGET").expect("TARGET is not set");
    let deployment_target = env::var("MACOSX_DEPLOYMENT_TARGET")
        .unwrap_or_else(|_| default_macos_deployment_target(&target).to_string());
    let swift_target = swift_macos_target(&target, &deployment_target);
    let library_path = out_dir.join("libstatus_overlay.a");
    let module_cache_path = out_dir.join("swift-module-cache");

    println!("cargo:rerun-if-changed=native/status_overlay_macos.swift");
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!(
        "cargo:rustc-link-search=native={}",
        swift_runtime_library_dir(swiftc_path.trim()).display()
    );
    println!("cargo:rustc-link-lib=static=status_overlay");

    let status = Command::new("xcrun")
        .arg("swiftc")
        .args([
            "-parse-as-library",
            "-emit-library",
            "-static",
            "-O",
            "-module-name",
            "StatusOverlayMacOS",
            "-target",
            &swift_target,
            "-sdk",
            sdk_path.trim(),
            "-module-cache-path",
            module_cache_path
                .to_str()
                .expect("module cache path was not valid UTF-8"),
            "-Xcc",
            &format!(
                "-fmodules-cache-path={}",
                module_cache_path
                    .to_str()
                    .expect("module cache path was not valid UTF-8")
            ),
            "-o",
        ])
        .arg(&library_path)
        .arg("native/status_overlay_macos.swift")
        .status()
        .expect("failed to invoke swiftc for native macOS status overlay");

    if !status.success() {
        panic!("swiftc failed while compiling native macOS status overlay");
    }
}

#[cfg(target_os = "macos")]
fn default_macos_deployment_target(target: &str) -> &'static str {
    if target.starts_with("aarch64-") {
        "11.0"
    } else {
        "10.15"
    }
}

#[cfg(target_os = "macos")]
fn swift_macos_target(rust_target: &str, deployment_target: &str) -> String {
    let arch = if rust_target.starts_with("aarch64-") {
        "arm64"
    } else if rust_target.starts_with("x86_64-") {
        "x86_64"
    } else {
        panic!("unsupported macOS target for Swift overlay: {rust_target}");
    };

    format!("{arch}-apple-macosx{deployment_target}")
}

#[cfg(target_os = "macos")]
fn swift_runtime_library_dir(swiftc_path: &str) -> PathBuf {
    PathBuf::from(swiftc_path)
        .parent()
        .and_then(|path| path.parent())
        .expect("swiftc path did not contain a toolchain usr directory")
        .join("lib")
        .join("swift")
        .join("macosx")
}

#[cfg(target_os = "macos")]
fn command_output(command: &mut Command, error_message: &str) -> String {
    let output = command.output().expect(error_message);
    if !output.status.success() {
        panic!("{error_message}");
    }
    String::from_utf8(output.stdout).expect("command output was not valid UTF-8")
}
