use ok_math::Vec2;
use serde::{Deserialize, Serialize};

/// Stable identifier of a sketch entity within its sketch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntityId(pub u32);

/// Stable identifier of a constraint within its sketch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ConstraintId(pub u32);

/// Sketch geometry. Curves reference point entities for their defining
/// positions so that constraints only ever act on points and radii.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Entity {
    Point {
        pos: Vec2,
    },
    Line {
        start: EntityId,
        end: EntityId,
    },
    Circle {
        center: EntityId,
        radius: f64,
    },
    /// Counter-clockwise arc from `start` to `end` about `center`. The solver
    /// keeps `start` and `end` equidistant from `center`.
    Arc {
        center: EntityId,
        start: EntityId,
        end: EntityId,
    },
}

impl Entity {
    /// Entities this entity depends on.
    pub fn references(&self) -> Vec<EntityId> {
        match self {
            Entity::Point { .. } => vec![],
            Entity::Line { start, end } => vec![*start, *end],
            Entity::Circle { center, .. } => vec![*center],
            Entity::Arc { center, start, end } => vec![*center, *start, *end],
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            Entity::Point { .. } => "point",
            Entity::Line { .. } => "line",
            Entity::Circle { .. } => "circle",
            Entity::Arc { .. } => "arc",
        }
    }
}

/// Geometric and dimensional constraints. Angles are in degrees, lengths in
/// model units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Constraint {
    /// Two points share a position.
    Coincident {
        a: EntityId,
        b: EntityId,
    },
    /// A point is locked at its current position.
    Fixed {
        point: EntityId,
    },
    Horizontal {
        line: EntityId,
    },
    Vertical {
        line: EntityId,
    },
    /// Straight-line distance between two points.
    Distance {
        a: EntityId,
        b: EntityId,
        value: f64,
    },
    /// Signed x distance from `a` to `b`.
    HorizontalDistance {
        a: EntityId,
        b: EntityId,
        value: f64,
    },
    /// Signed y distance from `a` to `b`.
    VerticalDistance {
        a: EntityId,
        b: EntityId,
        value: f64,
    },
    Length {
        line: EntityId,
        value: f64,
    },
    /// Radius of a circle or arc.
    Radius {
        entity: EntityId,
        value: f64,
    },
    Diameter {
        entity: EntityId,
        value: f64,
    },
    /// Equal lengths (lines) or equal radii (circles / arcs).
    Equal {
        a: EntityId,
        b: EntityId,
    },
    Parallel {
        a: EntityId,
        b: EntityId,
    },
    Perpendicular {
        a: EntityId,
        b: EntityId,
    },
    /// Counter-clockwise angle in degrees from line `a` to line `b`.
    Angle {
        a: EntityId,
        b: EntityId,
        value: f64,
    },
    PointOnLine {
        point: EntityId,
        line: EntityId,
    },
    /// Point lies on a circle or arc.
    PointOnCircle {
        point: EntityId,
        entity: EntityId,
    },
    /// Point is the midpoint of a line.
    Midpoint {
        point: EntityId,
        line: EntityId,
    },
    /// Line is tangent to a circle or arc.
    Tangent {
        line: EntityId,
        entity: EntityId,
    },
}

impl Constraint {
    pub fn references(&self) -> Vec<EntityId> {
        use Constraint::*;
        match self {
            Coincident { a, b } | Equal { a, b } | Parallel { a, b } | Perpendicular { a, b } => {
                vec![*a, *b]
            }
            Distance { a, b, .. }
            | HorizontalDistance { a, b, .. }
            | VerticalDistance { a, b, .. }
            | Angle { a, b, .. } => {
                vec![*a, *b]
            }
            Fixed { point } => vec![*point],
            Horizontal { line } | Vertical { line } | Length { line, .. } => vec![*line],
            Radius { entity, .. } | Diameter { entity, .. } => vec![*entity],
            PointOnLine { point, line } | Midpoint { point, line } => vec![*point, *line],
            PointOnCircle { point, entity } => vec![*point, *entity],
            Tangent { line, entity } => vec![*line, *entity],
        }
    }

    /// The numeric value of a dimensional constraint, if any.
    pub fn value(&self) -> Option<f64> {
        use Constraint::*;
        match self {
            Distance { value, .. }
            | HorizontalDistance { value, .. }
            | VerticalDistance { value, .. }
            | Length { value, .. }
            | Radius { value, .. }
            | Diameter { value, .. }
            | Angle { value, .. } => Some(*value),
            _ => None,
        }
    }

    /// Sets the numeric value of a dimensional constraint. Returns `false`
    /// for geometric (non-dimensional) constraints.
    pub fn set_value(&mut self, v: f64) -> bool {
        use Constraint::*;
        match self {
            Distance { value, .. }
            | HorizontalDistance { value, .. }
            | VerticalDistance { value, .. }
            | Length { value, .. }
            | Radius { value, .. }
            | Diameter { value, .. }
            | Angle { value, .. } => {
                *value = v;
                true
            }
            _ => false,
        }
    }

    pub fn kind_name(&self) -> &'static str {
        use Constraint::*;
        match self {
            Coincident { .. } => "coincident",
            Fixed { .. } => "fixed",
            Horizontal { .. } => "horizontal",
            Vertical { .. } => "vertical",
            Distance { .. } => "distance",
            HorizontalDistance { .. } => "horizontal_distance",
            VerticalDistance { .. } => "vertical_distance",
            Length { .. } => "length",
            Radius { .. } => "radius",
            Diameter { .. } => "diameter",
            Equal { .. } => "equal",
            Parallel { .. } => "parallel",
            Perpendicular { .. } => "perpendicular",
            Angle { .. } => "angle",
            PointOnLine { .. } => "point_on_line",
            PointOnCircle { .. } => "point_on_circle",
            Midpoint { .. } => "midpoint",
            Tangent { .. } => "tangent",
        }
    }
}
