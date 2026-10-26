module crepuscularity

import os
import crypto.rand

fn native_event(_payload &char, userdata voidptr) {
	unsafe {
		mut events := &int(userdata)
		(*events)++
	}
}

fn test_native_abi_renders_and_releases_strings() {
	session := C.crepus_session_new()
	assert session != unsafe { nil }
	defer { C.crepus_session_free(session) }
	template := 'span\n  "Native ABI {count}"'
	assert C.crepus_session_set_template_string(session, template.str, unsafe { nil }) == 0
	context := '{"count":1}'
	assert C.crepus_session_set_context_json(session, context.str) == 0
	result := C.crepus_session_render_ir_json(session)
	assert result != unsafe { nil }
	defer { C.crepus_string_free(result) }
	text := unsafe { cstring_to_vstring(result) }
	assert text.contains('Native ABI 1')
	mut events := 0
	assert C.crepus_session_set_event_callback(session, native_event, &events) == 0
	event := '"bind:count:2"'
	updated := C.crepus_session_dispatch_event_json(session, event.str)
	assert updated != unsafe { nil }
	defer { C.crepus_string_free(updated) }
	updated_text := unsafe { cstring_to_vstring(updated) }
	assert updated_text.contains('Native ABI 2')
	assert events == 1
}

fn concurrent_version(version int) int {
	ir := render_ir('unused.crepus', {
		'version': version.str()
	}) or {
		eprintln(err.msg())
		return -1
	}
	return ir.version
}

fn test_concurrent_context_files_are_independent() {
	$if windows {
		return
	}
	nonce := rand.bytes(16)!
	directory := os.join_path(os.temp_dir(), 'crepus-v-test-${nonce.hex()}')
	os.mkdir(directory)!
	defer { os.rmdir_all(directory) or {} }
	binary := os.join_path(directory, 'crepus')
	os.write_file(binary, '#!/usr/bin/env python3\nimport json,sys,time\ntime.sleep(0.1)\nwith open(sys.argv[-1]) as f: context=json.load(f)\nprint(json.dumps({"version":int(context["version"])}))\n')!
	os.chmod(binary, 0o700)!
	original := os.getenv_opt('CREPUS_BIN')
	os.setenv('CREPUS_BIN', binary, true)
	defer {
		if value := original {
			os.setenv('CREPUS_BIN', value, true)
		} else {
			os.unsetenv('CREPUS_BIN')
		}
	}
	mut renders := []thread int{}
	for version in 1 .. 9 {
		renders << spawn concurrent_version(version)
	}
	mut versions := []int{}
	for render in renders {
		versions << render.wait()
	}
	for index, version in versions {
		assert version == index + 1
	}
}

fn test_render_ir() {
	ir := render_ir('plugins/fixtures/hello.crepus', {
		'name': 'Ada'
	})!
	assert ir.version == 7
	html := render_html('plugins/fixtures/hello.crepus', {
		'name': 'Ada'
	})!
	assert html.trim_space() == '<div class="flex flex-col gap-2"><span class="text-sm">Hello Ada</span></div>'
}

fn test_html_renders_all_nodes_with_escaping_and_actions() {
	html := render_html('plugins/fixtures/html-multiple.crepus', {
		'name': 'Not the template'
	})!
	assert html.trim_space() == '<span>First &lt;&amp;&quot;雪</span><span>Second</span><button data-onclick="save">Action</button>'
	if output := os.getenv_opt('CREPUS_HTML_OUTPUT') {
		os.write_file(output, html)!
	}
}

fn test_html_preserves_include_context_and_reports_missing_templates() {
	nonce := rand.bytes(16)!
	directory := os.join_path(os.temp_dir(), 'crepus-v-html-${nonce.hex()}')
	os.mkdir(directory)!
	defer { os.rmdir_all(directory) or {} }
	source := os.join_path(directory, 'view with spaces.crepus')
	os.write_file(source, 'include part.crepus name={name}\nspan "Second"')!
	os.write_file(os.join_path(directory, 'part.crepus'), 'span "Included {name}"')!
	os.write_file(os.join_path(directory, 'context.toml'), 'name = "AUTOLOAD-MUST-NOT-APPEAR"')!
	html := render_html(source, {
		'name': '雪<&'
	})!
	assert html.trim_space() == '<span>Included 雪&lt;&amp;</span><span>Second</span>'
	empty := render_html(source, {})!
	assert !empty.contains('AUTOLOAD-MUST-NOT-APPEAR')
	render_html(os.join_path(directory, 'missing.crepus'), {}) or {
		assert err.msg().len > 0
		return
	}
	assert false, 'missing template should fail'
}

fn test_successful_transport_keeps_large_stderr_out_of_response() {
	$if windows {
		return
	}
	nonce := rand.bytes(16)!
	directory := os.join_path(os.temp_dir(), 'crepus-v-transport-${nonce.hex()}')
	os.mkdir(directory)!
	defer { os.rmdir_all(directory) or {} }
	binary := os.join_path(directory, 'crepus')
	// Labelled CLI byte fixture: warning output exceeds a typical pipe buffer.
	os.write_file(binary, '#!/usr/bin/env python3\nimport json,sys\nwith open(sys.argv[-1]) as f: context=json.load(f)\nif context.get("fail"):\n sys.stderr.write("雪 failure diagnostic\\n")\n sys.exit(9)\nsys.stderr.write("雪 warning\\n" * 32768)\nsys.stderr.flush()\nif sys.argv[1] == "render": sys.stdout.write("<span>Clean 雪</span>")\nelse: print(json.dumps({"version":7,"root":[]}))\n')!
	os.chmod(binary, 0o700)!
	original := os.getenv_opt('CREPUS_BIN')
	os.setenv('CREPUS_BIN', binary, true)
	defer {
		if value := original {
			os.setenv('CREPUS_BIN', value, true)
		} else {
			os.unsetenv('CREPUS_BIN')
		}
	}
	mut failures := []string{}
	ir := render_ir('unused.crepus', {}) or {
		failures << 'successful IR rejected noisy stderr'
		ViewIr{}
	}
	if ir.version != 7 {
		failures << 'successful IR version changed'
	}
	html := render_html('unused.crepus', {}) or {
		failures << 'successful HTML rejected noisy stderr'
		''
	}
	if html != '<span>Clean 雪</span>' {
		failures << 'successful HTML includes stderr'
	}
	for html_route in [false, true] {
		mut rejected := false
		if html_route {
			render_html('unused.crepus', {
				'fail': 'yes'
			}) or {
				rejected = true
				assert err.msg().trim_space() == '雪 failure diagnostic'
				''
			}
		} else {
			render_ir('unused.crepus', {
				'fail': 'yes'
			}) or {
				rejected = true
				assert err.msg().trim_space() == '雪 failure diagnostic'
				ViewIr{}
			}
		}
		assert rejected, 'nonzero CLI exit accepted'
	}
	assert failures.len == 0, failures.join('; ')
}
