use std::fmt;

#[derive(Debug, Clone, Copy)]
pub struct Box {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Box {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    pub fn intersects(&self, other: &Box) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }

        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);

        x2 > x1 && y2 > y1
    }

    pub fn intersection(&self, other: &Box) -> Option<Box> {
        if !self.intersects(other) {
            return None;
        }

        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);

        Some(Box::new(x1, y1, x2 - x1, y2 - y1))
    }
}

impl fmt::Display for Box {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{} {}x{}", self.x, self.y, self.width, self.height)
    }
}

impl std::str::FromStr for Box {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(' ').collect();
        if parts.len() != 2 {
            return Err(crate::Error::InvalidGeometry(s.to_string()));
        }

        let xy: Vec<&str> = parts[0].split(',').collect();
        let wh: Vec<&str> = parts[1].split('x').collect();
        
        if xy.len() != 2 || wh.len() != 2 {
            return Err(crate::Error::InvalidGeometry(s.to_string()));
        }

        let x = xy[0].parse().map_err(|_| crate::Error::InvalidGeometry(s.to_string()))?;
        let y = xy[1].parse().map_err(|_| crate::Error::InvalidGeometry(s.to_string()))?;
        let width = wh[0].parse().map_err(|_| crate::Error::InvalidGeometry(s.to_string()))?;
        let height = wh[1].parse().map_err(|_| crate::Error::InvalidGeometry(s.to_string()))?;

        Ok(Box::new(x, y, width, height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_parsing() {
        let box_str = "10,20 300x400";
        let parsed: Box = box_str.parse().unwrap();
        assert_eq!(parsed.x, 10);
        assert_eq!(parsed.y, 20);
        assert_eq!(parsed.width, 300);
        assert_eq!(parsed.height, 400);
    }

    #[test]
    fn test_box_intersection() {
        let box1 = Box::new(0, 0, 100, 100);
        let box2 = Box::new(50, 50, 100, 100);
        
        assert!(box1.intersects(&box2));
        
        let intersection = box1.intersection(&box2).unwrap();
        assert_eq!(intersection.x, 50);
        assert_eq!(intersection.y, 50);
        assert_eq!(intersection.width, 50);
        assert_eq!(intersection.height, 50);
    }
}