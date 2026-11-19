use grim_rs::{apply_image_transform, rotate_180, rotate_270, rotate_90, OutputTransform};

fn make_rgba_2x2() -> Vec<u8> {
    vec![0, 1, 2, 255, 3, 4, 5, 255, 6, 7, 8, 255, 9, 10, 11, 255]
}

#[test]
fn rotate_90_swaps_dimensions() {
    let src = make_rgba_2x2();
    let (result, w, h) = rotate_90(&src, 2, 2);
    assert_eq!(w, 2);
    assert_eq!(h, 2);
    // new(0,0) = old(1,0)
    assert_eq!(&result[0..4], &[6, 7, 8, 255]);
    // new(1,0) = old(0,0)
    assert_eq!(&result[4..8], &[0, 1, 2, 255]);
}

#[test]
fn rotate_180_keeps_dimensions() {
    let src = make_rgba_2x2();
    let (result, w, h) = rotate_180(&src, 2, 2);
    assert_eq!(w, 2);
    assert_eq!(h, 2);
    assert_eq!(&result[0..4], &[9, 10, 11, 255]);
    assert_eq!(&result[12..16], &[0, 1, 2, 255]);
}

#[test]
fn rotate_270_swaps_dimensions() {
    let src = make_rgba_2x2();
    let (result, w, h) = rotate_270(&src, 2, 2);
    assert_eq!(w, 2);
    assert_eq!(h, 2);
    assert_eq!(&result[0..4], &[3, 4, 5, 255]);
    assert_eq!(&result[4..8], &[9, 10, 11, 255]);
}

#[test]
fn rotate_identity_roundtrip() {
    let src = make_rgba_2x2();
    let (r90, w, h) = rotate_90(&src, 2, 2);
    let (r180, w2, h2) = rotate_90(&r90, w, h);
    let (r270, w3, h3) = rotate_90(&r180, w2, h2);
    let (r360, _, _) = rotate_90(&r270, w3, h3);
    assert_eq!(r360, src);
}

#[test]
fn rotate_non_square_image() {
    let src = vec![
        0, 1, 2, 255, 3, 4, 5, 255, 6, 7, 8, 255, 9, 10, 11, 255, 12, 13, 14, 255, 15, 16, 17, 255,
    ];
    let (result, w, h) = rotate_90(&src, 3, 2);
    assert_eq!(w, 2);
    assert_eq!(h, 3);
    let (restored, rw, rh) = rotate_270(&result, w, h);
    assert_eq!(rw, 3);
    assert_eq!(rh, 2);
    assert_eq!(restored, src);
}

#[test]
fn apply_transform_normal_is_identity() {
    let src = vec![10u8, 20, 30, 255, 40, 50, 60, 255];
    let (result, w, h) = apply_image_transform(&src, 2, 1, OutputTransform::Normal);
    assert_eq!(result, src);
    assert_eq!(w, 2);
    assert_eq!(h, 1);
}

#[test]
fn apply_transform_rotate_90_matches_direct() {
    let src = make_rgba_2x2();
    let (result, w, h) = apply_image_transform(&src, 2, 2, OutputTransform::Rotate90);
    let (expected, ew, eh) = rotate_90(&src, 2, 2);
    assert_eq!(w, ew);
    assert_eq!(h, eh);
    assert_eq!(result, expected);
}

#[test]
fn apply_transform_rotate_180_matches_direct() {
    let src = make_rgba_2x2();
    let (result, _w, _h) = apply_image_transform(&src, 2, 2, OutputTransform::Rotate180);
    let (expected, _, _) = rotate_180(&src, 2, 2);
    assert_eq!(result, expected);
}

#[test]
fn apply_transform_rotate_270_matches_direct() {
    let src = make_rgba_2x2();
    let (result, _w, _h) = apply_image_transform(&src, 2, 2, OutputTransform::Rotate270);
    let (expected, _, _) = rotate_270(&src, 2, 2);
    assert_eq!(result, expected);
}

