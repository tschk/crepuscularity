<?php

final class CrepuscularityPlugin
{
    public const BIND_BLOCKLIST = ['baseDir', '_']; // ponytail: block security-sensitive keys only

    // Keep aligned with the native ViewStyle text-compatible input type contract.
    private const HTML_INPUT_TYPES = [
        'text', 'search', 'tel', 'url', 'email', 'password', 'number', 'date',
        'datetime-local', 'month', 'time', 'week', 'color', 'hidden',
    ];

    public static ?string $allowedDirectory = null;

    private static function crepusBin(): string
    {
        $bin = getenv('CREPUS_BIN');
        if ($bin === false || $bin === '') {
            return 'crepus';
        }

        if (preg_match('/[\x00-\x1F\x7F"\'<>|&;$]/', $bin)) {
            throw new RuntimeException('Invalid characters in CREPUS_BIN');
        }

        if (str_contains($bin, '/') || str_contains($bin, '\\')) {
            throw new RuntimeException('CREPUS_BIN must be a binary name, not a path');
        }
        return $bin;
    }

    public static function renderIr(string $path, ?array $context = null): array
    {
        $realPath = realpath($path);
        if ($realPath === false) {
            throw new RuntimeException('Invalid path: ' . $path);
        }
        $allowedDir = self::$allowedDirectory ?? getcwd();
        $realAllowed = realpath($allowedDir);
        if ($realAllowed === false || ($realPath !== $realAllowed && !str_starts_with($realPath, rtrim($realAllowed, DIRECTORY_SEPARATOR) . DIRECTORY_SEPARATOR))) {
            throw new RuntimeException('Path traversal detected');
        }

        $bin = self::crepusBin();
        $args = [$bin, 'native', 'ir', $realPath];
        $payload = null;
        if ($context !== null) {
            $payload = json_encode([
                'template' => file_get_contents($realPath),
                'filename' => $path,
                'context' => (object) $context,
                'baseDir' => dirname($realPath),
            ], JSON_THROW_ON_ERROR);
            $args = [$bin, 'native', 'ir', '--stdin-json'];
        }
        // A temporary stderr file cannot fill a pipe while stdout is drained.
        $stderrFile = tmpfile();
        if ($stderrFile === false) {
            throw new RuntimeException('could not create diagnostic buffer');
        }
        $stdinFile = null;
        try {
            // Seekable stdin cannot block against a child filling stdout first.
            $stdinFile = tmpfile();
            if ($stdinFile === false) {
                throw new RuntimeException('could not create input buffer');
            }
            $offset = 0;
            while ($payload !== null && $offset < strlen($payload)) {
                $written = fwrite($stdinFile, substr($payload, $offset));
                if ($written === false || $written === 0) {
                    throw new RuntimeException('could not write input buffer');
                }
                $offset += $written;
            }
            if (!rewind($stdinFile)) {
                throw new RuntimeException('could not rewind input buffer');
            }
            $descriptor = [
                0 => $stdinFile,
                1 => ['pipe', 'w'],
                2 => $stderrFile,
            ];
            $proc = proc_open($args, $descriptor, $pipes);
            if (!is_resource($proc)) {
                throw new RuntimeException('crepus native ir failed');
            }
            $stdout = stream_get_contents($pipes[1]);
            fclose($pipes[1]);
            $code = proc_close($proc);
            rewind($stderrFile);
            $stderr = stream_get_contents($stderrFile);
        } finally {
            if (is_resource($stdinFile)) {
                fclose($stdinFile);
            }
            fclose($stderrFile);
        }
        if ($code !== 0) {
            throw new RuntimeException(trim($stderr));
        }
        return json_decode($stdout, true, 512, JSON_THROW_ON_ERROR);
    }

    public static function renderHtml(string $path, ?array $context = null): string
    {
        return implode('', array_map([self::class, 'renderNode'], self::renderIr($path, $context)['root']));
    }

    private static function safeLinkHref(string $value): bool
    {
        // Match browser scheme parsing; entity literals are escaped by the caller.
        $normalized = str_replace(["\t", "\r", "\n"], '', ltrim($value, "\x00..\x20"));
        $colon = strpos($normalized, ':');
        return $colon === false || !in_array(strtolower(substr($normalized, 0, $colon)), ['javascript', 'vbscript', 'data'], true);
    }

    private static function escapeAttribute(string $value): string
    {
        return str_replace("\r", '&#13;', htmlspecialchars($value, ENT_QUOTES));
    }

