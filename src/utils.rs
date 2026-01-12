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
