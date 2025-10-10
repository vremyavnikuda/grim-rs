use grim_rs::geometry::Box;
use proptest::prelude::*;

proptest! {
    #[test]
    fn box_getters_match_construction(x in -10000i32..10000, y in -10000i32..10000,
                                       w in 0i32..10000, h in 0i32..10000) {
        let b = Box::new(x, y, w, h);
        prop_assert_eq!(b.x(), x);
        prop_assert_eq!(b.y(), y);
        prop_assert_eq!(b.width(), w);
        prop_assert_eq!(b.height(), h);
    }

    #[test]
    fn box_is_empty_iff_zero_area(x in -1000i32..1000, y in -1000i32..1000,
                                   w in -100i32..100, h in -100i32..100) {
        let b = Box::new(x, y, w, h);
        let expected_empty = w <= 0 || h <= 0;
        prop_assert_eq!(b.is_empty(), expected_empty);
    }

    #[test]
    fn intersection_is_commutative(
        x1 in -1000i32..1000, y1 in -1000i32..1000, w1 in 1i32..500, h1 in 1i32..500,
        x2 in -1000i32..1000, y2 in -1000i32..1000, w2 in 1i32..500, h2 in 1i32..500
    ) {
        let box1 = Box::new(x1, y1, w1, h1);
        let box2 = Box::new(x2, y2, w2, h2);

        let int1 = box1.intersection(&box2);
        let int2 = box2.intersection(&box1);

        prop_assert_eq!(int1, int2, "Intersection should be commutative");
    }

    #[test]
    fn intersects_is_commutative(
        x1 in -1000i32..1000, y1 in -1000i32..1000, w1 in 1i32..500, h1 in 1i32..500,
        x2 in -1000i32..1000, y2 in -1000i32..1000, w2 in 1i32..500, h2 in 1i32..500
    ) {
        let box1 = Box::new(x1, y1, w1, h1);
        let box2 = Box::new(x2, y2, w2, h2);

        prop_assert_eq!(
            box1.intersects(&box2),
            box2.intersects(&box1),
            "intersects() should be commutative"
        );
    }

    #[test]
    fn intersection_exists_iff_intersects(
        x1 in -1000i32..1000, y1 in -1000i32..1000, w1 in 1i32..500, h1 in 1i32..500,
        x2 in -1000i32..1000, y2 in -1000i32..1000, w2 in 1i32..500, h2 in 1i32..500
    ) {
        let box1 = Box::new(x1, y1, w1, h1);
        let box2 = Box::new(x2, y2, w2, h2);

        let has_intersection = box1.intersection(&box2).is_some();
        let intersects = box1.intersects(&box2);

        prop_assert_eq!(
            has_intersection,
            intersects,
            "intersection() should return Some iff intersects() is true"
        );
    }

    #[test]
    fn intersection_is_subset_of_both(
        x1 in -1000i32..1000, y1 in -1000i32..1000, w1 in 1i32..500, h1 in 1i32..500,
        x2 in -1000i32..1000, y2 in -1000i32..1000, w2 in 1i32..500, h2 in 1i32..500
    ) {
        let box1 = Box::new(x1, y1, w1, h1);
        let box2 = Box::new(x2, y2, w2, h2);

        if let Some(intersection) = box1.intersection(&box2) {
            prop_assert!(
                intersection.x() >= box1.x() &&
                intersection.y() >= box1.y() &&
                intersection.x() + intersection.width() <= box1.x() + box1.width() &&
                intersection.y() + intersection.height() <= box1.y() + box1.height(),
                "Intersection should be within box1 bounds"
            );

            prop_assert!(
                intersection.x() >= box2.x() &&
                intersection.y() >= box2.y() &&
                intersection.x() + intersection.width() <= box2.x() + box2.width() &&
                intersection.y() + intersection.height() <= box2.y() + box2.height(),
                "Intersection should be within box2 bounds"
            );
        }
    }

    #[test]
    fn box_with_self_returns_self(x in -1000i32..1000, y in -1000i32..1000,
                                   w in 1i32..500, h in 1i32..500) {
        let b = Box::new(x, y, w, h);
        let intersection = b.intersection(&b);

        prop_assert_eq!(intersection, Some(b), "Box intersected with itself should return itself");
    }

    #[test]
    fn empty_boxes_dont_intersect(
        x1 in -1000i32..1000, y1 in -1000i32..1000,
        x2 in -1000i32..1000, y2 in -1000i32..1000
    ) {
        let empty1 = Box::new(x1, y1, 0, 0);
        let empty2 = Box::new(x2, y2, 0, 100);
        let empty3 = Box::new(x2, y2, 100, 0);

        prop_assert!(!empty1.intersects(&empty2), "Empty boxes should not intersect");
        prop_assert!(!empty2.intersects(&empty3), "Empty boxes should not intersect");
        prop_assert_eq!(empty1.intersection(&empty2), None);
        prop_assert_eq!(empty2.intersection(&empty3), None);
    }

    #[test]
    fn parse_and_display_roundtrip(x in -1000i32..1000, y in -1000i32..1000,
                                     w in 0i32..500, h in 0i32..500) {
        let original = Box::new(x, y, w, h);
        let serialized = original.to_string();
        let parsed: Box = serialized.parse().unwrap();

        prop_assert_eq!(parsed, original, "Parsing should be inverse of Display");
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn non_overlapping_boxes_dont_intersect() {
        let box1 = Box::new(0, 0, 10, 10);
        let box2 = Box::new(20, 20, 10, 10);

        assert!(!box1.intersects(&box2));
        assert_eq!(box1.intersection(&box2), None);
    }

    #[test]
    fn adjacent_boxes_dont_intersect() {
        let box1 = Box::new(0, 0, 10, 10);
        let box2 = Box::new(10, 0, 10, 10);

        assert!(!box1.intersects(&box2));
        assert_eq!(box1.intersection(&box2), None);
    }

    #[test]
    fn contained_box_intersection_is_smaller_box() {
        let outer = Box::new(0, 0, 100, 100);
        let inner = Box::new(25, 25, 50, 50);

        assert!(outer.intersects(&inner));
        let intersection = outer.intersection(&inner).unwrap();
        assert_eq!(intersection, inner);
    }

    #[test]
    fn negative_dimensions_are_empty() {
        let box1 = Box::new(0, 0, -10, 10);
        let box2 = Box::new(0, 0, 10, -10);
        let box3 = Box::new(0, 0, -10, -10);

        assert!(box1.is_empty());
        assert!(box2.is_empty());
        assert!(box3.is_empty());
    }
}
