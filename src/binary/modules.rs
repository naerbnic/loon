use std::{collections::HashMap, rc::Rc};

use crate::util::imm_string::ImmString;

use super::const_table::ConstValue;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ModuleId(Rc<Vec<ImmString>>);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ModuleMemberId(ImmString);

impl ModuleMemberId {
    pub fn new(name: &str) -> Self {
        ModuleMemberId(ImmString::from_str(name))
    }
}

#[derive(Clone, Debug)]
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
    const_table: Vec<ConstValue>,

    /// The imports into this module. The key is the name of the import in the
    /// module scope, and the value is the source of the import.
    imports: Vec<ImportSource>,

    /// Exports from this module. Values are indexes into the const table.
    exports: HashMap<ModuleMemberId, u32>,

    /// The initializer for this module, if it has one.
    ///
    /// The value is an index into the const table.
    initializer: Option<u32>,

    /// The size of the module global table. At runtime, all globals will start
    /// empty, and will cause an error if read in this state. The initializer
    /// will be responsible for setting the globals to their initial values.
    global_table_size: u32,
}

impl ConstModule {
    pub fn new(
        const_table: Vec<ConstValue>,
        imports: Vec<ImportSource>,
        exports: HashMap<ModuleMemberId, u32>,
        initializer: Option<u32>,
        global_table_size: u32,
    ) -> Self {
        ConstModule {
            const_table,
            imports,
            exports,
            initializer,
            global_table_size,
        }
    }
    pub fn const_table(&self) -> &[ConstValue] {
        &self.const_table
    }
    pub fn imports(&self) -> &[ImportSource] {
        &self.imports
    }
    pub fn exports(&self) -> &HashMap<ModuleMemberId, u32> {
        &self.exports
    }
    pub fn global_table_size(&self) -> u32 {
        self.global_table_size
    }
    pub fn initializer(&self) -> Option<u32> {
        self.initializer
    }
}
