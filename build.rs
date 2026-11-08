//! Compiles translations and embeds the icon and version information in
//! Windows executables.

fn main() {
    fastframe_i18n::build::compile_catalogs("assets/i18n");

    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=packaging/windows/zfplus.ico");
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("packaging/windows/zfplus.ico")
            .set("ProductName", "ZFPlus")
            .set("FileDescription", "ZFPlus");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=Windows resources not embedded: {error}");
        }
    }

    // ZFPlus: a Windows build made on Linux embeds the icon through
    // MinGW's windres.
    #[cfg(not(windows))]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu")
    {
        println!("cargo:rerun-if-changed=packaging/windows/zfplus.ico");
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let icon = std::path::Path::new("packaging/windows/zfplus.ico")
            .canonicalize()
            .unwrap();
        let rc = out.join("zapfast-plus.rc");
        let version = env!("CARGO_PKG_VERSION").replace('.', ",");
        std::fs::write(
            &rc,
            format!(
                "1 ICON \"{}\"\n1 VERSIONINFO\nFILEVERSION {version},0\nPRODUCTVERSION {version},0\nBEGIN\n BLOCK \"StringFileInfo\"\n BEGIN\n  BLOCK \"040904B0\"\n  BEGIN\n   VALUE \"ProductName\", \"ZFPlus\"\n   VALUE \"FileDescription\", \"ZFPlus\"\n   VALUE \"ProductVersion\", \"{}\"\n  END\n END\n BLOCK \"VarFileInfo\"\n BEGIN\n  VALUE \"Translation\", 0x409, 1200\n END\nEND\n",
                icon.display().to_string().replace('\\', "/"),
                env!("CARGO_PKG_VERSION"),
            ),
        )
        .unwrap();
        let object = out.join("zapfast-plus-res.o");
        let windres = std::env::var("RC_x86_64_pc_windows_gnu")
            .unwrap_or_else(|_| "x86_64-w64-mingw32-windres".into());
        let status = std::process::Command::new(windres)
            .arg(&rc)
            .arg("-O")
            .arg("coff")
            .arg("-o")
            .arg(&object)
            .status();
        if status.is_ok_and(|status| status.success()) {
            println!("cargo:rustc-link-arg-bins={}", object.display());
        } else {
            println!("cargo:warning=Windows icon not embedded");
        }
    }
}
