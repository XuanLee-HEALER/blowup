fn main() {
    tauri_build::build();
    link_libmpv();
    compile_native();
}

fn compile_native() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-changed=native/metal_layer.m");
        println!("cargo:rerun-if-changed=native/metal_layer.h");
        cc::Build::new()
            .file("native/metal_layer.m")
            .flag("-fobjc-arc")
            .flag("-DGL_SILENCE_DEPRECATION")
            .compile("native_metal");

        println!("cargo:rustc-link-lib=framework=OpenGL");
        println!("cargo:rustc-link-lib=framework=QuartzCore");
        println!("cargo:rustc-link-lib=framework=AppKit");
    }

    #[cfg(target_os = "windows")]
    {
        println!("cargo:rerun-if-changed=native/win_gl_layer.c");
        println!("cargo:rerun-if-changed=native/win_gl_layer.h");
        cc::Build::new()
            .file("native/win_gl_layer.c")
            .compile("native_win_gl");

        println!("cargo:rustc-link-lib=opengl32");
        println!("cargo:rustc-link-lib=user32");
        println!("cargo:rustc-link-lib=gdi32");
        println!("cargo:rustc-link-lib=comctl32");
    }
}

fn link_libmpv() {
    #[cfg(target_os = "macos")]
    {
        if std::path::Path::new("/opt/homebrew/lib").exists() {
            println!("cargo:rustc-link-search=native=/opt/homebrew/lib");
        }
        if std::path::Path::new("/usr/local/lib").exists() {
            println!("cargo:rustc-link-search=native=/usr/local/lib");
        }
        println!("cargo:rustc-link-lib=dylib=mpv");
    }

    #[cfg(target_os = "linux")]
    {
        let lib = pkg_config::probe_library("mpv")
            .expect("libmpv not found. Install with: sudo apt install libmpv-dev");
        for path in lib.link_paths {
            println!("cargo:rustc-link-search=native={}", path.display());
        }
    }

    #[cfg(target_os = "windows")]
    {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let lib_dir = std::path::Path::new(&manifest_dir).join("lib");
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-link-lib=dylib=mpv");
    }
}
