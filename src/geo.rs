//! Reading a location out of the text a reader pastes.
//!
//! A computer has no position of its own to offer and ZapFast asks no service
//! for one, so the location dialog takes the spot as text: a pair of
//! coordinates, or a link to a spot on a map. Everything here is offline
//! parsing, and anything it cannot read with confidence it refuses, so the
//! dialog can keep the send button dark instead of pinning the wrong place.

/// A spot on the earth, in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub latitude: f64,
    pub longitude: f64,
}

impl Spot {
    /// The coordinates as text, dot decimals and five places (about a metre),
    /// which is how coordinates are written everywhere, links included.
    pub fn text(self) -> String {
        format!("{:.5}, {:.5}", self.latitude, self.longitude)
    }

    /// Whether the pair is a real position, so a typo cannot pin the sea.
    fn is_on_earth(self) -> bool {
        self.latitude.is_finite()
            && self.longitude.is_finite()
            && (-90.0..=90.0).contains(&self.latitude)
            && (-180.0..=180.0).contains(&self.longitude)
    }
}

/// Reads the spot in `text`, or `None` when the text is no location.
pub fn spot(text: &str) -> Option<Spot> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    link(text)
        .or_else(|| pair(text))
        .filter(|spot| spot.is_on_earth())
}

/// The link to open for a spot, the one the location card offers too. The
/// browser fetches it, never ZapFast, so no map is ever loaded in the app.
/// ZFPlus: Google Maps, as WhatsApp itself opens a location.
pub fn map_url(spot: Spot) -> String {
    let (latitude, longitude) = (spot.latitude, spot.longitude);
    format!("https://www.google.com/maps/search/?api=1&query={latitude},{longitude}")
}

/// Whether `text` is a shortened map link, which names its spot only on the
/// server it redirects through. ZFPlus follows it there ([`look_up`]) when
/// the reader pastes one to send.
pub fn is_short_link(text: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(text.trim()) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some_and(|host| {
            let host = host.trim_end_matches('.').to_ascii_lowercase();
            matches!(
                host.as_str(),
                "goo.gl" | "maps.app.goo.gl" | "app.goo.gl" | "g.co" | "g.page"
            )
        })
}

/// ZFPlus: whether `text` is a web link that does not say where it points
/// by itself (a shortener of any map service, a place page), so it is
/// followed to find out. Links that name their spot never reach here.
pub fn needs_lookup(text: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(text.trim()) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https") && url.host_str().is_some()
}

/// ZFPlus: what looking up a short link came to.
#[derive(Clone, Debug, PartialEq)]
pub enum Lookup {
    Looking,
    /// The spot, and the full link it came from.
    Found(Spot, String),
    Failed(String),
}

/// Short links looked up so far, by link.
static LOOKUPS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, Lookup>>> =
    std::sync::LazyLock::new(Default::default);

/// ZFPlus: where a short map link (`maps.app.goo.gl/…`) points, looked up
/// once on a thread of its own: the redirects are followed to the full Google
/// Maps link, which names the spot, and failing that the map page's own
/// coordinates are read. Only the link the reader pasted is asked for, from
/// the server it belongs to. `wake` is called when the answer is in.
pub fn look_up(link: &str, wake: impl FnOnce() + Send + 'static) -> Lookup {
    let link = link.trim().to_owned();
    let mut lookups = LOOKUPS.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(known) = lookups.get(&link) {
        return known.clone();
    }
    lookups.insert(link.clone(), Lookup::Looking);
    drop(lookups);
    let spawned = std::thread::Builder::new()
        .name("zfplus-map-link".into())
        .spawn(move || {
            let answer = match follow(&link) {
                Ok((spot, full)) => Lookup::Found(spot, full),
                Err(error) => {
                    log::info!("a short map link could not be read: {error}");
                    Lookup::Failed(error)
                }
            };
            LOOKUPS
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(link, answer);
            wake();
        });
    if spawned.is_err() {
        return Lookup::Failed("Could not start the lookup".to_owned());
    }
    Lookup::Looking
}

