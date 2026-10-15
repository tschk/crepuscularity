#ifndef CREPUSCULARITY_ABI_H
#define CREPUSCULARITY_ABI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct CrepusSession CrepusSession;
/* event_json is valid until the next crepus_session_dispatch_event_json on the same session. */
typedef void (*CrepusEventCallback)(const char *event_json, void *userdata);

/* CrepusSession is not thread-safe: use one session per thread or external locking. */

CrepusSession *crepus_session_new(void);
void crepus_session_free(CrepusSession *session);
int32_t crepus_session_set_template_string(CrepusSession *session, const char *template_utf8, const char *base_dir_utf8);
int32_t crepus_session_set_component(CrepusSession *session, const char *component_utf8);
int32_t crepus_session_set_files_json(CrepusSession *session, const char *files_json_utf8);
int32_t crepus_session_set_context_json(CrepusSession *session, const char *context_json_utf8);
int32_t crepus_session_apply_context_patch_json(CrepusSession *session, const char *context_json_utf8);
int32_t crepus_session_set_event_callback(CrepusSession *session, CrepusEventCallback callback, void *userdata);
char *crepus_session_render_ir_json(CrepusSession *session);
char *crepus_session_dispatch_event_json(CrepusSession *session, const char *event_json_utf8);
char *crepus_session_take_last_error(CrepusSession *session);
char *crepus_last_error(void);
void crepus_string_free(char *ptr);

#ifdef __cplusplus
}
#endif

#endif
