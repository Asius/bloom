fn main() {
    compile_observer();
    tauri_build::build()
}

fn compile_observer() {
    // A narrowly scoped Explorer notification-area observer. Built from our
    // source and embedded in the executable, never downloaded at runtime.
    println!("cargo:rerun-if-changed=native/tray_observer.cpp");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let compiler = cc::Build::new().cpp(true).static_crt(true).get_compiler();
    assert!(compiler.is_like_msvc(), "The tray observer requires MSVC");
    let status = compiler
        .to_command()
        .args([
            "/nologo",
            "/LD",
            "/O2",
            "/W4",
            "/WX",
            "/GS",
            "/guard:cf",
            "/MT",
            "/DUNICODE",
            "/D_UNICODE",
        ])
        .arg("native/tray_observer.cpp")
        .arg(format!("/Fo{}", out.join("tray_observer.obj").display()))
        .arg("/link")
        .args([
            "user32.lib",
            "gdi32.lib",
            "/DYNAMICBASE",
            "/NXCOMPAT",
            "/guard:cf",
        ])
        .arg(format!(
            "/OUT:{}",
            out.join("bloom_tray_observer.dll").display()
        ))
        .arg(format!(
            "/IMPLIB:{}",
            out.join("tray_observer.lib").display()
        ))
        .status()
        .expect("compile tray observer");
    assert!(status.success(), "Tray observer compilation failed");
}
