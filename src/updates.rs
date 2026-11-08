//! Self-update from GitHub releases, through fastframe-update.
//!
//! The crate checks for a newer release, refuses package-managed copies,
//! downloads and verifies the update against the publisher signature, and
//! hands it to a helper that installs it and rolls back if it does not start.
//! ZapFast keeps its names, its key, its proxy and its interface.

pub use fastframe_update::{
    CHECK_INTERVAL, DownloadState, Installation, Kind, Prepared, Release, Source, Unsupported,
    Updater,
};
use fastframe_update::{MacConfig, ReqwestTransport, UpdateConfig};

/// ZapFast Plus's own release feed: the GitHub repository its build
/// publishes to, set at build time. A build without one does not update.
pub const PLUS_REPOSITORY: Option<&str> = option_env!("ZAPFAST_PLUS_REPO");

/// ZapFast Plus's releases. They are signed with ZapFast Plus's own key,
/// never ZapFast's, so an official release can never replace this build and
/// lose its Enhancements: updates come from the Plus repository, which
/// rebuilds every official release with the Enhancements on top.
pub const CONFIG: UpdateConfig = UpdateConfig {
    legacy_names: &["fastsapp"],
    macos: MacConfig {
        bundle_ids: &["me.paolino.fastsapp"],
        executable_names: &[],
        legacy_bundle_names: &["FastsApp.app"],
    },
    publisher_key: Some(include_str!("../assets/plus-update-public-key.hex")),
    ..UpdateConfig::new(
        match PLUS_REPOSITORY {
            Some(repository) => repository,
            None => "crmne/zapfast",
        },
        "ZapFast Plus",
        "zapfast",
        env!("CARGO_PKG_VERSION"),
    )
};

/// An updater on the proxy-aware reqwest client.
pub fn updater() -> anyhow::Result<Updater> {
    if PLUS_REPOSITORY.is_none() && !cfg!(test) {
        anyhow::bail!("This ZapFast Plus build has no update channel");
    }
    let mut builder = reqwest::blocking::Client::builder();
    if let Some(proxy) = crate::proxy::reqwest_proxy() {
        builder = builder.proxy(proxy);
    }
    Ok(Updater::new(CONFIG, ReqwestTransport::new(builder)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_config_is_valid() {
        CONFIG.validate().unwrap();
        assert_eq!(CONFIG.current_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(CONFIG.slug, "zapfast");
    }

    #[test]
    fn the_updater_starts_on_github() {
        assert!(updater().unwrap().source().is_github());
    }
}
