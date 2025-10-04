use grim_rs::{ Grim, CaptureParameters, Box, Result };

fn main() -> Result<()> {
    env_logger::init();

    let mut grim = Grim::new()?;

    // Get available outputs
    let outputs = grim.get_outputs()?;
    println!("Available outputs:");
    for output in &outputs {
        println!(
            "  {}: {}x{} at {},{}",
            output.name,
            output.geometry.width,
            output.geometry.height,
            output.geometry.x,
            output.geometry.y
        );
    }

    if outputs.is_empty() {
        println!("No outputs available");
        return Ok(());
    }

    // Capture multiple outputs with different parameters
    let mut parameters = Vec::new();

    // Capture the first output entirely
    parameters.push(CaptureParameters {
        output_name: outputs[0].name.clone(),
        region: None, // None means entire output
        overlay_cursor: true,
        scale: None,
    });

    // If we have more than one output, capture a region of the second one
    if outputs.len() > 1 {
        let second_output = &outputs[1];
        // Capture a region in the center of the second output
        let region_width = second_output.geometry.width / 2;
        let region_height = second_output.geometry.height / 2;
        let region_x = second_output.geometry.x + region_width / 2;
        let region_y = second_output.geometry.y + region_height / 2;

        parameters.push(CaptureParameters {
            output_name: second_output.name.clone(),
            region: Some(Box::new(region_x, region_y, region_width, region_height)),
            overlay_cursor: false,
            scale: None,
        });
    }

    // Perform the multi-output capture
    let results = grim.capture_outputs(parameters)?;

    println!("Captured {} outputs:", results.outputs.len());
    for (output_name, capture_result) in &results.outputs {
        println!("  {}: {}x{}", output_name, capture_result.width, capture_result.height);

        // Save each capture to a separate file
        let filename = format!("{}_capture.png", output_name.replace("/", "_"));
        grim.save_png(
            &capture_result.data,
            capture_result.width,
            capture_result.height,
            &filename
        )?;
        println!("    Saved to {}", filename);
    }

    Ok(())
}
