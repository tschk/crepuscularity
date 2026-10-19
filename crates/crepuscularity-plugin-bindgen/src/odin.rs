use std::path::Path;

use syn::{ForeignItem, Item, ReturnType, Type};

// Target lexer keywords and reserved tokens: https://github.com/odin-lang/Odin/blob/master/src/tokenizer.cpp
const RESERVED_NAMES: &str =
    "import foreign package typeid when where if else for switch in not_in notin do
case break continue fallthrough defer return proc struct union enum bit_set
bit_field map dynamic auto_cast cast transmute distinct using context or_else
or_return or_break or_continue asm matrix";

pub(super) fn generate(header: &Path) -> Result<String, String> {
    let declarations = super::header::typed_declarations(header)?;
    let mut code = String::from(
        "package abi_generated\nimport c \"core:c\"\nforeign import crepus \"system:crepuscularity_abi\"\n\n\
// Returned strings are owned; release them with crepus_string_free.\n\
// Callback strings are borrowed/read-only until the next dispatch; never free them.\n\
// Synchronize shared sessions; callbacks must not reenter their own session.\n\n",
    );
    for item in declarations.items {
        match item {
            Item::Use(_) => {}
            Item::Struct(item) if item.ident == "CrepusSession" => {
                code.push_str("CrepusSession :: struct {}\n");
            }
            Item::Type(item) if item.ident == "CrepusEventCallback" => code.push_str(
                "CrepusEventCallback :: proc \"c\" (event_json: cstring, userdata: rawptr)\n",
            ),
            Item::ForeignMod(module) => {
                if module.abi.name.as_ref().map(syn::LitStr::value).as_deref() != Some("C") {
                    return Err("unsupported Odin ABI calling convention".into());
                }
                code.push_str("foreign crepus {\n");
                for item in module.items {
                    let ForeignItem::Fn(function) = item else {
                        return Err("unsupported Odin ABI foreign declaration".into());
                    };
                    if function.sig.variadic.is_some() {
                        return Err("variadic Odin ABI functions are unsupported".into());
                    }
                    let name = function.sig.ident.to_string();
                    if RESERVED_NAMES
                        .split_ascii_whitespace()
                        .any(|reserved| reserved == name)
                    {
                        return Err(format!(
                            "unsupported Odin ABI function name {name:?}: reserved word"
                        ));
                    }
                    let mut params = Vec::new();
                    for (index, input) in function.sig.inputs.into_iter().enumerate() {
                        let syn::FnArg::Typed(input) = input else {
                            return Err("unsupported Odin ABI receiver".into());
                        };
                        // C parameter names are not ABI data and may be Odin keywords.
                        params.push(format!("p{index}: {}", odin_type(&input.ty)?));
                    }
                    let result = match function.sig.output {
                        ReturnType::Default => String::new(),
                        ReturnType::Type(_, ty) => format!(" -> {}", odin_type(&ty)?),
                    };
                    code.push_str(&format!(
                        "    {name} :: proc \"c\" ({}){result} ---\n",
                        params.join(", ")
                    ));
                }
                code.push_str("}\n");
            }
            _ => return Err("unsupported Odin ABI declaration".into()),
        }
    }
    Ok(code)
}

fn odin_type(ty: &Type) -> Result<String, String> {
    match ty {
        Type::Ptr(pointer) => match pointer.elem.as_ref() {
            Type::Path(path) if path.path.is_ident("c_char") => Ok("cstring".into()),
            Type::Path(path) if path.path.is_ident("c_void") => Ok("rawptr".into()),
            Type::Path(path) if path.path.is_ident("CrepusSession") => Ok("^CrepusSession".into()),
            ty => Ok(format!("^{}", odin_type(ty)?)),
        },
        Type::Path(path) if path.qself.is_none() => {
            let name = path
                .path
                .get_ident()
                .ok_or("unsupported Odin ABI type path")?
                .to_string();
            Ok(match name.as_str() {
                "CrepusEventCallback"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "f32"
                | "f64"
                | "bool" => name,
                "c_float" => "f32".into(),
                "c_double" => "f64".into(),
                "c_schar" => "c.schar".into(),
                "c_uchar" => "c.uchar".into(),
                "c_short" => "c.short".into(),
                "c_ushort" => "c.ushort".into(),
                "c_int" => "c.int".into(),
                "c_uint" => "c.uint".into(),
                "c_long" => "c.long".into(),
                "c_ulong" => "c.ulong".into(),
                "c_longlong" => "c.longlong".into(),
                "c_ulonglong" => "c.ulonglong".into(),
                _ => return Err(format!("unsupported Odin ABI type: {name}")),
            })
        }
        _ => Err("unsupported Odin ABI type".into()),
    }
}
