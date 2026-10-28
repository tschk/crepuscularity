//! Isolate stack-overflow regressions so an abort cannot kill the test runner.
use crepuscularity_reactive::{batch_begin, batch_end, Memo, Signal};

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn warm_memo_chain_validates_on_a_small_stack() {
    const CHILD: &str = "CREPUS_WARM_CHAIN_CHILD";
    if std::env::var_os(CHILD).is_some() {
        std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(check_chain)
            .unwrap()
            .join()
            .unwrap();
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "warm_memo_chain_validates_on_a_small_stack",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test::wasm_bindgen_test]
fn warm_memo_chain_validates_in_wasm() {
    check_chain();
}

fn check_chain() {
    let source = Signal::new(0usize);
    let read = source.clone();
    let mut chain = vec![Memo::new(move || read.get() % 2)];
    assert_eq!(chain[0].get(), 0);
    for depth in 1..16384 {
        let previous = chain.last().unwrap().clone();
        let memo = Memo::new(move || previous.get() + 1);
        assert_eq!(memo.get(), depth);
        chain.push(memo);
    }
    batch_begin();
    source.set(2);
    assert_eq!(chain.last().unwrap().get(), 16383);
    batch_end();
    batch_begin();
    source.set(3);
    assert_eq!(chain.last().unwrap().get(), 16384);
    batch_end();
    for memo in chain.into_iter().rev() {
        memo.dispose();
    }
}
