<?php
require __DIR__ . '/CrepuscularityPlugin.php';

// Labelled CLI stub isolates the public HTML adapter from the Rust compiler.
$directory = sys_get_temp_dir() . '/crepus-link-' . bin2hex(random_bytes(8));
mkdir($directory, 0700);
$oldPath = getenv('PATH');
$oldBin = getenv('CREPUS_BIN');
$oldAllowed = CrepuscularityPlugin::$allowedDirectory;
try {
    $nodes = [
        ['kind' => 'stack', 'children' => [
            ['kind' => 'link', 'href' => '/雪?q="<&', 'hrefBind' => 'scope.next', 'target' => '"_blank', 'rel' => 'next & "safe"',
             'children' => [['kind' => 'text', 'content' => 'Help 雪 <&"'],
                            ['kind' => 'image', 'src' => '/icon?x="', 'alt' => 'icon']]],
        ]],
        ['kind' => 'link', 'href' => '', 'target' => '', 'rel' => null,
         'children' => [['kind' => 'text', 'content' => 'Empty']]],
        ['kind' => 'link', 'href' => '/plain', 'children' => []],
    ];
    $hrefs = [
        "JaVaScRiPt:alert(1)",
        "\x00 \tjava\nscript:alert(1)",
        "vB\rScRiPt:bad",
        "\x1fDaTa:text/html,bad",
        "&#106;avascript:literal",
        "/relative?q=1&x=2",
        "https://example.test/雪",
        "mailto:user@example.test",
    ];
    foreach ($hrefs as $href) {
        $nodes[] = ['kind' => 'link', 'href' => $href,
                    'children' => [['kind' => 'text', 'content' => 'safe']]];
    }
    file_put_contents($directory . '/input.crepus', 'a href="/help"' . "\n" . '  "Help"' . "\n");
    file_put_contents($directory . '/crepus', '#!' . PHP_BINARY . "\n<?php echo " . var_export(json_encode(['version' => 7, 'root' => $nodes], JSON_THROW_ON_ERROR), true) . ";\n");
    chmod($directory . '/crepus', 0700);
    putenv('PATH=' . $directory . PATH_SEPARATOR . $oldPath);
    putenv('CREPUS_BIN=crepus');
    CrepuscularityPlugin::$allowedDirectory = $directory;
    $expected = '<div data-crepus-kind="stack" data-axis="column"><a href="/雪?q=&quot;&lt;&amp;" target="&quot;_blank" rel="next &amp; &quot;safe&quot;">Help 雪 &lt;&amp;&quot;<img src="/icon?x=&quot;" alt="icon"></a></div><a href="" target="">Empty</a><a href="/plain"></a>';
    $expected .= '<a>safe</a><a>safe</a><a>safe</a><a>safe</a><a href="&amp;#106;avascript:literal">safe</a><a href="/relative?q=1&amp;x=2">safe</a><a href="https://example.test/雪">safe</a><a href="mailto:user@example.test">safe</a>';
    $actual = CrepuscularityPlugin::renderHtml($directory . '/input.crepus');
    if ($actual !== $expected) {
        throw new RuntimeException('link attributes/children HTML mismatch: ' . $actual);
    }
    echo "link attributes/children public HTML PASS\n";
} finally {
    putenv($oldPath === false ? 'PATH' : 'PATH=' . $oldPath);
    putenv($oldBin === false ? 'CREPUS_BIN' : 'CREPUS_BIN=' . $oldBin);
    CrepuscularityPlugin::$allowedDirectory = $oldAllowed;
    foreach (['input.crepus', 'crepus'] as $name) {
        if (is_file($directory . '/' . $name)) unlink($directory . '/' . $name);
    }
    rmdir($directory);
}
