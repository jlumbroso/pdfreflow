//! Rectangles and PDF transformation matrices, in PDF points.

/// An axis-aligned rectangle with `x0 <= x1` and `y0 <= y1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    pub fn new(xa: f64, ya: f64, xb: f64, yb: f64) -> Rect {
        Rect {
            x0: xa.min(xb),
            y0: ya.min(yb),
            x1: xa.max(xb),
            y1: ya.max(yb),
        }
    }

    /// The bounding box of a set of points; `None` if there are none or any is not finite.
    pub fn from_points(points: &[(f64, f64)]) -> Option<Rect> {
        let (&(x, y), rest) = points.split_first()?;
        let mut rect = Rect::new(x, y, x, y);
        for &(x, y) in rest {
            rect = rect.union(Rect::new(x, y, x, y));
        }
        rect.is_finite().then_some(rect)
    }

    pub fn is_finite(&self) -> bool {
        [self.x0, self.y0, self.x1, self.y1]
            .iter()
            .all(|v| v.is_finite() && v.abs() < 1e6)
    }

    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }

    pub fn union(self, other: Rect) -> Rect {
        Rect {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }

    /// The overlap of two rectangles; `None` when they do not touch.
    pub fn intersect(self, other: Rect) -> Option<Rect> {
        let rect = Rect {
            x0: self.x0.max(other.x0),
            y0: self.y0.max(other.y0),
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
        };
        (rect.x0 <= rect.x1 && rect.y0 <= rect.y1).then_some(rect)
    }
}

/// A PDF transformation matrix `[a b c d e f]`, acting on row vectors:
/// `(x, y) -> (a*x + c*y + e, b*x + d*y + f)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    pub fn translate(tx: f64, ty: f64) -> Matrix {
        Matrix {
            e: tx,
            f: ty,
            ..Matrix::IDENTITY
        }
    }

    /// The transformation that applies `self` first and `next` second
    /// (the PDF product `self × next`).
    pub fn then(self, next: Matrix) -> Matrix {
        Matrix {
            a: self.a * next.a + self.b * next.c,
            b: self.a * next.b + self.b * next.d,
            c: self.c * next.a + self.d * next.c,
            d: self.c * next.b + self.d * next.d,
            e: self.e * next.a + self.f * next.c + next.e,
            f: self.e * next.b + self.f * next.d + next.f,
        }
    }

    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// The bounding box of an axis-aligned rectangle after transformation.
    pub fn apply_rect(&self, xa: f64, ya: f64, xb: f64, yb: f64) -> Option<Rect> {
        Rect::from_points(&[
            self.apply(xa, ya),
            self.apply(xb, ya),
            self.apply(xa, yb),
            self.apply(xb, yb),
        ])
    }
}
