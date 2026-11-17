#[cfg(target_os = "windows")]
mod windows_cursor_blend {
    use grim_rs::{blend_cursor_rgba, blend_mono_cursor_rgba};

    fn make_frame(w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
        let mut buf = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                buf[i] = r;
                buf[i + 1] = g;
                buf[i + 2] = b;
                buf[i + 3] = a;
            }
        }
        buf
    }

    fn make_cursor(w: u32, h: u32, b: u8, g: u8, r: u8, a: u8) -> Vec<u8> {
        let mut buf = vec![0u8; (w * h * 4) as usize];
        for i in (0..buf.len()).step_by(4) {
            buf[i] = b;
            buf[i + 1] = g;
            buf[i + 2] = r;
            buf[i + 3] = a;
        }
        buf
    }

    #[test]
    fn opaque_cursor_overwrites_frame_pixel() {
        let mut frame = make_frame(10, 10, 0, 0, 0, 255);
        let cursor = make_cursor(2, 2, 50, 100, 200, 255);
        let pitch = 2 * 4;
        blend_cursor_rgba(&mut frame, (10, 10), &cursor, (2, 2), pitch, (0, 0));
        let i = 0;
        assert_eq!(frame[i], 200);
        assert_eq!(frame[i + 1], 100);
        assert_eq!(frame[i + 2], 50);
        assert_eq!(frame[i + 3], 255);
    }

    #[test]
    fn transparent_cursor_leaves_frame_unchanged() {
        let mut frame = make_frame(10, 10, 100, 150, 200, 255);
        let original = frame.clone();
        let cursor = make_cursor(4, 4, 50, 60, 70, 0);
        let pitch = 4 * 4;
        blend_cursor_rgba(&mut frame, (10, 10), &cursor, (4, 4), pitch, (0, 0));
        assert_eq!(frame, original);
    }

    #[test]
    fn cursor_at_offset_blends_correct_pixels() {
        let mut frame = make_frame(10, 10, 10, 10, 10, 255);
        let cursor = make_cursor(1, 1, 100, 200, 255, 255);
        let pitch = 4;
        blend_cursor_rgba(&mut frame, (10, 10), &cursor, (1, 1), pitch, (5, 3));
        let i = ((3 * 10 + 5) * 4) as usize;
        assert_eq!(frame[i], 255);
        assert_eq!(frame[i + 1], 200);
        assert_eq!(frame[i + 2], 100);
        // pixel at (0,0) unchanged
        assert_eq!(frame[0], 10);
    }

    #[test]
    fn cursor_offscreen_left_clipped() {
        let mut frame = make_frame(10, 10, 50, 50, 50, 255);
        let cursor = make_cursor(6, 6, 100, 200, 255, 255);
        let pitch = 6 * 4;
        blend_cursor_rgba(&mut frame, (10, 10), &cursor, (6, 6), pitch, (-3, 0));
        let i = 0;
        assert_eq!(frame[i], 255);
        assert_eq!(frame[i + 1], 200);
        assert_eq!(frame[i + 2], 100);
        let j = (4 * 4) as usize;
        assert_eq!(frame[j], 50);
    }

    #[test]
    fn cursor_offscreen_top_clipped() {
        let mut frame = make_frame(10, 10, 50, 50, 50, 255);
        let cursor = make_cursor(4, 6, 100, 200, 255, 255);
        let pitch = 4 * 4;
        blend_cursor_rgba(&mut frame, (10, 10), &cursor, (4, 6), pitch, (0, -2));
        let top_i = 0usize;
        assert_eq!(frame[top_i], 255);
        assert_eq!(frame[top_i + 1], 200);
    }

    #[test]
    fn cursor_fully_offscreen_no_effect() {
        let mut frame = make_frame(10, 10, 50, 50, 50, 255);
        let original = frame.clone();
        let cursor = make_cursor(4, 4, 100, 200, 255, 255);
        let pitch = 4 * 4;
        blend_cursor_rgba(&mut frame, (10, 10), &cursor, (4, 4), pitch, (-10, -10));
        assert_eq!(frame, original);
    }

    #[test]
    fn cursor_with_stride_larger_than_width() {
        let mut frame = make_frame(8, 8, 0, 0, 0, 255);
        // row_pitch=16, width=3
        let mut cursor = vec![0u8; 16 * 3];
        for row in 0..3 {
            let base = row * 16;
            cursor[base] = 0;
            cursor[base + 1] = 100;
            cursor[base + 2] = 200;
            cursor[base + 3] = 255;
        }
        blend_cursor_rgba(&mut frame, (8, 8), &cursor, (3, 3), 16, (2, 2));
        let i = ((2 * 8 + 2) * 4) as usize;
        assert_eq!(frame[i], 200);
        assert_eq!(frame[i + 1], 100);
        assert_eq!(frame[i + 2], 0);
        assert_eq!(frame[i + 3], 255);
    }

    #[test]
    fn partial_alpha_cursor_blends_correctly() {
        let mut frame = make_frame(5, 5, 100, 100, 100, 255);
        let cursor = make_cursor(1, 1, 0, 0, 255, 128);
        let pitch = 4;
        blend_cursor_rgba(&mut frame, (5, 5), &cursor, (1, 1), pitch, (0, 0));
        let a = 128.0 / 255.0;
        let expected_r = (255.0 * a + 100.0 * (1.0 - a)) as u8;
        let expected_g = (0.0 * a + 100.0 * (1.0 - a)) as u8;
        assert_eq!(frame[0], expected_r);
        assert_eq!(frame[1], expected_g);
        assert_eq!(frame[3], 255);
    }

    #[test]
    fn mono_like_buffer_does_not_panic() {
        // A DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROMATIC buffer is 1bpp,
        // so its pitch is far smaller than width * 4 and the old indexing
        // ran past the end. Feeding such a buffer must not panic.
        let mut frame = make_frame(10, 10, 50, 50, 50, 255);
        let original = frame.clone();
        let mono_buf = vec![0xFFu8; 8]; // 8x4 cursor at 1bpp, pitch 1
        blend_cursor_rgba(&mut frame, (10, 10), &mono_buf, (8, 4), 1, (0, 0));
        assert_eq!(frame, original);
    }

    #[test]
    fn mono_like_buffer_with_large_pitch_does_not_panic() {
        let mut frame = make_frame(10, 10, 50, 50, 50, 255);
        let original = frame.clone();
        let mono_buf = vec![0xAAu8; 4]; // 32x4 cursor at 1bpp, pitch 1
        blend_cursor_rgba(&mut frame, (10, 10), &mono_buf, (32, 4), 1, (0, 0));
        assert_eq!(frame, original);
    }

    /// Build a 4x4 visible monochrome cursor whose four columns exercise the
    /// AND/XOR combinations: transparent, black, white, invert.
    fn make_mono_cursor() -> Vec<u8> {
        // bits are MSB-first per byte; only the top nibble is used (width 4)
        // AND rows: 0,0,1,1 -> 0x30 ; XOR rows: 0,1,0,1 -> 0x50
        let mut buf = vec![0x30u8; 4];
        buf.extend_from_slice(&[0x50u8; 4]);
        buf
    }

    #[test]
    fn mono_cursor_decodes_all_bit_combinations() {
        let mut frame = make_frame(4, 4, 100, 100, 100, 255);
        let cursor = make_mono_cursor();
        blend_mono_cursor_rgba(&mut frame, (4, 4), &cursor, (4, 4), 1, (0, 0));
        let px = |x: u32| (x * 4) as usize;
        // AND=0/XOR=0 -> opaque black
        assert_eq!(&frame[px(0)..px(0) + 4], &[0, 0, 0, 255]);
        // AND=0/XOR=1 -> opaque white
        assert_eq!(&frame[px(1)..px(1) + 4], &[255, 255, 255, 255]);
        // AND=1/XOR=0 -> transparent
        assert_eq!(&frame[px(2)..px(2) + 4], &[100, 100, 100, 255]);
        // AND=1/XOR=1 -> inverted frame pixel
        assert_eq!(&frame[px(3)..px(3) + 4], &[155, 155, 155, 255]);
    }

    #[test]
    fn mono_cursor_short_buffer_does_not_panic() {
        let mut frame = make_frame(4, 4, 100, 100, 100, 255);
        let original = frame.clone();
        let truncated = vec![0x30u8; 2]; // XOR mask missing entirely
        blend_mono_cursor_rgba(&mut frame, (4, 4), &truncated, (4, 4), 1, (0, 0));
        assert_eq!(frame, original);
    }

    #[test]
    fn mono_cursor_offset_blends_at_position() {
        let mut frame = make_frame(8, 8, 100, 100, 100, 255);
        let cursor = make_mono_cursor();
        blend_mono_cursor_rgba(&mut frame, (8, 8), &cursor, (4, 4), 1, (2, 3));
        // (0,0) untouched
        assert_eq!(&frame[0..4], &[100, 100, 100, 255]);
        // black pixel landed at (2,3)
        let i = ((3 * 8 + 2) * 4) as usize;
        assert_eq!(&frame[i..i + 4], &[0, 0, 0, 255]);
        // white pixel at (3,3)
        let j = ((3 * 8 + 3) * 4) as usize;
        assert_eq!(&frame[j..j + 4], &[255, 255, 255, 255]);
    }
}
