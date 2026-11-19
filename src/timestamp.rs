//! Local date-time for timestamped screenshot filenames (chrono-free).
//!
//! `LocalDateTime::now()` resolves the system timezone natively
//! (`GetTimeZoneInformation` on Windows, `localtime_r` on Linux) and the
//! calendar conversion is plain arithmetic — no external dependencies.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDateTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl LocalDateTime {
    pub fn now() -> Self {
        let utc_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Self::from_unix_utc(utc_secs + local_offset_seconds(utc_secs))
    }

    /// Converts a non-negative Unix timestamp (seconds, already shifted to
    /// the desired zone) into civil date fields.
    /// Uses Howard Hinnant's `civil_from_days` algorithm.
    pub(crate) fn from_unix_utc(secs: i64) -> Self {
        let days = secs.div_euclid(86_400);
        let secs_of_day = secs.rem_euclid(86_400);
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = if month <= 2 { y + 1 } else { y };
        LocalDateTime {
            year: year as i32,
            month: month as u8,
            day: day as u8,
            hour: (secs_of_day / 3_600) as u8,
            minute: (secs_of_day / 60 % 60) as u8,
            second: (secs_of_day % 60) as u8,
        }
    }
}

/// `YYYYMMDD_HHhMMmSSs` — the grim-rs default filename timestamp.
impl fmt::Display for LocalDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}{:02}{:02}_{:02}h{:02}m{:02}s",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

#[cfg(windows)]
fn local_offset_seconds(_utc_secs: i64) -> i64 {
    use windows::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
    // winnt.h: TIME_ZONE_ID_UNKNOWN/STANDARD/DAYLIGHT. Deliberately literal —
    // pulling Win32_System_SystemServices just for these is not worth it.
    const TIME_ZONE_ID_DAYLIGHT: u32 = 2;
    const TIME_ZONE_ID_INVALID: u32 = u32::MAX;
    let mut tz = TIME_ZONE_INFORMATION::default();
    let id = unsafe { GetTimeZoneInformation(&mut tz) };
    if id == TIME_ZONE_ID_INVALID {
        return 0;
    }
    // Bias is minutes from local time to UTC (UTC = local + Bias), with an
    // extra DST bias while daylight saving is active. GetTimeZoneInformation
    // reports the values for the current moment, which is all `now()` needs.
    let active_bias = if id == TIME_ZONE_ID_DAYLIGHT {
        tz.DaylightBias
    } else {
        tz.StandardBias
    };
    -i64::from(tz.Bias + active_bias) * 60
}

#[cfg(target_os = "linux")]
fn local_offset_seconds(utc_secs: i64) -> i64 {
    let t = utc_secs as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::localtime_r(&t, &mut tm) };
    if rc.is_null() {
        0
    } else {
        tm.tm_gmtoff as i64
    }
}

#[cfg(test)]
mod tests {
    use super::LocalDateTime;
    fn fmt(secs: i64) -> String {
        LocalDateTime::from_unix_utc(secs).to_string()
    }

    #[test]
    fn epoch_is_1970_01_01() {
        assert_eq!(fmt(0), "19700101_00h00m00s");
    }

    #[test]
    fn known_dates() {
        assert_eq!(fmt(1_000_000_000), "20010909_01h46m40s");
        assert_eq!(fmt(1_704_067_200), "20240101_00h00m00s");
        assert_eq!(fmt(86_399), "19700101_23h59m59s");
        assert_eq!(fmt(86_400), "19700102_00h00m00s");
    }

    #[test]
    fn leap_day() {
        assert_eq!(fmt(951_782_400), "20000229_00h00m00s");
        // The day after 2000-02-29 is 2000-03-01.
        assert_eq!(fmt(951_868_800), "20000301_00h00m00s");
    }

    #[test]
    fn display_formats_components() {
        let dt = LocalDateTime {
            year: 2024,
            month: 3,
            day: 5,
            hour: 7,
            minute: 9,
            second: 11,
        };
        assert_eq!(dt.to_string(), "20240305_07h09m11s");
    }
}
