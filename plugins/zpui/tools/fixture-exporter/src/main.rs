use crepuscularity_core::TemplateContext;
use std::{env, error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let path = PathBuf::from(env::args_os().nth(1).ok_or("expected template path")?);
    let source = fs::read_to_string(&path)?;
    let ir = crepuscularity_native::render_template_to_ir_with_path(
        &source,
        &TemplateContext::default(),
        Some(&path),
    )?;
    if ir.version != 7 {
        return Err(format!("expected IR v7, got {}", ir.version).into());
    }
    println!("{}", crepuscularity_native::to_json_pretty(&ir)?);
    Ok(())
}
