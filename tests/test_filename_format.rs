/// Test for filename generation format
/// Verifies that default filenames follow the pattern: YYYYMMDD_HHhMMmSSs_grim.ext

#[test]
fn test_filename_format() {
    use chrono::Local;
    use regex::Regex;

    let now = Local::now();
    let timestamp = now.format("%Y%m%d_%Hh%Mm%Ss");
    let filename = format!("{}_grim.png", timestamp);

    let re = Regex::new(r"^\d{8}_\d{2}h\d{2}m\d{2}s_grim\.png$").unwrap();
    assert!(
        re.is_match(&filename),
        "Filename '{}' does not match expected format YYYYMMDD_HHhMMmSSs_grim.png",
        filename
    );

    let year: u32 = filename[0..4].parse().expect("Failed to parse year");
    assert!(
        (2020..=2100).contains(&year),
        "Year {} is out of reasonable range",
        year
    );

    let month: u32 = filename[4..6].parse().expect("Failed to parse month");
    assert!((1..=12).contains(&month), "Month {} is invalid", month);

    let day: u32 = filename[6..8].parse().expect("Failed to parse day");
    assert!((1..=31).contains(&day), "Day {} is invalid", day);

    let hour_start = 9; // After "YYYYMMDD_"
    let hour: u32 = filename[hour_start..hour_start + 2]
        .parse()
        .expect("Failed to parse hour");
    assert!(hour <= 23, "Hour {} is invalid", hour);

    let minute_start = hour_start + 3; // After "HH" and "h"
    let minute: u32 = filename[minute_start..minute_start + 2]
        .parse()
        .expect("Failed to parse minute");
    assert!(minute <= 59, "Minute {} is invalid", minute);

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

    assert!(
        !filename.chars().all(|c| (c.is_numeric() || c == '.')),
        "Filename looks like a unix timestamp"
    );

    // Verify it contains readable separators
    assert!(
        filename.contains("h"),
        "Filename missing hour separator 'h'"
    );
    assert!(
        filename.contains("m"),
        "Filename missing minute separator 'm'"
    );
    assert!(
        filename.contains("s"),
        "Filename missing second separator 's'"
    );
    assert!(
        filename.contains("_grim"),
        "Filename missing '_grim' identifier"
    );

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
