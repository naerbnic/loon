//! A description of a text format to describe the contents of a Loon VM program.

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
};

use crate::binary::{
    error::BuilderError,
    instructions::{CallInstruction, CompareOp, StackIndex},
    module_set::ModuleSet,
    modules::{ImportSource, ModuleId, ModuleMemberId},
    ConstModule, DeferredValue, FunctionBuilder, ModuleBuilder, ValueRef,
};

#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum SExprType {
    Null,
    Cons,
    Bool,
    Number,
    String,
    Symbol,
    Keyword,
    Unsupported,
}

impl SExprType {
    fn from_value(lexpr: &lexpr::Value) -> Self {
        match lexpr {
            lexpr::Value::Null => SExprType::Null,
            lexpr::Value::Cons(_) => SExprType::Cons,
            lexpr::Value::Bool(_) => SExprType::Bool,
            lexpr::Value::Number(_) => SExprType::Number,
            lexpr::Value::String(_) => SExprType::String,
            lexpr::Value::Symbol(_) => SExprType::Symbol,
            lexpr::Value::Keyword(_) => SExprType::Keyword,
            _ => SExprType::Unsupported,
        }
    }
}

#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Lexpr(#[from] lexpr::parse::Error),

    #[error("Unexpected value type: expected {0:?}, got {1:?}")]
    UnexpectedValueType(HashSet<SExprType>, SExprType),

    #[error("Unexpected symbol: {0:?}")]
    UnexpectedSymbol(String),

    #[error("Invalid module name")]
    InvalidModuleName,

    #[error("Wrong param size: expected {0}, got {1}")]
    WrongParamSize(usize, usize),

    #[error(transparent)]
    Builder(#[from] BuilderError),

    #[error("Unknown reference: {0}")]
    UnknownReference(String),
}

impl Error {
    pub fn new_unexpected_value_type(
        expected: impl IntoIterator<Item = SExprType>,
        got: &lexpr::Value,
    ) -> Self {
        Error::UnexpectedValueType(expected.into_iter().collect(), SExprType::from_value(got))
    }
}

type Result<T> = std::result::Result<T, Error>;

// Helper to parse list with given head symbol
fn parse_list_with_initial_symbol(expr: &lexpr::Value) -> Result<(&str, &lexpr::Value)> {
    let (head, rest) = parse_cons(expr)?;
    let head_symbol = parse_symbol(head)?;
    Ok((head_symbol, rest))
}

fn parse_cons(expr: &lexpr::Value) -> Result<(&lexpr::Value, &lexpr::Value)> {
    let cons = expr
        .as_cons()
        .ok_or_else(|| Error::new_unexpected_value_type([SExprType::Cons], expr))?;
    Ok((cons.car(), cons.cdr()))
}

fn parse_symbol(expr: &lexpr::Value) -> Result<&str> {
    expr.as_symbol()
        .ok_or_else(|| Error::new_unexpected_value_type([SExprType::Symbol], expr))
}

fn parse_keyword(expr: &lexpr::Value) -> Result<&str> {
    expr.as_keyword()
        .ok_or_else(|| Error::new_unexpected_value_type([SExprType::Keyword], expr))
}

fn parse_str(expr: &lexpr::Value) -> Result<&str> {
    expr.as_str()
        .ok_or_else(|| Error::new_unexpected_value_type([SExprType::String], expr))
}

fn parse_list(expr: &lexpr::Value) -> Result<impl Iterator<Item = &lexpr::Value>> {
    // A list should only consist of Cons and Null cells. Validate here.
    let mut curr = expr;
    loop {
        match curr {
            lexpr::Value::Cons(cons) => {
                curr = cons.cdr();
            }
            lexpr::Value::Null => {
                break;
            }
            _ => {
                return Err(Error::new_unexpected_value_type(
                    [SExprType::Cons, SExprType::Null],
                    curr,
                ));
            }
        }
    }

    expr.list_iter()
        .ok_or_else(|| Error::new_unexpected_value_type([SExprType::Cons, SExprType::Null], expr))
}

