/* Minimal stereo gain plugin used by Sonix's LV2 host tests.
 *
 * Exports `lv2_descriptor` with two URIs over the same five ports (two audio
 * inputs 0-1, two audio outputs 2-3, one control input 4):
 *   http://sonix.test/lv2/amp       — requires `urid:map`, has `state:interface`
 *   http://sonix.test/lv2/stateless — same audio path, no state interface
 * so one build covers both "state is carried through the host's URID map" and
 * "no interface" cases.
 *
 * The amp refuses to instantiate without the `urid:map` feature, and its state
 * key is the URID the host mints for GAIN_URI — a plugin that must map its own
 * keys is exactly what the host's URID map is for.
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

#define STATE_URI "http://lv2plug.in/ns/ext/state#interface"
#define URID_MAP_URI "http://lv2plug.in/ns/ext/urid#map"
#define PROBE_URI "http://sonix.test/lv2/amp#probe"
#define GAIN_URI "http://sonix.test/lv2/amp#gain"

typedef uint32_t (*LV2_URID_Map_Function)(void *handle, const char *uri);

typedef struct {
    void *handle;
    LV2_URID_Map_Function map;
} LV2_URID_Map;

struct _LV2_Feature {
    char const *URI;
    void *data;
};

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
    /* The host's map, copied at instantiate, plus the id it gave PROBE_URI. */
    LV2_URID_Map map;
    uint32_t probe;
} SonixAmp;

/* The `urid:map` feature out of a feature array, or NULL when absent. */
static const LV2_URID_Map *find_urid_map(LV2_Feature const *const *features) {
    for (uint32_t i = 0; features && features[i]; ++i) {
        if (strcmp(features[i]->URI, URID_MAP_URI) == 0) {
            return features[i]->data;
        }
    }
    return 0;
}

/* The state key: the host's URID for GAIN_URI, read through the array at hand
 * (save and restore carry their own), falling back to the one from instantiate. */
static int gain_key(SonixAmp *amp, LV2_Feature const *const *features,
                    uint32_t *key) {
    const LV2_URID_Map *map = find_urid_map(features);
    if (!map) {
        map = &amp->map;
    }
    if (!map->map) {
        return 0;
    }
    *key = map->map(map->handle, GAIN_URI);
    return *key != 0;
}

static LV2_Handle instantiate_plain(LV2_Descriptor const *descriptor,
                                   double sample_rate,
                                   LV2_Feature const *const *features) {
    (void)descriptor;
    (void)sample_rate;
    (void)features;
    return calloc(1, sizeof(SonixAmp));
}

/* Refuses to run without `urid:map`: the bundle lists it as a required feature,
 * so a host that does not offer it must not get an instance. */
static LV2_Handle instantiate_mapped(LV2_Descriptor const *descriptor,
                                     double sample_rate,
                                     LV2_Feature const *const *features) {
    (void)descriptor;
    (void)sample_rate;
    const LV2_URID_Map *map = find_urid_map(features);
    if (!map || !map->map) {
        return 0;
    }
    SonixAmp *amp = calloc(1, sizeof(SonixAmp));
    if (!amp) {
        return 0;
    }
    amp->map = *map;
    /* Mapped once here so the state key is not simply the first id minted. */
    amp->probe = map->map(map->handle, PROBE_URI);
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

/* Saves exactly one value: the gain, as raw float bytes, keyed by the URID the
 * host maps for GAIN_URI. */
static LV2_State_Status state_save(LV2_Handle instance,
                                   LV2_State_Store_Function store,
                                   LV2_State_Handle handle, uint32_t flags,
                                   LV2_Feature const *const *features) {
    (void)flags;
    SonixAmp *amp = instance;
    uint32_t key = 0;
    if (!gain_key(amp, features, &key)) {
        return 1; /* LV2_STATE_ERR_UNKNOWN */
    }
    float gain = amp->gain ? *amp->gain : 1.0f;
    return store(handle, key, &gain, sizeof(gain), 0,
                 LV2_STATE_IS_POD | LV2_STATE_IS_PORTABLE);
}

/* Restores it into the buffer the host connected as the gain port. */
static LV2_State_Status state_restore(LV2_Handle instance,
                                      LV2_State_Retrieve_Function retrieve,
                                      LV2_State_Handle handle, uint32_t flags,
                                      LV2_Feature const *const *features) {
    (void)flags;
    SonixAmp *amp = instance;
    uint32_t key = 0;
    if (!gain_key(amp, features, &key)) {
        return 1;
    }
    size_t size = 0;
    uint32_t type = 0, value_flags = 0;
    const void *value = retrieve(handle, key, &size, &type, &value_flags);
    if (value && size == sizeof(float) && amp->gain) {
        memcpy(amp->gain, value, sizeof(float));
        return LV2_STATE_SUCCESS;
    }
    return 1;
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
    {"http://sonix.test/lv2/amp", instantiate_mapped, connect_port, activate,
     run, deactivate, cleanup, amp_extension_data},
    {"http://sonix.test/lv2/stateless", instantiate_plain, connect_port,
     activate, run, deactivate, cleanup, no_extension_data},
};

LV2_Descriptor const *lv2_descriptor(uint32_t index) {
    return index < sizeof(descriptors) / sizeof(descriptors[0])
               ? &descriptors[index]
               : 0;
}
