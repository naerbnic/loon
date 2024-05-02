use std::{collections::HashMap, rc::Rc};

use crate::util::imm_string::ImmString;

use super::const_table::ConstTable;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ModuleId(Rc<Vec<ImmString>>);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ModuleMemberId(ImmString);

pub struct ImportSource {
    module_id: ModuleId,
    import_name: ModuleMemberId,
}

impl ImportSource {
    pub fn new(module_id: ModuleId, import_name: ModuleMemberId) -> Self {
        ImportSource {
            module_id,
            import_name,
        }
    }

    pub fn module_id(&self) -> &ModuleId {
        &self.module_id
    }

    pub fn import_name(&self) -> &ModuleMemberId {
        &self.import_name
    }
}

pub struct ConstModule {
    /// The set of constants defined in this module. This const table must
    /// be fully defined, with no escaping local references, and globals
    /// must be covered by the global set, or the module's imports.
    const_table: ConstTable,

    /// The imports into this module. The key is the name of the import in the
    /// module scope, and the value is the source of the import.
    imports: Vec<ImportSource>,

    /// Exports from this module. Values are indexes into the const table.
    exports: HashMap<ImmString, u32>,

    /// The initializer for this module, if it has one.
    ///
    /// The value is an index into the const table.
    initializer: Option<u32>,

    /// The size of the module global table. At runtime, all globals will start
    /// empty, and will cause an error if read in this state. The initializer
    /// will be responsible for setting the globals to their initial values.
    global_table_size: u32,
}
