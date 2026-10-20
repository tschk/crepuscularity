#[cfg(all(target_arch = "wasm32", feature = "reactive"))]
mod tests {
    use crepuscularity_web::reactive::{
        batch_begin, batch_end, bind_attr, bind_class, bind_text, Memo, Signal,
    };
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn public_bindings_track_batches_and_stop_after_disposal_without_a_dom() {
        let signal = Signal::new(1);
        let memo = {
            let signal = signal.clone();
            Memo::new(move || signal.get() * 2)
        };
        let seen = Rc::new(RefCell::new(Vec::new()));
        let text = {
            let memo = memo.clone();
            let seen = seen.clone();
            bind_text("missing-facade-text", move || {
                let value = memo.get();
                seen.borrow_mut().push(value);
                value.to_string()
            })
        };
        let attr = {
            let signal = signal.clone();
            bind_attr("missing-facade-attr", "data-value", move || {
                signal.get().to_string()
            })
        };
        let class = {
            let signal = signal.clone();
            bind_class("missing-facade-class", "active", move || signal.get() > 1)
        };
        assert_eq!(&*seen.borrow(), &[2]);
        batch_begin();
        signal.set(2);
        signal.set(3);
        batch_end();
        assert_eq!(&*seen.borrow(), &[2, 6]);
        text.dispose();
        attr.dispose();
        class.dispose();
        signal.set(4);
        assert_eq!(&*seen.borrow(), &[2, 6]);
        memo.dispose();
    }
}
