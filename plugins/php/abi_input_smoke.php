<?php

require __DIR__ . '/CrepuscularityAbi.php';

// Exercise the actual native ABI selected by CREPUS_ABI_LIB; no CLI/compiler stub.
$failures = [];
foreach (['template', 'baseDir', 'event'] as $boundary) {
    $session = new CrepuscularityAbiSession();
    try {
        $session->setTemplate('p "Original {count}"');
        $session->setContext(['count' => 1]);
        $original = $session->renderIr();
        $rejected = false;
        try {
            if ($boundary === 'template') {
                $session->setTemplate("p \"Truncated\"\0\np \"suffix\"");
            } elseif ($boundary === 'baseDir') {
                $session->setTemplate('p "Changed"', __DIR__ . "\0ignored");
            } else {
                $session->dispatchEvent('{"handler":"bind:count:9"}' . "\0INVALID");
            }
        } catch (InvalidArgumentException $error) {
            $rejected = true;
        }
        if (!$rejected) {
            $failures[] = "$boundary NUL was not rejected with InvalidArgumentException";
        }
        if ($session->renderIr() !== $original) {
            $failures[] = "$boundary NUL changed the original session state";
        }
    } finally {
        $session->close();
    }
}

$session = new CrepuscularityAbiSession();
try {
    $session->setTemplate('p "雪🦀 {value}"');
    $session->setContext(['value' => "left\0right"]);
    if (($session->renderIr()['root'][0]['content'] ?? null) !== "雪🦀 left\0right") {
        $failures[] = 'valid Unicode template or JSON-escaped context NUL changed';
    }
    $session->dispatchEvent(['handler' => 'noop', 'payload' => "payload\0data", 'context' => ['value' => "next\0value"]]);
    if (($session->renderIr()['root'][0]['content'] ?? null) !== "雪🦀 next\0value") {
        $failures[] = 'JSON-escaped array event NUL was not preserved';
    }
} finally {
    $session->close();
}

if ($failures !== []) {
    throw new RuntimeException(implode("\n", $failures));
}
echo "PHP ABI input admission passed\n";
