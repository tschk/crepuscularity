<?php
require __DIR__ . '/CrepuscularityPlugin.php';

$directory = sys_get_temp_dir() . '/crepus-context-' . bin2hex(random_bytes(8));
mkdir($directory, 0700);
$oldPath = getenv('PATH');
$oldBin = getenv('CREPUS_BIN');
$oldAllowed = CrepuscularityPlugin::$allowedDirectory;
try {
    file_put_contents($directory . '/input.crepus', "text \"Context\"\n");
    // Labelled CLI stub verifies the actual stdin envelope, without Rust parsing.
    $stub = <<<'STUB'
<?php
$payload = json_decode(stream_get_contents(STDIN), false, 512, JSON_THROW_ON_ERROR);
if (!isset($payload->context) || !($payload->context instanceof stdClass)) {
    fwrite(STDERR, 'context must be a JSON object');
    exit(7);
}
echo json_encode(['version' => 7, 'root' => [['kind' => 'text', 'content' => json_encode($payload->context, JSON_THROW_ON_ERROR)]]], JSON_THROW_ON_ERROR);
STUB;
    file_put_contents($directory . '/crepus', '#!' . PHP_BINARY . "\n" . $stub);
    chmod($directory . '/crepus', 0700);
    putenv('PATH=' . $directory . PATH_SEPARATOR . $oldPath);
    putenv('CREPUS_BIN=crepus');
    CrepuscularityPlugin::$allowedDirectory = $directory;
    $source = $directory . '/input.crepus';
    $direct = CrepuscularityPlugin::renderIr($source, []);
    $session = new CrepusViewSession($source);
    if ($direct['root'][0]['content'] !== '{}' || $session->renderIr()['root'][0]['content'] !== '{}'
        || $session->renderHtml() !== '{}') {
        throw new RuntimeException('empty context did not remain a JSON object');
    }
    $named = CrepuscularityPlugin::renderIr($source, ['name' => 'Ada', 'rows' => [['title' => 'Row']]]);
    $context = json_decode($named['root'][0]['content'], false, 512, JSON_THROW_ON_ERROR);
    if ($context->name !== 'Ada' || !is_array($context->rows)
        || !($context->rows[0] instanceof stdClass) || $context->rows[0]->title !== 'Row') {
        throw new RuntimeException('named context or nested list shape changed');
    }
    echo "empty and named context envelope PASS\n";
} finally {
    putenv($oldPath === false ? 'PATH' : 'PATH=' . $oldPath);
    putenv($oldBin === false ? 'CREPUS_BIN' : 'CREPUS_BIN=' . $oldBin);
    CrepuscularityPlugin::$allowedDirectory = $oldAllowed;
    foreach (['input.crepus', 'crepus'] as $name) {
        if (is_file($directory . '/' . $name)) unlink($directory . '/' . $name);
    }
    rmdir($directory);
}
