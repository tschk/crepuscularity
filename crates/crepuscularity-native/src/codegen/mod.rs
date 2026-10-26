mod compose;
mod shared;
mod swiftui;

use crate::ir::ViewIr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCodegenTarget {
    SwiftUi,
    Compose,
}

pub fn generate_native_source(ir: &ViewIr, target: NativeCodegenTarget, view_name: &str) -> String {
    match target {
        NativeCodegenTarget::SwiftUi => swiftui::generate_swiftui(ir, view_name),
        NativeCodegenTarget::Compose => compose::generate_compose(ir, view_name),
    }
}
