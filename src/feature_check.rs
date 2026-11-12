//! ZFPlus: making sure nothing goes missing.
//!
//! Two lists live here. [`INVENTORY`] names every ZFPlus feature by what the
//! screen says, and a test (`demo::tests::every_inventoried_feature_is_on_screen`)
//! looks each one up in the real interface; the release and nightly
//! workflows run it, so a merge or a change that loses a switch, a button or
//! a menu entry stops the release before it reaches anyone. [`run`] is the
//! in-app checklist under Settings → Diagnostics → Feature check: what can
//! only be known on the computer itself (the GPU blur, the tray icon,
//! notifications, starting with Windows, the link, updates), to copy and
//! paste when something seems off.

use crate::app::App;

/// Where a feature is found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    /// A row in Settings, found by searching for its label.
    Settings,
    /// An entry in the tray icon's menu.
    Tray,
}

/// Every ZFPlus feature a release must have, by the label people see.
/// Adding a feature means adding it here; removing one from here needs a
/// reason, since the test then stops guarding it.
pub const INVENTORY: &[(Place, &str)] = &[
    // Enhancements · Private Mode
    (
        Place::Settings,
        "Show the Private Mode eye in the left rail",
    ),
    (Place::Settings, "Blur contact names"),
    (Place::Settings, "Blur contact photos"),
    (Place::Settings, "Blur recent messages"),
    (Place::Settings, "Blur conversation messages"),
    (Place::Settings, "Blur message composer input"),
    (Place::Settings, "Hide typing status"),
    (Place::Settings, "Hide recording status"),
    (Place::Settings, "View statuses privately"),
    (Place::Settings, "Play audio messages privately"),
    (Place::Settings, "Disable read receipts"),
    (Place::Settings, "Hide online status (Invisible Mode)"),
    // Enhancements · Customizations
    (Place::Settings, "Restore deleted messages"),
    (Place::Settings, "Notify about online activities"),
    (
        Place::Settings,
        "Notify about typing and recording activities",
    ),
    (
        Place::Settings,
        "Prevent chats list jumping on message submission",
    ),
    (Place::Settings, "Set custom chat background image"),
    (Place::Settings, "Full chat history when linking"),
    // Enhancements · Extra buttons
    (Place::Settings, "Like button"),
    (Place::Settings, "Pin chat button"),
    (Place::Settings, "Start new chat"),
    (Place::Settings, "Mark conversation as read/unread"),
    (Place::Settings, "Translate message button"),
    // Updates
    (Place::Settings, "Check for updates"),
    (Place::Settings, "Install updates automatically"),
    (Place::Settings, "Stable and Nightly"),
    // Diagnostics
    (Place::Settings, "Record performance"),
    (Place::Settings, "Copy performance report"),
    (Place::Settings, "Feature check"),
    (Place::Settings, "Show memory and CPU in the left rail"),
    (Place::Settings, "Warn when memory passes"),
    (Place::Settings, "Diagnostics folder"),
    // The tray icon's menu
    (Place::Tray, "Show or hide ZapFast"),
    (Place::Tray, "Lock ZapFast"),
    (Place::Tray, "Check for updates"),
    (Place::Tray, "Update and restart"),
    (Place::Tray, "Quit"),
];

/// How one check came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Good,
    /// Works, but not the best way, or turned off by a setting.
    Note,
    Problem,
}

impl Status {
    pub fn mark(self) -> &'static str {
        match self {
            Status::Good => "OK",
            Status::Note => "NOTE",
            Status::Problem => "PROBLEM",
        }
    }
}

/// One line of the checklist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub status: Status,
    pub name: &'static str,
    pub detail: String,
}

fn check(status: Status, name: &'static str, detail: impl Into<String>) -> Check {
    Check {
        status,
        name,
        detail: detail.into(),
    }
}