/// Follows a short link's redirects to a spot.
fn follow(link: &str) -> Result<(Spot, String), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .proxy(crate::proxy::ureq_proxy())
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(15)))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36")
        .build()
        .into();
    let mut at = link.to_owned();
    for _ in 0..8 {
        if let Some(spot) = spot(&at) {
            return Ok((spot, at));
        }
        let mut response = agent
            .get(&at)
            .call()
            .map_err(|error| format!("could not reach the link: {error}"))?;
        let status = response.status().as_u16();
        if (300..400).contains(&status) {
            let next = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .ok_or("a redirect without a place")?;
            let next = reqwest::Url::parse(&at)
                .and_then(|base| base.join(next))
                .map_err(|_| "a redirect to an unreadable place")?;
            // A consent page carries the real link in `continue`.
            at = next
                .query_pairs()
                .find(|(key, _)| key == "continue")
                .map(|(_, value)| value.into_owned())
                .filter(|_| {
                    next.host_str()
                        .is_some_and(|host| host.starts_with("consent."))
                })
                .unwrap_or_else(|| next.to_string());
            continue;
        }
        if !(200..300).contains(&status) {
            return Err(format!("the link answered {status}"));
        }
        // The last page: Google's map page states where it centres.
        let page = response
            .body_mut()
            .with_config()
            .limit(4 * 1024 * 1024)
            .read_to_string()
            .map_err(|error| format!("could not read the map page: {error}"))?;
        return page_spot(&page)
            .map(|spot| (spot, at.clone()))
            .ok_or_else(|| "the map page names no spot".to_owned());
    }
    Err("too many redirects".to_owned())
}

/// The spot a Google Maps page names: a place marker or `@lat,long` in the
/// links it carries, else the centre its start-up state holds
/// (`APP_INITIALIZATION_STATE=[[[zoom,long,lat]`).
fn page_spot(page: &str) -> Option<Spot> {
    let numeric = |marker: &str| {
        let (_, rest) = page.split_once(marker)?;
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
            .unwrap_or(rest.len());
        rest[..end].parse::<f64>().ok()
    };
    if let (Some(latitude), Some(longitude)) = (numeric("!3d"), numeric("!4d")) {
        let spot = Spot {
            latitude,
            longitude,
        };
        if spot.is_on_earth() {
            return Some(spot);
        }
    }
    for (at, _) in page.match_indices("/@") {
        if let Some(spot) = two(&page[at + 2..]).filter(|spot| spot.is_on_earth()) {
            return Some(spot);
        }
    }
    let state = page.split_once("APP_INITIALIZATION_STATE=[[[")?.1;
    let mut numbers = state
        .split([',', ']'])
        .take(3)
        .map(|value| value.trim().parse::<f64>().ok());
    let (_zoom, longitude, latitude) = (numbers.next()??, numbers.next()??, numbers.next()??);
    Some(Spot {
        latitude,
        longitude,
    })
    .filter(|spot| spot.is_on_earth())
}

/// The map service a link belongs to. Only these links are read, so a page
/// elsewhere that happens to carry `q=12,34` is never taken for a spot.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Service {
    Google,
    Apple,
    OpenStreetMap,
}

fn service(url: &reqwest::Url) -> Option<Service> {
    let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    if matches!(host.as_str(), "maps.apple.com" | "maps.apple") {
        return Some(Service::Apple);
    }
    if matches!(
        host.as_str(),
        "openstreetmap.org" | "www.openstreetmap.org" | "osm.org" | "www.osm.org"
    ) {
        return Some(Service::OpenStreetMap);
    }
    // `google.<tld>` or `google.co.<tld>`, with `www.` before it and a `/maps`
    // path, or `maps.google.<tld>` with any path.
    let (maps_host, rest) = match host.strip_prefix("maps.") {
        Some(rest) => (true, rest),
        None => (false, host.strip_prefix("www.").unwrap_or(&host)),
    };
    let tld = rest.strip_prefix("google.")?;
    let letters = |label: &str, lengths: std::ops::RangeInclusive<usize>| {
        lengths.contains(&label.len())
            && label
                .chars()
                .all(|character| character.is_ascii_alphabetic())
    };
    let tld_ok = match tld.split('.').collect::<Vec<_>>().as_slice() {
        [country] => letters(country, 2..=3),
        ["co" | "com", country] => letters(country, 2..=2),
        _ => false,
    };
    let path_ok = maps_host || url.path() == "/maps" || url.path().starts_with("/maps/");
    (tld_ok && path_ok).then_some(Service::Google)
}