fn parse_int(expr: &lexpr::Value) -> Result<i64> {
    expr.as_i64()
        .ok_or_else(|| Error::new_unexpected_value_type([SExprType::Number], expr))
}

fn parse_const_len_list<const L: usize>(list: &lexpr::Value) -> Result<[&lexpr::Value; L]> {
    let iter = parse_list(list)?;
    iter.collect::<Vec<_>>()
        .try_into()
        .map_err(|v: Vec<_>| Error::WrongParamSize(L, v.len()))
}

fn parse_list_with_head<'a>(head: &str, expr: &'a lexpr::Value) -> Result<&'a lexpr::Value> {
    let (head_symbol, contents) = parse_list_with_initial_symbol(expr)?;
    if head_symbol != head {
        return Err(Error::UnexpectedSymbol(head_symbol.to_string()));
    }
    Ok(contents)
}

fn parse_module_id(name: &str) -> Result<ModuleId> {
    let mut items = Vec::new();
    for component in name.split('.') {
        // FIXME: Validate component contents
        if component.is_empty() {
            return Err(Error::InvalidModuleName);
        }
        items.push(component);
    }
    Ok(ModuleId::new(items))
}

pub fn from_str(text: &str) -> Result<ModuleSet> {
    let expr = lexpr::from_str(text)?;

    parse_module_set(&expr)
}

fn parse_module_set(expr: &lexpr::Value) -> Result<ModuleSet> {
    let modules = parse_list_with_head("module-set", expr)?;
    let mut module_list = Vec::new();
    for module_expr in parse_list(modules)? {
        let module = parse_module(module_expr)?;
        module_list.push(module);
    }
    Ok(ModuleSet::new(module_list))
}

struct ImportItem<'a> {
    local_name: &'a str,
    value_ref: ValueRef,
}

struct ExportItem<'a> {
    local_name: &'a str,
}

struct ConstantItem<'a> {
    local_name: &'a str,
    value: ValueRef,
    deferred_value: Cell<Option<DeferredValue>>,
    expr: &'a lexpr::Value,
}

impl ConstantItem<'_> {
    pub fn resolve(&self, builder: &ModuleBuilder, references: &ReferenceSet) -> Result<()> {
        resolve_constant_expr(
            builder,
            references,
            self.deferred_value
                .take()
                .expect("Deferred value already resolved"),
            self.expr,
        )
    }
}

struct GlobalItem<'a> {
    local_name: &'a str,
    value: ValueRef,
}

struct InitItem<'a> {
    body: &'a lexpr::Value,
}

enum ModuleItem<'a> {
    Import(ImportItem<'a>),
    Export(ExportItem<'a>),
    Const(ConstantItem<'a>),
    Global(GlobalItem<'a>),
    Init(InitItem<'a>),
}

fn parse_module(expr: &lexpr::Value) -> Result<ConstModule> {
    let (module_str_value, module_contents) = parse_cons(expr)?;
    let module_id = parse_module_id(parse_str(module_str_value)?)?;
    let builder = ModuleBuilder::new(module_id.clone());
    let mut items = Vec::new();
    for module_item_expr in parse_list(module_contents)? {
        items.push(parse_module_item(&builder, module_item_expr)?)
    }

    resolve_items(&builder, &items)?;

    let module = builder.into_const_module()?;
    Ok(module)
}

struct ReferenceSet<'a>(HashMap<&'a str, ValueRef>);

impl ReferenceSet<'_> {
    fn get(&self, name: &str) -> Result<&ValueRef> {
        self.0
            .get(name)
            .ok_or_else(|| Error::UnknownReference(name.to_string()))
    }
}

fn gather_item_references<'a>(items: &[ModuleItem<'a>]) -> Result<ReferenceSet<'a>> {
    let mut references = HashMap::new();
    for item in items {
        match item {
            ModuleItem::Const(constant) => {
                references.insert(constant.local_name, constant.value.clone());
            }
            ModuleItem::Import(import) => {
                references.insert(import.local_name, import.value_ref.clone());
            }
            ModuleItem::Global(global) => {
                references.insert(global.local_name, global.value.clone());
            }
            ModuleItem::Init(_) | ModuleItem::Export(_) => {}
        }
    }
    Ok(ReferenceSet(references))
}

