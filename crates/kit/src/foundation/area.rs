//! A rectangle in a surface's pixels: where something is drawn, for the
//! pointer to find it and the frost to follow it.

/// A rectangle in a surface's pixels, from its top-left corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Area {
    /// The left edge.
    pub x: f32,
    /// The top edge.
    pub y: f32,
    /// How wide it is.
    pub width: f32,
    /// How tall it is.
    pub height: f32,
}

impl Area {
    /// The rectangle at `x`, `y`, `width` by `height`.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Whether the point `x`, `y` is inside: the top and left edges are,
    /// the bottom and right aren't.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    /// The middle.
    pub fn centre(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// The part of `self` inside `other`, if any.
    pub fn clipped(&self, other: &Area) -> Option<Area> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);
        (right > x && bottom > y).then_some(Area::new(x, y, right - x, bottom - y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_and_overlaps() {
        let a = Area::new(0.0, 0.0, 10.0, 10.0);
        assert!(a.contains(0.0, 0.0));
        assert!(!a.contains(10.0, 5.0));
        assert_eq!(a.centre(), (5.0, 5.0));
        let b = Area::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.clipped(&b), Some(Area::new(5.0, 5.0, 5.0, 5.0)));
        assert_eq!(a.clipped(&Area::new(20.0, 0.0, 1.0, 1.0)), None);
    }
}
