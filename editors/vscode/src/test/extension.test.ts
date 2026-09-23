import * as assert from 'assert';
import * as vscode from 'vscode';

suite('Extension Test Suite', () => {
  vscode.window.showInformationMessage('Start all tests.');

  test('Extension should be present', () => {
    assert.ok(vscode.extensions.getExtension('crepuscularity.crepuscularity-vscode'));
  });

  test('should register restart command', async () => {
    const ext = vscode.extensions.getExtension('crepuscularity.crepuscularity-vscode');
    if (ext && !ext.isActive) {
      await ext.activate();
    }
    const commands = await vscode.commands.getCommands(true);
    assert.ok(commands.includes('crepus.restartLanguageServer'), 'Restart command not registered');
  });
});
