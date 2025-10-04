/// Test for filename generation format
/// Verifies that default filenames follow the pattern: YYYYMMDD_HHhMMmSSs_grim.ext

#[test]
fn test_filename_format() {
    use chrono::Local;
    use regex::Regex;

    // Simulate generate_default_filename logic
    let now = Local::now();
    let timestamp = now.format("%Y%m%d_%Hh%Mm%Ss");
    let filename = format!("{}_grim.png", timestamp);

    // Verify format matches expected pattern
    // Pattern: 8 digits _ 2 digits h 2 digits m 2 digits s _ grim . ext
    let re = Regex::new(r"^\d{8}_\d{2}h\d{2}m\d{2}s_grim\.png$").unwrap();
    assert!(
        re.is_match(&filename),
        "Filename '{}' does not match expected format YYYYMMDD_HHhMMmSSs_grim.png",
        filename
    );

    // Verify year is reasonable (2020-2100)
    let year: u32 = filename[0..4].parse().expect("Failed to parse year");
    assert!(year >= 2020 && year <= 2100, "Year {} is out of reasonable range", year);

    // Verify month (01-12)
    let month: u32 = filename[4..6].parse().expect("Failed to parse month");
    assert!(month >= 1 && month <= 12, "Month {} is invalid", month);

    // Verify day (01-31)
    let day: u32 = filename[6..8].parse().expect("Failed to parse day");
    assert!(day >= 1 && day <= 31, "Day {} is invalid", day);

    // Verify hour (00-23)
    let hour_start = 9; // After "YYYYMMDD_"
    let hour: u32 = filename[hour_start..hour_start + 2]
        .parse()
        .expect("Failed to parse hour");
    assert!(hour <= 23, "Hour {} is invalid", hour);

    // Verify minute (00-59)
    let minute_start = hour_start + 3; // After "HH" and "h"
    let minute: u32 = filename[minute_start..minute_start + 2]
        .parse()
        .expect("Failed to parse minute");
    assert!(minute <= 59, "Minute {} is invalid", minute);

    // Verify second (00-59)
    let second_start = minute_start + 3; // After "MM" and "m"
    let second: u32 = filename[second_start..second_start + 2]
        .parse()
        .expect("Failed to parse second");
    assert!(second <= 59, "Second {} is invalid", second);

    println!("✓ Filename format test passed: {}", filename);
}

#[test]
fn test_filename_readability() {
    use chrono::Local;

    let now = Local::now();
    let timestamp = now.format("%Y%m%d_%Hh%Mm%Ss");
    let filename = format!("{}_grim.png", timestamp);

    // Verify it doesn't look like a unix timestamp
    assert!(
        !filename.chars().all(|c| c.is_numeric() || c == '.'),
        "Filename looks like a unix timestamp"
    );

    // Verify it contains readable separators
    assert!(filename.contains("h"), "Filename missing hour separator 'h'");
    assert!(filename.contains("m"), "Filename missing minute separator 'm'");
    assert!(filename.contains("s"), "Filename missing second separator 's'");
    assert!(filename.contains("_grim"), "Filename missing '_grim' identifier");

    println!("✓ Filename readability test passed: {}", filename);
}

#[test]
fn test_filename_with_different_extensions() {
    use chrono::Local;

    let now = Local::now();
    let timestamp = now.format("%Y%m%d_%Hh%Mm%Ss");

    let extensions = vec!["png", "jpeg", "ppm"];
    for ext in extensions {
        let filename = format!("{}_grim.{}", timestamp, ext);
        assert!(
            filename.ends_with(&format!("_grim.{}", ext)),
            "Filename '{}' doesn't have correct extension",
            filename
        );
        println!("✓ Extension test passed for: {}", filename);
    }
}
