use std::path::Path;

use syn::{ForeignItem, Item, ReturnType, Type};

pub(super) fn generate(header: &Path) -> Result<String, String> {
    let declarations = super::header::typed_declarations(header)?;
    let library = header
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or("ABI header has no UTF-8 library name")?;
    let library = format!("@\"{}\"", library.replace('"', "\"\""));
    let mut code = String::from(
        "using System;\nusing System.Runtime.InteropServices;\n\npublic static class EquilibriumImports\n{\n",
    );
    for item in declarations.items {
        match item {
            Item::Use(_) => {}
            Item::Struct(item) if item.ident == "CrepusSession" => {}
            Item::Type(item) if item.ident == "CrepusEventCallback" => {}
            Item::ForeignMod(module) => {
                if module.abi.name.as_ref().map(syn::LitStr::value).as_deref() != Some("C") {
                    return Err("unsupported C# ABI calling convention".into());
                }
                for item in module.items {
                    let ForeignItem::Fn(function) = item else {
                        return Err("unsupported C# ABI foreign declaration".into());
                    };
                    if function.sig.variadic.is_some() {
                        return Err("variadic C# ABI functions are unsupported".into());
                    }
                    let name = function.sig.ident.to_string();
                    let name = name.strip_prefix("r#").unwrap_or(&name);
                    if name == "EquilibriumImports" {
                        return Err(
                            "C# function name conflicts with generated class: EquilibriumImports"
                                .into(),
                        );
                    }
                    let mut params = Vec::new();
                    for (index, input) in function.sig.inputs.iter().enumerate() {
                        let syn::FnArg::Typed(input) = input else {
                            return Err("unsupported C# ABI receiver".into());
                        };
                        params.push(format!("{} p{index}", csharp_type(&input.ty)?));
                    }
                    let result = match &function.sig.output {
                        ReturnType::Default => "void",
                        ReturnType::Type(_, ty) => csharp_type(ty)?,
                    };
                    code.push_str(&format!(
                        "    [DllImport({library}, CallingConvention = global::System.Runtime.InteropServices.CallingConvention.Cdecl)]\n    public static extern {result} @{name}({});\n",
                        params.join(", ")
                    ));
                }
            }
            _ => return Err("unsupported C# ABI declaration".into()),
        }
    }
    code.push_str("}\n");
    Ok(code)
}

fn csharp_type(ty: &Type) -> Result<&'static str, String> {
    match ty {
        Type::Ptr(pointer) => {
            match pointer.elem.as_ref() {
                Type::Path(path)
                    if path.path.is_ident("c_void")
                        || path.path.is_ident("c_char")
                        || path.path.is_ident("CrepusSession") => {}
                element => {
                    csharp_type(element)?;
                }
            }
            Ok("IntPtr")
        }
        Type::Path(path) if path.qself.is_none() => {
            let name = path
                .path
                .get_ident()
                .ok_or("unsupported C# ABI type path")?
                .to_string();
            Ok(match name.as_str() {
                "CrepusEventCallback" => "IntPtr",
                "c_schar" | "i8" => "sbyte",
                "c_uchar" | "u8" => "byte",
                "c_short" | "i16" => "short",
                "c_ushort" | "u16" => "ushort",
                "c_int" | "i32" => "int",
                "c_uint" | "u32" => "uint",
                "c_longlong" | "i64" => "long",
                "c_ulonglong" | "u64" => "ulong",
                "c_float" | "f32" => "float",
                "c_double" | "f64" => "double",
                _ => return Err(format!("unsupported C# ABI type: {name}")),
            })
        }
        _ => Err("unsupported C# ABI type".into()),
    }
}
