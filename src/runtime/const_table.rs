use std::rc::Rc;

use crate::{refs::GcContext, Float, Integer, List, Value};

pub enum Constant {
    Integer(Integer),
    Float(Float),
    String(Rc<String>),
    List(Vec<ConstIndex>),
}

#[derive(Copy, Clone)]
pub struct ConstIndex(usize);

pub struct ConstTable {
    values: Vec<Constant>,
}

pub struct ConstTableCache {
    table: Rc<ConstTable>,
    values: Vec<Option<Value>>,
}

impl ConstTableCache {
    pub fn new(table: Rc<ConstTable>) -> Self {
        let tables_len = table.values.len();
        ConstTableCache {
            table,
            values: vec![None; tables_len],
        }
    }

    pub fn get(&mut self, ctxt: &GcContext, index: ConstIndex) -> super::Result<Value> {
        if let Some(cached_value) = self.values.get(index.0).ok_or_else(|| {
            super::error::RuntimeError::new_operation_precondition_error("Invalid constant index.")
        })? {
            return Ok(cached_value.clone());
        }

        let new_value = match &self.table.values[index.0] {
            Constant::Integer(i) => Value::Integer(i.clone()),
            Constant::Float(f) => Value::Float(f.clone()),
            Constant::String(s) => Value::String(s.clone()),
            Constant::List(list) => {
                let list = list.clone();
                let mut values = Vec::with_capacity(list.len());
                for index in list {
                    values.push(self.get(ctxt, index)?);
                }
                Value::List(ctxt.create_ref(List::from_iter(values)))
            }
        };

        self.values[index.0] = Some(new_value.clone());

        Ok(new_value)
    }
}
