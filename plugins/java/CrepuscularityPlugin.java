package dev.crepuscularity.plugin;

import java.io.IOException;
import java.io.OutputStreamWriter;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.regex.Pattern;
import java.util.concurrent.FutureTask;

public final class CrepuscularityPlugin {
    public record ViewIr(int version, String json) {}
    public record Event(String handler, Object payload) {}

    @FunctionalInterface
    public interface EventHandler {
        void handle(Event event, ViewSession session) throws Exception;
    }

    public static final class ViewSession {
        private final String path;
        private final Map<String, Object> context;
        private final Map<String, EventHandler> handlers = new LinkedHashMap<>();

        public ViewSession(String path) {
            this(path, Map.of());
        }

        public ViewSession(String path, Map<String, Object> context) {
            this.path = path;
            this.context = new LinkedHashMap<>(context);
        }

        public String path() {
            return path;
        }

        public Map<String, Object> context() {
            return context;
        }

        public ViewSession on(String handler, EventHandler callback) {
            handlers.put(handler, callback);
            return this;
        }

        public ViewIr renderIr() throws Exception {
            return CrepuscularityPlugin.renderIr(path, context);
        }

        public String renderHtml() throws Exception {
            return CrepuscularityPlugin.renderHtml(path, context);
        }

        public ViewIr dispatch(String handler) throws Exception {
            return dispatch(new Event(handler, null));
        }

        public ViewIr dispatch(Event event) throws Exception {
            applyBind(event.handler());
            EventHandler callback = handlers.get(event.handler());
            if (callback != null) {
                callback.handle(event, this);
            }
            return renderIr();
        }

        private void applyBind(String handler) {
            if (!handler.startsWith("bind:")) {
                return;
            }
            String rest = handler.substring("bind:".length());
            int colon = rest.indexOf(':');
            if (colon <= 0) {
                return;
            }
            context.put(rest.substring(0, colon), rest.substring(colon + 1));
        }
    }

    public static ViewIr renderIr(String path) throws Exception {
        return renderIr(path, Map.of());
    }

    public static ViewIr renderIr(String path, Map<String, Object> context) throws Exception {
        Path templatePath = Path.of(path).toAbsolutePath();
        String template = Files.readString(templatePath);
        String json = runCli(List.of("native", "ir", "--stdin-json"),
            toJson(Map.of("template", template, "context", context, "filename", path,
                "baseDir", templatePath.getParent().toString())));
        var versionMatch = Pattern.compile("\"version\"\\s*:\\s*(\\d+)").matcher(json);
        if (!versionMatch.find()) {
            throw new IllegalStateException("IR version missing or invalid");
        }
        int version = Integer.parseInt(versionMatch.group(1));
        return new ViewIr(version, json);
    }

    public static String renderHtml(String path) throws Exception {
        return renderHtml(path, Map.of());
    }

    public static String renderHtml(String path, Map<String, Object> context) throws Exception {
        Path contextFile = Files.createTempFile("crepus-java-context-", ".json");
        try {
            Files.writeString(contextFile, toJson(context), StandardCharsets.UTF_8);
            return runCli(List.of("render", Path.of(path).toAbsolutePath().toString(),
                "--ctx", contextFile.toString()), "");
        } finally {
            Files.deleteIfExists(contextFile);
        }
    }

    private static String runCli(List<String> arguments, String input) throws Exception {
        String bin = System.getenv().getOrDefault("CREPUS_BIN", "crepus");
        var command = new java.util.ArrayList<String>();
        command.add(bin);
        command.addAll(arguments);
        Process process = new ProcessBuilder(command).start();
        var stderr = new FutureTask<>(() -> process.getErrorStream().readAllBytes());
        Thread reader = new Thread(stderr, "crepus-java-stderr");
        reader.setDaemon(true);
        reader.start();
        // Start draining stdout before writing a potentially large request.
        var stdout = new FutureTask<>(() -> process.getInputStream().readAllBytes());
        Thread outputReader = new Thread(stdout, "crepus-java-stdout");
        outputReader.setDaemon(true);
        outputReader.start();
        try {
            IOException writeError = null;
            try (OutputStreamWriter stdin = new OutputStreamWriter(process.getOutputStream(), StandardCharsets.UTF_8)) {
                stdin.write(input);
            } catch (IOException error) {
                writeError = error;
            }
            byte[] output = stdout.get();
            int code = process.waitFor();
            String diagnostic = new String(stderr.get(), StandardCharsets.UTF_8);
            if (code != 0) {
                throw new IllegalStateException(diagnostic);
            }
            if (writeError != null) throw writeError;
            return StandardCharsets.UTF_8.newDecoder().decode(ByteBuffer.wrap(output)).toString();
        } finally {
            process.destroy();
        }
    }

    private static String toJson(Object value) {
        if (value == null) return "null";
        if (value instanceof String string) return quoteJson(string);
        if (value instanceof Number || value instanceof Boolean) return value.toString();
        if (value instanceof Map<?, ?> map) {
            StringBuilder out = new StringBuilder("{");
            boolean first = true;
            for (Map.Entry<?, ?> entry : map.entrySet()) {
                if (!first) out.append(',');
                first = false;
                out.append(quoteJson(String.valueOf(entry.getKey()))).append(':').append(toJson(entry.getValue()));
            }
            return out.append('}').toString();
        }
        if (value instanceof Iterable<?> iterable) {
            StringBuilder out = new StringBuilder("[");
            boolean first = true;
            for (Object item : iterable) {
                if (!first) out.append(',');
                first = false;
                out.append(toJson(item));
            }
            return out.append(']').toString();
        }
        return quoteJson(String.valueOf(value));
    }

    private static String quoteJson(String value) {
        StringBuilder out = new StringBuilder("\"");
        for (int i = 0; i < value.length(); i++) {
            char ch = value.charAt(i);
            switch (ch) {
                case '"' -> out.append("\\\"");
                case '\\' -> out.append("\\\\");
                case '\b' -> out.append("\\b");
                case '\f' -> out.append("\\f");
                case '\n' -> out.append("\\n");
                case '\r' -> out.append("\\r");
                case '\t' -> out.append("\\t");
                default -> out.append(ch < 0x20 || Character.isSurrogate(ch) ? String.format("\\u%04x", (int) ch) : ch);
            }
        }
        return out.append('"').toString();
    }
}
