//! Compiler-owned global access; authored names and expressions stay unchanged.
#[derive(Clone, Debug)]
pub(crate) struct SourceGlobals {
    realm_alias: Option<String>,
    pub(crate) realm: String,
    pub(crate) types: String,
    pub(crate) imports: String,
}
impl Default for SourceGlobals {
    fn default() -> Self {
        Self {
            realm_alias: None,
            realm: "globalThis".into(),
            types: "globalThis".into(),
            imports: String::new(),
        }
    }
}
impl SourceGlobals {
    pub(crate) fn is_active(&self) -> bool {
        self.realm_alias.is_some()
    }

    pub(crate) fn matches_realm(&self, expression: &oxc_ast::ast::Expression<'_>) -> bool {
        use oxc_ast::ast::Expression;
        let expression = expression.get_inner_expression();
        match self.realm_alias.as_deref() {
            None => expression.is_specific_id("globalThis"),
            Some(alias) => matches!(expression, Expression::CallExpression(call)
                if !call.optional && call.type_arguments.is_none()
                    && call.arguments.is_empty() && call.callee.is_specific_id(alias)),
        }
    }

    pub(crate) fn with_aliases(realm_alias: &str, types_alias: &str, typed: bool) -> Self {
        let mut imports = format!(
            "import{{sourceRealm as {realm_alias}}}from\"@tschk/moonshine-dom/platform\";\n"
        );
        if typed {
            imports.push_str(&format!("import type{{GlobalTypes as {types_alias}}}from\"@tschk/moonshine-dom/platform\";\n"));
        }
        Self {
            realm_alias: Some(realm_alias.into()),
            realm: format!("({realm_alias}())"),
            types: if typed {
                types_alias.into()
            } else {
                "globalThis".into()
            },
            imports,
        }
    }
}

/// Match Svelte's known-tag precision and open string-tag reference type.
pub(crate) fn dynamic_element_type(types: &str, tag: &str, namespace: &str) -> String {
    let map = match namespace {
        "http://www.w3.org/2000/svg" => format!("{types}.SVGElementTagNameMap"),
        "http://www.w3.org/1998/Math/MathML" => format!("{types}.MathMLElementTagNameMap"),
        _ => format!("({types}.HTMLElementTagNameMap & {{svg:{types}.SVGSVGElement}})"),
    };
    format!("(typeof {tag} extends infer __CrepusElementTag ? __CrepusElementTag extends keyof {map} ? {map}[__CrepusElementTag] : any : never)")
}
