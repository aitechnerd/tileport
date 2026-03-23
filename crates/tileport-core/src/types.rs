/// Screen rectangle in logical coordinates.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Opaque window identifier (CGWindowID on macOS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct WindowId(pub u32);

/// Gap configuration for layout spacing.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Gaps {
    pub inner: f64,
    pub outer: f64,
}

impl Default for Gaps {
    fn default() -> Self {
        Self {
            inner: 8.0,
            outer: 8.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_equality() {
        let a = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let b = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn test_window_id_equality_and_hash() {
        use std::collections::HashSet;

        let id1 = WindowId(42);
        let id2 = WindowId(42);
        let id3 = WindowId(99);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);

        let mut set = HashSet::new();
        set.insert(id1);
        assert!(set.contains(&id2));
        assert!(!set.contains(&id3));
    }

    #[test]
    fn test_gaps_default() {
        let gaps = Gaps::default();
        assert_eq!(gaps.inner, 8.0);
        assert_eq!(gaps.outer, 8.0);
    }

    #[test]
    fn test_rect_serialization_roundtrip() {
        let rect = Rect {
            x: 10.0,
            y: 20.0,
            width: 800.0,
            height: 600.0,
        };
        let json = serde_json::to_string(&rect).unwrap();
        let deserialized: Rect = serde_json::from_str(&json).unwrap();
        assert_eq!(rect, deserialized);
    }

    #[test]
    fn test_window_id_serialization_roundtrip() {
        let id = WindowId(12345);
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: WindowId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_gaps_serialization_roundtrip() {
        let gaps = Gaps {
            inner: 4.0,
            outer: 12.0,
        };
        let json = serde_json::to_string(&gaps).unwrap();
        let deserialized: Gaps = serde_json::from_str(&json).unwrap();
        assert_eq!(gaps, deserialized);
    }
}
