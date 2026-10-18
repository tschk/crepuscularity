use std::collections::HashSet;
use std::path::Path;

use syn::{ForeignItem, Item, ReturnType, Type};

pub(super) fn generate(header: &Path) -> Result<String, String> {
    let declarations = super::header::typed_declarations(header)?;
    // Nim supplies raw ABI prototypes so cstring callbacks need no C constness casts.
    let mut code = String::new();
    let mut names = HashSet::new();
    let mut overloads: Vec<(String, Vec<String>)> = Vec::new();
    for item in declarations.items {
        match item {
            Item::Use(_) => {}
            Item::Struct(item) if item.ident == "CrepusSession" => code.push_str(
                "type CrepusSession* {.incompleteStruct.} = object\n"
            ),
            Item::Type(item) if item.ident == "CrepusEventCallback" => code.push_str(
                "type CrepusEventCallback* = proc(event_json: cstring; userdata: pointer) {.cdecl.}\n",
            ),
            Item::ForeignMod(module) => {
                if module.abi.name.as_ref().map(syn::LitStr::value).as_deref() != Some("C") {
                    return Err("unsupported Nim ABI calling convention".into());
                }
                for item in module.items {
                    let ForeignItem::Fn(function) = item else {
                        return Err("unsupported Nim ABI foreign declaration".into());
                    };
                    if function.sig.variadic.is_some() {
                        return Err("variadic Nim ABI functions are unsupported".into());
                    }
                    let name = function.sig.ident.to_string();
                    validate_identifier(&name)?;
                    let mut params = Vec::new();
                    let mut param_types = Vec::new();
                    let mut param_names = HashSet::new();
                    let mut emitted_types = Vec::new();
                    for input in function.sig.inputs {
                        let syn::FnArg::Typed(input) = input else {
                            return Err("unsupported Nim ABI receiver".into());
                        };
                        let syn::Pat::Ident(param) = *input.pat else {
                            return Err("unsupported Nim ABI parameter pattern".into());
                        };
                        validate_identifier(&param.ident.to_string())?;
                        if !param_names.insert(identifier_key(&param.ident.to_string())) {
                            return Err(format!("Nim parameter collision in {name}: {}", param.ident));
                        }
                        let ty = nim_type(&input.ty)?;
                        params.push(format!("`{}`: {ty}", param.ident));
                        param_types.push(type_key(&ty));
                        emitted_types.push(ty);
                    }
                    if !names.insert((identifier_key(&name), param_types)) {
                        return Err(format!("Nim identifier collision: {name}"));
                    }
                    let result = match function.sig.output {
                        ReturnType::Default => String::new(),
                        ReturnType::Type(_, ty) => format!(": {}", nim_type(&ty)?),
                    };
                    let key = identifier_key(&name);
                    let guards: Vec<String> = overloads.iter()
                        .filter(|(previous, types)| previous == &key && types.len() == emitted_types.len())
                        .filter(|(_, types)| types.iter().chain(&emitted_types).any(|ty| target_dependent_type(ty)))
                        .map(|(_, types)| types.iter().zip(&emitted_types)
                            .map(|(left, right)| format!("(({left}) is ({right}))"))
                            .collect::<Vec<_>>().join(" and "))
                        .collect();
                    let declaration = format!(
                        "proc `{name}`*({}){result} {{.importc: \"{name}\", cdecl.}}\n",
                        params.join("; ")
                    );
                    if guards.is_empty() {
                        code.push_str(&declaration);
                    } else {
                        code.push_str(&format!(
                            "when {}:\n  {{.error: \"Nim target-dependent identifier collision: {name}\".}}\nelse:\n  {declaration}",
                            guards.iter().map(|guard| format!("({guard})")).collect::<Vec<_>>().join(" or ")
                        ));
                    }
                    overloads.push((key, emitted_types));
                }
            }
            _ => return Err("unsupported Nim ABI declaration".into()),
        }
    }
    Ok(code)
}

fn nim_type(ty: &Type) -> Result<String, String> {
    match ty {
        Type::Ptr(pointer) => match pointer.elem.as_ref() {
            Type::Path(path) if path.path.is_ident("c_char") => Ok("cstring".into()),
            Type::Path(path) if path.path.is_ident("c_void") => Ok("pointer".into()),
            Type::Path(path) if path.path.is_ident("CrepusSession") => {
                Ok("ptr CrepusSession".into())
            }
            ty => Ok(format!("ptr {}", nim_type(ty)?)),
        },
        Type::Path(path) if path.qself.is_none() => {
            let name = path
                .path
                .get_ident()
                .ok_or("unsupported Nim ABI type path")?
                .to_string();
            Ok(match name.as_str() {
                "CrepusEventCallback" => name,
                "c_char" => "cchar".into(),
                "c_schar" => "cschar".into(),
                "c_uchar" => "cuchar".into(),
                "c_short" => "cshort".into(),
                "c_ushort" => "cushort".into(),
                "c_int" => "cint".into(),
                "c_uint" => "cuint".into(),
                "c_long" => "clong".into(),
                "c_ulong" => "culong".into(),
                "c_longlong" => "clonglong".into(),
                "c_ulonglong" => "culonglong".into(),
                "i8" => "int8".into(),
                "i16" => "int16".into(),
                "i32" => "int32".into(),
                "i64" => "int64".into(),
                "u8" => "uint8".into(),
                "u16" => "uint16".into(),
                "u32" => "uint32".into(),
                "u64" => "uint64".into(),
                "c_float" | "f32" => "float32".into(),
                "c_double" | "f64" => "float64".into(),
                "bool" => "bool".into(),
                _ => return Err(format!("unsupported Nim ABI type: {name}")),
            })
        }
        _ => Err("unsupported Nim ABI type".into()),
    }
}

// Backticks admit keywords, but do not relax Nim's underscore token rules.
fn validate_identifier(name: &str) -> Result<(), String> {
    if name != "_" && (name.starts_with('_') || name.ends_with('_') || name.contains("__")) {
        return Err(format!("unsupported Nim identifier: {name}"));
    }
    Ok(())
}

fn target_dependent_type(ty: &str) -> bool {
    matches!(ty.trim_start_matches("ptr "), "clong" | "culong")
}

// Compare Nim aliases by identity without changing the emitted C ABI spelling.
fn type_key(ty: &str) -> String {
    if let Some(element) = ty.strip_prefix("ptr ") {
        return format!("ptr {}", type_key(element));
    }
    match ty {
        "cchar" | "cuchar" => "char",
        "cschar" => "int8",
        "cshort" => "int16",
        "cint" => "int32",
        "clonglong" => "int64",
        "cushort" => "uint16",
        "cuint" => "uint32",
        "culonglong" => "uint64",
        // clong/culong identity is checked by emitted target-dependent guards.
        _ => ty,
    }
    .to_owned()
}

// Nim ignores underscores and case after an identifier's first character.
fn identifier_key(name: &str) -> String {
    name.chars()
        .filter(|ch| *ch != '_')
        .enumerate()
        .map(|(index, ch)| {
            if index == 0 {
                ch
            } else {
                ch.to_ascii_lowercase()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{identifier_key, validate_identifier};

    #[test]
    fn identifier_style_keeps_first_character_case() {
        assert_eq!(
            identifier_key("crepus_extra"),
            identifier_key("crepusExtra")
        );
        assert_ne!(identifier_key("CrepusExtra"), identifier_key("crepusExtra"));
    }

    #[test]
    fn lone_underscore_remains_a_valid_nim_identifier() {
        assert!(validate_identifier("_").is_ok());
    }
}
