<?php
require __DIR__ . '/CrepuscularityPlugin.php';

// Labelled CLI stub isolates the public HTML adapter from the Rust compiler.
$directory = sys_get_temp_dir() . '/crepus-secure-' . bin2hex(random_bytes(8));
mkdir($directory, 0700);
$oldPath = getenv('PATH');
$oldBin = getenv('CREPUS_BIN');
$oldAllowed = CrepuscularityPlugin::$allowedDirectory;
try {
    $nodes = [
        ['kind' => 'input', 'secure' => true, 'bind' => 'password', 'placeholder' => ''],
        ['kind' => 'input', 'secure' => false, 'bind' => 'plain', 'placeholder' => ''],
        ['kind' => 'input', 'secure' => true, 'multiline' => true, 'bind' => 'notes', 'placeholder' => ''],
    ];
    file_put_contents($directory . '/input.crepus', "input secure bind=password\n");
    file_put_contents($directory . '/crepus', '#!' . PHP_BINARY . "\n<?php echo " . var_export(json_encode(['version' => 7, 'root' => $nodes], JSON_THROW_ON_ERROR), true) . ";\n");
    chmod($directory . '/crepus', 0700);
    putenv('PATH=' . $directory . PATH_SEPARATOR . $oldPath);
    putenv('CREPUS_BIN=crepus');
    CrepuscularityPlugin::$allowedDirectory = $directory;
    $expected = '<input type="password" data-bind="password" placeholder=""><input data-bind="plain" placeholder=""><textarea data-bind="notes" placeholder=""></textarea>';
    $actual = CrepuscularityPlugin::renderHtml($directory . '/input.crepus');
    if ($actual !== $expected) {
        throw new RuntimeException('secure/plain/multiline HTML mismatch: ' . $actual);
    }
    echo "secure/plain/multiline public HTML PASS\n";
} finally {
    putenv($oldPath === false ? 'PATH' : 'PATH=' . $oldPath);
    putenv($oldBin === false ? 'CREPUS_BIN' : 'CREPUS_BIN=' . $oldBin);
    CrepuscularityPlugin::$allowedDirectory = $oldAllowed;
    foreach (['input.crepus', 'crepus'] as $name) {
        if (is_file($directory . '/' . $name)) unlink($directory . '/' . $name);
    }
    rmdir($directory);
}
