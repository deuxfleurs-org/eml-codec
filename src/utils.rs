use bounded_static::ToStatic;
use crate::print::{Print, Formatter};

#[derive(Debug, PartialEq, Clone, ToStatic)]
pub enum Deductible<T: Default> {
    Inferred,
    Explicit(T),
}
impl<T: Default> Default for Deductible<T> {
    fn default() -> Self {
        Self::Inferred
    }
}
impl<T: Default + Clone> Deductible<T> {
    pub fn value(&self) -> T {
        match self {
            Deductible::Inferred => T::default(),
            Deductible::Explicit(x) => x.clone(),
        }
    }
}
impl<T: Default + Print> Print for Deductible<T> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self {
            Deductible::Inferred => T::default().print(fmt),
            Deductible::Explicit(x) => x.print(fmt),
        }
    }
}

pub(crate) fn set_opt<T>(o: &mut Option<T>, x: T) -> bool {
    match *o {
        None => { *o = Some(x); true },
        Some(_) => false,
    }
}

pub(crate) fn append_opt<T>(o: &mut Option<Vec<T>>, x: Vec<T>) -> bool {
    match o {
        None => { *o = Some(x); true },
        Some(v) => { v.extend(x); false },
    }
}

pub(crate) fn vec_filter_none_nonempty<T>(v: Vec<Option<T>>) -> Option<Vec<T>> {
    let v: Vec<T> = v.into_iter().flatten().collect();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}
