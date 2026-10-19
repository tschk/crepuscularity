use std::path::Path;

pub(super) fn generate(header: &Path) -> Result<String, String> {
    // Explicit block unsafety is required by the 2024 edition.
    super::header::validated_bindings(header)
        .map(|(_, code, _)| code.replace("\nextern \"C\" {", "\nunsafe extern \"C\" {"))
}
