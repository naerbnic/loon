use std::{collections::HashMap, rc::Rc};

use crate::util::imm_string::ImmString;

use super::{
    const_table::{ConstIndex, ConstValue},
    error::ValidationError,
};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ModuleId(Rc<Vec<ImmString>>);

impl ModuleId {
    pub fn new<I>(path: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<ImmString>,
    {
        ModuleId(Rc::new(path.into_iter().map(Into::into).collect()))
    }
}

impl<I> From<I> for ModuleId
where
    I: IntoIterator,
    I::Item: Into<ImmString>,
{
    fn from(path: I) -> Self {
        ModuleId::new(path)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ModuleMemberId(ImmString);

impl ModuleMemberId {
    pub fn new<T>(name: T) -> Self
    where
        T: Into<ImmString>,
    {
        ModuleMemberId(name.into())
    }
}

impl<T> From<T> for ModuleMemberId
where
    T: Into<ImmString>,
{
    fn from(name: T) -> Self {
        ModuleMemberId(name.into())
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ImportSource {
    module_id: ModuleId,
    import_name: ModuleMemberId,
}

impl ImportSource {
    pub fn new(module_id: impl Into<ModuleId>, import_name: impl Into<ModuleMemberId>) -> Self {
        ImportSource {
            module_id: module_id.into(),
            import_name: import_name.into(),
        }
    }

    pub fn module_id(&self) -> &ModuleId {
        &self.module_id
    }

    pub fn import_name(&self) -> &ModuleMemberId {
        &self.import_name
    }
}

/// Check that the constant values are valid, and return the set of constraints
/// the table has to meet.
pub fn validate_module(
    table_elements: &[ConstValue],
    _globals_size: u32,
    imports_size: u32,
) -> Result<(), ValidationError> {
    let check_index = |index: &ConstIndex| {
        match index {
            ConstIndex::ModuleConst(i) => {
                if *i >= table_elements.len() as u32 {
                    return Err(ValidationError::LocalIndexResolutionError);
                }
            }
            ConstIndex::ModuleImport(i) => {
                if *i >= imports_size {
                    return Err(ValidationError::LocalIndexResolutionError);
                }
            }
        }
        Ok(())
    };

    for value in table_elements {
        match value {
            ConstValue::List(list) => {
                for index in list {
                    check_index(index)?;
                }
            }
            ConstValue::Function(_) => {
                // FIXME: Const tables should preserve the enviroment they
                // expect, to allow for validation outside of the context of
                // building the const table.
            }
            _ => {}
        }
    }
    Ok(())
}

pub struct ConstModule {
    /// The unique identifier for this module.
    id: ModuleId,

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
        id: ModuleId,
        const_table: Vec<ConstValue>,
        imports: Vec<ImportSource>,
        exports: HashMap<ModuleMemberId, u32>,
        initializer: Option<u32>,
        global_table_size: u32,
    ) -> Result<Self, ValidationError> {
        validate_module(&const_table, global_table_size, imports.len() as u32)?;
        Ok(ConstModule {
            id,
            const_table,
            imports,
            exports,
            initializer,
            global_table_size,
        })
    }
    pub fn id(&self) -> &ModuleId {
        &self.id
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
    pub fn dependencies(&self) -> impl Iterator<Item = &ModuleId> {
        self.imports.iter().map(|import| import.module_id())
    }
}
