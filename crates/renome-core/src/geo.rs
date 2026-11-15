//! The place a GPS position is in: city, state and country, from
//! GeoNames' places of 500 people or more (CC BY 4.0), kept inside the
//! program (`data/places.tsv.gz`, made by `scripts/places.js`, which also
//! names a city's districts after it). Nothing is sent anywhere.

use std::collections::HashMap;
use std::io::Read;
use std::sync::LazyLock;

/// The language of a country's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    Pt,
    #[default]
    En,
    Es,
}

impl Lang {
    pub fn from_code(code: &str) -> Self {
        match code.get(..2).unwrap_or("") {
            "pt" => Lang::Pt,
            "es" => Lang::Es,
            _ => Lang::En,
        }
    }
}

/// Where a photo was taken.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Place {
    pub city: String,
    pub state: String,
    pub country: String,
}

struct Town {
    /// Degrees × 10 000.
    lat: i32,
    lon: i32,
    name: Box<str>,
    /// How many people live there (its city's, for a part of one).
    people: u32,
    /// Index in `Places::groups`.
    group: u32,
}

struct Places {
    towns: Vec<Town>,
    /// Country code and state (first-level division) name.
    groups: Vec<(Box<str>, Box<str>)>,
    /// Country names in Portuguese, English and Spanish, by code.
    countries: HashMap<Box<str>, [Box<str>; 3]>,
    /// The towns in each cell of one degree.
    grid: HashMap<(i16, i16), Vec<u32>>,
}

static DATA: &[u8] = include_bytes!("../data/places.tsv.gz");

static PLACES: LazyLock<Places> = LazyLock::new(|| {
    let mut text = String::new();
    let _ = flate2::read::GzDecoder::new(DATA).read_to_string(&mut text);
    let mut states: HashMap<&str, &str> = HashMap::new();
    let mut p = Places {
        towns: Vec::new(),
        groups: Vec::new(),
        countries: HashMap::new(),
        grid: HashMap::new(),
    };
    let (mut lat, mut lon) = (0i32, 0i32);
    for line in text.lines() {
        let mut f = line.split('\t');
        match f.next() {
            Some("C") => {
                let cc = f.next().unwrap_or_default();
                let mut n = || Box::<str>::from(f.next().unwrap_or(cc));
                let names = [n(), n(), n()];
                p.countries.insert(cc.into(), names);
            }
            Some("A") => {
                if let (Some(code), Some(name)) = (f.next(), f.next()) {
                    states.insert(code, name);
                }
            }
            Some("G") => {
                let cc = f.next().unwrap_or_default();
                let adm = f.next().unwrap_or_default();
                let state = states.get(format!("{cc}.{adm}").as_str()).copied();
                p.groups.push((cc.into(), state.unwrap_or_default().into()));
                (lat, lon) = (0, 0);
            }
            Some(dlat) => {
                let (Ok(dlat), Some(Ok(dlon)), Some(name)) = (
                    dlat.parse::<i32>(),
                    f.next().map(str::parse::<i32>),
                    f.next(),
                ) else {
                    continue;
                };
                let people = f.next().and_then(|n| n.parse().ok()).unwrap_or(0);
                (lat, lon) = (lat + dlat, lon + dlon);
                let id = u32::try_from(p.towns.len()).unwrap_or(u32::MAX);
                p.grid.entry(cell(lat, lon)).or_default().push(id);
                p.towns.push(Town {
                    lat,
                    lon,
                    name: name.into(),
                    people,
                    group: u32::try_from(p.groups.len().saturating_sub(1)).unwrap_or(0),
                });
            }
            None => {}
        }
    }
    p
});

fn cell(lat: i32, lon: i32) -> (i16, i16) {
    let deg = |v: i32| i16::try_from(v.div_euclid(10_000)).unwrap_or(0);
    (deg(lat), deg(lon))
}

/// The weight of a city of 40 million people.
const MAX_WEIGHT: f64 = 79.53;

/// Farthest a place is taken from (km): beyond it, open sea or desert.
const MAX_KM: f64 = 150.0;

/// The place `lat`, `lon` (degrees) is in, its country named in `lang`;
/// `None` far from any. Not simply the nearest: distances are weighed by
/// the fourth root of how many people live there, so a point in a big
/// city's outskirts goes to the city rather than to a small town next to
/// it, while a village's own centre still wins nearby.
pub fn nearest(lat: f64, lon: f64, lang: Lang) -> Option<Place> {
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    let p = &*PLACES;
    #[allow(clippy::cast_possible_truncation)]
    let (la, lo) = ((lat * 1e4).round() as i32, (lon * 1e4).round() as i32);
    let (cy, cx) = cell(la, lo);
    let cos = lat.to_radians().cos().max(0.01);
    // Squared distance in degrees, east-west shrunk by the latitude.
    let dist = |t: &Town| {
        let dy = f64::from(t.lat - la) / 1e4;
        let mut dx = (f64::from(t.lon - lo) / 1e4).abs();
        if dx > 180.0 {
            dx = 360.0 - dx;
        }
        dy * dy + (dx * cos) * (dx * cos)
    };
    let weight = |t: &Town| f64::from(t.people.max(500)).powf(0.25);
    let mut best: Option<(f64, &Town)> = None;
    // Rings of cells around the position's own, until one more ring
    // cannot hold anything nearer.
    for r in 0i16..=3 {
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                if (y - cy).abs() != r && (x - cx).abs() != r {
                    continue;
                }
                let x = if x < -180 { x + 360 } else if x >= 180 { x - 360 } else { x };
                for &i in p.grid.get(&(y, x)).into_iter().flatten() {
                    let t = &p.towns[i as usize];
                    let d = dist(t).sqrt() / weight(t);
                    if best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, t));
                    }
                }
            }
        }
        // Anything in the next ring is at least `r × cos` degrees away,
        // and weighs at most as the most populous place on Earth.
        if let Some((d, _)) = best
            && d <= f64::from(r) * cos / MAX_WEIGHT
        {
            break;
        }
    }
    let (_, town) = best?;
    if dist(town).sqrt() * 111.0 > MAX_KM {
        return None;
    }
    let (cc, state) = &p.groups[town.group as usize];
    let country = p.countries.get(cc).map_or_else(
        || cc.to_string(),
        |n| n[lang as usize].to_string(),
    );
    Some(Place {
        city: town.name.to_string(),
        state: state.to_string(),
        country,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_places() {
        let sp = nearest(-23.5489, -46.6388, Lang::Pt).unwrap();
        assert_eq!(sp.city, "São Paulo");
        assert_eq!(sp.state, "São Paulo");
        assert_eq!(sp.country, "Brasil");
        let paris = nearest(48.8584, 2.2945, Lang::En).unwrap();
        assert_eq!(paris.city, "Paris");
        assert_eq!(paris.country, "France");
        assert_eq!(nearest(40.4168, -3.7038, Lang::Es).unwrap().country, "España");
        // Across the date line.
        assert!(nearest(-17.7134, 178.065, Lang::En).is_some());
        // Mid-ocean.
        assert_eq!(nearest(0.0, -30.0, Lang::En), None);
        assert_eq!(nearest(200.0, 0.0, Lang::En), None);
    }
}
