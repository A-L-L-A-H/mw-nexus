use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MTIME_PROFILE: &str = "antikythera-mtime-v1";
pub const MTIME_AUTHORITY_DISCLAIMER: &str = "Software-only astronomical and tabular Hijri approximation; strictly NOT a religious authority, moonsighting verdict, or legal/jurisdictional calendar ruling.";

const UNIX_EPOCH_JULIAN_DAY: f64 = 2_440_587.5;
const SECONDS_PER_DAY: f64 = 86_400.0;
const J2000_JULIAN_DAY: f64 = 2_451_545.0;
const HIJRI_TABULAR_EPOCH_JDN: i64 = 1_948_440;

const HIJRI_MONTH_NAMES: [&str; 12] = [
    "Muharram",
    "Safar",
    "Rabi' al-Awwal",
    "Rabi' al-Thani",
    "Jumada al-Ula",
    "Jumada al-Akhirah",
    "Rajab",
    "Sha'ban",
    "Ramadan",
    "Shawwal",
    "Dhu al-Qi'dah",
    "Dhu al-Hijjah",
];

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MTimeError {
    #[error("invalid timestamp '{input}': expected RFC3339 datetime, Unix seconds, or 'now'")]
    InvalidTimestamp { input: String },

    #[error("unix timestamp {unix_secs} is out of representable UTC range")]
    OutOfRangeTimestamp { unix_secs: i64 },

    #[error("invalid julian day value: {value}")]
    InvalidJulianDay { value: String },

    #[error("invalid tabular Hijri date {year:04}-{month:02}-{day:02}: {reason}")]
    InvalidHijriDate {
        year: i32,
        month: u8,
        day: u8,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TabularHijriDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub month_name: String,
    pub formatted: String,
    pub scheme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MTimeConversion {
    pub profile: String,
    pub religious_authority: bool,
    pub authority_disclaimer: String,
    pub utc_rfc3339: String,
    pub unix_timestamp: i64,
    pub julian_day: f64,
    pub hijri_tabular: TabularHijriDate,
    pub solar_longitude_deg: f64,
}

/// Converts a UTC `DateTime` into an Antikythera M-Time software-only temporal snapshot.
pub fn convert_datetime(dt: DateTime<Utc>) -> MTimeConversion {
    let unix_secs = dt.timestamp();
    let subsec_nanos = f64::from(dt.timestamp_subsec_nanos());
    let exact_unix = unix_secs as f64 + (subsec_nanos / 1_000_000_000.0);
    let julian_day = exact_unix / SECONDS_PER_DAY + UNIX_EPOCH_JULIAN_DAY;
    let hijri_tabular = julian_day_to_tabular_hijri(julian_day);
    let solar_longitude_deg = approximate_solar_longitude_deg(julian_day);

    MTimeConversion {
        profile: MTIME_PROFILE.to_owned(),
        religious_authority: false,
        authority_disclaimer: MTIME_AUTHORITY_DISCLAIMER.to_owned(),
        utc_rfc3339: dt.to_rfc3339(),
        unix_timestamp: unix_secs,
        julian_day,
        hijri_tabular,
        solar_longitude_deg,
    }
}

/// Converts a Unix timestamp (seconds since 1970-01-01T00:00:00Z) into M-Time.
pub fn convert_unix_timestamp(unix_secs: i64) -> Result<MTimeConversion, MTimeError> {
    let dt = DateTime::<Utc>::from_timestamp(unix_secs, 0)
        .ok_or(MTimeError::OutOfRangeTimestamp { unix_secs })?;
    Ok(convert_datetime(dt))
}

/// Parses an optional query parameter (`None`, `""`, `"now"`, Unix seconds, or RFC3339)
/// and returns its Antikythera M-Time conversion.
pub fn convert_ts_param(ts: Option<&str>) -> Result<MTimeConversion, MTimeError> {
    let raw = match ts {
        Some(value) => value.trim(),
        None => return Ok(convert_datetime(Utc::now())),
    };

    if raw.is_empty() || raw.eq_ignore_ascii_case("now") {
        return Ok(convert_datetime(Utc::now()));
    }

    if let Ok(unix_secs) = raw.parse::<i64>() {
        return convert_unix_timestamp(unix_secs);
    }

    let parsed = DateTime::parse_from_rfc3339(raw).map_err(|_| MTimeError::InvalidTimestamp {
        input: raw.to_owned(),
    })?;
    Ok(convert_datetime(parsed.with_timezone(&Utc)))
}

/// Converts an Astronomical Julian Day back to a UTC `DateTime`.
pub fn julian_day_to_utc(julian_day: f64) -> Result<DateTime<Utc>, MTimeError> {
    if !julian_day.is_finite() {
        return Err(MTimeError::InvalidJulianDay {
            value: julian_day.to_string(),
        });
    }
    let exact_unix = (julian_day - UNIX_EPOCH_JULIAN_DAY) * SECONDS_PER_DAY;
    let total_millis = (exact_unix * 1_000.0).round() as i128;
    let unix_secs = total_millis.div_euclid(1_000) as i64;
    let millis = total_millis.rem_euclid(1_000) as u32;
    DateTime::<Utc>::from_timestamp(unix_secs, millis * 1_000_000)
        .ok_or(MTimeError::OutOfRangeTimestamp { unix_secs })
}

/// Converts an Astronomical Julian Day to a Tabular Islamic (Kuwaiti/civil) Hijri date.
/// Purely arithmetic software calculation; never a religious authority.
pub fn julian_day_to_tabular_hijri(julian_day: f64) -> TabularHijriDate {
    let jdn = (julian_day + 0.5).floor() as i64;
    let l = jdn - HIJRI_TABULAR_EPOCH_JDN + 10_632;
    let n = (l - 1).div_euclid(10_631);
    let l = l - 10_631 * n + 354;
    let j = ((10_985 - l).div_euclid(5_316)) * ((50 * l).div_euclid(17_719))
        + (l.div_euclid(5_670)) * ((43 * l).div_euclid(15_238));
    let l = l
        - ((30 - j).div_euclid(15)) * ((17_719 * j).div_euclid(50))
        - (j.div_euclid(16)) * ((15_238 * j).div_euclid(43))
        + 29;
    let month = (24 * l).div_euclid(709) as u8;
    let day = (l - (709 * i64::from(month)).div_euclid(24)) as u8;
    let year = (30 * n + j - 30) as i32;

    let month_idx = usize::from(month.saturating_sub(1)).min(HIJRI_MONTH_NAMES.len() - 1);
    let month_name = HIJRI_MONTH_NAMES[month_idx].to_owned();
    let formatted = format!("{year:04}-{month:02}-{day:02}");

    TabularHijriDate {
        year,
        month,
        day,
        month_name,
        formatted,
        scheme: "kuwaiti-tabular-civil".to_owned(),
    }
}

/// Converts a Tabular Islamic (Kuwaiti/civil) Hijri date to the Astronomical Julian Day
/// at 00:00:00 UTC of that civil day.
pub fn tabular_hijri_to_julian_day(year: i32, month: u8, day: u8) -> Result<f64, MTimeError> {
    if !(1..=12).contains(&month) {
        return Err(MTimeError::InvalidHijriDate {
            year,
            month,
            day,
            reason: "month must be in 1..=12".to_owned(),
        });
    }
    if !(1..=30).contains(&day) {
        return Err(MTimeError::InvalidHijriDate {
            year,
            month,
            day,
            reason: "day must be in 1..=30".to_owned(),
        });
    }

    let y = i64::from(year);
    let m = i64::from(month);
    let d = i64::from(day);
    let jdn = (11 * y + 3).div_euclid(30) + 354 * y + 30 * m - (m - 1).div_euclid(2) + d
        + HIJRI_TABULAR_EPOCH_JDN
        - 385;
    Ok(jdn as f64 - 0.5)
}

/// Low-precision USNO/Meeus apparent ecliptic solar longitude approximation in degrees `[0.0, 360.0)`.
pub fn approximate_solar_longitude_deg(julian_day: f64) -> f64 {
    let days_since_j2000 = julian_day - J2000_JULIAN_DAY;
    let mean_longitude = (280.460 + 0.985_647_4 * days_since_j2000).rem_euclid(360.0);
    let mean_anomaly_rad = (357.528 + 0.985_600_3 * days_since_j2000)
        .rem_euclid(360.0)
        .to_radians();
    let ecliptic_longitude = mean_longitude
        + 1.915 * mean_anomaly_rad.sin()
        + 0.020 * (2.0 * mean_anomaly_rad).sin();
    ecliptic_longitude.rem_euclid(360.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn j2000_epoch_converts_accurately_and_never_claims_religious_authority() {
        let converted = convert_ts_param(Some("2000-01-01T12:00:00Z")).unwrap();
        assert_eq!(converted.profile, "antikythera-mtime-v1");
        assert!(!converted.religious_authority);
        assert_eq!(converted.unix_timestamp, 946_728_000);
        assert!((converted.julian_day - 2_451_545.0).abs() < 1e-6);
        assert!((0.0..360.0).contains(&converted.solar_longitude_deg));
        // Around winter solstice + 11 days: solar longitude ~280.4 deg
        assert!((converted.solar_longitude_deg - 280.4).abs() < 1.0);
    }

    #[test]
    fn unix_and_julian_day_roundtrip() {
        let original_unix = 1_700_000_000_i64;
        let converted = convert_unix_timestamp(original_unix).unwrap();
        assert!(!converted.religious_authority);
        let roundtrip = julian_day_to_utc(converted.julian_day).unwrap();
        assert_eq!(roundtrip.timestamp(), original_unix);
    }

    #[test]
    fn tabular_hijri_roundtrip_matches_epoch_and_modern_date() {
        let epoch_jd = tabular_hijri_to_julian_day(1, 1, 1).unwrap();
        let epoch_hijri = julian_day_to_tabular_hijri(epoch_jd);
        assert_eq!(epoch_hijri.year, 1);
        assert_eq!(epoch_hijri.month, 1);
        assert_eq!(epoch_hijri.day, 1);
        assert_eq!(epoch_hijri.month_name, "Muharram");

        let modern_jd = tabular_hijri_to_julian_day(1446, 9, 1).unwrap();
        let modern_hijri = julian_day_to_tabular_hijri(modern_jd);
        assert_eq!(modern_hijri.year, 1446);
        assert_eq!(modern_hijri.month, 9);
        assert_eq!(modern_hijri.day, 1);
        assert_eq!(modern_hijri.month_name, "Ramadan");
    }

    #[test]
    fn now_and_invalid_timestamp_handling() {
        let now_res = convert_ts_param(Some("now")).unwrap();
        assert!(!now_res.religious_authority);
        assert_eq!(now_res.profile, MTIME_PROFILE);

        let err = convert_ts_param(Some("not-a-date")).unwrap_err();
        assert!(matches!(err, MTimeError::InvalidTimestamp { .. }));
    }
}
