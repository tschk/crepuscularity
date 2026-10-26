mod batch;
mod dom;
mod effect;
mod hydrate;
mod memo;
mod runtime;
mod signal;

pub use batch::{batch_begin, batch_end, flush};
pub use effect::Effect;
pub use hydrate::hydrate_root;
pub use memo::Memo;
pub use signal::Signal;

#[cfg(all(target_arch = "wasm32", feature = "dom"))]
pub use dom::bindings::{
    bind_attr, bind_bool_attr, bind_checked, bind_class, bind_text, bind_value,
};
