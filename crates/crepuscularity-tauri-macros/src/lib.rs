use heck::{ToLowerCamelCase, ToSnakeCase};
use proc_macro::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::{
    ext::IdentExt,
    parse::{Parse, ParseStream, Parser},
    parse_macro_input,
    punctuated::Punctuated,
    FnArg, GenericArgument, Ident, ItemFn, LitStr, Pat, PathArguments, ReturnType, Token, Type,
};

#[derive(Clone, Copy)]
enum ArgumentCase {
    Camel,
    Snake,
}

struct CommandAttributes {
    argument_case: Option<ArgumentCase>,
}

impl Parse for CommandAttributes {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut argument_case = None;
        while !input.is_empty() {
            let name = input.call(Ident::parse_any)?;
            if name != "rename_all" {
                return Err(syn::Error::new_spanned(
                    name,
                    "unsupported command option; expected rename_all",
                ));
            }
            if argument_case.is_some() {
                return Err(syn::Error::new_spanned(
                    name,
                    "duplicate rename_all command option",
                ));
            }
            input.parse::<Token![=]>()?;
            let value = input.parse::<LitStr>()?;
            argument_case = Some(match value.value().as_str() {
                "camelCase" => ArgumentCase::Camel,
                "snake_case" => ArgumentCase::Snake,
                _ => {
                    return Err(syn::Error::new_spanned(
                        value,
                        "expected \"camelCase\" or \"snake_case\"",
                    ))
                }
            });
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Self { argument_case })
    }
}

#[proc_macro_attribute]
pub fn command(attribute: TokenStream, item: TokenStream) -> TokenStream {
    let attributes = parse_macro_input!(attribute as CommandAttributes);
    let function = parse_macro_input!(item as ItemFn);
    let name = &function.sig.ident;
    let tauri = tauri_path();
    let bridge = format_ident!("__crepus_command_{}", name.unraw());
    let app = Ident::new("__crepus_app", proc_macro2::Span::mixed_site());
    let payload = Ident::new("__crepus_payload", proc_macro2::Span::mixed_site());
    let mut bindings = Vec::new();
    let mut arguments = Vec::new();
    let mut wire_keys = std::collections::HashSet::new();
    for (index, argument) in function.sig.inputs.iter().enumerate() {
        let FnArg::Typed(argument) = argument else {
            return quote!(compile_error!("crepuscularity-tauri commands cannot use a receiver");)
                .into();
        };
        let Pat::Ident(pattern) = argument.pat.as_ref() else {
            return quote!(compile_error!("crepuscularity-tauri command parameters need names");)
                .into();
        };
        let binding = format_ident!(
            "__crepus_argument_{index}",
            span = proc_macro2::Span::mixed_site()
        );
        let ty = &argument.ty;
        if let Some(state) = state_type(ty) {
            bindings.push(quote! {
                    let #binding: #ty = #app.state::<#state>()?;
            });
            arguments.push(quote!(#binding));
            continue;
        }
        if path_ident(ty).is_some_and(|ident| ident == "AppHandle") {
            bindings.push(quote! {
                let #binding: #ty = #app.handle();
            });
            arguments.push(quote!(#binding));
            continue;
        }
        if path_ident(ty).is_some_and(|ident| ident == "Window") {
            bindings.push(quote! {
                let #binding: #ty = #app.get_window("main").ok_or_else(|| "main window unavailable".to_string())?;
            });
            arguments.push(quote!(#binding));
            continue;
        }
        let key = pattern.ident.unraw().to_string();
        let key = match attributes.argument_case {
            Some(ArgumentCase::Camel) => key.to_lower_camel_case(),
            Some(ArgumentCase::Snake) => key.to_snake_case(),
            None => key,
        };
        if !wire_keys.insert(key.clone()) {
            return syn::Error::new_spanned(
                &pattern.ident,
                format!("duplicate command argument wire key `{key}`"),
            )
            .to_compile_error()
            .into();
        }
        match ty.as_ref() {
            Type::Reference(reference) if matches!(reference.elem.as_ref(), Type::Path(path) if path.path.is_ident("str")) =>
            {
                bindings.push(quote! {
                    let #binding: String = #tauri::serde_json::from_value(#payload.get(#key).cloned().unwrap_or(#tauri::serde_json::Value::Null))
                        .map_err(|error| format!("invalid command argument {}: {error}", #key))?;
                });
                arguments.push(quote!(&#binding));
            }
            _ => {
                bindings.push(quote! {
                    let #binding: #ty = #tauri::serde_json::from_value(#payload.get(#key).cloned().unwrap_or(#tauri::serde_json::Value::Null))
                        .map_err(|error| format!("invalid command argument {}: {error}", #key))?;
                });
                arguments.push(quote!(#binding));
            }
        }
    }
    let invoke = if function.sig.asyncness.is_some() {
        quote!(#tauri::block_on(#name(#(#arguments),*)))
    } else {
        quote!(#name(#(#arguments),*))
    };
    let call = match &function.sig.output {
        ReturnType::Default => quote! { #invoke; #tauri::serde_json::Value::Null },
        ReturnType::Type(_, output) if is_result(output) => {
            quote! { #invoke.map_err(|error| error.to_string())? }
        }
        ReturnType::Type(_, _) => quote! { #invoke },
    };
    quote! {
        #function
        #[doc(hidden)]
        pub fn #bridge(#app: &#tauri::App, #payload: #tauri::serde_json::Value) -> Result<#tauri::serde_json::Value, String> {
            #(#bindings)*
            #tauri::serde_json::to_value(#call).map_err(|error| error.to_string())
        }
    }
    .into()
}

fn is_result(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "Result"))
}

fn state_type(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "State" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments.args.iter().find_map(|argument| match argument {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

fn path_ident(ty: &Type) -> Option<&Ident> {
    let Type::Path(path) = ty else {
        return None;
    };
    path.path.segments.last().map(|segment| &segment.ident)
}

fn tauri_path() -> proc_macro2::TokenStream {
    match crate_name("crepuscularity-tauri") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, proc_macro2::Span::call_site());
            quote!(::#ident)
        }
        Err(_) => quote!(::crepuscularity_tauri),
    }
}

#[proc_macro]
pub fn generate_handler(input: TokenStream) -> TokenStream {
    let names = match Punctuated::<Ident, Token![,]>::parse_terminated.parse(input) {
        Ok(names) => names,
        Err(error) => return error.into_compile_error().into(),
    };
    let tauri = tauri_path();
    let commands = names.iter().map(|name| {
        let bridge = format_ident!("__crepus_command_{}", name.unraw());
        let wire_name = name.unraw().to_string();
        quote!(#tauri::Command::new(#wire_name, #bridge))
    });
    quote!(vec![#(#commands),*]).into()
}
