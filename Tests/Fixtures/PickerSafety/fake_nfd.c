/* Test-only NFD/SDL shim: no native dialog SDK calls. */
#include <stdarg.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum { INIT = 1, FOLDER, FILE_PICK, FREE_PATH, QUIT, GET_ERROR, LOG, CALLBACK };
static _Atomic int events[128], event_count, response, trigger, in_callback;
static _Atomic int fail_next_init, policy, old_policy, callback_count;
static char last_format[256], filter_spec[256];
static _Atomic uint32_t filter_count;
extern void picker_test_nested(void);

static void event(int value) {
    int index = atomic_fetch_add(&event_count, 1);
    if (index >= 0 && index < 128) atomic_store(&events[index], value);
}
static void maybe_callback(int where) {
    if (atomic_load(&trigger) != where || atomic_exchange(&in_callback, 1)) return;
    event(CALLBACK);
    atomic_fetch_add(&callback_count, 1);
    picker_test_nested(); /* Rust callback never panics or asserts. */
    atomic_store(&in_callback, 0);
}
void fake_setup(int result, int callback_stage, int init_fail) {
    atomic_store(&event_count, 0); atomic_store(&response, result);
    atomic_store(&trigger, callback_stage); atomic_store(&in_callback, 0);
    atomic_store(&fail_next_init, init_fail); atomic_store(&policy, 0);
    atomic_store(&old_policy, 0); atomic_store(&callback_count, 0);
    last_format[0] = '\0'; filter_spec[0] = '\0';
    atomic_store(&filter_count, 0);
}
int fake_count(void) { return atomic_load(&event_count); }
int fake_event(int i) { return i >= 0 && i < 128 ? atomic_load(&events[i]) : -1; }
int fake_policy(void) { return atomic_load(&policy); }
int fake_callbacks(void) { return atomic_load(&callback_count); }
const char *fake_last_format(void) { return last_format; }
uint32_t fake_filter_count(void) { return atomic_load(&filter_count); }
const char *fake_filter_spec(void) { return filter_spec; }

int NFD_Init(void) {
    event(INIT);
    atomic_store(&old_policy, atomic_load(&policy));
    if (atomic_exchange(&fail_next_init, 0)) return 0;
    if (atomic_load(&policy) == 0) atomic_store(&policy, 1);
    return 1;
}
void NFD_Quit(void) {
    event(QUIT);
    maybe_callback(QUIT);
    atomic_store(&policy, atomic_load(&old_policy));
}
const char *NFD_GetError(void) { event(GET_ERROR); return "fake error"; }
void NFD_FreePathU8(char *path) {
    event(FREE_PATH);
    maybe_callback(FREE_PATH);
    free(path);
}
static int dialog(char **out, int kind) {
    event(kind);
    maybe_callback(kind);
    int status = atomic_load(&response);
    if (status == 1) {
        char *path = malloc(4);
        if (!path) abort();
        path[0] = 'x'; path[1] = (char)0xff; path[2] = 'z'; path[3] = '\0';
        *out = path;
    } else if (status == 3) {
        *out = NULL; return 1;
    }
    return status;
}
int NFD_PickFolderU8(char **out, const char *default_path) {
    (void)default_path; return dialog(out, FOLDER);
}
struct filter_item { const char *name; const char *spec; };
int NFD_OpenDialogU8(char **out, const struct filter_item *filters,
                     uint32_t count, const char *default_path) {
    (void)default_path;
    atomic_store(&filter_count, count);
    if (count && filters && filters[0].spec) {
        snprintf(filter_spec, sizeof(filter_spec), "%s", filters[0].spec);
    } else {
        filter_spec[0] = '\0';
    }
    return dialog(out, FILE_PICK);
}
/* The real SDL function is variadic. Read its %s argument with the C ABI. */
void SDL_Log(const char *format, ...) {
    event(LOG);
    snprintf(last_format, sizeof(last_format), "%s", format);
    va_list args;
    va_start(args, format);
    const char *message = va_arg(args, const char *);
    (void)message;
    va_end(args);
    maybe_callback(LOG);
}
