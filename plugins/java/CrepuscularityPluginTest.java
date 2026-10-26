package dev.crepuscularity.plugin;

import java.util.Map;
import java.nio.file.Files;
import java.nio.file.Path;

public final class CrepuscularityPluginTest {
    public static void main(String[] args) throws Exception {
        if (args.length > 1 && args[1].equals("failure")) {
            try {
                CrepuscularityPlugin.renderIr(args[0], Map.of("name", "Ada"));
                throw new AssertionError("expected CLI failure");
            } catch (IllegalStateException error) {
                if (!error.getMessage().contains("diagnostic")) throw error;
            }
            return;
        }
        var ir = CrepuscularityPlugin.renderIr(args[0], Map.of("name", "Ada"));
        if (ir.version() != 7 || !ir.json().startsWith("{") || !ir.json().contains("Hello Ada")) {
            throw new AssertionError("unexpected IR: " + ir);
        }
        var frontend = CrepuscularityPlugin.renderIr(Path.of(args[0]).getParent().resolve("frontend.svelte").toString(), Map.of("name", "Ada"));
        if (!frontend.json().contains("Frontend content") || frontend.json().contains("ignored")) {
            throw new AssertionError("frontend output: " + frontend);
        }
        var directory = Files.createTempDirectory("crepus-java-include-");
        var source = directory.resolve("index.crepus");
        var part = directory.resolve("part.crepus");
        try {
            Files.writeString(source, "include part.crepus name={name}\n");
            Files.writeString(part, "span\n  \"Included {name}\"\n");
            var included = CrepuscularityPlugin.renderIr(source.toString(), Map.of("name", "Ada"));
            if (!included.json().contains("Included Ada")) {
                throw new AssertionError("include output: " + included);
            }
        } finally {
            Files.deleteIfExists(source);
            Files.deleteIfExists(part);
            Files.deleteIfExists(directory);
        }
    }
}
