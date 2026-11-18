<?php

final class CrepuscularityAbiSession
{
    private FFI $ffi;
    private FFI\CData $session;

    public function __construct(?string $libPath = null)
    {
        $root = dirname(__DIR__, 2);
        $defaultLib = PHP_OS_FAMILY === 'Darwin'
            ? "$root/target/debug/libcrepuscularity_abi.dylib"
            : "$root/target/debug/libcrepuscularity_abi.so";
        $this->ffi = FFI::cdef(<<<'CDEF'
typedef struct CrepusSession CrepusSession;
CrepusSession *crepus_session_new(void);
void crepus_session_free(CrepusSession *session);
int crepus_session_set_template_string(CrepusSession *session, const char *template_utf8, const char *base_dir_utf8);
int crepus_session_set_context_json(CrepusSession *session, const char *context_json_utf8);
int crepus_session_apply_context_patch_json(CrepusSession *session, const char *context_json_utf8);
char *crepus_session_render_ir_json(CrepusSession *session);
char *crepus_session_dispatch_event_json(CrepusSession *session, const char *event_json_utf8);
char *crepus_session_take_last_error(CrepusSession *session);
void crepus_string_free(char *ptr);
CDEF, $libPath ?: (getenv('CREPUS_ABI_LIB') ?: $defaultLib));
        $this->session = $this->ffi->crepus_session_new();
        if ($this->session === null) {
            throw new RuntimeException('crepus_session_new failed');
        }
    }

    public function close(): void
    {
        if (isset($this->session)) {
            $this->ffi->crepus_session_free($this->session);
            unset($this->session);
        }
    }

    public function __destruct()
    {
        $this->close();
    }

    public function setTemplate(string $template, ?string $baseDir = null): void
    {
        $this->check($this->ffi->crepus_session_set_template_string($this->session, $template, $baseDir));
    }

    public function setContext(array $context): void
    {
        $this->check($this->ffi->crepus_session_set_context_json($this->session, json_encode($context, JSON_THROW_ON_ERROR)));
    }

    public function patchContext(array $context): void
    {
        $this->check($this->ffi->crepus_session_apply_context_patch_json($this->session, json_encode($context, JSON_THROW_ON_ERROR)));
    }

    public function renderIr(): array
    {
        return $this->takeJson($this->ffi->crepus_session_render_ir_json($this->session));
    }

    public function dispatchEvent(string|array $event): array
    {
        $payload = is_array($event) ? json_encode($event, JSON_THROW_ON_ERROR) : $event;
        return $this->takeJson($this->ffi->crepus_session_dispatch_event_json($this->session, $payload));
    }

    private function check(int $code): void
    {
        if ($code !== 0) {
            throw new RuntimeException($this->takeError());
        }
    }

    private function takeJson(FFI\CData|null $ptr): array
    {
        if ($ptr === null) {
            throw new RuntimeException($this->takeError());
        }
        $raw = FFI::string($ptr);
        $this->ffi->crepus_string_free($ptr);
        return json_decode($raw, true, 512, JSON_THROW_ON_ERROR);
    }

    private function takeError(): string
    {
        $ptr = $this->ffi->crepus_session_take_last_error($this->session);
        if ($ptr === null) {
            return 'crepuscularity ABI call failed';
        }
        $raw = FFI::string($ptr);
        $this->ffi->crepus_string_free($ptr);
        return $raw;
    }
}
