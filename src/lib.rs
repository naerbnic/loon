use std::rc::Rc;

use function::Function;
use refs::{GcRef, GcRefVisitor, GcTraceable};

use num_traits::ToPrimitive;

mod function;
mod integer;
mod list;
pub mod refs;
mod runtime;

pub use integer::Integer;
pub use list::List;
use runtime::error::RuntimeError;

#[derive(Clone, Debug)]
pub struct Float(f64);

impl refs::GcTraceable for Float {
    fn trace<V>(&self, _visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        // No nested values to trace
    }
}
