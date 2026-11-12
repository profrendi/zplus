//! ZFPlus: removing the app from Settings.
//!
//! The installer (a per-user MSI) is asked to remove this channel's copy,
//! as Windows' own Apps settings would; with `remove_data`, the chats,
//! settings and caches this computer keeps go too once it is gone. The app
//! quits first, so nothing holds its files. A copy run from the ZIP was
//! never installed: its folder is shown to delete by hand.

/// The installer's upgrade code for this channel, from `plus/*.wxs`.
#[cfg(windows)]
const UPGRADE_CODE: &str = if crate::channel::NIGHTLY {
    "{C45D724D-F07C-40FE-867C-AC37FE8815E6}"
} else {
    "{6E3B1A52-6B0B-4C7E-9E0A-1F2D7C5A9B31}"
};

/// The folder the running app is in.
pub fn app_folder() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(std::path::Path::to_path_buf)
}

/// Whether this copy was installed by the installer, so it can be removed
/// from here.
pub fn installed() -> bool {
    product_code().is_some()
}

#[cfg(windows)]
fn product_code() -> Option<String> {
    #[link(name = "msi")]
    unsafe extern "system" {
        fn MsiEnumRelatedProductsW(
            upgrade_code: *const u16,
            reserved: u32,
            index: u32,
            product: *mut u16,
        ) -> u32;
    }
    let code: Vec<u16> = UPGRADE_CODE.encode_utf16().chain(Some(0)).collect();
    let mut product = [0u16; 39];
    // SAFETY: the upgrade code is a NUL-terminated GUID and the buffer has
    // the 39 characters a product code takes.
    let status = unsafe { MsiEnumRelatedProductsW(code.as_ptr(), 0, 0, product.as_mut_ptr()) };
    if status != 0 {
        return None;
    }
    let end = product
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(product.len());
    String::from_utf16(&product[..end])
        .ok()
        .filter(|code| code.starts_with('{'))
}

#[cfg(not(windows))]
fn product_code() -> Option<String> {
    None
}

/// Starts removing the app: a small script waits for it to quit, runs the
/// installer's removal and, with `remove_data`, deletes `folders` after a
/// removal that went through. The caller quits right after.
pub fn start(remove_data: bool, folders: &[std::path::PathBuf]) -> Result<(), String> {
    let Some(code) = product_code() else {
        return Err(
            "This copy was not installed with the installer. Quit ZFPlus and delete its folder."
                .to_owned(),
        );
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut script = String::from("@echo off\r\ntimeout /t 3 /nobreak >nul\r\n");
        script.push_str(&format!("msiexec /x {code} /passive\r\n"));
        script.push_str("if errorlevel 1 goto end\r\n");
        if remove_data {
            for folder in folders {
                script.push_str(&format!("rmdir /s /q \"{}\"\r\n", folder.display()));
            }
        }
        script.push_str(":end\r\n(goto) 2>nul & del \"%~f0\"\r\n");
        let path =
            std::env::temp_dir().join(format!("zfplus-uninstall-{}.cmd", std::process::id()));
        std::fs::write(&path, script).map_err(|error| error.to_string())?;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        std::process::Command::new("cmd")
            .arg("/c")
            .arg(&path)
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn()
            .map_err(|error| error.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (code, remove_data, folders);
        Err("Removing the app from here works on Windows.".to_owned())
    }
}
