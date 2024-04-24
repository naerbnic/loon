use std::rc::Rc;

use refs::{GcRef, GcRefVisitor, GcTraceable};

use num_traits::ToPrimitive;

pub mod refs;
mod runtime;

use runtime::error::RuntimeError;
