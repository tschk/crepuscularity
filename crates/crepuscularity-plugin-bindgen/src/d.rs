use std::path::Path;

use syn::{ForeignItem, Item, ReturnType, Type};

// Target lexer keywords and reserved tokens: https://dlang.org/spec/lex.html#keywords
const RESERVED_NAMES: &str =
    "abstract alias align asm assert auto body bool break byte case cast catch cdouble
cent cfloat char class const continue creal dchar debug default delegate delete
deprecated do double else enum export extern false final finally float for foreach
foreach_reverse function goto idouble if ifloat immutable import in inout int
interface invariant ireal is lazy long macro mixin module new nothrow null out
override package pragma private protected public pure real ref return scope shared
short static struct super switch synchronized template this throw true try typeid
typeof ubyte ucent uint ulong union unittest ushort version void wchar while with
__FILE__ __FILE_FULL_PATH__ __FUNCTION__ __LINE__ __MODULE__ __PRETTY_FUNCTION__
__gshared __parameters __rvalue __traits __vector __DATE__ __EOF__ __TIME__
__TIMESTAMP__ __VENDOR__ __VERSION__";

pub(super) fn generate(header: &Path) -> Result<String, String> {
    let declarations = super::header::typed_declarations(header)?;
    let mut code = String::from("module abi_generated;\nimport core.stdc.config;\n\n");
    for item in declarations.items {
        match item {
            Item::Use(_) => {}
            Item::Struct(item) if item.ident == "CrepusSession" => {
                code.push_str("struct CrepusSession;\n");
            }
            Item::Type(item) if item.ident == "CrepusEventCallback" => code.push_str(
                "alias CrepusEventCallback = extern(C) void function(const(char)* event_json, void* userdata);\n",
            ),
            Item::ForeignMod(module) => {
                if module.abi.name.as_ref().map(syn::LitStr::value).as_deref() != Some("C") {
                    return Err("unsupported D ABI calling convention".into());
                }
                for item in module.items {
                    let ForeignItem::Fn(function) = item else {
                        return Err("unsupported D ABI foreign declaration".into());
                    };
                    if function.sig.variadic.is_some() {
                        return Err("variadic D ABI functions are unsupported".into());
                    }
                    let name = function.sig.ident.to_string();
                    if RESERVED_NAMES.split_ascii_whitespace().any(|reserved| reserved == name) {
                        return Err(format!("unsupported D ABI function name {name:?}: reserved word"));
                    }
                    let mut params = Vec::new();
                    for input in function.sig.inputs {
                        let syn::FnArg::Typed(input) = input else {
                            return Err("unsupported D ABI receiver".into());
                        };
                        // Parameter names are not part of the ABI; omit them to avoid D keywords.
                        params.push(d_type(&input.ty)?);
                    }
                    let result = match function.sig.output {
                        ReturnType::Default => "void".into(),
                        ReturnType::Type(_, ty) => d_type(&ty)?,
                    };
                    code.push_str(&format!(
                        "extern(C) {result} {name}({});\n",
                        params.join(", ")
                    ));
                }
            }
            _ => return Err("unsupported D ABI declaration".into()),
        }
    }
    Ok(code)
}

fn d_type(ty: &Type) -> Result<String, String> {
    match ty {
        Type::Ptr(pointer) => {
            let element = match pointer.elem.as_ref() {
                Type::Path(path) if path.path.is_ident("c_void") => "void".into(),
                Type::Path(path) if path.path.is_ident("CrepusSession") => "CrepusSession".into(),
                ty => d_type(ty)?,
            };
            Ok(if pointer.const_token.is_some() {
                format!("const({element})*")
            } else {
                format!("{element}*")
            })
        }
        Type::Path(path) if path.qself.is_none() => {
            let name = path
                .path
                .get_ident()
                .ok_or("unsupported D ABI type path")?
                .to_string();
            Ok(match name.as_str() {
                "CrepusEventCallback" => name,
                "c_char" => "char".into(),
                "c_schar" | "i8" => "byte".into(),
                "c_uchar" | "u8" => "ubyte".into(),
                "c_short" | "i16" => "short".into(),
                "c_ushort" | "u16" => "ushort".into(),
                "c_int" | "i32" => "int".into(),
                "c_uint" | "u32" => "uint".into(),
                "c_long" => "c_long".into(),
                "c_ulong" => "c_ulong".into(),
                "c_longlong" | "i64" => "long".into(),
                "c_ulonglong" | "u64" => "ulong".into(),
                "c_float" | "f32" => "float".into(),
                "c_double" | "f64" => "double".into(),
                "bool" => "bool".into(),
                _ => return Err(format!("unsupported D ABI type: {name}")),
            })
        }
        _ => Err("unsupported D ABI type".into()),
    }
}
