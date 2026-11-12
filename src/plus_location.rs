//! ZFPlus: where this computer is, for Send location's "My current
//! location". On Windows it asks the system's own location service (the
//! one a browser's "allow location" uses), which follows Windows Settings →
//! Privacy & security → Location: when apps may not use the location, it says
//! so and how to allow it. Asked only when the reader clicks the button, on a
//! thread of its own; the answer stays with the dialog and is not stored.

use std::sync::Mutex;

use crate::geo::Spot;

/// How asking for the location went.
#[derive(Clone, Debug, PartialEq)]
pub enum Here {
    Looking,
    /// The spot, and how far off it may be, in metres.
    Found(Spot, f64),
    Failed(String),
}

static HERE: Mutex<Option<Here>> = Mutex::new(None);

/// The last answer, if the reader asked.
pub fn here() -> Option<Here> {
    HERE.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// Forgets the answer, when the dialog closes.
pub fn forget() {
    *HERE.lock().unwrap_or_else(|p| p.into_inner()) = None;
}

/// Asks where this computer is; `wake` is called with the answer.
pub fn locate(wake: impl FnOnce() + Send + 'static) {
    {
        let mut here = HERE.lock().unwrap_or_else(|p| p.into_inner());
        if matches!(*here, Some(Here::Looking)) {
            return;
        }
        *here = Some(Here::Looking);
    }
    let spawned = std::thread::Builder::new()
        .name("zfplus-location".into())
        .spawn(move || {
            let answer = match locate_now() {
                Ok((spot, accuracy)) => Here::Found(spot, accuracy),
                Err(error) => {
                    log::info!("the location was not available: {error}");
                    Here::Failed(error)
                }
            };
            *HERE.lock().unwrap_or_else(|p| p.into_inner()) = Some(answer);
            wake();
        });
    if spawned.is_err() {
        *HERE.lock().unwrap_or_else(|p| p.into_inner()) = Some(Here::Failed(
            "Could not start asking for the location".to_owned(),
        ));
    }
}

#[cfg(windows)]
fn locate_now() -> Result<(Spot, f64), String> {
    use windows::Devices::Geolocation::{GeolocationAccessStatus, Geolocator, PositionAccuracy};
    let denied = || {
        "Windows does not let apps use your location. Turn on Location in Windows Settings, \
         Privacy & security, Location, and allow desktop apps to use it."
            .to_owned()
    };
    let access = Geolocator::RequestAccessAsync()
        .and_then(|asking| asking.join())
        .map_err(|error| format!("{error}"))?;
    if access != GeolocationAccessStatus::Allowed {
        return Err(denied());
    }
    let locator = Geolocator::new().map_err(|error| format!("{error}"))?;
    let _ = locator.SetDesiredAccuracy(PositionAccuracy::High);
    let position = locator
        .GetGeopositionAsync()
        .and_then(|asking| asking.join())
        .map_err(|error| format!("{error}"))?;
    let coordinate = position.Coordinate().map_err(|error| format!("{error}"))?;
    let point = coordinate
        .Point()
        .and_then(|point| point.Position())
        .map_err(|error| format!("{error}"))?;
    let accuracy = coordinate.Accuracy().unwrap_or(0.0);
    Ok((
        Spot {
            latitude: point.Latitude,
            longitude: point.Longitude,
        },
        accuracy,
    ))
}

#[cfg(not(windows))]
fn locate_now() -> Result<(Spot, f64), String> {
    Err(
        "Your current location is read through Windows. Here, paste a map link or the coordinates."
            .to_owned(),
    )
}
