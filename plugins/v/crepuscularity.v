module crepuscularity

import json
import os
import crypto.rand

pub struct ViewIr {
pub:
	version int
}

fn crepus_bin() string {
	return os.getenv_opt('CREPUS_BIN') or { 'crepus' }
}

fn run_cli(path string, context map[string]string, html bool) !string {
	nonce := rand.bytes(16)!
	ctx_dir := os.join_path(os.temp_dir(), 'crepus-context-${nonce.hex()}')
	os.mkdir(ctx_dir, mode: 0o700)!
	defer {
		os.rmdir_all(ctx_dir) or {}
	}
	ctx_path := os.join_path(ctx_dir, 'context.json')
	os.write_file(ctx_path, json.encode(context))!
	stderr_path := os.join_path(ctx_dir, 'stderr')
	command := if html { 'render' } else { 'native ir' }
	// os.execute merges stderr by default; keep diagnostics out of successful output.
	res := os.execute('${os.quoted_path(crepus_bin())} ${command} ${os.quoted_path(path)} --ctx ${os.quoted_path(ctx_path)} 2> ${os.quoted_path(stderr_path)}')
	if res.exit_code != 0 {
		diagnostic := os.read_file(stderr_path) or { '' }
		return error(if diagnostic.len > 0 { diagnostic } else { res.output })
	}
	return res.output
}

pub fn render_ir(path string, context map[string]string) !ViewIr {
	output := run_cli(path, context, false)!
	return json.decode(ViewIr, output)!
}

pub fn render_html(path string, context map[string]string) !string {
	return run_cli(path, context, true)!
}
