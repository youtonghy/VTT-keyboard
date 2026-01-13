fn main() {
    tauri_build::build();

    // Compile native status overlay for Windows
    #[cfg(target_os = "windows")]
    {
        cc::Build::new()
            .cpp(true)
            .include("native")
            .file("native/status_overlay_win.cpp")
            .compile("status_overlay");

        println!("cargo:rustc-link-lib=user32");
        println!("cargo:rustc-link-lib=gdi32");
        println!("cargo:rustc-link-lib=gdiplus");
    }
}
