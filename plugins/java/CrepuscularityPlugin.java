package dev.crepuscularity.plugin;

import java.io.ByteArrayOutputStream;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

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
        String bin = System.getenv().getOrDefault("CREPUS_BIN", "crepus");
        Process process = new ProcessBuilder(List.of(bin, "native", "ir", "--stdin-json"))
            .redirectErrorStream(true)
            .start();
        try (OutputStreamWriter stdin = new OutputStreamWriter(process.getOutputStream(), StandardCharsets.UTF_8)) {
            stdin.write(toJson(Map.of("template", Files.readString(Path.of(path)), "context", context)));
        }
        ByteArrayOutputStream stdout = new ByteArrayOutputStream();
        process.getInputStream().transferTo(stdout);
        int code = process.waitFor();
        String json = stdout.toString(StandardCharsets.UTF_8);
        if (code != 0) {
            throw new IllegalStateException(json);
        }
        int version = json.contains("\"version\":5") || json.contains("\"version\": 5") ? 5 : -1;
        return new ViewIr(version, json);
    }

    public static String renderHtml(String path) throws Exception {
        return renderHtml(path, Map.of());
    }

    public static String renderHtml(String path, Map<String, Object> context) throws Exception {
        String json = renderIr(path, context).json();
        String text = extract(json, "\"content\":\"", "\"");
        return "<div data-crepus-kind=\"stack\" data-axis=\"column\">" + escapeHtml(text) + "</div>";
    }

    private static String extract(String src, String start, String end) {
        int s = src.indexOf(start);
        if (s < 0) return "";
        s += start.length();
        int e = src.indexOf(end, s);
        return e < 0 ? "" : src.substring(s, e);
    }

    private static String escapeHtml(String value) {
        return value.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\"", "&quot;");
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
                default -> out.append(ch < 0x20 ? String.format("\\u%04x", (int) ch) : ch);
            }
        }
        return out.append('"').toString();
    }
}
