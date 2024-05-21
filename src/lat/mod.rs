//! A description of a text format to describe the contents of a Loon VM program.

use std::{cell::Cell, collections::HashMap};

use crate::binary::{
    error::BuilderError,
    module_set::ModuleSet,
    modules::{ImportSource, ModuleId, ModuleMemberId},
    ConstModule, DeferredValue, FunctionBuilder, ModuleBuilder, ValueRef,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Lexpr(#[from] lexpr::parse::Error),

    #[error("Unexpected value type")]
    UnexpectedValueType,

    #[error("Unexpected symbol: {0:?}")]
    UnexpectedSymbol(String),

    #[error("Invalid module name")]
    InvalidModuleName,

    #[error("Wrong param size")]
    WrongParamSize,

    #[error(transparent)]
    Builder(#[from] BuilderError),

    #[error("Unknown reference: {0}")]
    UnknownReference(String),
}

type Result<T> = std::result::Result<T, Error>;

// Helper to parse list with given head symbol
fn parse_list_with_initial_symbol(expr: &lexpr::Value) -> Result<(&str, &lexpr::Value)> {
    let cons = expr.as_cons().ok_or(Error::UnexpectedValueType)?;
    let head_symbol = cons.car().as_symbol().ok_or(Error::UnexpectedValueType)?;
    Ok((head_symbol, cons.cdr()))
}

fn parse_cons(expr: &lexpr::Value) -> Result<(&lexpr::Value, &lexpr::Value)> {
    let cons = expr.as_cons().ok_or(Error::UnexpectedValueType)?;
    Ok((cons.car(), cons.cdr()))
}

fn parse_symbol(expr: &lexpr::Value) -> Result<&str> {
    expr.as_symbol().ok_or(Error::UnexpectedValueType)
}

fn parse_str(expr: &lexpr::Value) -> Result<&str> {
    expr.as_str().ok_or(Error::UnexpectedValueType)
}

fn parse_const_len_list<const L: usize>(list: &lexpr::Value) -> Result<[&lexpr::Value; L]> {
    let iter = list.list_iter().ok_or(Error::UnexpectedValueType)?;
    iter.collect::<Vec<_>>()
        .try_into()
        .map_err(|_| Error::WrongParamSize)
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
    for module_expr in modules.list_iter().ok_or(Error::UnexpectedValueType)? {
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
    pub fn resolve(
        &self,
        builder: &ModuleBuilder,
        references: &HashMap<&str, ValueRef>,
    ) -> Result<()> {
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

enum ModuleItem<'a> {
    Import(ImportItem<'a>),
    Export(ExportItem<'a>),
    Const(ConstantItem<'a>),
}

fn parse_module(expr: &lexpr::Value) -> Result<ConstModule> {
    let (module_str_value, module_contents) = parse_cons(expr)?;
    let module_id = parse_module_id(parse_str(module_str_value)?)?;
    let builder = ModuleBuilder::new(module_id.clone());
    let mut items = Vec::new();
    for module_item_expr in module_contents
        .list_iter()
        .ok_or(Error::UnexpectedValueType)?
    {
        items.push(parse_module_item(&builder, module_item_expr)?)
    }

    resolve_items(&builder, &items)?;

    let module = builder.into_const_module()?;
    Ok(module)
}

fn gather_item_references<'a>(items: &[ModuleItem<'a>]) -> Result<HashMap<&'a str, ValueRef>> {
    let mut references = HashMap::new();
    for item in items {
        match item {
            ModuleItem::Const(constant) => {
                references.insert(constant.local_name, constant.value.clone());
            }
            ModuleItem::Import(import) => {
                references.insert(import.local_name, import.value_ref.clone());
            }
            _ => {}
        }
    }
    Ok(references)
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
                    .get(export.local_name)
                    .ok_or_else(|| Error::UnknownReference(export.local_name.to_string()))?
                    .export(ModuleMemberId::new(export.local_name))?;
            }
            _ => {}
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

fn parse_constant_expr(
    builder: &ModuleBuilder,
    references: &HashMap<&str, ValueRef>,
    expr: &lexpr::Value,
) -> Result<ValueRef> {
    let (value, deferred_value) = builder.new_deferred();
    resolve_constant_expr(builder, references, deferred_value, expr)?;
    Ok(value)
}

fn resolve_constant_expr(
    builder: &ModuleBuilder,
    references: &HashMap<&str, ValueRef>,
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
        if let Some(value) = references.get(name) {
            deferred.resolve_other(value)?;
        } else {
            return Err(Error::UnknownReference(name.to_string()));
        }
    } else if let Some(cons) = expr.as_cons() {
        resolve_constant_compound_expr(builder, references, deferred, cons)?;
    } else {
        return Err(Error::UnexpectedValueType);
    }
    Ok(())
}

fn resolve_constant_compound_expr(
    builder: &ModuleBuilder,
    references: &HashMap<&str, ValueRef>,
    deferred: DeferredValue,
    expr: &lexpr::Cons,
) -> Result<()> {
    let body = expr.cdr();
    match parse_symbol(expr.car())? {
        "list" => resolve_list_expr(builder, references, deferred, body)?,
        "fn" => resolve_fn_expr(builder, references, deferred, body)?,
        unknown_symbol => return Err(Error::UnexpectedSymbol(unknown_symbol.to_string())),
    }
    Ok(())
}

fn resolve_list_expr(
    builder: &ModuleBuilder,
    references: &HashMap<&str, ValueRef>,
    deferred: DeferredValue,
    expr: &lexpr::Value,
) -> Result<()> {
    let mut values = Vec::new();
    for item_expr in expr.list_iter().ok_or(Error::UnexpectedValueType)? {
        let (item, item_deferred) = builder.new_deferred();
        resolve_constant_expr(builder, references, item_deferred, item_expr)?;
        values.push(item);
    }
    deferred.resolve_list(values)?;
    Ok(())
}

fn resolve_fn_expr(
    builder: &ModuleBuilder,
    references: &HashMap<&str, ValueRef>,
    deferred: DeferredValue,
    body: &lexpr::Value,
) -> Result<()> {
    let mut fn_builder = deferred.into_function_builder();
    for inst_expr in body.list_iter().ok_or(Error::UnexpectedValueType)? {
        apply_fn_inst(builder, &mut fn_builder, references, inst_expr)?;
    }
    fn_builder.build()?;
    Ok(())
}

fn apply_fn_inst(
    builder: &ModuleBuilder,
    fn_builder: &mut FunctionBuilder,
    references: &HashMap<&str, ValueRef>,
    body: &lexpr::Value,
) -> Result<()> {
    match body {
        lexpr::Value::Keyword(kw) => {
            fn_builder.define_branch_target(kw);
        }
        lexpr::Value::Cons(cons) => {
            let (head, args) = (cons.car(), cons.cdr());
            match parse_symbol(head)? {
                "push" => {
                    let [value_expr] = parse_const_len_list(args)?;
                    let value = parse_constant_expr(builder, references, value_expr)?;
                    fn_builder.push_value(&value)?;
                }
                "add" => {
                    let [] = parse_const_len_list(args)?;
                    fn_builder.add();
                }
                "return" => {
                    let [num_args] = parse_const_len_list(args)?;
                    fn_builder.return_(num_args.as_i64().ok_or(Error::UnexpectedValueType)? as u32);
                }
                "return_dynamic" => {
                    let [] = parse_const_len_list(args)?;
                    fn_builder.return_dynamic();
                }
                "branch" => {
                    let [target] = parse_const_len_list(args)?;
                    fn_builder.branch(target.as_keyword().ok_or(Error::UnexpectedValueType)?);
                }
                "pop" => {
                    let [n_pop] = parse_const_len_list(args)?;
                    fn_builder.pop(n_pop.as_i64().ok_or(Error::UnexpectedValueType)? as u32);
                }
                unknown_opcode => return Err(Error::UnexpectedSymbol(unknown_opcode.to_string())),
            }
        }
        _ => return Err(Error::UnexpectedValueType),
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
            return anyhow::bail!("Wrong type");
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
            return anyhow::bail!("Wrong type");
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
}
