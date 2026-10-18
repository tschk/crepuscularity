use std::io::Read;
use std::path::Path;

pub(super) fn typed_declarations(header: &Path) -> Result<syn::File, String> {
    validated_bindings(header).map(|(_, _, declarations)| declarations)
}

pub(super) fn validated_bindings(header: &Path) -> Result<(String, String, syn::File), String> {
    let header = header
        .canonicalize()
        .map_err(|error| format!("resolve ABI header {}: {error}", header.display()))?;
    let (types, source) = prepare(&header)?;
    let expected = expected_signatures(&source)?;
    // equilibrium applies base const to the outer pointer; restore qualifiers below.
    let unqualified = source
        .lines()
        .map(|line| {
            line.replace('*', " * ")
                .split_whitespace()
                .filter(|word| *word != "const")
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let generated = equilibrium_ffi::generate_bindings_from_content(
        &header,
        &unqualified,
        &equilibrium_ffi::BindingOptions::default(),
    )?;
    if !generated.warnings.is_empty() {
        return Err(generated.warnings.join("; "));
    }
    let mut code = format!("{types}\n{}", generated.code);
    let declarations =
        syn::parse_file(&code).map_err(|error| format!("parse typed ABI declarations: {error}"))?;
    validate_signatures(&expected, &declarations)?;
    restore_pointer_qualifiers(&source, &declarations, &mut code)?;
    let declarations = syn::parse_file(&code)
        .map_err(|error| format!("parse qualified ABI declarations: {error}"))?;
    Ok((source, code, declarations))
}

fn pointer_type(source: &str, generated: &syn::Type) -> Result<String, String> {
    let mut parts = source.split('*');
    let base = parts.next().unwrap_or_default();
    let mut is_const = base.split_whitespace().any(|word| word == "const");
    let mut pointers = Vec::new();
    for qualifier in parts {
        if !qualifier.split_whitespace().all(|word| word == "const") {
            return Err(format!("unsupported ABI pointer qualifiers: {source}"));
        }
        pointers.push(is_const);
        is_const = qualifier.split_whitespace().any(|word| word == "const");
    }
    let mut element = generated;
    for _ in &pointers {
        let syn::Type::Ptr(pointer) = element else {
            return Err(format!("ABI parser changed pointer depth: {source}"));
        };
        element = &pointer.elem;
    }
    let syn::Type::Path(path) = element else {
        return Err(format!("unsupported ABI pointer element: {source}"));
    };
    let mut ty = path
        .path
        .get_ident()
        .ok_or("unsupported ABI type path")?
        .to_string();
    for is_const in pointers {
        ty = format!("*{} {ty}", if is_const { "const" } else { "mut" });
    }
    Ok(ty)
}

fn rust_type(ty: &syn::Type) -> Result<String, String> {
    match ty {
        syn::Type::Path(path) => path
            .path
            .get_ident()
            .map(ToString::to_string)
            .ok_or_else(|| "unsupported ABI type path".into()),
        syn::Type::Ptr(pointer) => Ok(format!(
            "*{} {}",
            if pointer.const_token.is_some() {
                "const"
            } else {
                "mut"
            },
            rust_type(&pointer.elem)?
        )),
        syn::Type::Tuple(tuple) if tuple.elems.is_empty() => Ok("()".into()),
        _ => Err("unsupported generated ABI type".into()),
    }
}

fn restore_pointer_qualifiers(
    source: &str,
    declarations: &syn::File,
    code: &mut String,
) -> Result<(), String> {
    let prototypes: std::collections::HashMap<_, _> = source
        .lines()
        .filter_map(|line| {
            let (head, _) = line.split_once('(')?;
            Some((head.split_whitespace().last()?, line))
        })
        .collect();
    let mut corrected = std::collections::HashMap::new();
    for item in &declarations.items {
        let syn::Item::ForeignMod(module) = item else {
            continue;
        };
        for item in &module.items {
            let syn::ForeignItem::Fn(function) = item else {
                continue;
            };
            let name = function.sig.ident.to_string();
            let source_line = prototypes
                .get(name.as_str())
                .ok_or_else(|| format!("missing ABI prototype: {name}"))?;
            let (head, params) = source_line
                .split_once('(')
                .ok_or("missing ABI parameter list")?;
            let result = head
                .trim_end()
                .rsplit_once(char::is_whitespace)
                .ok_or("missing ABI result")?
                .0;
            let params = params
                .rsplit_once(')')
                .ok_or("missing ABI parameter list end")?
                .0;
            let mut arguments = Vec::new();
            for (param, input) in params.split(',').zip(&function.sig.inputs) {
                let syn::FnArg::Typed(input) = input else {
                    return Err("unsupported ABI receiver".into());
                };
                let syn::Pat::Ident(ident) = input.pat.as_ref() else {
                    return Err("unsupported ABI parameter name".into());
                };
                let c_type = param
                    .trim_end()
                    .rsplit_once(char::is_whitespace)
                    .ok_or("missing ABI parameter type")?
                    .0;
                let ty = if c_type.contains('*') {
                    pointer_type(c_type, &input.ty)?
                } else {
                    rust_type(&input.ty)?
                };
                arguments.push(format!("{}: {ty}", ident.ident));
            }
            let result = match &function.sig.output {
                syn::ReturnType::Default => String::new(),
                syn::ReturnType::Type(_, ty) => format!(
                    " -> {}",
                    if result.contains('*') {
                        pointer_type(result, ty)?
                    } else {
                        rust_type(ty)?
                    }
                ),
            };
            corrected.insert(
                name.clone(),
                format!("    pub fn {name}({}){result};", arguments.join(", ")),
            );
        }
    }
    let mut output = String::with_capacity(code.len());
    for line in code.lines() {
        let replacement = line
            .trim_start()
            .strip_prefix("pub fn ")
            .and_then(|line| line.split_once('('))
            .and_then(|(name, _)| corrected.remove(name));
        output.push_str(replacement.as_deref().unwrap_or(line));
        output.push('\n');
    }
    if !corrected.is_empty() {
        return Err("missing generated ABI function declarations".into());
    }
    *code = output;
    Ok(())
}

pub(super) fn prepare(header: &Path) -> Result<(String, String), String> {
    // Preserve equilibrium's header admission limits when using its content API.
    const MAX_BYTES: u64 = 10 * 1024 * 1024;
    let mut content = String::new();
    std::fs::File::open(header)
        .and_then(|file| file.take(MAX_BYTES + 1).read_to_string(&mut content))
        .map_err(|error| format!("read {}: {error}", header.display()))?;
    if content.len() as u64 > MAX_BYTES || content.lines().count() > 200_000 {
        return Err(format!(
            "ABI header exceeds size or line limit: {}",
            header.display()
        ));
    }

    let content = logical_declarations(&strip_comments(&splice_lines(&content))?)?;
    let mut source = String::new();
    let mut types = String::new();
    let mut lines = content.lines();
    while let Some(line) = lines.next() {
        let mut declaration = line.to_owned();
        if line.split_whitespace().next() == Some("typedef") {
            while !declaration.contains(';') {
                let next = lines.next().ok_or("unterminated ABI typedef")?;
                declaration.push(' ');
                declaration.push_str(next);
            }
        }
        let compact: String = declaration.chars().filter(|c| !c.is_whitespace()).collect();
        match compact.as_str() {
            "typedefstructCrepusSessionCrepusSession;" => types.push_str(
                "#[repr(C)]\npub struct CrepusSession { _private: [u8; 0] }\n",
            ),
            "typedefvoid(*CrepusEventCallback)(constchar*event_json,void*userdata);" => types.push_str(
                "pub type CrepusEventCallback = Option<unsafe extern \"C\" fn(*const c_char, *mut c_void)>;\n",
            ),
            _ if compact.starts_with("typedef")
                && (compact.contains("CrepusSession") || compact.contains("CrepusEventCallback")) =>
            {
                return Err("unsupported CrepusSession or CrepusEventCallback typedef".into());
            }
            _ => {
                // equilibrium 0.1.3 otherwise treats return stars as part of the function name.
                source.push_str(&declaration.replace('*', "* "));
                source.push('\n');
            }
        }
    }
    Ok((types, source))
}

fn splice_lines(content: &str) -> String {
    let mut output = String::new();
    let mut chars = content.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if chars.peek() == Some(&'\n') {
                chars.next();
                continue;
            }
            let mut next = chars.clone();
            if next.next() == Some('\r') && next.next() == Some('\n') {
                chars.next();
                chars.next();
                continue;
            }
        }
        output.push(ch);
    }
    output
}

