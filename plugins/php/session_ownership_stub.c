#include <stdint.h>
/* Labelled counter fixture: sessions deliberately use static storage, never free(). */
typedef struct CrepusSession { int value; } CrepusSession;
static CrepusSession owned;
static int freed;
CrepusSession *crepus_session_new(void) { return &owned; }
void crepus_session_free(CrepusSession *session) { if (session == &owned) ++freed; }
int crepus_session_set_template_string(CrepusSession *s, const char *t, const char *b) { return 0; }
int crepus_session_set_context_json(CrepusSession *s, const char *j) { return 0; }
int crepus_session_apply_context_patch_json(CrepusSession *s, const char *j) { return 0; }
char *crepus_session_render_ir_json(CrepusSession *s) { return 0; }
char *crepus_session_dispatch_event_json(CrepusSession *s, const char *j) { return 0; }
char *crepus_session_take_last_error(CrepusSession *s) { return 0; }
void crepus_string_free(char *p) {}
int fake_free_count(void) { return freed; }
