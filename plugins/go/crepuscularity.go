package crepuscularity

import (
	"bytes"
	"encoding/json"
	"fmt"
	"html"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

type ViewIr struct {
	Version int              `json:"version"`
	Root    []map[string]any `json:"root"`
}

type Event struct {
	Handler string         `json:"handler"`
	Payload map[string]any `json:"payload,omitempty"`
}

type EventHandler func(Event, *ViewSession) error

type ViewSession struct {
	Path    string
	Context map[string]any
	events  map[string]EventHandler
}

func crepusBin() string {
	if bin := os.Getenv("CREPUS_BIN"); bin != "" {
		if filepath.IsAbs(bin) || filepath.Base(bin) == bin {
			return bin
		}
	}
	return "crepus"
}

var bindBlocklist = map[string]bool{"baseDir": true, "_": true} // ponytail: block security-sensitive keys only

func RenderIR(path string, context map[string]any) (ViewIr, error) {
	cwd, err := os.Getwd()
	if err != nil {
		return ViewIr{}, err
	}

	allowedDir := cwd
	if override := os.Getenv("CREPUS_ALLOWED_DIR"); override != "" {
		allowedDir = override
	}

	absAllowed, err := filepath.Abs(allowedDir)
	if err != nil {
		return ViewIr{}, err
	}
	evalAllowed, err := filepath.EvalSymlinks(absAllowed)
	if err != nil {
		evalAllowed = absAllowed
	}

	absPath, err := filepath.Abs(path)
	if err != nil {
		return ViewIr{}, err
	}
	evalPath, err := filepath.EvalSymlinks(absPath)
	if err != nil {
		evalPath = absPath
	}
	evalAllowed = filepath.Clean(evalAllowed)
	evalPath = filepath.Clean(evalPath)
	if !strings.HasPrefix(evalPath, evalAllowed+string(filepath.Separator)) && evalPath != evalAllowed {
		return ViewIr{}, fmt.Errorf("path traversal denied")
	}

	payload, err := json.Marshal(map[string]any{
		"template": mustRead(absPath),
		"context":  context,
	})
	if err != nil {
		return ViewIr{}, err
	}
	cmd := exec.Command(crepusBin(), "native", "ir", "--stdin-json")
	cmd.Stdin = bytes.NewReader(payload)
	out, err := cmd.CombinedOutput()
	if err != nil {
		return ViewIr{}, fmt.Errorf("%w: %s", err, string(out))
	}
	var ir ViewIr
	if err := json.Unmarshal(out, &ir); err != nil {
		return ViewIr{}, err
	}
	return ir, nil
}

func mustRead(path string) string {
	data, err := os.ReadFile(path)
	if err != nil {
		panic(err)
	}
	return string(data)
}

func RenderHTML(path string, context map[string]any) (string, error) {
	ir, err := RenderIR(path, context)
	if err != nil {
		return "", err
	}
	var out strings.Builder
	for _, node := range ir.Root {
		out.WriteString(renderNode(node))
	}
	return out.String(), nil
}

func NewViewSession(path string, context map[string]any) *ViewSession {
	if context == nil {
		context = map[string]any{}
	}
	return &ViewSession{Path: path, Context: context, events: map[string]EventHandler{}}
}

func (session *ViewSession) On(handler string, callback EventHandler) *ViewSession {
	session.events[handler] = callback
	return session
}

func (session *ViewSession) RenderIR() (ViewIr, error) {
	return RenderIR(session.Path, session.Context)
}

func (session *ViewSession) RenderHTML() (string, error) {
	return RenderHTML(session.Path, session.Context)
}

func (session *ViewSession) Dispatch(event Event) (ViewIr, error) {
	if strings.HasPrefix(event.Handler, "bind:") {
		parts := strings.SplitN(strings.TrimPrefix(event.Handler, "bind:"), ":", 2)
		// ponytail: only allowlisted keys from bind:
		if len(parts) == 2 && !bindBlocklist[parts[0]] {
			session.Context[parts[0]] = parts[1]
		}
	}
	if callback := session.events[event.Handler]; callback != nil {
		if err := callback(event, session); err != nil {
			return ViewIr{}, err
		}
	}
	return session.RenderIR()
}

func renderNode(node map[string]any) string {
	switch node["kind"] {
	case "text":
		return html.EscapeString(fmt.Sprint(node["content"]))
	case "stack", "scroll":
		var out strings.Builder
		out.WriteString(`<div data-crepus-kind="`)
		out.WriteString(html.EscapeString(fmt.Sprint(node["kind"])))
		out.WriteString(`" data-axis="`)
		out.WriteString(html.EscapeString(fmt.Sprint(node["axis"])))
		out.WriteString(`">`)
		for _, child := range nodeList(node["children"]) {
			out.WriteString(renderNode(child))
		}
		out.WriteString("</div>")
		return out.String()
	case "button":
		label := html.EscapeString(fmt.Sprint(node["label"]))
		if node["onClick"] != nil {
			return `<button data-onclick="` + html.EscapeString(fmt.Sprint(node["onClick"])) + `">` + label + `</button>`
		}
		return "<button>" + label + "</button>"
	case "image":
		return `<img src="` + html.EscapeString(fmt.Sprint(node["src"])) + `" alt="` + html.EscapeString(fmt.Sprint(node["alt"])) + `">`
	case "slotRotate":
		phrases := valueList(node["phrases"])
		if len(phrases) == 0 {
			return `<span data-crepus-kind="slotRotate"></span>`
		}
		return `<span data-crepus-kind="slotRotate">` + html.EscapeString(fmt.Sprint(phrases[0])) + `</span>`
	case "input":
		bind := html.EscapeString(fmt.Sprint(node["bind"]))
		placeholder := html.EscapeString(fmt.Sprint(node["placeholder"]))
		if node["multiline"] == true {
			return `<textarea data-bind="` + bind + `" placeholder="` + placeholder + `"></textarea>`
		}
		return `<input data-bind="` + bind + `" placeholder="` + placeholder + `">`
	case "picker":
		var out strings.Builder
		out.WriteString(`<select data-bind="`)
		out.WriteString(html.EscapeString(fmt.Sprint(node["bind"])))
		out.WriteString(`">`)
		for _, option := range nodeList(node["options"]) {
			out.WriteString(`<option value="`)
			out.WriteString(html.EscapeString(fmt.Sprint(option["value"])))
			out.WriteString(`">`)
			out.WriteString(html.EscapeString(fmt.Sprint(option["label"])))
			out.WriteString(`</option>`)
		}
		out.WriteString(`</select>`)
		return out.String()
	default:
		return ""
	}
}

func nodeList(value any) []map[string]any {
	raw, ok := value.([]any)
	if !ok {
		return nil
	}
	out := make([]map[string]any, 0, len(raw))
	for _, item := range raw {
		if node, ok := item.(map[string]any); ok {
			out = append(out, node)
		}
	}
	return out
}

func valueList(value any) []any {
	raw, _ := value.([]any)
	return raw
}
