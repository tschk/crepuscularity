<?php

require __DIR__ . '/CrepuscularityAbi.php';

if (PHP_OS_FAMILY === 'Windows') {
    throw new RuntimeException('PHP session ownership compiler fixture requires a Unix host');
}

// Count releases without freeing memory: a broken wrapper cannot double-free here.
$directory = sys_get_temp_dir() . '/crepus-php-ownership-' . bin2hex(random_bytes(8));
mkdir($directory, 0700);
$library = $directory . (PHP_OS_FAMILY === 'Darwin' ? '/stub.dylib' : '/stub.so');
try {
    $command = [getenv('CC') ?: 'cc'];
    array_push($command, ...(PHP_OS_FAMILY === 'Darwin' ? ['-dynamiclib'] : ['-shared', '-fPIC']));
    array_push($command, __DIR__ . '/session_ownership_stub.c', '-o', $library);
    $process = proc_open($command, [1 => ['pipe', 'w'], 2 => ['pipe', 'w']], $pipes);
    if (!is_resource($process)) {
        throw new RuntimeException('could not start ownership stub compiler');
    }
    $stdout = stream_get_contents($pipes[1]);
    $stderr = stream_get_contents($pipes[2]);
    fclose($pipes[1]);
    fclose($pipes[2]);
    if (proc_close($process) !== 0) {
        throw new RuntimeException("ownership stub compiler failed: $stdout$stderr");
    }
    $counter = FFI::cdef('int fake_free_count(void);', $library);
    $session = new CrepuscularityAbiSession($library);
    $session->close();
    $session->close();
    unset($session);
    gc_collect_cycles();
    if ($counter->fake_free_count() !== 1) {
        throw new RuntimeException('repeated close/destructor must release once');
    }
    $session = new CrepuscularityAbiSession($library);
    unset($session);
    gc_collect_cycles();
    if ($counter->fake_free_count() !== 2) {
        throw new RuntimeException('destructor must release an open session once');
    }
    $session = new CrepuscularityAbiSession($library);
    $copy = null;
    $rejected = false;
    try {
        $copy = clone $session;
    } catch (Throwable $error) {
        $rejected = true;
    }
    $session->close();
    $copy?->close();
    unset($session, $copy);
    gc_collect_cycles();
    if (!$rejected || $counter->fake_free_count() !== 3) {
        throw new RuntimeException('native session clone must be rejected; free count=' . $counter->fake_free_count());
    }
    echo "PHP session ownership passed\n";
} finally {
    if (is_file($library)) unlink($library);
    rmdir($directory);
}