/// Reads a link to a spot: Google Maps, Apple Maps, OpenStreetMap, and any
/// `geo:` URI. Google's place markers name the exact spot, so they win over the
/// viewport around them. Links to other sites are not read.
fn link(text: &str) -> Option<Spot> {
    let url = reqwest::Url::parse(text).ok()?;
    if url.scheme() == "geo" {
        return geo_uri(&url);
    }
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let Some(service) = service(&url) else {
        return other_service(&url);
    };
    match service {
        Service::Google => marked_place(text)
            .or_else(|| url.path().split_once("/@").and_then(|(_, rest)| two(rest)))
            .or_else(|| {
                query_spot(
                    &url,
                    &["q", "ll", "query", "daddr", "destination", "center"],
                )
            }),
        Service::Apple => query_spot(&url, &["ll", "q", "daddr", "sll", "coordinate"]),
        // A `mlat`/`mlon` pair is the marker itself, so it is read before the
        // viewport the fragment describes.
        Service::OpenStreetMap => {
            query_spot(&url, &[]).or_else(|| url.fragment().and_then(openstreetmap_position))
        }
    }
}

/// ZFPlus: links of other map services: Waze, Bing Maps, HERE and Yandex.
fn other_service(url: &reqwest::Url) -> Option<Spot> {
    let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    let site = host.strip_prefix("www.").unwrap_or(&host);
    // Waze: `?ll=lat,long`, `?to=ll.lat,long`, or a geohash in `/ul/h…`.
    if site.ends_with("waze.com") {
        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "ll" | "latlng" => {
                    if let Some(spot) = pair(&value) {
                        return Some(spot);
                    }
                }
                "to" | "from" => {
                    if let Some(spot) = value.strip_prefix("ll.").and_then(pair) {
                        return Some(spot);
                    }
                }
                _ => {}
            }
        }
        if let Some(hash) = url.path().strip_prefix("/ul/h") {
            return geohash(hash.trim_end_matches('/'));
        }
    }
    // Bing: `cp=lat~long`, or `sp=point.lat_long`.
    if site == "bing.com" && url.path().starts_with("/maps") {
        for (key, value) in url.query_pairs() {
            if key == "cp"
                && let Some((latitude, longitude)) = value.split_once('~')
            {
                return joined(latitude, longitude);
            }
            if key == "sp"
                && let Some(point) = value.strip_prefix("point.")
                && let Some((latitude, rest)) = point.split_once('_')
            {
                return joined(latitude, rest.split('_').next()?);
            }
        }
    }
    // HERE: `?map=lat,long,zoom`, or `/l/lat,long` on its share links.
    if site.ends_with("here.com") {
        if let Some(spot) = url
            .query_pairs()
            .find(|(key, _)| key == "map")
            .and_then(|(_, value)| two(&value))
        {
            return Some(spot);
        }
        if let Some(rest) = url.path().strip_prefix("/l/") {
            return two(rest);
        }
    }
    // Yandex writes longitude first: `ll=long,lat` or `pt=long,lat`.
    if site.starts_with("yandex.") || site.contains(".yandex.") {
        for (key, value) in url.query_pairs() {
            if (key == "pt" || key == "ll")
                && let Some(swapped) = two(&value)
            {
                return Some(Spot {
                    latitude: swapped.longitude,
                    longitude: swapped.latitude,
                });
            }
        }
    }
    // Any other site is not read: `q=12,34` means a spot only on a map.
    None
}

