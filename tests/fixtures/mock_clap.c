/*
 * A minimal, real CLAP plugin used only to test Sonix's plugin host.
 *
 * It implements the CLAP 1.x ABI by hand: an exported `clap_entry`, a
 * plugin-factory with a single descriptor, a real processing vtable and the
 * `clap.params`, `clap.audio-ports` and `clap.latency` extensions.
 *
 * It is a stereo gain effect: each output sample is
 *     out = in * (1 - mix + mix * gain)
 * so the default settings (gain = 1, mix = 1) are transparent, and setting the
 * "Gain" parameter scales the signal — which lets the Rust tests verify that
 * audio really flows through the plugin.
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

typedef struct clap_audio_buffer {
    float **data32;
    double **data64;
    uint32_t channel_count;
    uint32_t latency;
    uint64_t constant_mask;
} clap_audio_buffer_t;

typedef struct clap_process {
    int64_t steady_time;
    uint32_t frames_count;
    const void *transport;
    const clap_audio_buffer_t *audio_inputs;
    clap_audio_buffer_t *audio_outputs;
    uint32_t audio_inputs_count;
    uint32_t audio_outputs_count;
    const void *in_events;
    const void *out_events;
} clap_process_t;

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
    int32_t (*process)(const clap_plugin_t *plugin, const clap_process_t *process);
    const void *(*get_extension)(const clap_plugin_t *plugin, const char *id);
    void (*on_main_thread)(const clap_plugin_t *plugin);
};

typedef struct clap_param_info {
    uint32_t id;
    uint32_t flags;
    char name[CLAP_NAME_SIZE];
    char module[CLAP_PATH_SIZE];
    double min_value;
    double max_value;
    double default_value;
} clap_param_info_t;

typedef struct clap_event_header {
    uint32_t size;
    uint32_t time;
    uint16_t space_id;
    uint16_t type;
    uint32_t flags;
} clap_event_header_t;

typedef struct clap_event_param_value {
    clap_event_header_t header;
    uint32_t param_id;
    void *cookie;
    int32_t note_id;
    int16_t port_index;
    int16_t channel;
    int16_t key;
    double value;
} clap_event_param_value_t;

typedef struct clap_input_events {
    void *ctx;
    uint32_t (*size)(const struct clap_input_events *list);
    const clap_event_header_t *(*get)(const struct clap_input_events *list, uint32_t index);
} clap_input_events_t;

typedef struct clap_output_events {
    void *ctx;
    bool (*try_push)(const struct clap_output_events *list, const clap_event_header_t *event);
} clap_output_events_t;

typedef struct clap_plugin_params {
    uint32_t (*count)(const clap_plugin_t *plugin);
    bool (*get_info)(const clap_plugin_t *plugin, uint32_t param_index, clap_param_info_t *param_info);
    bool (*get_value)(const clap_plugin_t *plugin, uint32_t param_id, double *value);
    bool (*value_to_text)(const clap_plugin_t *plugin, uint32_t param_id, double value, char *display,
                          uint32_t size);
    bool (*text_to_value)(const clap_plugin_t *plugin, uint32_t param_id, const char *display,
                          double *value);
    void (*flush)(const clap_plugin_t *plugin, const clap_input_events_t *in,
                  const clap_output_events_t *out);
} clap_plugin_params_t;

typedef struct clap_audio_port_info {
    uint32_t id;
    char name[CLAP_NAME_SIZE];
    uint32_t flags;
    uint32_t channel_count;
    const char *port_type;
    uint32_t in_place_pair;
} clap_audio_port_info_t;

typedef struct clap_plugin_audio_ports {
    uint32_t (*count)(const clap_plugin_t *plugin, bool is_input);
    bool (*get)(const clap_plugin_t *plugin, uint32_t index, bool is_input,
                clap_audio_port_info_t *info);
} clap_plugin_audio_ports_t;

typedef struct clap_plugin_latency {
    uint32_t (*get)(const clap_plugin_t *plugin);
} clap_plugin_latency_t;

typedef struct clap_ostream {
    void *ctx;
    int64_t (*write)(const struct clap_ostream *stream, const void *buffer, uint64_t size);
} clap_ostream_t;

typedef struct clap_istream {
    void *ctx;
    int64_t (*read)(const struct clap_istream *stream, void *buffer, uint64_t size);
} clap_istream_t;

typedef struct clap_plugin_state {
    bool (*save)(const clap_plugin_t *plugin, const clap_ostream_t *stream);
    bool (*load)(const clap_plugin_t *plugin, const clap_istream_t *stream);
} clap_plugin_state_t;

typedef struct clap_plugin_preset_load {
    bool (*from_location)(const clap_plugin_t *plugin, uint32_t location_kind,
                          const char *location, const char *load_key);
} clap_plugin_preset_load_t;

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

static const char *const plugin_features[] = {"audio-effect", NULL};

static const clap_plugin_descriptor_t mock_desc = {
    .clap_version = {1, 2, 0},
    .id = "com.sonix.mock-gain",
    .name = "Sonix Mock Gain",
    .vendor = "Sonix Test",
    .version = "1.0.0",
    .description = "A tiny in-process CLAP gain effect for host tests",
    .features = plugin_features,
};

typedef struct mock_state {
    double gain;
    double mix;
} mock_state_t;

static mock_state_t mock_state = {1.0, 1.0};

static bool p_init(const clap_plugin_t *p) { (void)p; return true; }
static void p_destroy(const clap_plugin_t *p) { (void)p; }
static bool p_activate(const clap_plugin_t *p, double sr, uint32_t a, uint32_t b) {
    (void)p; (void)sr; (void)a; (void)b;
    mock_state.gain = 1.0;
    mock_state.mix = 1.0;
    return true;
}
static void p_deactivate(const clap_plugin_t *p) { (void)p; }
static bool p_start_processing(const clap_plugin_t *p) { (void)p; return true; }
static void p_stop_processing(const clap_plugin_t *p) { (void)p; }
static void p_reset(const clap_plugin_t *p) { (void)p; }

static int32_t p_process(const clap_plugin_t *p, const clap_process_t *proc) {
    (void)p;
    if (!proc || proc->audio_outputs_count == 0) return 0;
    clap_audio_buffer_t *out = &proc->audio_outputs[0];
    if (out->channel_count == 0 || out->data32 == NULL) return 0;
    const clap_audio_buffer_t *in =
        (proc->audio_inputs_count > 0) ? &proc->audio_inputs[0] : NULL;
    double wet = 1.0 - mock_state.mix + mock_state.mix * mock_state.gain;
    for (uint32_t c = 0; c < out->channel_count; c++) {
        float *o = out->data32[c];
        if (!o) continue;
        const float *src = (in && c < in->channel_count) ? in->data32[c] : NULL;
        for (uint32_t i = 0; i < proc->frames_count; i++) {
            float x = src ? src[i] : 0.0f;
            o[i] = (float)(x * wet);
        }
    }
    return 0;
}

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
        info->default_value = 1.0;
        return true;
    }
    return false;
}

static bool params_get_value(const clap_plugin_t *p, uint32_t id, double *value) {
    (void)p;
    if (id == 0) { *value = mock_state.gain; return true; }
    if (id == 1) { *value = mock_state.mix; return true; }
    return false;
}

static bool params_value_to_text(const clap_plugin_t *p, uint32_t id, double value, char *display,
                                 uint32_t size) {
    (void)p; (void)id;
    snprintf(display, size, "%.3f", value);
    return true;
}

static bool params_text_to_value(const clap_plugin_t *p, uint32_t id, const char *display,
                                 double *value) {
    (void)p; (void)id;
    *value = atof(display);
    return true;
}

static void params_flush(const clap_plugin_t *p, const clap_input_events_t *in,
                         const clap_output_events_t *out) {
    (void)p; (void)out;
    if (!in || !in->size || !in->get) return;
    uint32_t n = in->size(in);
    for (uint32_t i = 0; i < n; i++) {
        const clap_event_header_t *h = in->get(in, i);
        if (!h) continue;
        if (h->space_id == 0 && h->type == 5 &&
            h->size >= (uint32_t)sizeof(clap_event_param_value_t)) {
            const clap_event_param_value_t *ev = (const clap_event_param_value_t *)h;
            if (ev->param_id == 0) mock_state.gain = ev->value;
            else if (ev->param_id == 1) mock_state.mix = ev->value;
        }
    }
}

static const clap_plugin_params_t mock_params = {
    params_count, params_get_info, params_get_value,
    params_value_to_text, params_text_to_value, params_flush,
};

static uint32_t audio_ports_count(const clap_plugin_t *p, bool is_input) {
    (void)p;
    return is_input ? 1u : 1u;
}

static bool audio_ports_get(const clap_plugin_t *p, uint32_t index, bool is_input,
                            clap_audio_port_info_t *info) {
    (void)p;
    if (index != 0) return false;
    memset(info, 0, sizeof(*info));
    info->id = is_input ? 0u : 1u;
    info->flags = 0;
    info->channel_count = 2;
    info->port_type = "audio";
    info->in_place_pair = 0;
    strncpy(info->name, is_input ? "Input" : "Output", CLAP_NAME_SIZE - 1);
    return true;
}

static const clap_plugin_audio_ports_t mock_audio_ports = {
    audio_ports_count, audio_ports_get,
};

static uint32_t latency_get(const clap_plugin_t *p) { (void)p; return 0; }

static const clap_plugin_latency_t mock_latency = {latency_get};

/* `clap.state`: serialise the two parameters as raw doubles. */
static bool state_save(const clap_plugin_t *p, const clap_ostream_t *stream) {
    (void)p;
    if (!stream || !stream->write) return false;
    double values[2] = {mock_state.gain, mock_state.mix};
    int64_t n = stream->write(stream, values, sizeof(values));
    return n == (int64_t)sizeof(values);
}