fn transforms() -> Vec<OutputTransform> {
    #[allow(unused_mut)]
    let mut transforms = vec![
        OutputTransform::Normal,
        OutputTransform::Rotate90,
        OutputTransform::Rotate180,
        OutputTransform::Rotate270,
    ];
    #[cfg(target_os = "linux")]
    transforms.extend([
        OutputTransform::Flipped,
        OutputTransform::Flipped90,
        OutputTransform::Flipped180,
        OutputTransform::Flipped270,
    ]);
    transforms
}

#[test]
fn invalid_rgba_returns_empty_without_panicking() {
    for (data, width, height) in [
        (&[0u8; 10][..], 100, 100),
        (&[0u8; 3][..], 1, 1),
        (&[][..], u32::MAX, u32::MAX),
        (&[][..], 1 << 30, 1),
        (&[][..], 134_217_729, 1),
    ] {
        for transform in transforms() {
            assert_eq!(
                apply_image_transform(data, width, height, transform),
                (Vec::new(), 0, 0),
                "{width}x{height}, {transform:?}"
            );
        }
        for rotate in [rotate_90, rotate_180, rotate_270] {
            assert_eq!(rotate(data, width, height), (Vec::new(), 0, 0));
        }
    }
}

#[test]
fn non_square_transforms_preserve_pixel_orientation_and_trailing_data_policy() {
    let mut src: Vec<u8> = (1..=6).flat_map(|id| [id, 10, 20, 255]).collect();
    src.extend([99, 98, 97]);
    let mut cases = vec![
        (OutputTransform::Rotate90, 2, 3, [4, 1, 5, 2, 6, 3]),
        (OutputTransform::Rotate180, 3, 2, [6, 5, 4, 3, 2, 1]),
        (OutputTransform::Rotate270, 2, 3, [3, 6, 2, 5, 1, 4]),
    ];
    #[cfg(target_os = "linux")]
    cases.extend([
        (OutputTransform::Flipped, 3, 2, [3, 2, 1, 6, 5, 4]),
        (OutputTransform::Flipped90, 2, 3, [6, 3, 5, 2, 4, 1]),
        (OutputTransform::Flipped180, 3, 2, [4, 5, 6, 1, 2, 3]),
        (OutputTransform::Flipped270, 2, 3, [1, 4, 2, 5, 3, 6]),
    ]);
    for (transform, width, height, ids) in cases.drain(..) {
        let expected: Vec<u8> = ids.into_iter().flat_map(|id| [id, 10, 20, 255]).collect();
        assert_eq!(
            apply_image_transform(&src, 3, 2, transform),
            (expected, width, height)
        );
    }
    assert_eq!(
        apply_image_transform(&src, 3, 2, OutputTransform::Normal),
        (src, 3, 2)
    );
}

#[test]
fn zero_area_with_extreme_dimensions_does_not_iterate_or_allocate() {
    for transform in transforms() {
        for (width, height) in [(0, u32::MAX), (u32::MAX, 0), (0, 0)] {
            let (data, w, h) = apply_image_transform(&[], width, height, transform);
            assert!(data.is_empty());
            let swapped = matches!(
                transform,
                OutputTransform::Rotate90 | OutputTransform::Rotate270
            );
            #[cfg(target_os = "linux")]
            let swapped = swapped
                || matches!(
                    transform,
                    OutputTransform::Flipped90 | OutputTransform::Flipped270
                );
            assert_eq!(
                (w, h),
                if swapped {
                    (height, width)
                } else {
                    (width, height)
                }
            );
        }
    }
    assert_eq!(rotate_90(&[], 0, u32::MAX), (Vec::new(), u32::MAX, 0));
    assert_eq!(rotate_180(&[], u32::MAX, 0), (Vec::new(), u32::MAX, 0));
    assert_eq!(rotate_270(&[], u32::MAX, 0), (Vec::new(), 0, u32::MAX));
}
