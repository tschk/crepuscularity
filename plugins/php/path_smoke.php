<?php
require __DIR__ . '/CrepuscularityPlugin.php';

$fixture = dirname(__DIR__) . '/fixtures/hello.crepus';
CrepuscularityPlugin::$allowedDirectory = DIRECTORY_SEPARATOR;
if (($argv[1] ?? '') === 'failure') {
    try {
        CrepuscularityPlugin::renderIr($fixture, ['name' => 'Ada']);
        throw new LogicException('expected CLI failure');
    } catch (RuntimeException $error) {
        if (!str_contains($error->getMessage(), 'diagnostic')) {
            throw $error;
        }
    }
    exit(0);
}
$ir = CrepuscularityPlugin::renderIr($fixture, ['name' => 'Ada']);
if ($ir['version'] !== 7 || !str_contains(json_encode($ir, JSON_THROW_ON_ERROR), 'Hello Ada')) {
    throw new RuntimeException('root directory did not permit template');
}
$largeName = str_repeat('x', 262144);
$frontend = CrepuscularityPlugin::renderIr(dirname($fixture) . '/frontend.svelte', ['name' => 'Ada']);
$frontendText = json_encode($frontend, JSON_THROW_ON_ERROR);
if (!str_contains($frontendText, 'Frontend content') || str_contains($frontendText, 'ignored')) {
    throw new RuntimeException('frontend filename was lost');
}
$largeIr = CrepuscularityPlugin::renderIr($fixture, ['name' => $largeName]);
if (!str_contains(json_encode($largeIr, JSON_THROW_ON_ERROR), $largeName)) {
    throw new RuntimeException('large context was truncated');
}
CrepuscularityPlugin::$allowedDirectory = dirname($fixture);
try {
    CrepuscularityPlugin::renderIr(__FILE__);
    throw new LogicException('outside entry file was permitted');
} catch (RuntimeException $error) {
    if (!str_contains($error->getMessage(), 'Path traversal detected')) {
        throw $error;
    }
}
