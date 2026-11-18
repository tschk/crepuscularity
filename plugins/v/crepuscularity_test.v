module crepuscularity

fn test_render_ir() {
	ir := render_ir('plugins/fixtures/hello.crepus', {
		'name': 'Ada'
	})!
	assert ir.version == 5
	html := render_html('plugins/fixtures/hello.crepus', {
		'name': 'Ada'
	})!
	assert html == '<div data-crepus-kind="stack" data-axis="column">Hello Ada</div>'
}