fn resolve_items(builder: &ModuleBuilder, items: &[ModuleItem]) -> Result<()> {
    let references = gather_item_references(items)?;
    for item in items {
        match item {
            ModuleItem::Const(constant) => {
                constant.resolve(builder, &references)?;
            }
            ModuleItem::Export(export) => {
                references
                    .get(export.local_name)?
                    .export(ModuleMemberId::new(export.local_name))?;
            }
            ModuleItem::Init(init) => {
                resolve_fn_expr(builder, &references, builder.new_initializer()?, init.body)?;
            }
            ModuleItem::Global(_) | ModuleItem::Import(_) => {}
        }
    }
    Ok(())
}

fn parse_module_item<'a>(
    builder: &ModuleBuilder,
    item: &'a lexpr::Value,
) -> Result<ModuleItem<'a>> {
    let (first, rest) = parse_cons(item)?;
    let item = match parse_symbol(first)? {
        "import" => ModuleItem::Import(parse_import_item(builder, rest)?),
        "export" => ModuleItem::Export(parse_export_item(rest)?),
        "const" => ModuleItem::Const(parse_constant_item(builder, rest)?),
        "global" => ModuleItem::Global(parse_global_item(builder, rest)?),
        "init" => ModuleItem::Init(InitItem { body: rest }),
        unknown_symbol => return Err(Error::UnexpectedSymbol(unknown_symbol.to_string())),
    };
    Ok(item)
}

fn parse_import_item<'a>(
    builder: &ModuleBuilder,
    body: &'a lexpr::Value,
) -> Result<ImportItem<'a>> {
    // Has the form (import <name-sym> <module-id-str> <module-item-symbol>)
    let [local_name, module_id_str, member_symbol] = parse_const_len_list(body)?;
    let module_id = parse_module_id(parse_str(module_id_str)?)?;
    let member_id = ModuleMemberId::new(parse_symbol(member_symbol)?);
    let import_source = ImportSource::new(module_id, member_id);
    let value_ref = builder.add_import(import_source);
    Ok(ImportItem {
        local_name: parse_symbol(local_name)?,
        value_ref,
    })
}

fn parse_export_item(body: &lexpr::Value) -> Result<ExportItem> {
    let [local_name] = parse_const_len_list(body)?;
    Ok(ExportItem {
        local_name: parse_symbol(local_name)?,
    })
}

fn parse_constant_item<'a>(
    builder: &ModuleBuilder,
    body: &'a lexpr::Value,
) -> Result<ConstantItem<'a>> {
    // Has the form (const <local-name-sym> <const-value>)
    let [local_name, expr] = parse_const_len_list(body)?;
    let (value, deferred_value) = builder.new_deferred();
    Ok(ConstantItem {
        local_name: parse_symbol(local_name)?,
        value,
        deferred_value: Cell::new(Some(deferred_value)),
        expr,
    })
}

fn parse_global_item<'a>(
    builder: &ModuleBuilder,
    body: &'a lexpr::Value,
) -> Result<GlobalItem<'a>> {
    // Has the form (global <local-name-sym>)
    let [local_name] = parse_const_len_list(body)?;
    Ok(GlobalItem {
        local_name: parse_symbol(local_name)?,
        value: builder.new_global(),
    })
}

fn parse_constant_expr(
    builder: &ModuleBuilder,
    references: &ReferenceSet,
    expr: &lexpr::Value,
) -> Result<ValueRef> {
    let (value, deferred_value) = builder.new_deferred();
    resolve_constant_expr(builder, references, deferred_value, expr)?;
    Ok(value)
}

