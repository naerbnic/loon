use std::{
    borrow::{Borrow, Cow},
    collections::HashSet,
    rc::Rc,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedString(Rc<String>);

impl SharedString {
    pub fn new(s: impl Into<String>) -> Self {
        SharedString(Rc::new(s.into()))
    }
}

impl Borrow<str> for SharedString {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl std::ops::Deref for SharedString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl std::hash::Hash for SharedString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_str().hash(state)
    }
}

/// An opcode for an instruction.
///
/// We're following the WASM idea for opcodes, where the opcodes are actually
/// full on identifiers. In the binary file, it can state a set of opcodes,
/// and mappings from integers to opcodes for the encoding to a file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Opcode(SharedString);

#[derive(Clone, Debug)]
pub enum InstArg {
    Integer(u32),
}

#[derive(Clone, Debug)]
pub struct Instruction {
    opcode: Opcode,
    args: Vec<InstArg>,
}

#[derive(Clone, Debug)]
pub struct InstructionList(Vec<Instruction>);

pub struct InstructionListBuilder {
    interned_opcodes: HashSet<SharedString>,
    instructions: Vec<Instruction>,
}

impl InstructionListBuilder {
    pub fn new() -> Self {
        InstructionListBuilder {
            interned_opcodes: HashSet::new(),
            instructions: Vec::new(),
        }
    }

    pub fn add_instruction<'a>(
        &mut self,
        opcode: impl Into<Cow<'a, str>>,
        args: Vec<InstArg>,
    ) -> &mut Self {
        let opcode: Cow<str> = opcode.into();
        let opcode = if let Some(opcode) = self.interned_opcodes.get(opcode.as_ref()) {
            opcode.clone()
        } else {
            let new_string = SharedString::new(opcode);
            self.interned_opcodes.insert(new_string.clone());
            new_string
        };

        self.instructions.push(Instruction {
            opcode: Opcode(opcode),
            args,
        });
        self
    }

    pub fn build(self) -> InstructionList {
        InstructionList(self.instructions)
    }
}
