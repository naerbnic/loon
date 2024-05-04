mod add;
mod branch;
mod branch_if;
mod call_dynamic;
mod pop;
mod push_const;
mod push_global;
mod return_dynamic;
mod set_global;

pub use add::Add;
pub use branch::Branch;
pub use branch_if::BranchIf;
pub use call_dynamic::CallDynamic;
pub use pop::Pop;
pub use push_const::PushConst;
pub use push_global::PushGlobal;
pub use return_dynamic::ReturnDynamic;
pub use set_global::SetGlobal;
