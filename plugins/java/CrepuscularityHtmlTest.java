package dev.crepuscularity.plugin;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.Map;

/** Actual CLI HTML acceptance checks; run with CREPUS_BIN pointing to the built CLI. */
public final class CrepuscularityHtmlTest {
    private static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) throws Exception {
        if (args.length == 2 && args[0].equals("failure")) {
            try {
                CrepuscularityPlugin.renderHtml(args[1]);
                throw new AssertionError("expected CLI failure");
            } catch (IllegalStateException error) {
                require(error.getMessage().contains("html diagnostic"), error.toString());
            }
            Path context = Path.of(Files.readString(Path.of(System.getenv("CREPUS_HTML_CONTEXT_LOG"))).trim());
            require(!Files.exists(context), "failed CLI leaked context file: " + context);
            return;
        }
        Path dir = Files.createTempDirectory("crepus-java-html-test-");
        try {
            Path source = dir.resolve("index.crepus");
            Files.writeString(source, "span\n  \"First {name}\"\nbutton @click=\"save\"\n  \"Second\"\n");
            String html = CrepuscularityPlugin.renderHtml(source.toString(), Map.of("name", "雪 & \"quoted\"\nnext"));
            require(html.contains("First 雪 &amp; &quot;quoted&quot;\nnext"), html);
            require(html.contains("<button data-onclick=\"save\">Second</button>"), html);
            require(html.indexOf("First") < html.indexOf("Second"), html);

            Files.writeString(dir.resolve("part.crepus"), "span\n  \"Included {name}\"\n");
            Files.writeString(source, "include part.crepus name={name}\n");
            require(CrepuscularityPlugin.renderHtml(source.toString(), Map.of("name", "Ada")).contains("Included Ada"), "include context lost");

            Path frontend = dir.resolve("frontend.svelte");
            Files.writeString(frontend, "<p>Frontend {name}</p><button>Second</button>");
            html = CrepuscularityPlugin.renderHtml(frontend.toString(), Map.of("name", "Ada"));
            require(html.contains("Frontend Ada") && html.contains("Second"), html);

            Files.writeString(source, "span\n  \"Value {name}\"\n");
            Files.writeString(dir.resolve("context.toml"), "name = \"AUTOLOAD-MUST-NOT-APPEAR\"\n");
            html = CrepuscularityPlugin.renderHtml(source.toString());
            require(!html.contains("AUTOLOAD-MUST-NOT-APPEAR"), html);
            var session = new CrepuscularityPlugin.ViewSession(source.toString(), Map.of("name", "Session"));
            require(session.renderHtml().contains("Value Session"), "session context lost");

            try {
                CrepuscularityPlugin.renderHtml(source.toString(), Map.of("nested", Map.of("name", "Ada")));
                throw new AssertionError("unsupported nested context silently accepted");
            } catch (IllegalStateException expected) {
                require(expected.getMessage().contains("context object values"), expected.toString());
            }
        } finally {
            try (var paths = Files.walk(dir)) {
                for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);
            }
        }
    }
}
