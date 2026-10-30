/* Minimal stereo gain plugin used by Sonix's LV2 host tests.
 *
 * Exports `lv2_descriptor` and nothing else: one URI, two audio inputs
 * (ports 0-1), two audio outputs (ports 2-3) and one control input (port 4).
 * Built by the test with `cc -shared -fPIC`.
 */
#include <stdint.h>
#include <stdlib.h>

typedef struct _LV2_Feature LV2_Feature;
typedef struct _LV2_Descriptor LV2_Descriptor;
typedef void *LV2_Handle;

struct _LV2_Descriptor {
    char const *URI;
    LV2_Handle (*instantiate)(LV2_Descriptor const *descriptor,
                              double sample_rate,
                              LV2_Feature const *const *features);
    void (*connect_port)(LV2_Handle instance, uint32_t port, void *data);
    void (*activate)(LV2_Handle instance);
    void (*run)(LV2_Handle instance, uint32_t sample_count);
    void (*deactivate)(LV2_Handle instance);
    void (*cleanup)(LV2_Handle instance);
    void *(*extension_data)(char const *uri);
};

typedef struct {
    float *in_l, *in_r, *out_l, *out_r, *gain;
} SonixAmp;

static LV2_Handle instantiate(LV2_Descriptor const *descriptor,
                              double sample_rate,
                              LV2_Feature const *const *features) {
    (void)descriptor;
    (void)sample_rate;
    (void)features;
    SonixAmp *amp = calloc(1, sizeof(SonixAmp));
    return amp;
}

static void connect_port(LV2_Handle instance, uint32_t port, void *data) {
    SonixAmp *amp = instance;
    switch (port) {
    case 0: amp->in_l = data; break;
    case 1: amp->in_r = data; break;
    case 2: amp->out_l = data; break;
    case 3: amp->out_r = data; break;
    case 4: amp->gain = data; break;
    default: break;
    }
}

static void activate(LV2_Handle instance) { (void)instance; }
static void deactivate(LV2_Handle instance) { (void)instance; }

static void run(LV2_Handle instance, uint32_t sample_count) {
    SonixAmp *amp = instance;
    float gain = amp->gain ? *amp->gain : 1.0f;
    for (uint32_t i = 0; i < sample_count; ++i) {
        amp->out_l[i] = amp->in_l[i] * gain;
        amp->out_r[i] = amp->in_r[i] * gain;
    }
}

static void cleanup(LV2_Handle instance) { free(instance); }

static void *extension_data(char const *uri) {
    (void)uri;
    return 0;
}

static const LV2_Descriptor descriptor = {
    "http://sonix.test/lv2/amp", instantiate, connect_port, activate,
    run, deactivate, cleanup, extension_data,
};

LV2_Descriptor const *lv2_descriptor(uint32_t index) {
    return index == 0 ? &descriptor : 0;
}
