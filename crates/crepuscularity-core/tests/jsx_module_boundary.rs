use crepuscularity_core::parse_template_with_path;
use std::path::Path;

#[test]
fn authored_modules_fail_instead_of_becoming_visible_template_text() {
    for source in [
        "export default function App() { return <p>Hello</p>; }",
        "export const App = () => <button>Hello</button>;",
        "export { App };",
        "export default(<p>Hello</p>);",
        "import App\nfrom './App'; <App/>",
        "import App from './App'; <App/>",
        "import { createSignal } from 'solid-js'; <p>Hello</p>",
        "import './styles.css'; <p>Hello</p>",
        "# template header\n{/* note */} export default <p>Hello</p>",
    ] {
        let error = parse_template_with_path(source, Some(Path::new("App.tsx"))).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("JavaScript module imports or exports"),
            "{source}: {error}"
        );
    }
}

#[test]
fn ordinary_text_and_nested_module_words_remain_templates() {
    for source in [
        "<p>export default function App</p>",
        "<p>import App from library</p>",
        "export goods",
        "important text",
        "exported text",
    ] {
        parse_template_with_path(source, Some(Path::new("view.jsx"))).unwrap();
    }
}