/// ZFPlus: the middle of a geohash's cell, as Waze's short links carry.
fn geohash(hash: &str) -> Option<Spot> {
    const ALPHABET: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";
    if hash.is_empty() || hash.len() > 12 {
        return None;
    }
    let (mut latitude, mut longitude) = ((-90.0f64, 90.0f64), (-180.0f64, 180.0f64));
    let mut even = true;
    for character in hash.bytes() {
        let value = ALPHABET
            .iter()
            .position(|&letter| letter == character.to_ascii_lowercase())?;
        for bit in (0..5).rev() {
            let range = if even { &mut longitude } else { &mut latitude };
            let middle = (range.0 + range.1) / 2.0;
            if value >> bit & 1 == 1 {
                range.0 = middle;
            } else {
                range.1 = middle;
            }
            even = !even;
        }
    }
    Some(Spot {
        latitude: (latitude.0 + latitude.1) / 2.0,
        longitude: (longitude.0 + longitude.1) / 2.0,
    })
}

/// Reads a `geo:` URI (RFC 5870): `geo:<lat>,<long>[,<alt>][;params]`. The
/// `geo:0,0?q=<lat>,<long>(Label)` form Android writes names its spot in the
/// query instead.
fn geo_uri(url: &reqwest::Url) -> Option<Spot> {
    let coordinates = url.path().split(';').next().unwrap_or_default();
    let spot = two(coordinates)?;
    let asked = url
        .query_pairs()
        .find(|(key, _)| key == "q")
        .and_then(|(_, value)| pair(value.split('(').next().unwrap_or_default()));
    match asked {
        Some(asked) if spot.latitude == 0.0 && spot.longitude == 0.0 => Some(asked),
        _ => Some(spot),
    }
}

/// Reads the `!3d<latitude>!4d<longitude>` markers a Google Maps link carries.
fn marked_place(text: &str) -> Option<Spot> {
    let latitude = number(between(text, "!3d", "!")?)?;
    let longitude = number(between(text, "!4d", "!")?)?;
    Some(Spot {
        latitude,
        longitude,
    })
}

/// The text between `start` and the next `end`, or to the end of the text.
fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let (_, rest) = text.split_once(start)?;
    Some(rest.split_once(end).map_or(rest, |(head, _)| head))
}

/// Reads `<latitude>/<longitude>` out of an OpenStreetMap `#map=zoom/lat/long`
/// fragment.
fn openstreetmap_position(fragment: &str) -> Option<Spot> {
    let (_, rest) = fragment.strip_prefix("map=")?.split_once('/')?;
    let rest = rest.split('&').next().unwrap_or(rest);
    let mut parts = rest.split('/');
    joined(parts.next()?, parts.next()?)
}

/// Reads a spot out of a link's query: one of the service's `keys` naming the
/// coordinates, or OpenStreetMap's `mlat` and `mlon` pair.
fn query_spot(url: &reqwest::Url, keys: &[&str]) -> Option<Spot> {
    let mut latitude = None;
    let mut longitude = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "mlat" => latitude = number(&value),
            "mlon" => longitude = number(&value),
            key if keys.contains(&key) => {
                if let Some(spot) = pair(&value) {
                    return Some(spot);
                }
            }
            _ => {}
        }
    }
    Some(Spot {
        latitude: latitude?,
        longitude: longitude?,
    })
}

/// Reads the first two comma-separated numbers of a chunk, which is how a
/// link's own `latitude,longitude,zoom` carries them.
fn two(chunk: &str) -> Option<Spot> {
    let mut parts = chunk.split(',');
    let latitude = number(parts.next()?)?;
    let longitude = number(parts.next()?)?;
    Some(Spot {
        latitude,
        longitude,
    })
}

/// Reads two numbers written as coordinates: `-23.55 -46.63`, `-23.55, -46.63`
/// or `-23,5505,-46,6333`. A comma is the decimal mark in much of the world and
/// the separator in the rest, so a text with several commas is read only when
/// one reading alone is possible.
fn pair(text: &str) -> Option<Spot> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    match tokens.as_slice() {
        [latitude, longitude] => joined(latitude.strip_suffix(',').unwrap_or(latitude), longitude),
        [single] => {
            let mut readings = single
                .char_indices()
                .filter(|(_, character)| *character == ',')
                .filter_map(|(index, _)| joined(&single[..index], &single[index + 1..]));
            let first = readings.next()?;
            readings.all(|other| other == first).then_some(first)
        }
        _ => None,
    }
}

