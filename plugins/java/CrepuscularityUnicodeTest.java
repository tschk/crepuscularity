package dev.crepuscularity.plugin;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.Map;

/** Default: actual CLI Unicode acceptance. Optional transport mode uses a labelled byte fixture. */
public final class CrepuscularityUnicodeTest {
    @FunctionalInterface
    private interface CheckedCall { void run() throws Exception; }

    private static void reject(CheckedCall call, String label) throws Exception {
        boolean rejected = false;
        try { call.run(); }
        catch (Exception expected) { rejected = true; }
        if (!rejected) throw new AssertionError("Unpaired UTF-16 accepted: " + label);
    }

    private static void transport(String mode, String path, boolean html) throws Exception {
        try {
            String output = html ? CrepuscularityPlugin.renderHtml(path) : CrepuscularityPlugin.renderIr(path).json();
            if (!mode.equals("valid")) throw new AssertionError("Malformed CLI UTF-8 accepted: " + mode + " html=" + html);
            if (!output.contains("雪 🦋")) throw new AssertionError("CLI Unicode changed");
        } catch (java.nio.charset.CharacterCodingException expected) {
            if (!mode.equals("ff") && !mode.equals("sequence")) throw expected;
        } catch (IllegalStateException expected) {
            if (!mode.equals("failure") || !expected.getMessage().equals("雪 diagnostic\n")) throw expected;
        }
    }

    private static void largeTransport(String mode, String path) throws Exception {
        try {
            var ir = CrepuscularityPlugin.renderIr(path, Map.of("blob", "雪".repeat(400_000)));
            if (!mode.equals("large") || !ir.json().contains("Exact payload 雪"))
                throw new AssertionError("Incomplete write accepted or response changed: " + mode);
        } catch (java.io.IOException expected) {
            if (!mode.equals("large_unread")) throw expected;
        } catch (IllegalStateException expected) {
            if (!mode.equals("large_failure") || !expected.getMessage().equals("雪 early diagnostic\n")) throw expected;
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length == 4 && args[0].equals("transport")) {
            if (args[1].startsWith("large")) largeTransport(args[1], args[2]);
            else transport(args[1], args[2], args[3].equals("html"));
            return;
        }
        Path directory = Files.createTempDirectory("crepus-java-unicode-");
        try {
            Path source = directory.resolve("view.crepus");
            Files.writeString(source, "span \"{value}\"\n");
            String valid = "雪 🦋";
            Map<String, Object> context = Map.of("value", valid);
            if (!CrepuscularityPlugin.renderIr(source.toString(), context).json().contains(valid))
                throw new AssertionError("Valid Unicode changed in IR");
            if (!CrepuscularityPlugin.renderHtml(source.toString(), context).contains(valid))
                throw new AssertionError("Valid Unicode changed in HTML");
            String[] invalid = {"\uD800", "\uDC00", "\uD800x", "\uDC00\uD800"};
            for (int index = 0; index < invalid.length; index++) {
                String value = invalid[index];
                Map<String, Object> badValue = Map.of("value", value);
                Map<String, Object> badKey = Map.of(value, "ordinary");
                reject(() -> CrepuscularityPlugin.renderIr(source.toString(), badValue), "IR value " + index);
                reject(() -> CrepuscularityPlugin.renderHtml(source.toString(), badValue), "HTML value " + index);
                reject(() -> CrepuscularityPlugin.renderIr(source.toString(), badKey), "IR key " + index);
                reject(() -> CrepuscularityPlugin.renderHtml(source.toString(), badKey), "HTML key " + index);
            }
            System.out.println("Java actual CLI Unicode preservation/rejection PASS");
        } finally {
            try (var paths = Files.walk(directory)) {
                for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }
}
