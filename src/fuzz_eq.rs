pub use eml_codec_derives::FuzzEq;
use eyre::{bail, Context};
use std::fmt::Debug;

pub trait FuzzEq {
    fn fuzz_eq(&self, other: &Self) -> eyre::Result<()>;
}

// pub type EqCtx = Vec<CtxItem<'static>>;

// #[derive(Clone, Copy)]
// pub enum EqCtxItem<'a> {
//     Ctor { typ: &'a str, name: &'a str },
//     CtorField { typ: &'a str, name: &'a str, field: &'a str },
//     Field { typ: &'a str, name: &'a str },
//     VecItem { idx: usize },
// }

// pub type EqCtx = Vec<Box<dyn std::fmt::Debug>>;

impl<T: FuzzEq> FuzzEq for Vec<T> {
    fn fuzz_eq(&self, other: &Self) -> eyre::Result<()> {
        if self.len() != other.len() {
            bail!("Vec length mismatch")
        }
        for (idx, (x1, x2)) in self.iter().zip(other.iter()).enumerate() {
            x1.fuzz_eq(x2).with_context(|| format!("Vec index {idx}"))?;
        }
        Ok(())
    }
}

impl<T: FuzzEq + Debug> FuzzEq for Option<T> {
    fn fuzz_eq(&self, other: &Self) -> eyre::Result<()> {
        match (self, other) {
            (None, None) => Ok(()),
            (Some(x), Some(y)) => x.fuzz_eq(y),
            (_, _) => bail!("Option case mismatch: {:?} vs {:?}", self, other),
        }
    }
}

impl<T: FuzzEq> FuzzEq for Box<T> {
    fn fuzz_eq(&self, other: &Self) -> eyre::Result<()> {
        self.as_ref().fuzz_eq(other.as_ref())
    }
}
