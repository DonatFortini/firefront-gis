use crate::utils::{create_directory_if_not_exists, get_project_bounding_box, projects_dir};
use image::{DynamicImage, GenericImageView};

pub async fn slice_images(project_name: &str, slice_factor: u32) -> Result<(), String> {
    let projects_dir_path = projects_dir();
    let project_folder = projects_dir_path.to_str().unwrap();
    let project_path = format!("{project_folder}/{project_name}/");
    let slice_path = format!("{project_folder}/{project_name}/slices/");

    create_directory_if_not_exists(&slice_path)
        .map_err(|e| format!("Failed to create slice directory: {e}"))?;

    let veget_image = load_image(&format!("{project_path}{project_name}_VEGET.jpeg"), "VEGET")?;
    let ortho_image = load_image(&format!("{project_path}{project_name}_ORTHO.jpeg"), "ORTHO")?;

    let project_coordinates = get_project_bounding_box(project_name).await.unwrap();
    let (base_x, base_y) = (
        (project_coordinates.xmin / 1000.0) as u32,
        (project_coordinates.ymin / 1000.0) as u32,
    );

    slice_and_process_images(
        &veget_image,
        &ortho_image,
        &slice_path,
        slice_factor,
        base_x,
        base_y,
    )
    .await
}

fn load_image(image_path: &str, image_type: &str) -> Result<DynamicImage, String> {
    image::ImageReader::open(image_path)
        .map_err(|e| format!("Failed to open {image_type} image: {e}"))?
        .decode()
        .map_err(|e| format!("Failed to decode {image_type} image: {e}"))
}

async fn slice_and_process_images(
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
            )
            .await?;
        }
    }

    Ok(())
}

async fn save_and_process_slice(
    cropped_veget: &DynamicImage,
    cropped_ortho: &DynamicImage,
    slice_path: &str,
    coord_x: u32,
    coord_y: u32,
    slice_factor: u32,
) -> Result<(), String> {
    let veget_path = format!("{slice_path}/{coord_x}_{coord_y}_veget_{slice_factor}.jpg");
    let ortho_path = format!("{slice_path}/{coord_x}_{coord_y}_{slice_factor}.jpg");

    cropped_veget
        .save(&veget_path)
        .map_err(|e| format!("Failed to save VEGET slice: {e}"))?;
    cropped_ortho
        .save(&ortho_path)
        .map_err(|e| format!("Failed to save ORTHO slice: {e}"))?;

    Ok(())
}

pub mod prelude {
    pub use super::slice_images;
}
