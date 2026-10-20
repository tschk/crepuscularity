"""Exercise the actual C/C++ HTML helpers against the CLI and a labelled pipe fixture."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
CLI = os.environ.get("CREPUS_BIN", str(ROOT / "target/debug/crepus"))


def run(command, environment):
    return subprocess.run(command, env=environment, capture_output=True, timeout=10)


with tempfile.TemporaryDirectory(prefix="crepus-c-html-") as directory:
    work = Path(directory)
    c_source = work / "html.c"
    c_source.write_text(
        '#define main adapter_smoke_main\n#include '
        + json.dumps(str(ROOT / "plugins/c/crepuscularity_plugin.c"))
        + '\n#undef main\n'
        'int main(int argc, char **argv) {\n'
        '    char output[8192];\n'
        '    size_t cap = argc == 3 ? (size_t)strtoul(argv[2], NULL, 10) : sizeof(output);\n'
        '    if (argc < 2 || cap > sizeof(output)) return 2;\n'
        '    if (crepus_render_html(argv[1], output, cap) != 0) return 1;\n'
        '    fputs(output, stdout);\n'
        '    return 0;\n}\n'
    )
    cpp_source = work / "html.cpp"
    cpp_source.write_text(
        '#define main adapter_smoke_main\n#include '
        + json.dumps(str(ROOT / "plugins/cpp/crepuscularity_plugin.cpp"))
        + '\n#undef main\n#include <iostream>\n'
        'int main(int argc, char **argv) {\n'
        '    if (argc != 2) return 2;\n'
        '    try { std::cout << render_html(argv[1]); return 0; }\n'
        '    catch (const std::exception&) { return 1; }\n}\n'
    )
    consumers = [(work / "html-c", c_source, "cc", "c11"),
                 (work / "html-cpp", cpp_source, "c++", "c++17")]
    for binary, source, compiler, standard in consumers:
        subprocess.run([compiler, f"-std={standard}", "-Wall", "-Wextra", "-Werror",
                        str(source), "-o", str(binary)], check=True)

    source = work / "template's $(literal).crepus"
    (work / "part.crepus").write_text('span\n  "Included content"\n', encoding="utf-8")
    # Former native IR rendering did not autoload sibling context files.
    (work / "context.toml").write_text('name = "AUTOLOAD-MUST-NOT-APPEAR"\n')
    cases = [
        ('span\n  "First <&\\\"雪"\nspan\n  "Second"\nbutton @click="save"\n  "Action"\n', ".crepus"),
        ('include part.crepus\nspan\n  "Value {name}"\n', ".crepus"),
        ('<p>Frontend content</p><button>Second</button>', ".svelte"),
        ("", ".crepus"),
    ]
    environment = {**os.environ, "CREPUS_BIN": CLI}
    for content, extension in cases:
        template = source.with_suffix(extension)
        template.write_text(content, encoding="utf-8")
        expected = run([CLI, "render", str(template), "--ctx", "/dev/null"], environment)
        assert expected.returncode == 0, expected.stderr
        assert b"AUTOLOAD-MUST-NOT-APPEAR" not in expected.stdout
        if "First" in content:
            assert b"Second" in expected.stdout and b"<button" in expected.stdout
            assert b"&lt;&amp;&quot;" in expected.stdout
        for binary, *_ in consumers:
            result = run([str(binary), str(template)], environment)
            assert result.returncode == 0, (binary, result.stderr)
            assert result.stdout == expected.stdout, (binary, result.stdout, expected.stdout)
    for binary, *_ in consumers:
        result = run([str(binary), str(work / "missing.crepus")], environment)
        assert result.returncode != 0 and not result.stdout, (binary, result)

    # Labelled CLI collaborator checks HTML argv, fragmented reads, overflow and exit status.
    stub = work / "crepus"
    stub.write_text(
        f"#!{sys.executable}\n"
        "import os,sys,time\n"
        "assert sys.argv[1] == 'render' and sys.argv[3:] == ['--ctx', '/dev/null']\n"
        "data=b'x'*int(os.environ.get('CREPUS_HTML_LENGTH','0'))\n"
        "sys.stdout.buffer.write(data[:16]);sys.stdout.buffer.flush();time.sleep(0.02)\n"
        "sys.stdout.buffer.write(data[16:]);sys.stdout.buffer.flush()\n"
        "sys.exit(int(os.environ.get('CREPUS_HTML_STATUS','0')))\n"
    )
    stub.chmod(0o700)
    for length in [0, 8191, 8192, 131072]:
        for index, (binary, *_) in enumerate(consumers):
            result = run([str(binary), str(source)],
                         {**environment, "CREPUS_BIN": str(stub), "CREPUS_HTML_LENGTH": str(length)})
            if index == 0 and length >= 8192:
                assert result.returncode != 0 and not result.stdout, (binary, length)
            else:
                assert result.returncode == 0 and result.stdout == b"x" * length, (binary, length)
    for binary, *_ in consumers:
        result = run([str(binary), str(source)],
                     {**environment, "CREPUS_BIN": str(stub), "CREPUS_HTML_LENGTH": "5000",
                      "CREPUS_HTML_STATUS": "3"})
        assert result.returncode != 0 and not result.stdout, binary
    c_binary = str(consumers[0][0])
    for cap, length, expected_status in [(0, 0, 1), (1, 0, 0), (1, 1, 1)]:
        result = run([c_binary, str(source), str(cap)],
                     {**environment, "CREPUS_BIN": str(stub), "CREPUS_HTML_LENGTH": str(length)})
        assert result.returncode == expected_status and not result.stdout, (cap, length, result)
print("C/C++ canonical HTML, empty context, escaping, frontend/include, capacity and pipe checks passed")
