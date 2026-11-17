//! Weather from Open-Meteo (open-meteo.com): request URLs, response parsing and sky codes.
//! The HTTP call itself lives in the app.

use serde::Deserialize;

use crate::settings::TempUnit;

/// Open-Meteo's data is licensed CC BY 4.0, which asks for this credit.
pub const ATTRIBUTION: &str = "Weather data by Open-Meteo.com (CC BY 4.0)";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sky {
    Clear,
    PartlyCloudy,
    Cloudy,
    Fog,
    Drizzle,
    Rain,
    Snow,
    Storm,
    Unknown,
}

impl Sky {
    /// Maps a WMO weather interpretation code, as Open-Meteo returns it.
    pub fn from_wmo(code: u32) -> Sky {
        match code {
            0 | 1 => Sky::Clear,
            2 => Sky::PartlyCloudy,
            3 => Sky::Cloudy,
            45 | 48 => Sky::Fog,
            51..=57 => Sky::Drizzle,
            61..=67 | 80..=82 => Sky::Rain,
            71..=77 | 85 | 86 => Sky::Snow,
            95..=99 => Sky::Storm,
            _ => Sky::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Sky::Clear => "Clear",
            Sky::PartlyCloudy => "Partly cloudy",
            Sky::Cloudy => "Cloudy",
            Sky::Fog => "Fog",
            Sky::Drizzle => "Drizzle",
            Sky::Rain => "Rain",
            Sky::Snow => "Snow",
            Sky::Storm => "Thunderstorm",
            Sky::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Current {
    pub temperature: f32,
    pub sky: Sky,
    pub is_day: bool,
    /// Today first, then the following days.
    pub days: Vec<DayForecast>,
    /// Local `HH:MM` of today's sunrise and sunset.
    pub sunrise: Option<String>,
    pub sunset: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DayForecast {
    /// `YYYY-MM-DD`, local to the place.
    pub date: String,
    pub max: f32,
    pub min: f32,
    pub sky: Sky,
}

/// Percent-encodes a query value (RFC 3986 unreserved characters pass through).
pub fn encode_query(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn geocoding_url(city: &str) -> String {
    format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&format=json",
        encode_query(city.trim())
    )
}

pub fn forecast_url(latitude: f64, longitude: f64, unit: TempUnit) -> String {
    let unit = match unit {
        TempUnit::Celsius => "celsius",
        TempUnit::Fahrenheit => "fahrenheit",
    };
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={latitude:.4}&longitude={longitude:.4}\
         &current=temperature_2m,weather_code,is_day\
         &daily=weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset\
         &forecast_days=4&temperature_unit={unit}&timezone=auto"
    )
}

#[derive(Deserialize)]
struct GeoResponse {
    #[serde(default)]
    results: Vec<GeoResult>,
}

#[derive(Deserialize)]
struct GeoResult {
    name: String,
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    admin1: Option<String>,
    #[serde(default)]
    country: Option<String>,
}

/// The first place a geocoding response found, or `None` when it found nothing usable.
pub fn parse_geocoding(json: &str) -> Option<Place> {
    let resp: GeoResponse = serde_json::from_str(json).ok()?;
    let r = resp.results.into_iter().next()?;
    if !(-90.0..=90.0).contains(&r.latitude) || !(-180.0..=180.0).contains(&r.longitude) {
        return None;
    }
    let mut name = r.name;
    for part in [r.admin1, r.country].into_iter().flatten() {
        if !part.is_empty() && part != name {
            name.push_str(", ");
            name.push_str(&part);
        }
    }
    Some(Place {
        name,
        latitude: r.latitude,
        longitude: r.longitude,
    })
}

#[derive(Deserialize)]
struct ForecastResponse {
    current: ForecastCurrent,
    #[serde(default)]
    daily: Option<ForecastDaily>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ForecastDaily {
    time: Vec<String>,
    weather_code: Vec<Option<u32>>,
    temperature_2m_max: Vec<Option<f32>>,
    temperature_2m_min: Vec<Option<f32>>,
    sunrise: Vec<Option<String>>,
    sunset: Vec<Option<String>>,
}

/// `2026-11-17T07:21` → `07:21`.
fn clock_part(iso: &str) -> Option<String> {
    iso.split_once('T')
        .map(|(_, t)| t.chars().take(5).collect())
        .filter(|t: &String| t.len() == 5)
}

fn daily_forecasts(d: &ForecastDaily) -> Vec<DayForecast> {
    d.time
        .iter()
        .enumerate()
        .take(7)
        .filter_map(|(i, date)| {
            let max = d.temperature_2m_max.get(i).copied().flatten()?;
            let min = d.temperature_2m_min.get(i).copied().flatten()?;
            let code = d.weather_code.get(i).copied().flatten()?;
            (max.is_finite() && min.is_finite()).then(|| DayForecast {
                date: date.clone(),
                max,
                min,
                sky: Sky::from_wmo(code),
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct ForecastCurrent {
    temperature_2m: f32,
    weather_code: u32,
    #[serde(default = "one")]
    is_day: u8,
}

fn one() -> u8 {
    1
}

pub fn parse_forecast(json: &str) -> Option<Current> {
    let resp: ForecastResponse = serde_json::from_str(json).ok()?;
    let c = resp.current;
    if !c.temperature_2m.is_finite() {
        return None;
    }
    let daily = resp.daily.unwrap_or_default();
    let today_clock =
        |list: &Vec<Option<String>>| list.first().cloned().flatten().and_then(|s| clock_part(&s));
    Some(Current {
        temperature: c.temperature_2m,
        sky: Sky::from_wmo(c.weather_code),
        is_day: c.is_day != 0,
        days: daily_forecasts(&daily),
        sunrise: today_clock(&daily.sunrise),
        sunset: today_clock(&daily.sunset),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_city_names() {
        assert_eq!(encode_query("Reggio Emilia"), "Reggio%20Emilia");
        assert_eq!(encode_query("Zürich"), "Z%C3%BCrich");
        assert!(geocoding_url("  Roma ").contains("name=Roma&"));
    }

    #[test]
    fn parses_geocoding() {
        let json = r#"{"results":[{"name":"Reggio Emilia","latitude":44.7,"longitude":10.63,"admin1":"Emilia-Romagna","country":"Italy"}]}"#;
        let p = parse_geocoding(json).unwrap();
        assert_eq!(p.name, "Reggio Emilia, Emilia-Romagna, Italy");
        assert!(parse_geocoding(r#"{"generationtime_ms":0.1}"#).is_none());
        assert!(parse_geocoding("not json").is_none());
    }

    #[test]
    fn parses_forecast() {
        let json = r#"{"current":{"time":"2026-11-17T12:00","temperature_2m":9.4,"weather_code":61,"is_day":1}}"#;
        let c = parse_forecast(json).unwrap();
        assert_eq!(c.sky, Sky::Rain);
        assert!(c.is_day);
        assert!((c.temperature - 9.4).abs() < 1e-6);
        assert!(c.days.is_empty());
        assert_eq!(c.sunrise, None);
    }

    #[test]
    fn parses_daily_forecast_and_skips_gaps() {
        let json = r#"{
            "current":{"temperature_2m":9.4,"weather_code":3,"is_day":0},
            "daily":{
                "time":["2026-11-17","2026-11-18","2026-11-19"],
                "weather_code":[3,61,null],
                "temperature_2m_max":[12.0,10.5,9.0],
                "temperature_2m_min":[4.0,5.5,3.0],
                "sunrise":["2026-11-17T07:21","2026-11-18T07:22","2026-11-19T07:23"],
                "sunset":["2026-11-17T16:45","2026-11-18T16:44","2026-11-19T16:43"]
            }
        }"#;
        let c = parse_forecast(json).unwrap();
        assert_eq!(c.days.len(), 2);
        assert_eq!(c.days[1].sky, Sky::Rain);
        assert_eq!(c.sunrise.as_deref(), Some("07:21"));
        assert_eq!(c.sunset.as_deref(), Some("16:45"));
        assert!(!c.is_day);
    }

    #[test]
    fn sky_codes() {
        assert_eq!(Sky::from_wmo(0), Sky::Clear);
        assert_eq!(Sky::from_wmo(95), Sky::Storm);
        assert_eq!(Sky::from_wmo(1234), Sky::Unknown);
    }
}
