/*
 * A minimal, real CLAP plugin used only to test Sonix's plugin host.
 *
 * It implements the CLAP 1.x ABI by hand: an exported `clap_entry`, a
 * plugin-factory with a single descriptor, a no-op plugin vtable and the
 * `clap.params` extension exposing two parameters ("Gain" and "Mix").
 *
 * Build (done automatically by build.rs when `--features plugin-host`):
 *   cc -shared -fPIC -O2 -o libsonix_mock_clap.so tests/fixtures/mock_clap.c
 */

#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

#define CLAP_NAME_SIZE 256
#define CLAP_PATH_SIZE 1024

#define CLAP_PARAM_IS_STEPPED (1u << 0)
#define CLAP_PARAM_IS_AUTOMATABLE (1u << 5)

typedef struct clap_version {
    uint32_t major;
    uint32_t minor;
    uint32_t revision;
} clap_version_t;

typedef struct clap_host clap_host_t;
typedef struct clap_plugin clap_plugin_t;

typedef struct clap_plugin_descriptor {
    clap_version_t clap_version;
    const char *id;
    const char *name;
    const char *vendor;
    const char *version;
    const char *description;
    const char *const *features;
} clap_plugin_descriptor_t;

struct clap_plugin {
    const clap_plugin_descriptor_t *desc;
    void *plugin_data;
    bool (*init)(const clap_plugin_t *plugin);
    void (*destroy)(const clap_plugin_t *plugin);
    bool (*activate)(const clap_plugin_t *plugin, double sample_rate, uint32_t min_frames_count,
                     uint32_t max_frames_count);
    void (*deactivate)(const clap_plugin_t *plugin);
    bool (*start_processing)(const clap_plugin_t *plugin);
    void (*stop_processing)(const clap_plugin_t *plugin);
    void (*reset)(const clap_plugin_t *plugin);
    int32_t (*process)(const clap_plugin_t *plugin, const void *process);
    const void *(*get_extension)(const clap_plugin_t *plugin, const char *id);
    void (*on_main_thread)(const clap_plugin_t *plugin);
};

typedef struct clap_param_info {
    uint64_t id;
    uint32_t flags;
    char name[CLAP_NAME_SIZE];
    char module[CLAP_PATH_SIZE];
    double min_value;
    double max_value;
    double default_value;
} clap_param_info_t;

typedef struct clap_plugin_params {
    uint32_t (*count)(const clap_plugin_t *plugin);
    bool (*get_info)(const clap_plugin_t *plugin, uint32_t param_index, clap_param_info_t *param_info);
    bool (*get_value)(const clap_plugin_t *plugin, uint64_t param_id, double *value);
    bool (*value_to_text)(const clap_plugin_t *plugin, uint64_t param_id, double value, char *display,
                          uint32_t size);
    bool (*text_to_value)(const clap_plugin_t *plugin, uint64_t param_id, const char *display,
                          double *value);
    void (*flush)(const clap_plugin_t *plugin, const void *in, const void *out);
} clap_plugin_params_t;

typedef struct clap_plugin_factory {
    uint32_t (*get_plugin_count)(const struct clap_plugin_factory *factory);
    const clap_plugin_descriptor_t *(*get_plugin_descriptor)(const struct clap_plugin_factory *factory,
                                                             uint32_t index);
    const clap_plugin_t *(*create_plugin)(const struct clap_plugin_factory *factory,
                                          const clap_host_t *host, const char *plugin_id);
} clap_plugin_factory_t;

typedef struct clap_plugin_entry {
    clap_version_t clap_version;
    bool (*init)(const char *plugin_path);
    void (*deinit)(void);
    const void *(*get_factory)(const char *factory_id);
} clap_plugin_entry_t;

/* ------------------------------------------------------------------ */

static const char *const plugin_features[] = {"instrument", "synthesizer", NULL};

static const clap_plugin_descriptor_t mock_desc = {
    .clap_version = {1, 2, 0},
    .id = "com.sonix.mock-synth",
    .name = "Sonix Mock Synth",
    .vendor = "Sonix Test",
    .version = "1.0.0",
    .description = "A tiny in-process CLAP test plugin",
    .features = plugin_features,
};

