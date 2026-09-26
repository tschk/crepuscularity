<?php

require_once __DIR__ . '/CrepuscularityPlugin.php';

$fixture = dirname(__DIR__) . '/fixtures/interactive.crepus';
$session = new CrepusViewSession($fixture, ['count' => '1']);

if (!str_contains($session->renderHtml(), 'Count 1')) {
    throw new RuntimeException('initial render did not include Count 1');
}

$ir = $session->dispatch('bind:count:2');

if (($session->context['count'] ?? null) !== '2') {
    throw new RuntimeException('dispatch did not update context');
}

if (!str_contains(json_encode($ir, JSON_THROW_ON_ERROR), 'Count 2')) {
    throw new RuntimeException('rerender did not include Count 2');
}

if (!str_contains($session->renderHtml(), 'Count 2')) {
    throw new RuntimeException('html rerender did not include Count 2');
}