fn resolve_constant_expr(
    builder: &ModuleBuilder,
    references: &ReferenceSet,
    deferred: DeferredValue,
    expr: &lexpr::Value,
) -> Result<()> {
    if let Some(i) = expr.as_i64() {
        deferred.resolve_int(i)?;
    } else if let Some(f) = expr.as_f64() {
        deferred.resolve_float(f)?;
    } else if let Some(b) = expr.as_bool() {
        deferred.resolve_bool(b)?;
    } else if let Some(s) = expr.as_str() {
        deferred.resolve_string(s)?;
    } else if let Some(name) = expr.as_symbol() {
        deferred.resolve_other(references.get(name)?)?;
    } else if let Some(cons) = expr.as_cons() {
        resolve_constant_compound_expr(builder, references, deferred, cons)?;
    } else {
        return Err(Error::new_unexpected_value_type(
            [
                SExprType::Number,
                SExprType::Symbol,
                SExprType::String,
                SExprType::Cons,
            ],
            expr,
        ));
    }
    Ok(())
}

fn resolve_constant_compound_expr(
    builder: &ModuleBuilder,
    references: &ReferenceSet,
    deferred: DeferredValue,
    expr: &lexpr::Cons,
) -> Result<()> {
    let body = expr.cdr();
    match parse_symbol(expr.car())? {
        "list" => resolve_list_expr(builder, references, deferred, body)?,
        "fn" => resolve_fn_expr(builder, references, deferred.into_function_builder(), body)?,
        unknown_symbol => return Err(Error::UnexpectedSymbol(unknown_symbol.to_string())),
    }
    Ok(())
}

fn resolve_list_expr(
    builder: &ModuleBuilder,
    references: &ReferenceSet,
    deferred: DeferredValue,
    expr: &lexpr::Value,
) -> Result<()> {
    let mut values = Vec::new();
    for item_expr in parse_list(expr)? {
        let (item, item_deferred) = builder.new_deferred();
        resolve_constant_expr(builder, references, item_deferred, item_expr)?;
        values.push(item);
    }
    deferred.resolve_list(values)?;
    Ok(())
}

fn resolve_fn_expr(
    builder: &ModuleBuilder,
    references: &ReferenceSet,
    mut fn_builder: FunctionBuilder,
    body: &lexpr::Value,
) -> Result<()> {
    for inst_expr in parse_list(body)? {
        apply_fn_inst(builder, &mut fn_builder, references, inst_expr)?;
    }
    fn_builder.build()?;
    Ok(())
}

macro_rules! op_parse {
    ($cons:expr => $(($name:literal $(, $arg:ident)* $(,)?) => $body:block)*) => {
        match parse_symbol($cons.car())? {
            $($name => {
                let [$( $arg ),*] = parse_const_len_list($cons.cdr())?;
                $body
            })*
            unknown_opcode => return Err(Error::UnexpectedSymbol(unknown_opcode.to_string())),
        }
    };
}

