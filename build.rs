//! Compiles translations and embeds the icon and version information in
//! Windows executables.

fn main() {
    fastframe_i18n::build::compile_catalogs("assets/i18n");
    engine_versions();

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
        // Windows version numbers hold four parts up to 65535 each, which a
        // nightly's date does not fit: keep the digits that do.
        let version = env!("CARGO_PKG_VERSION")
            .split('-')
            .next()
            .unwrap_or_default()
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0).min(65_535).to_string())
            .collect::<Vec<_>>()
            .join(",");
        let name = if std::env::var("ZFPLUS_CHANNEL").as_deref() == Ok("nightly") {
            "ZFPlus Nightly"
        } else {
            "ZFPlus"
        };
        std::fs::write(
            &rc,
            format!(
                "1 ICON \"{}\"\n1 VERSIONINFO\nFILEVERSION {version},0\nPRODUCTVERSION {version},0\nBEGIN\n BLOCK \"StringFileInfo\"\n BEGIN\n  BLOCK \"040904B0\"\n  BEGIN\n   VALUE \"ProductName\", \"{name}\"\n   VALUE \"FileDescription\", \"{name}\"\n   VALUE \"ProductVersion\", \"{}\"\n  END\n END\n BLOCK \"VarFileInfo\"\n BEGIN\n  VALUE \"Translation\", 0x409, 1200\n END\nEND\n",
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

/// ZFPlus: the engine this build runs on, read from `Cargo.lock` and the
/// compiler, for the performance report (`channel::engine_lines`).
fn engine_versions() {
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-env-changed=ZFPLUS_CHANNEL");
    println!("cargo:rerun-if-env-changed=ZFPLUS_NIGHTLY_UPSTREAM");
    let lock = std::fs::read_to_string("Cargo.lock").unwrap_or_default();
    let package = |name: &str| -> Option<(String, String)> {
        let mut lines = lock.lines();
        while let Some(line) = lines.next() {
            if line == format!("name = \"{name}\"") {
                let version = lines
                    .next()?
                    .strip_prefix("version = \"")?
                    .trim_end_matches('"')
                    .to_owned();
                let source = lines
                    .next()
                    .and_then(|line| line.strip_prefix("source = \""))
                    .map(|source| source.trim_end_matches('"').to_owned())
                    .unwrap_or_default();
                return Some((version, source));
            }
        }
        None
    };
    // `git+https://github.com/<owner>/<repo>?rev=..#<commit>` names its owner
    // and commit; `?tag=v0.16.1` its tag.
    let origin = |source: &str| -> String {
        let Some(rest) = source.strip_prefix("git+https://github.com/") else {
            return "crates.io".to_owned();
        };
        let owner = rest.split('/').next().unwrap_or_default();
        let commit = rest.rsplit('#').next().unwrap_or_default();
        match rest.split_once("?tag=") {
            Some((_, tag)) => tag.split('#').next().unwrap_or_default().to_owned(),
            None => format!("{owner} {}", commit.get(..7).unwrap_or(commit)),
        }
    };
    let describe = |name: &str, with_version: bool| -> String {
        match package(name) {
            Some((version, source)) if with_version => format!("{version} ({})", origin(&source)),
            Some((_, source)) => origin(&source),
            None => "unknown".to_owned(),
        }
    };
    println!(
        "cargo:rustc-env=ZFPLUS_ENGINE_WHATSAPP_RUST={}",
        describe("whatsapp-rust", false)
    );
    println!(
        "cargo:rustc-env=ZFPLUS_ENGINE_FASTFRAME={}",
        describe("fastframe-update", false)
    );
    println!(
        "cargo:rustc-env=ZFPLUS_ENGINE_EGUI={}",
        describe("egui", true)
    );
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let compiler = std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|text| text.split_whitespace().nth(1).map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=ZFPLUS_ENGINE_RUSTC={compiler}");
}
