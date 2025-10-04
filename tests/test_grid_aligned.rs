/// Tests for grid-aligned compositing optimization
/// These tests verify the detection logic for grid-aligned layouts,
/// which allows for optimized SRC-mode compositing instead of slower OVER mode.

use grim_rs::Box;

#[test]
fn test_box_no_overlap() {
    // Two boxes side by side (no overlap) - grid-aligned
    let box1 = Box::new(0, 0, 100, 100);
    let box2 = Box::new(100, 0, 100, 100);
    
    assert!(!box1.intersects(&box2), "Adjacent boxes should not intersect");
}

#[test]
fn test_box_with_overlap() {
    // Two boxes with overlap - NOT grid-aligned
    let box1 = Box::new(0, 0, 100, 100);
    let box2 = Box::new(50, 50, 100, 100);
    
    assert!(box1.intersects(&box2), "Overlapping boxes should intersect");
    
    let intersection = box1.intersection(&box2).unwrap();
    assert_eq!(intersection.x, 50);
    assert_eq!(intersection.y, 50);
    assert_eq!(intersection.width, 50);
    assert_eq!(intersection.height, 50);
}

#[test]
fn test_grid_aligned_horizontal_layout() {
    // Two monitors side by side horizontally: [1920x1080] [1920x1080]
    let box1 = Box::new(0, 0, 1920, 1080);
    let box2 = Box::new(1920, 0, 1920, 1080);
    
    assert!(!box1.intersects(&box2), "Horizontal layout should not overlap");
}

#[test]
fn test_grid_aligned_vertical_layout() {
    // Two monitors stacked vertically
    let box1 = Box::new(0, 0, 1920, 1080);
    let box2 = Box::new(0, 1080, 1920, 1080);
    
    assert!(!box1.intersects(&box2), "Vertical layout should not overlap");
}

#[test]
fn test_grid_aligned_l_shape_layout() {
    // L-shaped layout (common in multi-monitor setups)
    // [1920x1080]
    // [1920x1080][1920x1080]
    let box1 = Box::new(0, 0, 1920, 1080);      // Top
    let box2 = Box::new(0, 1080, 1920, 1080);   // Bottom-left  
    let box3 = Box::new(1920, 1080, 1920, 1080); // Bottom-right
    
    assert!(!box1.intersects(&box2), "Top and bottom-left should not overlap");
    assert!(!box1.intersects(&box3), "Top and bottom-right should not overlap");
    assert!(!box2.intersects(&box3), "Bottom monitors should not overlap");
}

#[test]
fn test_non_grid_aligned_overlapping_monitors() {
    let box1 = Box::new(0, 0, 1920, 1080);
    let box2 = Box::new(1800, 0, 1920, 1080); // 120px overlap
    
    assert!(box1.intersects(&box2), "Overlapping monitors should intersect");
}

#[test]
fn test_grid_aligned_triple_monitor() {
    let box_a = Box::new(0, 0, 1920, 1080);
    let box_b = Box::new(1920, 0, 1920, 1080);
    let box_c = Box::new(3840, 0, 1920, 1080);
    
    assert!(!box_a.intersects(&box_b), "Monitor A and B should not overlap");
    assert!(!box_b.intersects(&box_c), "Monitor B and C should not overlap");
    assert!(!box_a.intersects(&box_c), "Monitor A and C should not overlap");
}

#[test]
fn test_grid_aligned_different_sizes() {
    // Different size monitors but still grid-aligned
    // [2560x1440] [1920x1080]
    let box1 = Box::new(0, 0, 2560, 1440);
    let box2 = Box::new(2560, 0, 1920, 1080);
    
    assert!(!box1.intersects(&box2), "Different size monitors can be grid-aligned");
}

#[test]
fn test_region_intersection_within_output() {
    let output = Box::new(0, 0, 1920, 1080);
    let region = Box::new(100, 100, 800, 600);
    
    assert!(output.intersects(&region), "Region should be within output");
    
    let intersection = output.intersection(&region).unwrap();
    assert_eq!(intersection, region, "Intersection should equal the region");
}

#[test]
fn test_region_spanning_multiple_outputs() {
    let output1 = Box::new(0, 0, 1920, 1080);
    let output2 = Box::new(1920, 0, 1920, 1080);
    let region = Box::new(1800, 400, 240, 280);
    
    assert!(output1.intersects(&region), "Region should intersect first monitor");
    assert!(output2.intersects(&region), "Region should intersect second monitor");
    
    let int1 = output1.intersection(&region).unwrap();
    let int2 = output2.intersection(&region).unwrap();
    
    assert_eq!(int1.x, 1800);
    assert_eq!(int1.width, 120);
    
    assert_eq!(int2.x, 1920);
    assert_eq!(int2.width, 120);
    
    assert_eq!(int1.width + int2.width, region.width);
}

#[test]
fn test_pixel_alignment_check() {
    let box1 = Box::new(0, 0, 1920, 1080);
    let box2 = Box::new(1920, 0, 1920, 1080);
    
    assert_eq!(box1.x + box1.width, 1920);
    assert_eq!(box2.x, 1920);
    
    assert!(!box1.intersects(&box2));
}

#[test]
fn test_empty_box_no_intersection() {
    let box1 = Box::new(0, 0, 100, 100);
    let box2 = Box::new(0, 0, 0, 0); // Empty box
    
    assert!(!box1.intersects(&box2), "Empty box should not intersect");
    assert!(box2.is_empty(), "Box with zero dimensions should be empty");
}