    private static function renderInput(array $node): string
    {
        $bind = (string) ($node['bind'] ?? '');
        $attributes = ' data-bind="' . self::escapeAttribute($bind) . '" placeholder="'
            . self::escapeAttribute((string) ($node['placeholder'] ?? '')) . '"';
        $style = $node['style'] ?? [];
        if (is_string($style['formName'] ?? null)) {
            $attributes .= ' name="' . self::escapeAttribute($style['formName']) . '"';
        }
        if (($style['required'] ?? null) === true) $attributes .= ' required';
        $value = $bind === '' ? ($style['formValue'] ?? null) : null;
        if (!empty($node['multiline'])) {
            $text = $value === null ? '' : str_replace("\r", '&#13;', htmlspecialchars((string) $value, ENT_QUOTES));
            // HTML discards one initial LF; encode CR to retain it through parsing.
            if (str_starts_with($text, "\n")) $text = "\n" . $text;
            return '<textarea' . $attributes . '>' . $text . '</textarea>';
        }
        $type = !empty($node['secure']) ? 'password' : ($style['inputType'] ?? null);
        $typeAttribute = in_array($type, self::HTML_INPUT_TYPES, true)
            ? ' type="' . self::escapeAttribute($type) . '"' : '';
        return '<input' . $typeAttribute . $attributes
            . ($value === null ? '' : ' value="' . self::escapeAttribute((string) $value) . '"') . '>';
    }

    private static function renderNode(array $node): string
    {
        return match ($node['kind'] ?? '') {
            'text' => htmlspecialchars((string) ($node['content'] ?? ''), ENT_QUOTES),
            'stack', 'scroll' => '<div data-crepus-kind="' . self::escapeAttribute((string) $node['kind']) . '" data-axis="' . self::escapeAttribute((string) ($node['axis'] ?? 'column')) . '">' . implode('', array_map([self::class, 'renderNode'], $node['children'] ?? [])) . '</div>',
            'button' => '<button' . (isset($node['onClick']) ? ' data-onclick="' . self::escapeAttribute((string) $node['onClick']) . '"' : '') . '>' . htmlspecialchars((string) ($node['label'] ?? ''), ENT_QUOTES) . '</button>',
            'link' => '<a' . (self::safeLinkHref((string) ($node['href'] ?? ''))
                ? ' href="' . self::escapeAttribute((string) ($node['href'] ?? '')) . '"' : '')
                . (isset($node['target']) ? ' target="' . self::escapeAttribute((string) $node['target']) . '"' : '')
                . (isset($node['rel']) ? ' rel="' . self::escapeAttribute((string) $node['rel']) . '"' : '')
                . '>' . implode('', array_map([self::class, 'renderNode'], $node['children'] ?? [])) . '</a>',
            'image' => '<img src="' . self::escapeAttribute((string) ($node['src'] ?? '')) . '" alt="' . self::escapeAttribute((string) ($node['alt'] ?? '')) . '">',
            'slotRotate' => '<span data-crepus-kind="slotRotate">' . htmlspecialchars((string) (($node['phrases'] ?? [''])[0] ?? ''), ENT_QUOTES) . '</span>',
            'input' => self::renderInput($node),
            'picker' => '<select data-bind="' . self::escapeAttribute((string) ($node['bind'] ?? '')) . '">' . implode('', array_map(fn (array $option): string => '<option value="' . self::escapeAttribute((string) ($option['value'] ?? '')) . '">' . htmlspecialchars((string) ($option['label'] ?? ''), ENT_QUOTES) . '</option>', $node['options'] ?? [])) . '</select>',
            default => '',
        };
    }
}

final class CrepusViewSession
{
    public string $path;
    public array $context;
    private array $handlers = [];

    public function __construct(string $path, array $context = [])
    {
        $this->path = $path;
        $this->context = $context;
    }

    public function on(string $handler, callable $callback): self
    {
        $this->handlers[$handler] = $callback;
        return $this;
    }

    public function renderIr(): array
    {
        return CrepuscularityPlugin::renderIr($this->path, $this->context);
    }

    public function renderHtml(): string
    {
        return CrepuscularityPlugin::renderHtml($this->path, $this->context);
    }

    public function dispatch(string|array $event): array
    {
        $payload = is_string($event) ? ['handler' => $event] : $event;
        $handler = (string) ($payload['handler'] ?? '');
        if (str_starts_with($handler, 'bind:')) {
            $parts = explode(':', substr($handler, 5), 2);
            // ponytail: only allowlisted keys from bind:
            if (count($parts) === 2 && !in_array($parts[0], CrepuscularityPlugin::BIND_BLOCKLIST, true)) {
                $this->context[$parts[0]] = $parts[1];
            }
        }
        if (isset($this->handlers[$handler])) {
            ($this->handlers[$handler])($payload, $this);
        }
        return $this->renderIr();
    }
}
