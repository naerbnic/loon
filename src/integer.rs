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

impl GcTraceable for Integer {
    fn trace<V>(&self, _visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        // No nested values to trace
    }
}