fn strip_comments(content: &str) -> Result<String, String> {
    let mut chars = content.chars().peekable();
    let mut output = String::new();
    while let Some(ch) = chars.next() {
        match ch {
            '/' if chars.peek() == Some(&'/') => {
                for ch in chars.by_ref() {
                    if ch == '\n' {
                        output.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                output.push(' ');
                let mut closed = false;
                while let Some(ch) = chars.next() {
                    if ch == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        closed = true;
                        break;
                    }
                    if ch == '\n' {
                        output.push('\n');
                    }
                }
                if !closed {
                    return Err("unterminated ABI header comment".into());
                }
            }
            '"' | '\'' => {
                output.push(ch);
                let mut closed = false;
                while let Some(next) = chars.next() {
                    output.push(next);
                    if next == '\\' {
                        output.push(chars.next().ok_or("unterminated ABI header literal")?);
                    } else if next == ch {
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    return Err("unterminated ABI header literal".into());
                }
            }
            _ => output.push(ch),
        }
    }
    Ok(output)
}

fn logical_declarations(content: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut pending = String::new();
    let mut braces = 0usize;
    let mut wrappers = 0usize;
    let mut directive = false;
    let mut quote = None;
    let mut escaped = false;
    for line in content.lines() {
        let line = line.trim();
        if directive || line.starts_with('#') {
            if !pending.trim().is_empty() {
                return Err(
                    "preprocessor directive inside an ABI declaration is unsupported".into(),
                );
            }
            directive = line.ends_with('\\');
            continue;
        }
        for ch in line.chars() {
            if let Some(delimiter) = quote {
                pending.push(ch);
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == delimiter {
                    quote = None;
                }
                continue;
            }
            match ch {
                '"' | '\'' => {
                    quote = Some(ch);
                    pending.push(ch);
                }
                '{' if pending
                    .chars()
                    .filter(|ch| !ch.is_whitespace())
                    .eq("extern\"C\"".chars()) =>
                {
                    pending.clear();
                    wrappers += 1;
                }
                '{' => {
                    braces += 1;
                    pending.push(ch);
                }
                '}' if braces > 0 => {
                    braces -= 1;
                    pending.push(ch);
                }
                '}' if wrappers > 0 && pending.trim().is_empty() => wrappers -= 1,
                '}' => return Err("unmatched brace in ABI header".into()),
                ';' if braces == 0 => {
                    output.push_str(pending.trim());
                    output.push_str(";\n");
                    pending.clear();
                }
                _ => pending.push(ch),
            }
        }
        if !pending.is_empty() {
            pending.push(' ');
        }
    }
    if !pending.trim().is_empty() || braces != 0 || wrappers != 0 || directive {
        return Err("unterminated or unsupported ABI declaration".into());
    }
    Ok(output)
}

fn expected_signatures(source: &str) -> Result<std::collections::BTreeMap<String, usize>, String> {
    let mut expected = std::collections::BTreeMap::new();
    for line in source.lines() {
        if line.split_whitespace().next() == Some("typedef") {
            continue;
        }
        let mut braces = 0usize;
        let mut quote = None;
        let mut escaped = false;
        let open = line.char_indices().find_map(|(index, ch)| {
            if let Some(delimiter) = quote {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == delimiter {
                    quote = None;
                }
                return None;
            }
            match ch {
                '\"' | '\'' => quote = Some(ch),
                '{' => braces += 1,
                '}' => braces -= 1,
                '(' if braces == 0 => return Some(index),
                _ => {}
            }
            None
        });
        let Some(open) = open else { continue };
        let close = line.rfind(')').ok_or("incomplete ABI function prototype")?;
        if close <= open || line[..open].split_whitespace().count() < 2 {
            return Err("unsupported ABI function prototype or missing return type".into());
        }
        let name = line[..open]
            .split_whitespace()
            .last()
            .ok_or("missing ABI function name")?;
        let params = line[open + 1..close].trim();
        let mut depth = 0usize;
        let mut arity = usize::from(!params.is_empty() && params != "void");
        for ch in params.chars() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or("unbalanced ABI parameter list")?
                }
                ',' if depth == 0 => arity += 1,
                _ => {}
            }
        }
        if depth != 0 || expected.insert(name.to_owned(), arity).is_some() {
            return Err("unsupported or duplicate ABI function prototype".into());
        }
        if params != "void" && !params.is_empty() {
            if params.contains(['(', ')']) {
                return Err(
                    "inline function pointer or attribute parameters are unsupported".into(),
                );
            }
            for param in params.split(',') {
                let last = param
                    .split_whitespace()
                    .last()
                    .unwrap_or("")
                    .trim_start_matches('*');
                if last.is_empty()
                    || matches!(
                        last,
                        "void"
                            | "char"
                            | "short"
                            | "int"
                            | "long"
                            | "float"
                            | "double"
                            | "signed"
                            | "unsigned"
                            | "bool"
                            | "_Bool"
                            | "const"
                            | "volatile"
                            | "restrict"
                    )
                {
                    return Err(format!("unnamed or invalid ABI parameter in {name}"));
                }
            }
        }
    }
    Ok(expected)
}

fn validate_signatures(
    expected: &std::collections::BTreeMap<String, usize>,
    declarations: &syn::File,
) -> Result<(), String> {
    let mut actual = std::collections::BTreeMap::new();
    for item in &declarations.items {
        if let syn::Item::ForeignMod(module) = item {
            for item in &module.items {
                if let syn::ForeignItem::Fn(function) = item {
                    let name = function.sig.ident.to_string();
                    if expected.get(&name) != Some(&function.sig.inputs.len()) {
                        return Err(format!("ABI parser changed parameters for {name}; unnamed or unsupported parameters are not admitted"));
                    }
                    if actual.insert(name, function.sig.inputs.len()).is_some() {
                        return Err("duplicate generated ABI function".into());
                    }
                }
            }
        }
    }
    if &actual != expected {
        return Err("ABI parser omitted a function declaration".into());
    }
    Ok(())
}
