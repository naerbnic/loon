use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GlobalSymbol(Rc<String>);

impl GlobalSymbol {
    pub fn new(symbol: impl Into<String>) -> Self {
        GlobalSymbol(Rc::new(symbol.into()))
    }
}
