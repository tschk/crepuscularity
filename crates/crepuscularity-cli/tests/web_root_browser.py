"""Manual root-commit regression using the production DOM reconciliation source."""
import argparse
from pathlib import Path
from playwright.sync_api import sync_playwright

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--browser", choices=("chromium", "webkit"), default="chromium")
args = parser.parse_args()
source = (Path(__file__).resolve().parents[1] / "assets/web/app.js").read_text()
reconciliation = source[source.index("let restoringFocus"):source.index("// ── slot-rotate")]
with sync_playwright() as playwright:
    browser = getattr(playwright, args.browser).launch(headless=True)
    try:
        page = browser.new_page()
        page.set_default_timeout(30000)
        result = page.evaluate("""source => {
          const {setRootHtml} = new Function(source + '; return {setRootHtml};')();
          document.body.innerHTML = '<main id="root"><p>old</p></main>';
          const root = document.getElementById('root');
          const count = 150000;
          const start = performance.now();
          setRootHtml(root, '<i>x</i>'.repeat(count));
          const elapsed = performance.now() - start;
          if (root.childElementCount !== count || root.textContent.length !== count) {
            throw new Error('Incomplete large root commit');
          }
          setRootHtml(root, '<input id="edit" value="server"><!--tail-->text');
          const old = root.querySelector('input');
          old.value = 'draft'; old.focus(); old.setSelectionRange(1, 3);
          setRootHtml(root, '<input id="edit" value="server"><!--tail-->text');
          const next = root.querySelector('input');
          if (next === old || next.value !== 'server' || document.activeElement !== next ||
              next.selectionStart !== 1 || next.selectionEnd !== 3 || root.childNodes.length !== 3) {
            throw new Error('Form, focus, selection or mixed-node replacement changed');
          }
          setRootHtml(root, '<b>safe</b><script>window.unexpectedScript = true</script>');
          if (window.unexpectedScript || root.querySelector('b')?.textContent !== 'safe' || !root.querySelector('script')) throw new Error('Parser inertness changed');
          setRootHtml(root, '');
          if (root.childNodes.length) throw new Error('Empty replacement failed');
          return {count, elapsed};
        }""", reconciliation)
        print(f"PASS ({args.browser}): {result}")
    finally:
        browser.close()
