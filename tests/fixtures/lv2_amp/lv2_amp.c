/* Minimal stereo gain plugin used by Sonix's LV2 host tests.
 *
 * Exports `lv2_descriptor` with two URIs over the same five ports (two audio
 * inputs 0-1, two audio outputs 2-3, one control input 4):
 *   http://sonix.test/lv2/amp       — has `state:interface`, saves the gain
 *   http://sonix.test/lv2/stateless — same audio path, no state interface
 * so one build covers both "state is carried" and "no interface" cases.
 * Built by the test with `cc -shared -fPIC`.
 */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef struct _LV2_Feature LV2_Feature;
typedef struct _LV2_Descriptor LV2_Descriptor;
typedef void *LV2_Handle;
typedef void *LV2_State_Handle;
typedef int32_t LV2_State_Status;

#define LV2_STATE_SUCCESS 0
#define LV2_STATE_IS_POD (1 << 0)
#define LV2_STATE_IS_PORTABLE (1 << 2)

/* No URID map is offered to this plugin, so the key is a bare number. */
#define KEY_GAIN 1u
#define STATE_URI "http://lv2plug.in/ns/ext/state#interface"

typedef LV2_State_Status (*LV2_State_Store_Function)(
    LV2_State_Handle handle, uint32_t key, const void *value, uint32_t size,
    uint32_t type, uint32_t flags);
typedef const void *(*LV2_State_Retrieve_Function)(
    LV2_State_Handle handle, uint32_t key, size_t *size, uint32_t *type,
    uint32_t *flags);

typedef struct {
    LV2_State_Status (*save)(LV2_Handle instance, LV2_State_Store_Function store,
                             LV2_State_Handle handle, uint32_t flags,
                             LV2_Feature const *const *features);
    LV2_State_Status (*restore)(LV2_Handle instance,
                                LV2_State_Retrieve_Function retrieve,
                                LV2_State_Handle handle, uint32_t flags,
                                LV2_Feature const *const *features);
} LV2_State_Interface;

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

/* Saves exactly one value: the gain, as raw float bytes. */
static LV2_State_Status state_save(LV2_Handle instance,
                                   LV2_State_Store_Function store,
                                   LV2_State_Handle handle, uint32_t flags,
                                   LV2_Feature const *const *features) {
    (void)flags;
    (void)features;
    SonixAmp *amp = instance;
    float gain = amp->gain ? *amp->gain : 1.0f;
    return store(handle, KEY_GAIN, &gain, sizeof(gain), 0,
                 LV2_STATE_IS_POD | LV2_STATE_IS_PORTABLE);
}

/* Restores it into the buffer the host connected as the gain port. */
static LV2_State_Status state_restore(LV2_Handle instance,
                                      LV2_State_Retrieve_Function retrieve,
                                      LV2_State_Handle handle, uint32_t flags,
                                      LV2_Feature const *const *features) {
    (void)flags;
    (void)features;
    SonixAmp *amp = instance;
    size_t size = 0;
    uint32_t type = 0, value_flags = 0;
    const void *value = retrieve(handle, KEY_GAIN, &size, &type, &value_flags);
    if (value && size == sizeof(float) && amp->gain) {
        memcpy(amp->gain, value, sizeof(float));
        return LV2_STATE_SUCCESS;
    }
    return 1; /* LV2_STATE_ERR_UNKNOWN */
}

static const LV2_State_Interface amp_state = {state_save, state_restore};

static void *amp_extension_data(char const *uri) {
    return strcmp(uri, STATE_URI) == 0 ? (void *)&amp_state : 0;
}

static void *no_extension_data(char const *uri) {
    (void)uri;
    return 0;
}

static const LV2_Descriptor descriptors[] = {
    {"http://sonix.test/lv2/amp", instantiate, connect_port, activate, run,
     deactivate, cleanup, amp_extension_data},
    {"http://sonix.test/lv2/stateless", instantiate, connect_port, activate,
     run, deactivate, cleanup, no_extension_data},
};

LV2_Descriptor const *lv2_descriptor(uint32_t index) {
    return index < sizeof(descriptors) / sizeof(descriptors[0])
               ? &descriptors[index]
               : 0;
}
