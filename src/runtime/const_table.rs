use std::{cell::RefCell, rc::Rc};

use crate::refs::GcContext;

use super::{
    constants::ConstIndex,
    value::{Float, Integer, Value},
};

pub enum ConstValue {
    Integer(Integer),
    Float(Float),
    String(Rc<String>),
    List(Vec<ConstIndex>),
}

pub struct ConstTable {
    values: Vec<ConstValue>,
}

// struct ConstTableCacheInner {
//     table: Rc<ConstTable>,
//     values: RefCell<Vec<Option<Value>>>,
// }

// impl ConstTableCacheInner {
//     pub fn new(table: Rc<ConstTable>) -> Self {
//         let tables_len = table.values.len();
//         ConstTableCacheInner {
//             table,
//             values: RefCell::new(vec![None; tables_len]),
//         }
//     }

//     pub fn get(&self, ctxt: &GcContext, index: ConstIndex) -> super::Result<Value> {
//         {
//             let values = self.values.borrow();
//             if let Some(cached_value) = values.get(index.0).ok_or_else(|| {
//                 super::error::RuntimeError::new_operation_precondition_error(
//                     "Invalid constant index.",
//                 )
//             })? {
//                 return Ok(cached_value.clone());
//             }
//         }

//         let fill_value = |v: Value| {
//             let mut values = self.values.borrow_mut();
//             values[index.0] = Some(v.clone());
//         };

//         match &self.table.values[index.0] {
//             ConstValue::Integer(i) => fill_value(Value::Integer(i.clone())),
//             ConstValue::Float(f) => fill_value(Value::Float(f.clone())),
//             ConstValue::String(s) => fill_value(Value::String(s.clone())),
//             ConstValue::List(list) => {
//                 let (deferred, resolve_fn) = ctxt.create_deferred_ref();
//                 fill_value(Value::List(deferred.clone()));
//                 let mut values = Vec::with_capacity(list.len());
//                 for index in list {
//                     values.push(self.get(ctxt, *index)?);
//                 }
//                 resolve_fn(List::from_iter(values));
//             }
//         };

//         self.values.borrow()[index.0].clone().ok_or_else(|| {
//             super::error::RuntimeError::new_internal_error("Failed to fill constant value.")
//         })
//     }
// }

// #[derive(Clone)]
// pub struct ConstTableCache {
//     inner: Rc<ConstTableCacheInner>,
// }

// impl ConstTableCache {
//     pub fn new(table: Rc<ConstTable>) -> Self {
//         ConstTableCache {
//             inner: Rc::new(ConstTableCacheInner::new(table)),
//         }
//     }

//     pub fn get(&self, ctxt: &GcContext, index: ConstIndex) -> super::Result<Value> {
//         self.inner.get(ctxt, index)
//     }
// }