static bool p_init(const clap_plugin_t *p) { (void)p; return true; }
static void p_destroy(const clap_plugin_t *p) { (void)p; }
static bool p_activate(const clap_plugin_t *p, double sr, uint32_t a, uint32_t b) {
    (void)p; (void)sr; (void)a; (void)b; return true;
}
static void p_deactivate(const clap_plugin_t *p) { (void)p; }
static bool p_start_processing(const clap_plugin_t *p) { (void)p; return true; }
static void p_stop_processing(const clap_plugin_t *p) { (void)p; }
static void p_reset(const clap_plugin_t *p) { (void)p; }
static int32_t p_process(const clap_plugin_t *p, const void *proc) { (void)p; (void)proc; return 0; }
static void p_on_main_thread(const clap_plugin_t *p) { (void)p; }

static uint32_t params_count(const clap_plugin_t *p) { (void)p; return 2; }

static bool params_get_info(const clap_plugin_t *p, uint32_t index, clap_param_info_t *info) {
    (void)p;
    memset(info, 0, sizeof(*info));
    if (index == 0) {
        info->id = 0;
        info->flags = CLAP_PARAM_IS_AUTOMATABLE;
        strncpy(info->name, "Gain", CLAP_NAME_SIZE - 1);
        info->min_value = 0.0;
        info->max_value = 2.0;
        info->default_value = 1.0;
        return true;
    }
    if (index == 1) {
        info->id = 1;
        info->flags = CLAP_PARAM_IS_AUTOMATABLE | CLAP_PARAM_IS_STEPPED;
        strncpy(info->name, "Mix", CLAP_NAME_SIZE - 1);
        info->min_value = 0.0;
        info->max_value = 1.0;
        info->default_value = 0.5;
        return true;
    }
    return false;
}

static bool params_get_value(const clap_plugin_t *p, uint64_t id, double *value) {
    (void)p;
    *value = (id == 0) ? 1.0 : 0.5;
    return true;
}

static bool params_value_to_text(const clap_plugin_t *p, uint64_t id, double value, char *display,
                                 uint32_t size) {
    (void)p; (void)id;
    snprintf(display, size, "%.3f", value);
    return true;
}

static bool params_text_to_value(const clap_plugin_t *p, uint64_t id, const char *display,
                                 double *value) {
    (void)p; (void)id;
    *value = atof(display);
    return true;
}

static void params_flush(const clap_plugin_t *p, const void *in, const void *out) {
    (void)p; (void)in; (void)out;
}

static const clap_plugin_params_t mock_params = {
    params_count, params_get_info, params_get_value,
    params_value_to_text, params_text_to_value, params_flush,
};

static const void *p_get_extension(const clap_plugin_t *p, const char *id) {
    (void)p;
    if (strcmp(id, "clap.params") == 0) {
        return &mock_params;
    }
    return NULL;
}

static clap_plugin_t mock_plugin = {
    &mock_desc, NULL, p_init, p_destroy, p_activate, p_deactivate,
    p_start_processing, p_stop_processing, p_reset, p_process,
    p_get_extension, p_on_main_thread,
};

static uint32_t factory_get_plugin_count(const clap_plugin_factory_t *factory) {
    (void)factory;
    return 1;
}

static const clap_plugin_descriptor_t *factory_get_plugin_descriptor(
    const clap_plugin_factory_t *factory, uint32_t index) {
    (void)factory;
    return (index == 0) ? &mock_desc : NULL;
}

static const clap_plugin_t *factory_create_plugin(const clap_plugin_factory_t *factory,
                                                  const clap_host_t *host, const char *plugin_id) {
    (void)factory; (void)host;
    if (strcmp(plugin_id, mock_desc.id) == 0) {
        return &mock_plugin;
    }
    return NULL;
}

static const clap_plugin_factory_t mock_factory = {
    factory_get_plugin_count, factory_get_plugin_descriptor, factory_create_plugin,
};

static bool entry_init(const char *plugin_path) { (void)plugin_path; return true; }
static void entry_deinit(void) {}

static const void *entry_get_factory(const char *factory_id) {
    if (strcmp(factory_id, "clap.plugin-factory") == 0) {
        return &mock_factory;
    }
    return NULL;
}

__attribute__((visibility("default")))
const clap_plugin_entry_t clap_entry = {
    {1, 2, 0}, entry_init, entry_deinit, entry_get_factory,
};