/// The checklist for this computer, now. Cheap: it only reads state the app
/// already has.
pub fn run(app: &App) -> Vec<Check> {
    use crate::backend::LinkStatus;
    let mut checks = Vec::new();
    checks.push(match &app.account().link {
        LinkStatus::Connected => check(Status::Good, "WhatsApp link", "Connected"),
        LinkStatus::Connecting | LinkStatus::Starting => {
            check(Status::Note, "WhatsApp link", "Connecting")
        }
        LinkStatus::Disconnected { reason } => check(
            Status::Problem,
            "WhatsApp link",
            format!("Disconnected: {reason}"),
        ),
        LinkStatus::Failed(error) => check(Status::Problem, "WhatsApp link", error.clone()),
        // Never the QR code or pairing code it carries.
        LinkStatus::Unlinked { .. } => check(
            Status::Problem,
            "WhatsApp link",
            "Not linked: scan the code with the phone",
        ),
        LinkStatus::LoggedOut => check(
            Status::Problem,
            "WhatsApp link",
            "Logged out: WhatsApp ended this device's session",
        ),
    });
    checks.push(match app.account().business {
        Some(true) => check(
            Status::Good,
            "Account",
            format!(
                "WhatsApp Business, {} quick replies",
                app.account().quick_replies.len()
            ),
        ),
        Some(false) => check(Status::Good, "Account", "Personal WhatsApp"),
        None => check(
            Status::Note,
            "Account",
            "Not known yet: WhatsApp has not said",
        ),
    });
    checks.push(match crate::plus::blur_state() {
        crate::plus::BlurState::Gpu => check(
            Status::Good,
            "Private Mode blur",
            "Smooth blur on the graphics card",
        ),
        crate::plus::BlurState::Veil => check(
            Status::Note,
            "Private Mode blur",
            "The graphics card refused the blur, so a frosted veil hides instead",
        ),
        crate::plus::BlurState::NotYet => check(
            Status::Note,
            "Private Mode blur",
            "Not drawn yet: turn on a blur in Private Mode to test it",
        ),
    });
    checks.push(if !app.settings.keep_running_in_background {
        check(
            Status::Note,
            "Tray icon",
            "Off: closing the window quits (Settings → Keep running in the background)",
        )
    } else if app.has_tray() {
        check(Status::Good, "Tray icon", "In the notification area")
    } else {
        check(
            Status::Problem,
            "Tray icon",
            "Wanted, but the system did not take it",
        )
    });
    checks.push(if app.settings.notifications {
        check(Status::Good, "Notifications", "On")
    } else {
        check(Status::Note, "Notifications", "Off in Settings")
    });
    checks.push(match app.start_with_system {
        Some(true) => check(Status::Good, "Start with the computer", "On"),
        Some(false) => check(Status::Note, "Start with the computer", "Off"),
        None => check(Status::Note, "Start with the computer", "Not read yet"),
    });
    checks.push(match crate::plus::update_check() {
        crate::plus::UpdateCheck::Failed(error) => check(
            Status::Problem,
            "Updates",
            format!("The last check failed: {error}"),
        ),
        crate::plus::UpdateCheck::Available(version) => check(
            Status::Note,
            "Updates",
            format!("{} is available", crate::channel::display_version(&version)),
        ),
        crate::plus::UpdateCheck::UpToDate => check(Status::Good, "Updates", "Up to date"),
        _ if !app.settings.check_for_updates => {
            check(Status::Note, "Updates", "Checking is off in Settings")
        }
        _ => check(Status::Note, "Updates", "Not checked yet"),
    });
    // ZFPlus: what the app holds, from the last second's measurement.
    checks.push(match crate::plus_memory::latest() {
        Some(memory) => {
            let private = memory.process.map_or(0, |process| process.private);
            let status = if private > 700 << 20 {
                Status::Problem
            } else if private > 400 << 20 {
                Status::Note
            } else {
                Status::Good
            };
            check(status, "Memory", memory.line())
        }
        None => check(Status::Note, "Memory", "Not measured yet"),
    });
    checks.push(check(
        Status::Good,
        "Features in this build",
        format!("{} checked before release", INVENTORY.len()),
    ));
    checks
}

/// The checklist as text to paste: the app and its engine, then each check.
pub fn report(checks: &[Check]) -> String {
    let mut text = String::from("ZFPlus feature check\n");
    for line in crate::channel::engine_lines() {
        text.push_str(&line);
        text.push('\n');
    }
    text.push('\n');
    for check in checks {
        text.push_str(&format!(
            "[{}] {}: {}\n",
            check.status.mark(),
            check.name,
            check.detail
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_inventory_names_each_feature_once() {
        let mut seen = std::collections::HashSet::new();
        for entry in INVENTORY {
            assert!(seen.insert(entry), "{entry:?} twice");
        }
    }

    #[test]
    fn the_report_lists_every_check_with_its_mark() {
        let checks = [
            check(Status::Good, "WhatsApp link", "Connected"),
            check(
                Status::Problem,
                "Tray icon",
                "Wanted, but the system did not take it",
            ),
        ];
        let report = report(&checks);
        assert!(
            report.starts_with("ZFPlus feature check\nApp: "),
            "{report}"
        );
        assert!(
            report.contains("[OK] WhatsApp link: Connected\n"),
            "{report}"
        );
        assert!(
            report.contains("[PROBLEM] Tray icon: Wanted, but the system did not take it\n"),
            "{report}"
        );
    }
}
