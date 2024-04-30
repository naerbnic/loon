//! Values that can be shared between the binary and runtime.

use std::rc::Rc;

use num_traits::ToPrimitive;

#[derive(Clone, Debug)]
enum IntegerInner {
    Compact(i64),
    Big(Rc<num_bigint::BigInt>),
}

#[derive(Clone, Debug)]
pub struct Integer(IntegerInner);

impl Integer {
    pub fn to_compact_integer(&self) -> Option<i64> {
        match &self.0 {
            IntegerInner::Compact(i) => Some(*i),
            IntegerInner::Big(i) => i.to_i64(),
        }
    }

    pub fn normalize(&mut self) {
        match &self.0 {
            IntegerInner::Compact(_) => {}
            IntegerInner::Big(i) => {
                if let Some(i) = i.to_i64() {
                    self.0 = IntegerInner::Compact(i);
                }
            }
        }
    }
}

impl PartialEq for Integer {
    fn eq(&self, other: &Self) -> bool {
        if let (Some(i1), Some(i2)) = (self.to_compact_integer(), other.to_compact_integer()) {
            i1 == i2
        } else if let (IntegerInner::Big(i1), IntegerInner::Big(i2)) = (&self.0, &other.0) {
            i1 == i2
        } else {
            false
        }
    }
}

impl From<i64> for Integer {
    fn from(i: i64) -> Self {
        Integer(IntegerInner::Compact(i))
    }
}

impl From<num_bigint::BigInt> for Integer {
    fn from(i: num_bigint::BigInt) -> Self {
        Integer(if let Some(i) = i.to_i64() {
            IntegerInner::Compact(i)
        } else {
            IntegerInner::Big(Rc::new(i))
        })
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct Float(f64);

impl Float {
    pub fn new(value: f64) -> Self {
        Float(value)
    }

    pub fn value(&self) -> f64 {
        self.0
    }
}
