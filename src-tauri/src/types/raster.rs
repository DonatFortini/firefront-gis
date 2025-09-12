#[derive(Debug, Clone)]
pub struct RasterBand {
    pub index: usize,
    pub data_type: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub common_name: Option<String>,
    pub data: Option<RasterData<f64>>,
}

impl RasterBand {
    pub fn new(index: usize, data_type: String) -> Self {
        RasterBand {
            index,
            data_type,
            name: None,
            description: None,
            common_name: None,
            data: None,
        }
    }
}
#[derive(Debug, Clone)]
pub struct RasterData<T> {
    data: Vec<T>,
    size: (usize, usize),
}

impl<T> RasterData<T> {
    pub fn new(data: Vec<T>, size: (usize, usize)) -> Self {
        RasterData { data, size }
    }

    pub fn data(&self) -> &[T] {
        &self.data
    }

    pub fn into_data(self) -> Vec<T> {
        self.data
    }

    pub fn size(&self) -> (usize, usize) {
        self.size
    }
}

pub mod prelude {
    pub use super::{RasterBand, RasterData};
}
