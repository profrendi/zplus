//! Which ZFPlus this is, and what the other one has published.
//!
//! There are two apps built from this code:
//!
//! - **ZFPlus** (Stable), on official ZapFast releases. Its names, folders and
//!   update feed are the ones ZFPlus always had, so nothing moves for anyone
//!   already using it.
//! - **ZFPlus Nightly**, on ZapFast's main branch: what will be in the next
//!   ZapFast release, a day or so after it lands. A build made with
//!   `ZFPLUS_CHANNEL=nightly` is this one. It has its own name, its own data
//!   folder, its own login and its own install folder, so both can be
//!   installed side by side and neither can ever read or change the other's
//!   chats.
//!
//! Both are published to the same repository. Nightly releases are marked as
//! pre-releases, which GitHub's "latest release" never returns, so Stable
//! cannot see them. Nightly versions are `<major>.<yyyymmdd>.<hhmm>-nightly`,
//! above every stable version of the same major, so Nightly's updater (which
//! takes the highest version listed) never offers it a Stable release.

use std::sync::{LazyLock, Mutex};

/// Whether this build is ZFPlus Nightly.
pub const NIGHTLY: bool = is_nightly(option_env!("ZFPLUS_CHANNEL"));

const fn is_nightly(channel: Option<&str>) -> bool {
    let Some(channel) = channel else {
        return false;
    };
    let (channel, wanted) = (channel.as_bytes(), b"nightly");
    if channel.len() != wanted.len() {
        return false;
    }
    let mut index = 0;
    while index < wanted.len() {
        if channel[index] != wanted[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// The app's name, as windows, the tray and notifications show it.
pub const NAME: &str = if NIGHTLY { "ZFPlus Nightly" } else { "ZFPlus" };

/// The app's file and folder name: its data folders, its executable, its
/// release downloads, its single-instance lock and its desktop entry.
/// Stable keeps `zapfast`, the name its data has always lived under.
pub const SLUG: &str = if NIGHTLY { "zfplus-nightly" } else { "zapfast" };

/// The name Stable has always registered with the system under (login item,
/// volume mixer, window class), kept so existing registrations still match.
pub const SYSTEM_NAME: &str = if NIGHTLY { "ZFPlus Nightly" } else { "ZapFast" };

/// The identity notifications are sent under, outside macOS.
pub const NOTIFICATION_ID: &str = if NIGHTLY {
    "me.paolino.zfplus-nightly"
} else {
    "me.paolino.zapfast"
};

/// The ZapFast commit a nightly is built on, `<short sha> <yyyy-mm-dd>`, set
/// by the nightly workflow.
pub const NIGHTLY_UPSTREAM: Option<&str> = option_env!("ZFPLUS_NIGHTLY_UPSTREAM");

/// What this build is on, for About: `ZapFast v0.21.1` or
/// `ZapFast main 4497f08 2026-11-09`.
pub fn based_on() -> String {
    match (NIGHTLY, NIGHTLY_UPSTREAM) {
        (true, Some(upstream)) => format!("ZapFast main {upstream}"),
        (true, None) => "ZapFast main".to_owned(),
        (false, _) => format!("ZapFast {}", crate::plus::upstream_version()),
    }
}

/// A version as people read it: `0.21.105`, or for a nightly
/// `Nightly 10 Nov 2026, 14:30` rather than `0.20261110.1430-nightly`.
pub fn display_version(version: &str) -> String {
    match nightly_stamp(version) {
        Some((year, month, day, hour, minute)) => {
            const MONTHS: [&str; 12] = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ];
            format!(
                "Nightly {day} {} {year}, {hour:02}:{minute:02}",
                MONTHS[(month - 1) as usize]
            )
        }
        None => version.to_owned(),
    }
}

/// The date and time in a nightly version, in UTC.
fn nightly_stamp(version: &str) -> Option<(u32, u32, u32, u32, u32)> {
    let core = version.strip_suffix("-nightly")?;
    let mut parts = core.split('.');
    let (_major, date, time) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || date.len() != 8 {
        return None;
    }
    let date: u32 = date.parse().ok()?;
    let time: u32 = time.parse().ok()?;
    let (year, month, day) = (date / 10_000, date / 100 % 100, date % 100);
    let (hour, minute) = (time / 100, time % 100);
    ((1..=12).contains(&month) && (1..=31).contains(&day) && hour < 24 && minute < 60)
        .then_some((year, month, day, hour, minute))
}

/// One published release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Published {
    pub version: String,
    /// `yyyy-mm-dd`, when GitHub published it.
    pub date: String,
    /// Its release page, with the downloads.
    pub url: String,
}

/// The newest release of each app, as last fetched.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Latest {
    pub stable: Option<Published>,
    pub nightly: Option<Published>,
}

static LATEST: LazyLock<Mutex<Option<Latest>>> = LazyLock::new(|| Mutex::new(None));

