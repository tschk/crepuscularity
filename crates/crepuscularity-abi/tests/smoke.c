#include "crepuscularity_abi.h"
#include <assert.h>
#include <stddef.h>
#include <string.h>

struct callback_state {
    CrepusSession *session;
    int events;
};

static void check_reentry_error(void) {
    char *error = crepus_last_error();
    assert(error != NULL && strstr(error, "event callback") != NULL);
    crepus_string_free(error);
}

static void on_event(const char *json, void *userdata) {
    struct callback_state *state = userdata;
    assert(strstr(json, "bind:count:2") != NULL);
    assert(crepus_session_set_context_json(state->session, "{\"count\":9}") == -1);
    check_reentry_error();
    assert(crepus_session_render_ir_json(state->session) == NULL);
    check_reentry_error();
    assert(crepus_session_take_last_error(state->session) == NULL);
    check_reentry_error();
    assert(crepus_session_dispatch_event_json(state->session, "\"bind:count:9\"") == NULL);
    check_reentry_error();
    crepus_session_free(state->session);
    check_reentry_error();
    CrepusSession *other = crepus_session_new();
    assert(other != NULL);
    assert(crepus_session_set_template_string(other, "span\n  \"Other\"", NULL) == 0);
    char *ir = crepus_session_render_ir_json(other);
    assert(ir != NULL && strstr(ir, "Other") != NULL);
    crepus_string_free(ir);
    crepus_session_free(other);
    assert(strstr(json, "bind:count:2") != NULL);
    ++state->events;
}

static void check_files_component_selection(void) {
    CrepusSession *session = crepus_session_new();
    assert(session != NULL);
    assert(crepus_session_set_files_json(session,
        "{\"entry\":\"components.crepus#First\",\"files\":{"
        "\"components.crepus\":\"--- First\\nspan \\\"FIRST\\\"\\n"
        "--- Second\\nspan \\\"SECOND\\\"\\n\"}}") == 0);

    assert(crepus_session_set_component(session, "Second") == 0);
    char *ir = crepus_session_render_ir_json(session);
    assert(ir != NULL && strstr(ir, "SECOND") != NULL);
    assert(strstr(ir, "FIRST") == NULL);
    crepus_string_free(ir);
    char *event = crepus_session_dispatch_event_json(session, "\"selected\"");
    assert(event != NULL && strstr(event, "SECOND") != NULL);
    assert(strstr(event, "FIRST") == NULL);
    crepus_string_free(event);

    assert(crepus_session_set_component(session, NULL) == 0);
    ir = crepus_session_render_ir_json(session);
    assert(ir != NULL && strstr(ir, "FIRST") != NULL);
    assert(strstr(ir, "SECOND") == NULL);
    crepus_string_free(ir);

    assert(crepus_session_set_component(session, "Missing") == 0);
    assert(crepus_session_render_ir_json(session) == NULL);
    char *error = crepus_session_take_last_error(session);
    assert(error != NULL && strstr(error, "Missing") != NULL);
    crepus_string_free(error);
    assert(crepus_session_take_last_error(session) == NULL);
    crepus_session_free(session);
}

int main(void) {
    check_files_component_selection();
    assert(crepus_session_render_ir_json(NULL) == NULL);
    char *error = crepus_last_error();
    assert(error != NULL && strcmp(error, "session pointer is null") == 0);
    crepus_string_free(error);
    assert(crepus_last_error() == NULL);

    CrepusSession *session = crepus_session_new();
    assert(session != NULL);
    assert(crepus_session_set_template_string(session, "span\n  \"Count {count}\"", NULL) == 0);
    assert(crepus_session_set_context_json(session, "{\"count\":1}") == 0);
    char *ir = crepus_session_render_ir_json(session);
    assert(ir != NULL && strstr(ir, "Count 1") != NULL);
    crepus_string_free(ir);

    struct callback_state state = {session, 0};
    assert(crepus_session_set_event_callback(session, on_event, &state) == 0);
    char *result = crepus_session_dispatch_event_json(session, "\"bind:count:2\"");
    assert(result != NULL && strstr(result, "Count 2") != NULL);
    assert(state.events == 1);
    crepus_string_free(result);

    assert(crepus_session_apply_context_patch_json(session,
        "{\"count\":9,\"invalid\":[1]}") == -1);
    error = crepus_session_take_last_error(session);
    assert(error != NULL && strstr(error, "arrays must contain objects") != NULL);
    crepus_string_free(error);
    ir = crepus_session_render_ir_json(session);
    assert(ir != NULL && strstr(ir, "Count 2") != NULL);
    crepus_string_free(ir);

    assert(crepus_session_set_files_json(session,
        "{\"entry\":\"index.crepus\",\"files\":{\"index.crepus\":\"span\\n  \\\"Bundle {count}\\\"\"}}") == 0);
    ir = crepus_session_render_ir_json(session);
    assert(ir != NULL && strstr(ir, "Bundle 2") != NULL);
    crepus_string_free(ir);
    assert(crepus_session_set_files_json(session, "{broken") == -1);
    error = crepus_session_take_last_error(session);
    assert(error != NULL && strstr(error, "files JSON") != NULL);
    crepus_string_free(error);
    ir = crepus_session_render_ir_json(session);
    assert(ir != NULL && strstr(ir, "Bundle 2") != NULL);
    crepus_string_free(ir);
    crepus_session_free(session);
    crepus_session_free(NULL);
    crepus_string_free(NULL);
    return 0;
}
