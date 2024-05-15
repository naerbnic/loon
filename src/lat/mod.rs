//! A description of a text format to describe the contents of a Loon VM program.

use std::collections::HashMap;

use crate::binary::{
    modules::{ModuleId, ModuleMemberId},
    ConstModule, ConstValue, ModuleBuilder, ValueRef,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Lexpr(#[from] lexpr::parse::Error),

    #[error("Unexpected error type")]
    UnexpectedValueType,

    #[error("Unexpected symbol")]
    UnexpectedSymbol,

    #[error("Invalid module name")]
    InvalidModuleName,

    #[error("Wrong param size")]
    WrongParamSize,
}

type Result<T> = std::result::Result<T, Error>;

pub struct ModuleSet(HashMap<ModuleId, ConstModule>);

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
    Ok(iter
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| Error::WrongParamSize)?)
}

fn parse_list_with_head<'a>(head: &str, expr: &'a lexpr::Value) -> Result<&'a lexpr::Value> {
    let (head_symbol, contents) = parse_list_with_initial_symbol(expr)?;
    if head_symbol != head {
        return Err(Error::UnexpectedSymbol);
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
    let mut module_map = HashMap::new();
    for module_expr in modules.list_iter().ok_or(Error::UnexpectedValueType)? {
        let (module_id, module) = parse_module(module_expr)?;
        module_map.insert(module_id, module);
    }
    todo!()
}

struct ImportItem<'a> {
    local_name: &'a str,
    module_id: ModuleId,
    member_id: ModuleMemberId,
}

struct ExportItem<'a> {
    local_name: &'a str,
}

struct ConstantItem<'a> {
    local_name: &'a str,
    const_value: ValueRef,
}

enum ModuleItem<'a> {
    Import(ImportItem<'a>),
    Export(ExportItem<'a>),
    Const(ConstantItem<'a>),
}

fn parse_module(expr: &lexpr::Value) -> Result<(ModuleId, ConstModule)> {
    let (module_str_value, module_contents) = parse_cons(expr)?;
    let module_id = parse_module_id(parse_str(module_str_value)?)?;
    let mut items = Vec::new();
    for module_item_expr in module_contents
        .list_iter()
        .ok_or(Error::UnexpectedValueType)?
    {
        items.push(parse_module_item(todo!(), module_item_expr)?)
    }

    todo!()
}

fn parse_module_item<'a>(
    builder: &ModuleBuilder,
    item: &'a lexpr::Value,
) -> Result<ModuleItem<'a>> {
    let (first, rest) = parse_cons(item)?;
    let item = match parse_symbol(first)? {
        "import" => ModuleItem::Import(parse_import_item(rest)?),
        "export" => ModuleItem::Export(parse_export_item(rest)?),
        "const" => ModuleItem::Const(parse_constant_item(builder, rest)?),
        _ => return Err(Error::UnexpectedSymbol),
    };
    Ok(item)
}

fn parse_import_item(body: &lexpr::Value) -> Result<ImportItem> {
    // Has the form (import <name-sym> <module-id-str> <module-item-symbol>)
    let [local_name, module_id_str, member_symbol] = parse_const_len_list(body)?;
    Ok(ImportItem {
        local_name: parse_symbol(local_name)?,
        module_id: parse_module_id(parse_str(module_id_str)?)?,
        member_id: ModuleMemberId::new(parse_symbol(member_symbol)?),
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
    let [local_name, const_value] = parse_const_len_list(body)?;
    Ok(ConstantItem {
        local_name: parse_symbol(local_name)?,
        const_value: parse_constant_value(builder, const_value)?,
    })
}

fn parse_constant_value(builder: &ModuleBuilder, expr: &lexpr::Value) -> Result<ValueRef> {
    if let Some(i) = expr.as_i64() {
        Ok(builder.new_int(i))
    } else if let Some(f) = expr.as_f64() {
        Ok(builder.new_float(f))
    } else if let Some(b) = expr.as_bool() {
        Ok(builder.new_bool(b))
    } else if let Some(_cons) = expr.as_cons() {
        todo!("parse compound value")
    } else {
        Err(Error::UnexpectedValueType)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn parse_import_module_item_works() -> anyhow::Result<()> {
    //     let expr = lexpr::from_str(r#"(import foo "my.module" bar)"#)?;
    //     let ModuleItem::Import(imp) = parse_module_item(&expr)? else {
    //         return anyhow::bail!("Wrong type");
    //     };
    //     assert_eq!(imp.local_name, "foo");
    //     assert_eq!(imp.module_id, ModuleId::new(["my", "module"]));
    //     assert_eq!(imp.member_id, ModuleMemberId::new("bar"));
    //     Ok(())
    // }

    // #[test]
    // fn parse_export_module_item_works() -> anyhow::Result<()> {
    //     let expr = lexpr::from_str(r#"(export bar)"#)?;
    //     let ModuleItem::Export(exp) = parse_module_item(&expr)? else {
    //         return anyhow::bail!("Wrong type");
    //     };
    //     assert_eq!(exp.local_name, "bar");
    //     Ok(())
    // }
}
