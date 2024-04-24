use std::rc::Rc;

use num_traits::ToPrimitive;

use crate::refs::{GcRefVisitor, GcTraceable};

#[derive(Clone, Debug)]
pub enum Integer {
    Compact(i64),
    Big(Rc<num_bigint::BigInt>),
}

impl Integer {
    pub fn to_compact_integer(&self) -> Option<i64> {
        match self {
            Integer::Compact(i) => Some(*i),
            Integer::Big(i) => i.to_i64(),
        }
    }

    pub fn normalize(&mut self) {
        match self {
            Integer::Compact(_) => {}
            Integer::Big(i) => {
                if let Some(i) = i.to_i64() {
                    *self = Integer::Compact(i);
                }
            }
        }
    }
}

impl PartialEq for Integer {
    fn eq(&self, other: &Self) -> bool {
        if let (Some(i1), Some(i2)) = (self.to_compact_integer(), other.to_compact_integer()) {
            i1 == i2
        } else if let (Integer::Big(i1), Integer::Big(i2)) = (self, other) {
            i1 == i2
        } else {
            false
        }
    }
}

impl GcTraceable for Integer {
    fn trace<V>(&self, _visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        // No nested values to trace
    }
}

impl From<i64> for Integer {
    fn from(i: i64) -> Self {
        Integer::Compact(i)
    }
}

impl From<num_bigint::BigInt> for Integer {
    fn from(i: num_bigint::BigInt) -> Self {
        if let Some(i) = i.to_i64() {
            Integer::Compact(i)
        } else {
            Integer::Big(Rc::new(i))
        }
    }
}