/// The newest Stable and Nightly releases, once a check has fetched them.
pub fn latest() -> Option<Latest> {
    LATEST.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// Fetches the newest Stable and Nightly releases from the repository this
/// build updates from. Blocking: call it off the interface thread, next to
/// the update check. A failure keeps what was fetched before.
pub fn refresh_latest() {
    let Some(repository) = crate::updates::PLUS_REPOSITORY else {
        return;
    };
    match fetch(repository) {
        Ok(found) => *LATEST.lock().unwrap_or_else(|p| p.into_inner()) = Some(found),
        Err(error) => log::debug!("could not list the releases: {error:#}"),
    }
}

fn fetch(repository: &str) -> anyhow::Result<Latest> {
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent(concat!("ZFPlus/", env!("CARGO_PKG_VERSION")));
    if let Some(proxy) = crate::proxy::reqwest_proxy() {
        builder = builder.proxy(proxy);
    }
    let body = builder
        .build()?
        .get(format!(
            "https://api.github.com/repos/{repository}/releases?per_page=50"
        ))
        .header("Accept", "application/vnd.github+json")
        .send()?
        .error_for_status()?
        .text()?;
    newest(&body)
}

/// Picks the newest Stable and Nightly release from GitHub's release list.
fn newest(body: &str) -> anyhow::Result<Latest> {
    #[derive(serde::Deserialize)]
    struct Listed {
        tag_name: String,
        #[serde(default)]
        draft: bool,
        #[serde(default)]
        published_at: Option<String>,
        html_url: String,
    }
    let listed: Vec<Listed> = serde_json::from_str(body)?;
    let mut latest = Latest::default();
    for release in listed.iter().filter(|release| !release.draft) {
        let Some(version) = release.tag_name.strip_prefix('v') else {
            continue;
        };
        let Some(order) = order_key(version) else {
            continue;
        };
        let slot = if version.ends_with("-nightly") {
            &mut latest.nightly
        } else if version.contains('-') {
            continue;
        } else {
            &mut latest.stable
        };
        if slot
            .as_ref()
            .and_then(|best| order_key(&best.version))
            .is_none_or(|best| order > best)
        {
            *slot = Some(Published {
                version: version.to_owned(),
                date: release
                    .published_at
                    .as_deref()
                    .and_then(|at| at.get(..10))
                    .unwrap_or_default()
                    .to_owned(),
                url: release.html_url.clone(),
            });
        }
    }
    Ok(latest)
}

/// `major.minor.patch` as numbers, ignoring a `-suffix`.
fn order_key(version: &str) -> Option<[u64; 3]> {
    let core = version.split('-').next()?;
    let mut parts = core.split('.').map(|part| part.parse::<u64>().ok());
    let key = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(key)
}

/// The engine this build runs on, for the performance report: the ZapFast
/// it is based on, the libraries that matter most and the compiler.
pub fn engine_lines() -> Vec<String> {
    vec![
        format!("App: {NAME} {}", env!("CARGO_PKG_VERSION")),
        format!("Based on: {}", based_on()),
        format!("whatsapp-rust: {}", env!("ZFPLUS_ENGINE_WHATSAPP_RUST")),
        format!("fastframe: {}", env!("ZFPLUS_ENGINE_FASTFRAME")),
        format!("egui: {}", env!("ZFPLUS_ENGINE_EGUI")),
        format!("Rust: {}", env!("ZFPLUS_ENGINE_RUSTC")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_build_is_stable_unless_asked() {
        assert!(!is_nightly(None));
        assert!(!is_nightly(Some("stable")));
        assert!(!is_nightly(Some("nightly2")));
        assert!(is_nightly(Some("nightly")));
        if option_env!("ZFPLUS_CHANNEL").is_none() {
            assert_eq!((NAME, SLUG), ("ZFPlus", "zapfast"));
        }
    }

    #[test]
    fn a_nightly_reads_as_its_date() {
        assert_eq!(
            display_version("0.20261110.1430-nightly"),
            "Nightly 10 Nov 2026, 14:30"
        );
        assert_eq!(
            display_version("0.20261110.5-nightly"),
            "Nightly 10 Nov 2026, 00:05"
        );
        assert_eq!(display_version("0.21.105"), "0.21.105");
        assert_eq!(display_version("0.2026.1-nightly"), "0.2026.1-nightly");
    }

    /// A nightly must sort above every stable version of its major, or
    /// Nightly's updater would offer a Stable download it cannot install.
    #[test]
    fn a_nightly_sorts_above_any_stable_release() {
        let nightly = order_key("0.20261110.930-nightly").unwrap();
        for stable in ["0.21.105", "0.99.9999", "0.9999.0"] {
            assert!(nightly > order_key(stable).unwrap(), "{stable}");
        }
    }

    #[test]
    fn the_newest_of_each_kind_is_picked() {
        let body = r#"[
            {"tag_name": "v0.20261110.930-nightly", "prerelease": true,
             "published_at": "2026-11-10T09:31:00Z", "html_url": "https://n/930"},
            {"tag_name": "v0.21.105", "published_at": "2026-11-10T02:00:00Z",
             "html_url": "https://s/105"},
            {"tag_name": "v0.20261111.100-nightly", "prerelease": true,
             "published_at": "2026-11-11T01:01:00Z", "html_url": "https://n/111"},
            {"tag_name": "v0.21.106", "draft": true, "html_url": "https://s/draft"},
            {"tag_name": "v0.21.104", "published_at": "2026-11-09T02:00:00Z",
             "html_url": "https://s/104"},
            {"tag_name": "v0.22.0-rc1", "html_url": "https://s/rc"},
            {"tag_name": "notes", "html_url": "https://x"}
        ]"#;
        let latest = newest(body).unwrap();
        assert_eq!(
            latest.stable,
            Some(Published {
                version: "0.21.105".into(),
                date: "2026-11-10".into(),
                url: "https://s/105".into(),
            })
        );
        assert_eq!(
            latest.nightly.map(|n| (n.version, n.date)),
            Some(("0.20261111.100-nightly".into(), "2026-11-11".into()))
        );
    }

    #[test]
    fn the_engine_is_named_in_full() {
        let lines = engine_lines();
        assert!(lines.iter().all(|line| !line.ends_with(": ")), "{lines:?}");
        assert!(lines.iter().any(|line| line.starts_with("whatsapp-rust: ")));
    }
}
