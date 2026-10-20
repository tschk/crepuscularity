<?php
require __DIR__ . '/CrepuscularityPlugin.php';

// Labelled CLI stub isolates the public HTML adapter from the Rust compiler.
$directory = sys_get_temp_dir() . '/crepus-metadata-' . bin2hex(random_bytes(8));
mkdir($directory, 0700);
$oldPath = getenv('PATH');
$oldBin = getenv('CREPUS_BIN');
$oldAllowed = CrepuscularityPlugin::$allowedDirectory;
try {
    $fixture = json_decode(file_get_contents(__DIR__ . '/../fixtures/input-metadata.view-ir.json'), true, 512, JSON_THROW_ON_ERROR);
    $nodes = $fixture['root'];
    file_put_contents($directory . '/input.crepus', "input value=\"Ada\"\n");
    file_put_contents($directory . '/crepus', '#!' . PHP_BINARY . "\n<?php echo " . var_export(json_encode(['version' => 7, 'root' => $nodes], JSON_THROW_ON_ERROR), true) . ";\n");
    chmod($directory . '/crepus', 0700);
    putenv('PATH=' . $directory . PATH_SEPARATOR . $oldPath);
    putenv('CREPUS_BIN=crepus');
    CrepuscularityPlugin::$allowedDirectory = $directory;
    $expected = $fixture['expectedHtml'];
    $actual = CrepuscularityPlugin::renderHtml($directory . '/input.crepus');
    if ($actual !== $expected) {
        throw new RuntimeException('input metadata HTML mismatch: ' . $actual);
    }
    if ((new CrepusViewSession($directory . '/input.crepus'))->renderHtml() !== $actual) {
        throw new RuntimeException('session input metadata HTML mismatch');
    }
    if (($output = getenv('CREPUS_HTML_OUTPUT')) !== false) file_put_contents($output, $actual);
    echo "input metadata public HTML PASS\n";
} finally {
    putenv($oldPath === false ? 'PATH' : 'PATH=' . $oldPath);
    putenv($oldBin === false ? 'CREPUS_BIN' : 'CREPUS_BIN=' . $oldBin);
    CrepuscularityPlugin::$allowedDirectory = $oldAllowed;
    foreach (['input.crepus', 'crepus'] as $name) {
        if (is_file($directory . '/' . $name)) unlink($directory . '/' . $name);
    }
    rmdir($directory);
}
