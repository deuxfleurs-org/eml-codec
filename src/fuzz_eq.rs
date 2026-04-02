pub use derive_fuzz_eq::FuzzEq;

pub trait FuzzEq {
    fn fuzz_eq(&self, other: &Self) -> bool;
}

impl<T: FuzzEq> FuzzEq for Vec<T> {
    fn fuzz_eq(&self, other: &Self) -> bool {
        self.len() == other.len() &&
        self.iter().zip(other.iter()).all(|(x1, x2)| x1.fuzz_eq(x2))
    }
}

impl<T: FuzzEq> FuzzEq for Option<T> {
    fn fuzz_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (None, None) => true,
            (Some(x), Some(y)) => x.fuzz_eq(y),
            (_, _) => false,
        }
    }
}

impl<T: FuzzEq> FuzzEq for Box<T> {
    fn fuzz_eq(&self, other: &Self) -> bool {
        self.as_ref().fuzz_eq(other.as_ref())
    }
}
