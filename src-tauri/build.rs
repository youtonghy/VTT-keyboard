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

    // Compile native status overlay for macOS
    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("native/status_overlay_macos.m")
            .flag("-fobjc-arc")
            .compile("status_overlay");

        println!("cargo:rustc-link-lib=framework=Cocoa");
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
