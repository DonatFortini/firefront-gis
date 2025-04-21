use crate::utils::create_directory_if_not_exists;
use image::GenericImageView;
use serde_json::Value;
use std::process::Command;

pub fn slice_images(project_name: &str, slice_factor: u32) -> Result<(), String> {
    let project_path = format!("projects/{}/", project_name);
    let slice_path = format!("projects/{}/slices/", project_name);
    create_directory_if_not_exists(&slice_path)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let veget_image_path = format!("{}{}_VEGET.jpeg", project_path, project_name);
    let ortho_image_path = format!("{}{}_ORTHO.jpeg", project_path, project_name);

    let project_coordinates = get_project_bounding_box(project_name)?;
    let lower_left = project_coordinates.lower_left;

    let veget_image = image::ImageReader::open(&veget_image_path)
        .map_err(|e| format!("Failed to open VEGET image: {}", e))?
        .decode()
        .map_err(|e| format!("Failed to decode VEGET image: {}", e))?;

    let ortho_image = image::ImageReader::open(&ortho_image_path)
        .map_err(|e| format!("Failed to open ORTHO image: {}", e))?
        .decode()
        .map_err(|e| format!("Failed to decode ORTHO image: {}", e))?;

    let (width, height) = veget_image.dimensions();

    let base_x = (lower_left.0 / 1000.0) as u32;
    let base_y = (lower_left.1 / 1000.0) as u32;

    for img_y in (0..height).step_by(slice_factor as usize).rev() {
        for img_x in (0..width).step_by(slice_factor as usize) {
            if img_x + slice_factor > width || img_y + slice_factor > height {
                continue;
            }

            let cropped_veget = veget_image.crop_imm(img_x, img_y, slice_factor, slice_factor);
            let cropped_ortho = ortho_image.crop_imm(img_x, img_y, slice_factor, slice_factor);

            let coord_x = base_x + img_x / 100;
            let coord_y = base_y + (height - img_y - slice_factor) / 100;

            let veget_path = format!(
                "{}/{}_{}_veget_{}.jpeg",
                slice_path, coord_x, coord_y, slice_factor
            );

            let ortho_path = format!(
                "{}/{}_{}_{}.jpeg",
                slice_path, coord_x, coord_y, slice_factor
            );

            cropped_veget
                .save(&veget_path)
                .map_err(|e| format!("Failed to save VEGET slice: {}", e))?;

            cropped_ortho
                .save(&ortho_path)
                .map_err(|e| format!("Failed to save ORTHO slice: {}", e))?;
        }
    }

    Ok(())
}

pub struct Coordinates {
    pub upper_left: (f64, f64),
    pub lower_left: (f64, f64),
    pub upper_right: (f64, f64),
    pub lower_right: (f64, f64),
}

pub fn get_project_bounding_box(project_name: &str) -> Result<Coordinates, String> {
    let project_path = format!("projects/{}/", project_name);
    let output = Command::new("gdalinfo")
        .args([
            format!("{}{}.tiff", project_path, project_name),
            "-json".to_owned(),
        ])
        .output();

    let json_output: Value = serde_json::from_slice(&output.unwrap().stdout)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let corner_coordinates = json_output["cornerCoordinates"].as_object().unwrap();

    let coordinates = Coordinates {
        upper_left: (
            corner_coordinates["upperLeft"][0].as_f64().unwrap(),
            corner_coordinates["upperLeft"][1].as_f64().unwrap(),
        ),
        lower_left: (
            corner_coordinates["lowerLeft"][0].as_f64().unwrap(),
            corner_coordinates["lowerLeft"][1].as_f64().unwrap(),
        ),
        upper_right: (
            corner_coordinates["upperRight"][0].as_f64().unwrap(),
            corner_coordinates["upperRight"][1].as_f64().unwrap(),
        ),
        lower_right: (
            corner_coordinates["lowerRight"][0].as_f64().unwrap(),
            corner_coordinates["lowerRight"][1].as_f64().unwrap(),
        ),
    };

    Ok(coordinates)
}
