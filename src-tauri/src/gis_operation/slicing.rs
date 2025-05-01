use crate::utils::{create_directory_if_not_exists, get_project_bounding_box, projects_dir};
use image::{DynamicImage, GenericImageView};
use std::fs;
use std::process::Command;

pub fn slice_images(project_name: &str, slice_factor: u32) -> Result<(), String> {
    let projects_dir_path = projects_dir();
    let project_folder = projects_dir_path.to_str().unwrap();
    let project_path = format!("{}/{}/", project_folder, project_name);
    let slice_path = format!("{}/{}/slices/", project_folder, project_name);

    prepare_directories(&slice_path)?;

    let veget_image_path = format!("{}{}_VEGET.jpeg", project_path, project_name);
    let ortho_image_path = format!("{}{}_ORTHO.jpeg", project_path, project_name);

    let veget_image = load_image(&veget_image_path, "VEGET")?;
    let ortho_image = load_image(&ortho_image_path, "ORTHO")?;

    let project_coordinates = get_project_bounding_box(project_name)?;
    let (base_x, base_y) =
        calculate_base_coordinates(project_coordinates.xmin, project_coordinates.ymin);

    slice_and_process_images(
        &veget_image,
        &ortho_image,
        &slice_path,
        slice_factor,
        base_x,
        base_y,
    )?;

    Ok(())
}

fn prepare_directories(slice_path: &str) -> Result<(), String> {
    fs::remove_dir_all(slice_path).map_err(|e| format!("Failed to remove directory: {}", e))?;
    create_directory_if_not_exists(slice_path)
        .map_err(|e| format!("Failed to create directory: {}", e))?;
    Ok(())
}

fn load_image(image_path: &str, image_type: &str) -> Result<DynamicImage, String> {
    image::ImageReader::open(image_path)
        .map_err(|e| format!("Failed to open {} image: {}", image_type, e))?
        .decode()
        .map_err(|e| format!("Failed to decode {} image: {}", image_type, e))
}

fn calculate_base_coordinates(xmin: f64, ymin: f64) -> (u32, u32) {
    let base_x = (xmin / 1000.0) as u32;
    let base_y = (ymin / 1000.0) as u32;
    (base_x, base_y)
}

fn slice_and_process_images(
    veget_image: &DynamicImage,
    ortho_image: &DynamicImage,
    slice_path: &str,
    slice_factor: u32,
    base_x: u32,
    base_y: u32,
) -> Result<(), String> {
    let (width, height) = veget_image.dimensions();

    for img_y in (0..height).step_by(slice_factor as usize).rev() {
        for img_x in (0..width).step_by(slice_factor as usize) {
            if img_x + slice_factor > width || img_y + slice_factor > height {
                continue;
            }

            let cropped_veget = veget_image.crop_imm(img_x, img_y, slice_factor, slice_factor);
            let cropped_ortho = ortho_image.crop_imm(img_x, img_y, slice_factor, slice_factor);

            let coord_x = base_x + img_x / 100;
            let coord_y = base_y + (height - img_y - slice_factor) / 100;

            save_and_process_slice(
                &cropped_veget,
                &cropped_ortho,
                slice_path,
                coord_x,
                coord_y,
                slice_factor,
            )?;
        }
    }

    Ok(())
}

fn save_and_process_slice(
    cropped_veget: &DynamicImage,
    cropped_ortho: &DynamicImage,
    slice_path: &str,
    coord_x: u32,
    coord_y: u32,
    slice_factor: u32,
) -> Result<(), String> {
    let veget_path = format!(
        "{}/{}_{}_veget_{}.jpg",
        slice_path, coord_x, coord_y, slice_factor
    );

    let ortho_path = format!(
        "{}/{}_{}_{}.jpg",
        slice_path, coord_x, coord_y, slice_factor
    );

    cropped_veget
        .save(&veget_path)
        .map_err(|e| format!("Failed to save VEGET slice: {}", e))?;

    cropped_ortho
        .save(&ortho_path)
        .map_err(|e| format!("Failed to save ORTHO slice: {}", e))?;

    process_with_imagemagick(&veget_path, "VEGET")?;
    process_with_imagemagick(&ortho_path, "ORTHO")?;

    Ok(())
}

fn process_with_imagemagick(image_path: &str, image_type: &str) -> Result<(), String> {
    Command::new("magick")
        .args(["convert", image_path, "-enhance", image_path])
        .output()
        .map_err(|e| {
            format!(
                "Failed to process {} slice with ImageMagick: {}",
                image_type, e
            )
        })?;
    Ok(())
}
