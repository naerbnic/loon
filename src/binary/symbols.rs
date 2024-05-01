use crate::util::imm_string::ImmString;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GlobalSymbol(ImmString);

impl GlobalSymbol {
    pub fn new(symbol: &str) -> Self {
        GlobalSymbol(ImmString::from_str(symbol))
    }
}
