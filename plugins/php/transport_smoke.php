<?php
require __DIR__ . '/CrepuscularityPlugin.php';

$directory = sys_get_temp_dir() . '/crepus-transport-' . bin2hex(random_bytes(8));
mkdir($directory, 0700);
$oldPath = getenv('PATH');
$oldBin = getenv('CREPUS_BIN');
$oldAllowed = CrepuscularityPlugin::$allowedDirectory;
try {
    file_put_contents($directory . '/input.crepus', 'span "Source"');
    // Labelled CLI stub: failed stdout is not a JSON response or a diagnostic.
    $stub = <<<'STUB'
<?php
if (in_array('--stdin-json', $argv, true)) stream_get_contents(STDIN);
fwrite(STDOUT, "\xffnot JSON");
fwrite(STDERR, "雪 specific parse diagnostic\n");
exit(9);
STUB;
    file_put_contents($directory . '/crepus', '#!' . PHP_BINARY . "\n" . $stub);
    chmod($directory . '/crepus', 0700);
    putenv('PATH=' . $directory . PATH_SEPARATOR . $oldPath);
    putenv('CREPUS_BIN=crepus');
    CrepuscularityPlugin::$allowedDirectory = $directory;
    // Run the existing context route first as a control, then the bare file route.
    foreach ([[], null] as $context) {
        $error = null;
        try {
            CrepuscularityPlugin::renderIr($directory . '/input.crepus', $context);
        } catch (RuntimeException $caught) {
            $error = $caught;
        }
        if ($error === null || $error->getMessage() !== '雪 specific parse diagnostic') {
            throw new RuntimeException(($context === null ? 'bare' : 'context')
                . ' transport lost CLI diagnostic: ' . ($error?->getMessage() ?? 'no error'));
        }
    }
    // Labelled Python child bounds a duplex regression without pcntl or extensions.
    $pressure = <<<'STUB'
#!/usr/bin/env python3
import json,os,sys,threading,time
threading.Thread(target=lambda:(time.sleep(5),os._exit(88)),daemon=True).start()
sys.stdout.buffer.write(b' ' * 262144)
sys.stdout.buffer.flush()
payload=json.loads(sys.stdin.buffer.read().decode('utf-8'))
assert payload['template'] == 'span "Source"'
assert payload['context']['blob'] == '雪' * 400000
sys.stdout.buffer.write(json.dumps({'version':7,'root':[{'kind':'text','content':'Exact payload 雪'}]},ensure_ascii=False).encode('utf-8'))
STUB;
    file_put_contents($directory . '/crepus', $pressure . "\n");
    $largeContext = ['blob' => str_repeat('雪', 400000)];
    $failures = [];
    foreach ([false, true] as $html) {
        try {
            $output = $html
                ? CrepuscularityPlugin::renderHtml($directory . '/input.crepus', $largeContext)
                : CrepuscularityPlugin::renderIr($directory . '/input.crepus', $largeContext)['root'][0]['content'];
            if ($output !== 'Exact payload 雪') $failures[] = 'large input response changed';
        } catch (Throwable $error) {
            $failures[] = ($html ? 'HTML' : 'IR') . ' duplex failed: ' . $error->getMessage();
        }
    }
    $earlyExit = <<<'STUB'
#!/usr/bin/env python3
import sys
sys.stderr.buffer.write('雪 early diagnostic\n'.encode('utf-8'))
sys.stderr.buffer.flush()
sys.exit(9)
STUB;
    file_put_contents($directory . '/crepus', $earlyExit . "\n");
    foreach ([false, true] as $html) {
        try {
            if ($html) CrepuscularityPlugin::renderHtml($directory . '/input.crepus', $largeContext);
            else CrepuscularityPlugin::renderIr($directory . '/input.crepus', $largeContext);
            $failures[] = 'early failure accepted';
        } catch (RuntimeException $error) {
            if ($error->getMessage() !== '雪 early diagnostic')
                $failures[] = 'early diagnostic lost: ' . $error->getMessage();
        }
    }
    if ($failures !== []) throw new RuntimeException(implode('; ', $failures));
    echo "failed transport diagnostics PASS\n";
} finally {
    putenv($oldPath === false ? 'PATH' : 'PATH=' . $oldPath);
    putenv($oldBin === false ? 'CREPUS_BIN' : 'CREPUS_BIN=' . $oldBin);
    CrepuscularityPlugin::$allowedDirectory = $oldAllowed;
    foreach (['input.crepus', 'crepus'] as $name) {
        if (is_file($directory . '/' . $name)) unlink($directory . '/' . $name);
    }
    rmdir($directory);
}
