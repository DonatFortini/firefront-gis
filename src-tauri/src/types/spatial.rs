use geo::Geometry;
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

impl Default for BoundingBox {
    fn default() -> Self {
        Self {
            xmin: 0.0,
            ymin: 0.0,
            xmax: 1.0,
            ymax: 1.0,
        }
    }
}

impl BoundingBox {
    pub fn new(xmin: f64, ymin: f64, xmax: f64, ymax: f64) -> Self {
        Self {
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

    pub fn area(&self) -> f64 {
        self.width() * self.height()
    }

    fn to_polygon_coords(self) -> Vec<Coord<f64>> {
        vec![
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
        ]
    }

    pub fn to_wkt(&self) -> Wkt<f64> {
        let polygon = Polygon::new(self.to_polygon_coords().into(), vec![]);
        polygon.to_wkt()
    }

    pub fn to_geometry(&self) -> Geometry<f64> {
        let polygon = Polygon::new(self.to_polygon_coords().into(), vec![]);
        Geometry::from(polygon)
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

    pub fn union(&self, other: &BoundingBox) -> BoundingBox {
        BoundingBox::new(
            self.xmin.min(other.xmin),
            self.ymin.min(other.ymin),
            self.xmax.max(other.xmax),
            self.ymax.max(other.ymax),
        )
    }

    pub fn intersection(&self, other: &BoundingBox) -> Option<BoundingBox> {
        if !self.intersects(other) {
            return None;
        }

        Some(BoundingBox::new(
            self.xmin.max(other.xmin),
            self.ymin.max(other.ymin),
            self.xmax.min(other.xmax),
            self.ymax.min(other.ymax),
        ))
    }
}

pub mod prelude {
    pub use super::BoundingBox;
}