fn apply_fn_inst(
    builder: &ModuleBuilder,
    fn_builder: &mut FunctionBuilder,
    references: &ReferenceSet,
    body: &lexpr::Value,
) -> Result<()> {
    match body {
        lexpr::Value::Keyword(kw) => {
            fn_builder.define_branch_target(kw);
        }
        lexpr::Value::Cons(cons) => {
            op_parse! { cons =>
                ("push", value_expr) => {
                    let value = parse_constant_expr(builder, references, value_expr)?;
                    fn_builder.push_value(&value)?;
                }
                ("pop", n_pop) => {
                    fn_builder.pop(parse_int(n_pop)? as u32);
                }
                ("write_stack", stack_end, index) => {
                    let index = parse_int(index)? as u32;
                    let stack_end = parse_symbol(stack_end)?;
                    let stack_index = match stack_end {
                        "top" => {
                            StackIndex::FromTop(index)
                        }
                        "bot" => {
                            StackIndex::FromBottom(index)
                        }
                        _ => return Err(Error::UnexpectedSymbol(stack_end.to_string())),
                    };
                    fn_builder.write_stack(stack_index);
                }
                ("add") => {
                    fn_builder.add();
                }
                ("return", num_args) => {
                    fn_builder.return_(parse_int(num_args)? as u32);
                }
                ("return_dynamic") => {
                    fn_builder.return_dynamic();
                }
                ("branch", target) => {
                    fn_builder.branch(parse_keyword(target)?);
                }
                ("branch_if", target) => {
                    fn_builder.branch_if(parse_keyword(target)?);
                }
                ("push_copy", stack_end, index) => {
                    let index = parse_int(index)? as u32;
                    let stack_end = parse_symbol(stack_end)?;
                    let stack_index = match stack_end {
                        "top" => {
                            StackIndex::FromTop(index)
                        }
                        "bot" => {
                            StackIndex::FromBottom(index)
                        }
                        _ => return Err(Error::UnexpectedSymbol(stack_end.to_string())),
                    };
                    fn_builder.push_copy(stack_index);
                }
                ("call", num_args, num_returns) => {
                    let num_args = parse_int(num_args)? as u32;
                    let num_returns = parse_int(num_returns)? as u32;
                    fn_builder.call(CallInstruction { num_args, num_returns });
                }
                ("tail_call", num_args) => {
                    let num_args = parse_int(num_args)? as u32;
                    fn_builder.tail_call(num_args);
                }
                ("cmp", op) => {
                    let op = parse_symbol(op)?;
                    match op {
                        "ref_eq" => {
                            fn_builder.compare(CompareOp::RefEq);
                        }
                        _ => return Err(Error::UnexpectedSymbol(op.to_string())),
                    }
                }
                ("bind_front", num_args) => {
                    let num_args = parse_int(num_args)? as u32;
                    fn_builder.bind_front(num_args);
                }
            }
        }
        _ => {
            return Err(Error::new_unexpected_value_type(
                [SExprType::Keyword, SExprType::Cons],
                body,
            ))
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_import_module_item_works() -> anyhow::Result<()> {
        let expr = lexpr::from_str(r#"(import foo "my.module" bar)"#)?;
        let ModuleItem::Import(imp) =
            parse_module_item(&ModuleBuilder::new(ModuleId::new(["foo"])), &expr)?
        else {
            anyhow::bail!("Wrong type")
        };
        assert_eq!(imp.local_name, "foo");
        Ok(())
    }

    #[test]
    fn parse_export_module_item_works() -> anyhow::Result<()> {
        let expr = lexpr::from_str(r#"(export bar)"#)?;
        let ModuleItem::Export(exp) =
            parse_module_item(&ModuleBuilder::new(ModuleId::new(["foo"])), &expr)?
        else {
            anyhow::bail!("Wrong type")
        };
        assert_eq!(exp.local_name, "bar");
        Ok(())
    }

    #[test]
    fn parse_basic_module() -> anyhow::Result<()> {
        let expr = lexpr::from_str(
            r#"
                (module-set
                    ("my.module"
                        (const foo 42)
                        (const bar "baz")
                        (export foo)
                    )
                )
            "#,
        )?;
        let _module_set = parse_module_set(&expr)?;
        Ok(())
    }

    #[test]
    fn parse_add_function_module() -> anyhow::Result<()> {
        let expr = lexpr::from_str(
            r#"
                (module-set
                    ("my.module"
                        (const foo 1)
                        (const bar 2)
                        (export add)
                        (const add
                            (fn
                                (push foo)
                                (push bar)
                                (add)
                                (return 1)
                            )
                        )
                    )
                )
            "#,
        )?;
        let _module_set = parse_module_set(&expr)?;
        Ok(())
    }

    #[test]
    fn parse_infinite_loop() -> anyhow::Result<()> {
        let expr = lexpr::from_str(
            r#"
                (module-set
                    ("my.module"
                        (const foo 1)
                        (const bar 2)
                        (export loop)
                        (const loop
                            (fn
                                #:loop
                                (branch #:loop)
                            )
                        )
                    )
                )
            "#,
        )?;
        let _module_set = parse_module_set(&expr)?;
        Ok(())
    }

    #[test]
    fn parse_global_reference_fails() -> anyhow::Result<()> {
        let expr = lexpr::from_str(
            r#"
                (module-set
                    ("my.module"
                        (global foo)
                        (const bar (list foo))
                    )
                )
            "#,
        )?;
        let result = parse_module_set(&expr);
        assert!(
            matches!(result, Err(Error::Builder(BuilderError::ExpectedNonGlobal))),
            "found error {:?}",
            result.err()
        );
        Ok(())
    }
}