static bool state_load(const clap_plugin_t *p, const clap_istream_t *stream) {
    (void)p;
    if (!stream || !stream->read) return false;
    double values[2] = {0.0, 0.0};
    int64_t n = stream->read(stream, values, sizeof(values));
    if (n != (int64_t)sizeof(values)) return false;
    mock_state.gain = values[0];
    mock_state.mix = values[1];
    return true;
}

static const clap_plugin_state_t mock_state_ext = {state_save, state_load};

/* `clap.preset-load/2`: parse a tiny "gain=<v>;mix=<v>" pseudo-preset. */
static bool preset_from_location(const clap_plugin_t *p, uint32_t location_kind,
                                 const char *location, const char *load_key) {
    (void)p; (void)load_key;
    if (location_kind != 0 || !location) return false;
    double gain = mock_state.gain;
    double mix = mock_state.mix;
    const char *g = strstr(location, "gain=");
    if (g) gain = atof(g + 5);
    const char *m = strstr(location, "mix=");
    if (m) mix = atof(m + 4);
    mock_state.gain = gain;
    mock_state.mix = mix;
    return true;
}

static const clap_plugin_preset_load_t mock_preset_load = {preset_from_location};

static const void *p_get_extension(const clap_plugin_t *p, const char *id) {
    (void)p;
    if (strcmp(id, "clap.params") == 0) return &mock_params;
    if (strcmp(id, "clap.audio-ports") == 0) return &mock_audio_ports;
    if (strcmp(id, "clap.latency") == 0) return &mock_latency;
    if (strcmp(id, "clap.state") == 0) return &mock_state_ext;
    if (strcmp(id, "clap.preset-load/2") == 0) return &mock_preset_load;
    return NULL;
}

static clap_plugin_t mock_plugin = {
    &mock_desc, &mock_state, p_init, p_destroy, p_activate, p_deactivate,
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
