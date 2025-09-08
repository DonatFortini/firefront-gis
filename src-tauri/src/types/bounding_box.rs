use geo::Geometry;
use geo_types::Error as GeoError;
use geo_types::{Coord, Polygon};
use serde::{Deserialize, Serialize};
use wkt::{ToWkt, Wkt};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Copy)]
pub struct BoundingBox {
    pub xmin: f64,
    pub ymin: f64,
    pub xmax: f64,
    pub ymax: f64,
}

impl BoundingBox {
    pub fn new(xmin: f64, ymin: f64, xmax: f64, ymax: f64) -> Self {
        BoundingBox {
            xmin,
            ymin,
            xmax,
            ymax,
        }
    }

    pub fn width(&self) -> f64 {
        self.xmax - self.xmin
    }

    pub fn height(&self) -> f64 {
        self.ymax - self.ymin
    }

    pub fn to_wkt(&self) -> Wkt<f64> {
        let coords = vec![
            Coord {
                x: self.xmin,
                y: self.ymin,
            },
            Coord {
                x: self.xmax,
                y: self.ymin,
            },
            Coord {
                x: self.xmax,
                y: self.ymax,
            },
            Coord {
                x: self.xmin,
                y: self.ymax,
            },
            Coord {
                x: self.xmin,
                y: self.ymin,
            },
        ];

        let polygon = Polygon::new(coords.into(), vec![]);
        polygon.to_wkt()
    }

    pub fn to_geometry(&self) -> Result<Geometry, GeoError> {
        let coords = vec![
            Coord {
                x: self.xmin,
                y: self.ymin,
            },
            Coord {
                x: self.xmax,
                y: self.ymin,
            },
            Coord {
                x: self.xmax,
                y: self.ymax,
            },
            Coord {
                x: self.xmin,
                y: self.ymax,
            },
            Coord {
                x: self.xmin,
                y: self.ymin,
            },
        ];

        let polygon = Polygon::new(coords.into(), vec![]);
        Ok(Geometry::from(polygon))
    }

    pub fn intersects(&self, other: &BoundingBox) -> bool {
        self.xmin < other.xmax
            && self.xmax > other.xmin
            && self.ymin < other.ymax
            && self.ymax > other.ymin
    }

    pub fn contains(&self, other: &BoundingBox) -> bool {
        self.xmin <= other.xmin
            && self.xmax >= other.xmax
            && self.ymin <= other.ymin
            && self.ymax >= other.ymax
    }
}

pub mod prelude {
    pub use super::BoundingBox;
}