/// Reads two numbers that are already separate apart from their own spacing.
fn joined(latitude: &str, longitude: &str) -> Option<Spot> {
    Some(Spot {
        latitude: number(latitude)?,
        longitude: number(longitude)?,
    })
}

/// Reads one number, with a dot or a comma as the decimal mark. `1.234,56` is
/// read the way it is written in much of the world: the dot groups thousands.
/// A comma after the number is a separator, as in `-23.55, -46.63`.
fn number(token: &str) -> Option<f64> {
    let token = token.trim();
    match token.strip_suffix(',') {
        Some(bare) => parse(bare).or_else(|| parse(token)),
        None => parse(token),
    }
}

fn parse(token: &str) -> Option<f64> {
    if token.is_empty() {
        return None;
    }
    let negative = token.starts_with('-');
    let digits = token.strip_prefix(['-', '+']).unwrap_or(token);
    let normalized = match (digits.contains('.'), digits.contains(',')) {
        (true, true) => digits.replace('.', "").replace(',', "."),
        (false, true) => digits.replace(',', "."),
        _ => digits.to_owned(),
    };
    if normalized.is_empty()
        || normalized.matches('.').count() > 1
        || !normalized
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.')
    {
        return None;
    }
    let value: f64 = normalized.parse().ok()?;
    value
        .is_finite()
        .then_some(if negative { -value } else { value })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spot_of(text: &str) -> (f64, f64) {
        let spot = spot(text).unwrap_or_else(|| panic!("{text} is a location"));
        (spot.latitude, spot.longitude)
    }

    #[test]
    fn coordinates_are_read_with_a_comma_or_a_space_between_them() {
        assert_eq!(spot_of("-23.5505, -46.6333"), (-23.5505, -46.6333));
        assert_eq!(spot_of("-23.5505 -46.6333"), (-23.5505, -46.6333));
        assert_eq!(spot_of("51.5074,  -0.1278"), (51.5074, -0.1278));
        assert_eq!(spot_of(" -23.5505, -46.6333 "), (-23.5505, -46.6333));
        assert_eq!(spot_of("52.5200,13.4050"), (52.52, 13.405));
    }

    /// Much of the world writes the decimal mark as a comma, which collides
    /// with the separator: the reading has to survive both.
    #[test]
    fn decimal_commas_are_read_when_only_one_reading_is_possible() {
        assert_eq!(spot_of("-23,5505,-46,6333"), (-23.5505, -46.6333));
        assert_eq!(spot_of("-23,5505 -46,6333"), (-23.5505, -46.6333));
        // The dot groups thousands into a pair no position can be, so the
        // pair is read and then refused: `number` is where that reading is
        // checked.
        assert_eq!(spot("1.234,5, 8.765,4"), None);
        // Two different readings fit, so neither is taken.
        assert_eq!(spot("12,5,7"), None);
        // One reading fits here, the middle comma separating the pair, and the
        // dialog shows the spot it read before anything goes out.
        assert_eq!(spot_of("1,2,3,4"), (1.2, 3.4));
    }

    #[test]
    fn the_pair_has_to_be_a_position_on_the_earth() {
        assert_eq!(spot("91.0, 0.0"), None);
        assert_eq!(spot("-91.0, 0.0"), None);
        assert_eq!(spot("0.0, 181.0"), None);
        assert!(spot("-0.0, -180.0").is_some());
        assert!(spot("90.0, 180.0").is_some());
        assert_eq!(spot("nan, 12.0"), None);
        assert_eq!(spot("inf, 12.0"), None);
    }

    #[test]
    fn text_that_is_no_pair_of_numbers_is_refused() {
        assert_eq!(spot(""), None);
        assert_eq!(spot("   "), None);
        assert_eq!(spot("somewhere near the office"), None);
        assert_eq!(spot("-23.5505"), None);
        assert_eq!(spot("12 34 56"), None);
        assert_eq!(spot("--23.5505, -46.6333"), None);
    }

    #[test]
    fn google_maps_links_are_read_from_the_place_markers() {
        assert_eq!(
            spot_of(
                "https://www.google.com/maps/place/Sao+Paulo/data=!4m2!3m1!1s0x0!3d-23.5505!4d-46.6333"
            ),
            (-23.5505, -46.6333)
        );
        // The viewport around the marker is the fallback when there is no marker.
        assert_eq!(
            spot_of("https://www.google.com/maps/@-23.5505,-46.6333,15z"),
            (-23.5505, -46.6333)
        );
        assert_eq!(
            spot_of("https://maps.google.com/?q=-23.5505,-46.6333"),
            (-23.5505, -46.6333)
        );
        assert_eq!(
            spot_of("https://www.google.com/maps/dir/?api=1&destination=-23.5505,-46.6333"),
            (-23.5505, -46.6333)
        );
    }

    #[test]
    fn apple_and_openstreetmap_links_are_read() {
        assert_eq!(
            spot_of("https://maps.apple.com/?ll=-23.5505,-46.6333&q=Office"),
            (-23.5505, -46.6333)
        );
        assert_eq!(
            spot_of(
                "https://www.openstreetmap.org/?mlat=-23.5505&mlon=-46.6333#map=16/-23.55/-46.63"
            ),
            (-23.5505, -46.6333)
        );
        assert_eq!(
            spot_of("https://www.openstreetmap.org/#map=15/-23.5505/-46.6333"),
            (-23.5505, -46.6333)
        );
        assert_eq!(spot_of("geo:-23.5505,-46.6333"), (-23.5505, -46.6333));
    }

    #[test]
    fn geo_uris_may_carry_an_altitude_parameters_and_a_query() {
        assert_eq!(spot_of("geo:-23.5505,-46.6333;u=40"), (-23.5505, -46.6333));
        assert_eq!(
            spot_of("geo:-23.5505,-46.6333,760;crs=wgs84;u=40"),
            (-23.5505, -46.6333)
        );
        assert_eq!(
            spot_of("geo:0,0?q=-23.5505,-46.6333(Office)"),
            (-23.5505, -46.6333)
        );
        assert_eq!(spot_of("geo:0,0"), (0.0, 0.0));
        assert_eq!(spot("geo:;u=40"), None);
    }

    #[test]
    fn regional_google_hosts_are_read_only_on_their_maps_pages() {
        assert_eq!(
            spot_of("https://www.google.com.br/maps/@-23.5505,-46.6333,15z"),
            (-23.5505, -46.6333)
        );
        assert_eq!(
            spot_of("https://maps.google.co.uk/?q=51.5074,-0.1278"),
            (51.5074, -0.1278)
        );
        assert_eq!(
            spot("https://www.google.com/search?q=-23.5505,-46.6333"),
            None
        );
        assert_eq!(spot("https://google.example.net/maps?q=1,2"), None);
    }

    /// The `q=12,34` convention belongs to the map services: a link to any
    /// other site is not read as a spot.
    #[test]
    fn links_to_other_sites_are_refused() {
        assert_eq!(spot("https://example.com/?q=12,34"), None);
        assert_eq!(spot("https://example.com/@12,34,15z"), None);
        assert_eq!(spot("https://example.com/?mlat=12&mlon=34"), None);
        assert_eq!(spot("https://example.com/#map=15/12/34"), None);
        assert_eq!(spot("ftp://www.openstreetmap.org/?mlat=12&mlon=34"), None);
    }

    /// ZFPlus: links from other map services name their spot too.
    #[test]
    fn other_map_services_are_read() {
        let near = |text: &str, latitude: f64, longitude: f64| {
            let spot = spot(text).unwrap_or_else(|| panic!("{text} was not read"));
            assert!(
                (spot.latitude - latitude).abs() < 1e-3
                    && (spot.longitude - longitude).abs() < 1e-3,
                "{text}: {spot:?}"
            );
        };
        near(
            "https://waze.com/ul?ll=-6.9175,107.6191&navigate=yes",
            -6.9175,
            107.6191,
        );
        near(
            "https://www.waze.com/live-map/directions?to=ll.-6.9175%2C107.6191",
            -6.9175,
            107.6191,
        );
        near(
            "https://www.bing.com/maps?cp=-6.9175~107.6191&lvl=16",
            -6.9175,
            107.6191,
        );
        near(
            "https://wego.here.com/?map=-6.9175,107.6191,16",
            -6.9175,
            107.6191,
        );
        near(
            "https://share.here.com/l/-6.9175,107.6191",
            -6.9175,
            107.6191,
        );
        near(
            "https://yandex.com/maps/?ll=107.6191%2C-6.9175&z=16",
            -6.9175,
            107.6191,
        );
        // The geohash `qqguxyz` lies in Jakarta.
        let jakarta = spot("https://waze.com/ul/hqqguxyz").expect("a geohash");
        assert!(
            (jakarta.latitude + 6.2025).abs() < 0.01 && (jakarta.longitude - 106.8743).abs() < 0.01,
            "{jakarta:?}"
        );
        assert!(needs_lookup("https://goo.gl/maps/abc") && !needs_lookup("-6.9, 107.6"));
    }

    /// ZFPlus: the map page a short link ends on names its spot in a place
    /// marker or in its start-up state.
    #[test]
    fn a_map_page_names_its_spot() {
        let marked =
            "<a href=\"https://www.google.com/maps/place/X/data=!4m2!3m1!1s0x0!3d-6.2!4d106.8\">";
        let spot = super::page_spot(marked).unwrap();
        assert!((spot.latitude + 6.2).abs() < 1e-9 && (spot.longitude - 106.8).abs() < 1e-9);
        let centred = "x;window.APP_INITIALIZATION_STATE=[[[1234.5,106.81,-6.21],[0,0,0]";
        let spot = super::page_spot(centred).unwrap();
        assert!((spot.latitude + 6.21).abs() < 1e-9 && (spot.longitude - 106.81).abs() < 1e-9);
        assert!(super::page_spot("<html>nothing here</html>").is_none());
    }

    /// A short link names its spot only on the server it redirects through,
    /// so it is recognised, and not read as a spot by itself.
    #[test]
    fn short_links_are_recognised_and_not_read() {
        for link in [
            "https://maps.app.goo.gl/AbCdEf123",
            "https://goo.gl/maps/AbCdEf123",
            "https://g.co/kgs/AbCdEf",
        ] {
            assert!(is_short_link(link), "{link}");
            assert_eq!(spot(link), None, "{link}");
        }
        assert!(!is_short_link(
            "https://www.google.com/maps/@-23.5505,-46.6333,15z"
        ));
        assert!(!is_short_link("-23.5505, -46.6333"));
    }

    #[test]
    fn a_link_without_a_spot_in_it_is_refused() {
        assert_eq!(spot("https://www.google.com/maps/place/Sao+Paulo"), None);
        assert_eq!(spot("https://example.com/"), None);
        assert_eq!(spot("https://www.openstreetmap.org/way/123"), None);
    }

    /// The link the card opens is one the reader can paste back in.
    #[test]
    fn the_map_link_round_trips_through_the_reader() {
        let here = Spot {
            latitude: -23.5505,
            longitude: -46.6333,
        };
        assert_eq!(spot(&map_url(here)), Some(here));
        assert_eq!(here.text(), "-23.55050, -46.63330");
    }

    #[test]
    fn one_number_carries_a_sign_and_one_decimal_mark() {
        assert_eq!(number("  -12.5 "), Some(-12.5));
        assert_eq!(number("+12.5"), Some(12.5));
        assert_eq!(number("-12,5"), Some(-12.5));
        assert_eq!(number("-12.5,"), Some(-12.5));
        assert_eq!(number("12,"), Some(12.0));
        assert_eq!(number("12.5.5"), None);
        // A dot groups thousands, as it is written in much of the world.
        assert_eq!(number("1.234,5"), Some(1234.5));
        assert_eq!(number("1.234"), Some(1.234));
        assert_eq!(number("-"), None);
        assert_eq!(number(""), None);
        assert_eq!(number("1e3"), None);
    }
}
