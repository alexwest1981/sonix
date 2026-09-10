/*
 * A minimal, real VST3 module used only to test Sonix's VST3 host.
 *
 * It implements the VST 3 C ABI by hand (no SDK dependency): an exported
 * `GetPluginFactory`, an `IPluginFactory` exposing an "Audio Module Class"
 * component and its "Component Controller Class", and genuine
 * `IComponent`/`IAudioProcessor`/`IEditController` vtables laid out exactly as
 * the VST 3 headers declare them (non-COM `INLINE_UID` byte order).
 *
 * The component is a stereo gain effect with two parameters ("Gain" and "Mix"),
 * matching the CLAP mock so both backends can be exercised by the same tests.
 * Audio processing itself is completed in Fas 4.6b; this fixture already
 * exposes the full interface so inspection can be verified end-to-end.
 *
 * Build (done automatically by build.rs when `--features plugin-host`):
 *   cc -shared -fPIC -O2 -o libsonix_mock_vst3.so tests/fixtures/mock_vst3.c
 */

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include <string.h>
#include <stdlib.h>

typedef int32_t tresult;
typedef uint32_t uint32;
typedef int32_t int32;
typedef uint16_t char16;
typedef char char8;

enum {
    kNoInterface = -1,
    kResultOk = 0,
    kResultFalse = 1,
    kNotImplemented = 3,
};

/* Non-COM `INLINE_UID`: each 32-bit word is stored big-endian. */
#define UID(l1, l2, l3, l4)                                                     \
    {                                                                          \
        (uint8_t)((uint32)(l1) >> 24), (uint8_t)((uint32)(l1) >> 16),           \
            (uint8_t)((uint32)(l1) >> 8), (uint8_t)((uint32)(l1)),              \
            (uint8_t)((uint32)(l2) >> 24), (uint8_t)((uint32)(l2) >> 16),       \
            (uint8_t)((uint32)(l2) >> 8), (uint8_t)((uint32)(l2)),              \
            (uint8_t)((uint32)(l3) >> 24), (uint8_t)((uint32)(l3) >> 16),       \
            (uint8_t)((uint32)(l3) >> 8), (uint8_t)((uint32)(l3)),              \
            (uint8_t)((uint32)(l4) >> 24), (uint8_t)((uint32)(l4) >> 16),       \
            (uint8_t)((uint32)(l4) >> 8), (uint8_t)((uint32)(l4))               \
    }

static const char IID_FUnknown[16] = UID(0x00000000, 0x00000000, 0xC0000000, 0x00000046);
static const char IID_IPluginBase[16] = UID(0x22888DDB, 0x156E45AE, 0x8358B348, 0x08190625);
static const char IID_IComponent[16] = UID(0xE831FF31, 0xF2D54301, 0x928EBBEE, 0x25697802);
static const char IID_IAudioProcessor[16] = UID(0x42043F99, 0xB7DA453C, 0xA569E79D, 0x9AAEC33D);
static const char IID_IEditController[16] = UID(0xDCD7BBE3, 0x7742448D, 0xA874AACC, 0x979C759E);
static const char IID_IPluginFactory[16] = UID(0x7A4D811C, 0x52114A1F, 0xAED9D2EE, 0x0B43BF9F);

/* Our own class IDs (arbitrary but fixed). */
static const char COMPONENT_CID[16] = UID(0x11111111, 0x22222222, 0x33333333, 0x44444444);
static const char CONTROLLER_CID[16] = UID(0x55555555, 0x66666666, 0x77777777, 0x88888888);

#define FACTORY_VENDOR "Sonix Test"
#define FACTORY_URL "https://github.com/alexwest1981/sonix"
#define FACTORY_EMAIL "test@sonix.invalid"
#define FACTORY_FLAGS (1 << 4) /* PFactoryInfo::kUnicode */

#define COMPONENT_NAME "Sonix Mock VST3"
#define CONTROLLER_NAME "Sonix Mock VST3 Controller"

#define PARAM_GAIN_ID 100u
#define PARAM_MIX_ID 101u

/* ParameterFlags */
#define PARAM_CAN_AUTOMATE (1 << 0)
#define PARAM_IS_LIST (1 << 3)

#define CATEGORY_AUDIO "Audio Module Class"
#define CATEGORY_CONTROLLER "Component Controller Class"

typedef struct PFactoryInfo {
    char8 vendor[64];
    char8 url[256];
    char8 email[128];
    int32 flags;
} PFactoryInfo;

typedef struct PClassInfo {
    char cid[16];
    int32 cardinality;
    char8 category[32];
    char8 name[64];
} PClassInfo;

typedef struct BusInfo {
    int32 mediaType;
    int32 direction;
    int32 channelCount;
    char16 name[128];
    int32 busType;
    uint32 flags;
} BusInfo;

typedef struct ParameterInfo {
    uint32 id;
    char16 title[128];
    char16 shortTitle[128];
    char16 units[128];
    int32 stepCount;
    double defaultNormalizedValue;
    int32 unitId;
    int32 flags;
} ParameterInfo;

typedef struct RoutingInfo {
    int32 mediaType;
    int32 busIndex;
    int32 channel;
} RoutingInfo;

typedef struct ProcessSetup {
    int32 processMode;
    int32 symbolicSampleSize;
    int32 maxSamplesPerBlock;
    double sampleRate;
} ProcessSetup;

typedef struct AudioBusBuffers {
    int32 numChannels;
    uint64_t silenceFlags;
    union {
        float **channelBuffers32;
        double **channelBuffers64;
    } u;
} AudioBusBuffers;

typedef struct ProcessData {
    int32 processMode;
    int32 symbolicSampleSize;
    int32 numSamples;
    int32 numInputs;
    int32 numOutputs;
    AudioBusBuffers *inputs;
    AudioBusBuffers *outputs;
    void *inputParameterChanges;
    void *outputParameterChanges;
    void *inputEvents;
    void *outputEvents;
    void *processContext;
} ProcessData;

/* ------------------------------------------------------------------ vtables */

typedef struct IPluginFactoryVtbl {
    tresult (*queryInterface)(void *, const char *, void **);
    uint32 (*addRef)(void *);
    uint32 (*release)(void *);
    tresult (*getFactoryInfo)(void *, PFactoryInfo *);
    int32 (*countClasses)(void *);
    tresult (*getClassInfo)(void *, int32, PClassInfo *);
    tresult (*createInstance)(void *, const char *, const char *, void **);
} IPluginFactoryVtbl;

typedef struct IComponentVtbl {
    tresult (*queryInterface)(void *, const char *, void **);
    uint32 (*addRef)(void *);
    uint32 (*release)(void *);
    tresult (*initialize)(void *, void *);
    tresult (*terminate)(void *);
    tresult (*getControllerClassId)(void *, char *);
    tresult (*setIoMode)(void *, int32);
    int32 (*getBusCount)(void *, int32, int32);
    tresult (*getBusInfo)(void *, int32, int32, int32, BusInfo *);
    tresult (*getRoutingInfo)(void *, RoutingInfo *, RoutingInfo *);
    tresult (*activateBus)(void *, int32, int32, int32, uint8_t);
    tresult (*setActive)(void *, uint8_t);
    tresult (*setState)(void *, void *);
    tresult (*getState)(void *, void *);
} IComponentVtbl;

typedef struct IAudioProcessorVtbl {
    tresult (*queryInterface)(void *, const char *, void **);
    uint32 (*addRef)(void *);
    uint32 (*release)(void *);
    tresult (*setBusArrangements)(void *, uint64_t *, int32, uint64_t *, int32);
    tresult (*getBusArrangement)(void *, int32, int32, uint64_t *);
    tresult (*canProcessSampleSize)(void *, int32);
    uint32 (*getLatencySamples)(void *);
    tresult (*setupProcessing)(void *, ProcessSetup *);
    tresult (*setProcessing)(void *, uint8_t);
    tresult (*process)(void *, ProcessData *);
    uint32 (*getTailSamples)(void *);
} IAudioProcessorVtbl;

typedef struct IEditControllerVtbl {
    tresult (*queryInterface)(void *, const char *, void **);
    uint32 (*addRef)(void *);
    uint32 (*release)(void *);
    tresult (*initialize)(void *, void *);
    tresult (*terminate)(void *);
    tresult (*setComponentState)(void *, void *);
    tresult (*setState)(void *, void *);
    tresult (*getState)(void *, void *);
    int32 (*getParameterCount)(void *);
    tresult (*getParameterInfo)(void *, int32, ParameterInfo *);
    tresult (*getParamStringByValue)(void *, uint32, double, char16 *);
    tresult (*getParamValueByString)(void *, uint32, char16 *, double *);
    double (*normalizedParamToPlain)(void *, uint32, double);
    double (*plainParamToNormalized)(void *, uint32, double);
    double (*getParamNormalized)(void *, uint32);
    tresult (*setParamNormalized)(void *, uint32, double);
    tresult (*setComponentHandler)(void *, void *);
    void *(*createView)(void *, const char *);
} IEditControllerVtbl;

/* ------------------------------------------------------------------ objects */

typedef struct MockComponent MockComponent;

typedef struct MockProcessor {
    const IAudioProcessorVtbl *vtbl;
    MockComponent *owner;
} MockProcessor;

struct MockComponent {
    const IComponentVtbl *vtbl;
    int32 refcount;
    double gain;
    double mix;
    double sample_rate;
    int32 max_frames;
    int32 active;
    MockProcessor proc;
};

typedef struct MockController {
    const IEditControllerVtbl *vtbl;
    int32 refcount;
    double gain;
    double mix;
} MockController;

typedef struct MockFactory {
    const IPluginFactoryVtbl *vtbl;
    int32 refcount;
} MockFactory;

/* -------------------------------------------------------------- prototypes */

static tresult factory_query(void *, const char *, void **);
static uint32 factory_addref(void *);
static uint32 factory_release(void *);
static tresult factory_get_info(void *, PFactoryInfo *);
static int32 factory_count(void *);
static tresult factory_class_info(void *, int32, PClassInfo *);
static tresult factory_create(void *, const char *, const char *, void **);

static tresult component_query(void *, const char *, void **);
static uint32 component_addref(void *);
static uint32 component_release(void *);
static tresult component_initialize(void *, void *);
static tresult component_terminate(void *);
static tresult component_controller_id(void *, char *);
static tresult component_set_io_mode(void *, int32);
static int32 component_bus_count(void *, int32, int32);
static tresult component_bus_info(void *, int32, int32, int32, BusInfo *);
static tresult component_routing_info(void *, RoutingInfo *, RoutingInfo *);
static tresult component_activate_bus(void *, int32, int32, int32, uint8_t);
static tresult component_set_active(void *, uint8_t);
static tresult component_set_state(void *, void *);
static tresult component_get_state(void *, void *);

static tresult processor_query(void *, const char *, void **);
static uint32 processor_addref(void *);
static uint32 processor_release(void *);
static tresult processor_set_bus_arrangements(void *, uint64_t *, int32, uint64_t *, int32);
static tresult processor_get_bus_arrangement(void *, int32, int32, uint64_t *);
static tresult processor_can_sample_size(void *, int32);
static uint32 processor_latency(void *);
static tresult processor_setup(void *, ProcessSetup *);
static tresult processor_set_processing(void *, uint8_t);
static tresult processor_process(void *, ProcessData *);
static uint32 processor_tail(void *);

static tresult controller_query(void *, const char *, void **);
static uint32 controller_addref(void *);
static uint32 controller_release(void *);
static tresult controller_initialize(void *, void *);
static tresult controller_terminate(void *);
static tresult controller_set_component_state(void *, void *);
static tresult controller_set_state(void *, void *);
static tresult controller_get_state(void *, void *);
static int32 controller_param_count(void *);
static tresult controller_param_info(void *, int32, ParameterInfo *);
static tresult controller_param_string(void *, uint32, double, char16 *);
static tresult controller_param_value(void *, uint32, char16 *, double *);
static double controller_norm_to_plain(void *, uint32, double);
static double controller_plain_to_norm(void *, uint32, double);
static double controller_get_norm(void *, uint32);
static tresult controller_set_norm(void *, uint32, double);
static tresult controller_set_handler(void *, void *);
static void *controller_create_view(void *, const char *);

/* ----------------------------------------------------------------- vtables */

static const IPluginFactoryVtbl factory_vtbl = {
    factory_query,   factory_addref,    factory_release, factory_get_info,
    factory_count,   factory_class_info, factory_create,
};

static const IComponentVtbl component_vtbl = {
    component_query,      component_addref,       component_release,
    component_initialize, component_terminate,    component_controller_id,
    component_set_io_mode, component_bus_count,   component_bus_info,
    component_routing_info, component_activate_bus, component_set_active,
    component_set_state,  component_get_state,
};

static const IAudioProcessorVtbl processor_vtbl = {
    processor_query,       processor_addref,          processor_release,
    processor_set_bus_arrangements, processor_get_bus_arrangement,
    processor_can_sample_size, processor_latency,     processor_setup,
    processor_set_processing, processor_process,      processor_tail,
};

static const IEditControllerVtbl controller_vtbl = {
    controller_query,        controller_addref,      controller_release,
    controller_initialize,   controller_terminate,   controller_set_component_state,
    controller_set_state,    controller_get_state,   controller_param_count,
    controller_param_info,   controller_param_string, controller_param_value,
    controller_norm_to_plain, controller_plain_to_norm, controller_get_norm,
    controller_set_norm,     controller_set_handler, controller_create_view,
};

/* ---------------------------------------------------------------- helpers */

static int uid_eq(const char *a, const char *b) {
    return memcmp(a, b, 16) == 0;
}

static void copy_u16(char16 *dst, const char *src) {
    int i = 0;
    for (; src[i] != '\0' && i < 127; i++) {
        dst[i] = (char16)(unsigned char)src[i];
    }
    dst[i] = 0;
}

/* ----------------------------------------------------------------- factory */

static tresult factory_query(void *self, const char *iid, void **obj) {
    MockFactory *f = (MockFactory *)self;
    if (uid_eq(iid, IID_FUnknown) || uid_eq(iid, IID_IPluginFactory)) {
        f->refcount++;
        *obj = f;
        return kResultOk;
    }
    *obj = NULL;
    return kNoInterface;
}

static uint32 factory_addref(void *self) {
    MockFactory *f = (MockFactory *)self;
    return (uint32)(++f->refcount);
}

static uint32 factory_release(void *self) {
    MockFactory *f = (MockFactory *)self;
    if (--f->refcount <= 0) {
        f->refcount = 1; /* the factory is a process-wide singleton */
        return 0;
    }
    return (uint32)f->refcount;
}

static tresult factory_get_info(void *self, PFactoryInfo *info) {
    (void)self;
    memset(info, 0, sizeof(*info));
    memcpy(info->vendor, FACTORY_VENDOR, sizeof(FACTORY_VENDOR));
    memcpy(info->url, FACTORY_URL, sizeof(FACTORY_URL));
    memcpy(info->email, FACTORY_EMAIL, sizeof(FACTORY_EMAIL));
    info->flags = FACTORY_FLAGS;
    return kResultOk;
}

static int32 factory_count(void *self) {
    (void)self;
    return 2;
}

static tresult factory_class_info(void *self, int32 index, PClassInfo *info) {
    (void)self;
    memset(info, 0, sizeof(*info));
    if (index == 0) {
        memcpy(info->cid, COMPONENT_CID, 16);
        info->cardinality = 0x7FFFFFFF;
        memcpy(info->category, CATEGORY_AUDIO, sizeof(CATEGORY_AUDIO));
        memcpy(info->name, COMPONENT_NAME, sizeof(COMPONENT_NAME));
        return kResultOk;
    }
    if (index == 1) {
        memcpy(info->cid, CONTROLLER_CID, 16);
        info->cardinality = 0x7FFFFFFF;
        memcpy(info->category, CATEGORY_CONTROLLER, sizeof(CATEGORY_CONTROLLER));
        memcpy(info->name, CONTROLLER_NAME, sizeof(CONTROLLER_NAME));
        return kResultOk;
    }
    return kResultFalse;
}

static tresult factory_create(void *self, const char *cid, const char *iid, void **obj) {
    (void)self;
    *obj = NULL;
    if (uid_eq(cid, COMPONENT_CID)) {
        if (!uid_eq(iid, IID_IComponent) && !uid_eq(iid, IID_IPluginBase) &&
            !uid_eq(iid, IID_FUnknown)) {
            return kNoInterface;
        }
        MockComponent *c = (MockComponent *)calloc(1, sizeof(MockComponent));
        if (!c) {
            return kResultFalse;
        }
        c->vtbl = &component_vtbl;
        c->refcount = 1;
        c->gain = 1.0;
        c->mix = 1.0;
        c->proc.vtbl = &processor_vtbl;
        c->proc.owner = c;
        *obj = c;
        return kResultOk;
    }
    if (uid_eq(cid, CONTROLLER_CID)) {
        if (!uid_eq(iid, IID_IEditController) && !uid_eq(iid, IID_IPluginBase) &&
            !uid_eq(iid, IID_FUnknown)) {
            return kNoInterface;
        }
        MockController *c = (MockController *)calloc(1, sizeof(MockController));
        if (!c) {
            return kResultFalse;
        }
        c->vtbl = &controller_vtbl;
        c->refcount = 1;
        c->gain = 1.0;
        c->mix = 1.0;
        *obj = c;
        return kResultOk;
    }
    return kNoInterface;
}

/* --------------------------------------------------------------- component */

static tresult component_query(void *self, const char *iid, void **obj) {
    MockComponent *c = (MockComponent *)self;
    if (uid_eq(iid, IID_FUnknown) || uid_eq(iid, IID_IPluginBase) ||
        uid_eq(iid, IID_IComponent)) {
        c->refcount++;
        *obj = c;
        return kResultOk;
    }
    if (uid_eq(iid, IID_IAudioProcessor)) {
        c->refcount++;
        *obj = &c->proc;
        return kResultOk;
    }
    *obj = NULL;
    return kNoInterface;
}

static uint32 component_addref(void *self) {
    MockComponent *c = (MockComponent *)self;
    return (uint32)(++c->refcount);
}

static uint32 component_release(void *self) {
    MockComponent *c = (MockComponent *)self;
    if (--c->refcount <= 0) {
        free(c);
        return 0;
    }
    return (uint32)c->refcount;
}

static tresult component_initialize(void *self, void *context) {
    (void)self;
    (void)context;
    return kResultOk;
}

static tresult component_terminate(void *self) {
    (void)self;
    return kResultOk;
}

static tresult component_controller_id(void *self, char *class_id) {
    (void)self;
    memcpy(class_id, CONTROLLER_CID, 16);
    return kResultOk;
}

static tresult component_set_io_mode(void *self, int32 mode) {
    (void)self;
    (void)mode;
    return kResultOk;
}

static int32 component_bus_count(void *self, int32 type, int32 dir) {
    (void)self;
    if (type == 0 /* kAudio */) {
        return 1;
    }
    return 0;
}

static tresult component_bus_info(void *self, int32 type, int32 dir, int32 index,
                                  BusInfo *bus) {
    (void)self;
    if (type != 0 || index != 0) {
        return kResultFalse;
    }
    memset(bus, 0, sizeof(*bus));
    bus->mediaType = 0;
    bus->direction = dir;
    bus->channelCount = 2;
    copy_u16(bus->name, dir == 0 ? "Input" : "Output");
    bus->busType = 0;
    bus->flags = 1; /* kDefaultActive */
    return kResultOk;
}

static tresult component_routing_info(void *self, RoutingInfo *in_info, RoutingInfo *out_info) {
    (void)self;
    (void)in_info;
    (void)out_info;
    return kResultFalse;
}

static tresult component_activate_bus(void *self, int32 type, int32 dir, int32 index,
                                      uint8_t state) {
    (void)self;
    (void)type;
    (void)dir;
    (void)index;
    (void)state;
    return kResultOk;
}

static tresult component_set_active(void *self, uint8_t state) {
    MockComponent *c = (MockComponent *)self;
    c->active = state ? 1 : 0;
    return kResultOk;
}

static tresult component_set_state(void *self, void *state) {
    (void)self;
    (void)state;
    return kResultFalse;
}

static tresult component_get_state(void *self, void *state) {
    (void)self;
    (void)state;
    return kResultFalse;
}

/* --------------------------------------------------------------- processor */

static tresult processor_query(void *self, const char *iid, void **obj) {
    MockProcessor *p = (MockProcessor *)self;
    if (uid_eq(iid, IID_FUnknown) || uid_eq(iid, IID_IAudioProcessor)) {
        p->owner->refcount++;
        *obj = p;
        return kResultOk;
    }
    *obj = NULL;
    return kNoInterface;
}

static uint32 processor_addref(void *self) {
    MockProcessor *p = (MockProcessor *)self;
    return (uint32)(++p->owner->refcount);
}

static uint32 processor_release(void *self) {
    MockProcessor *p = (MockProcessor *)self;
    MockComponent *c = p->owner;
    if (--c->refcount <= 0) {
        free(c);
        return 0;
    }
    return (uint32)c->refcount;
}

static tresult processor_set_bus_arrangements(void *self, uint64_t *inputs, int32 num_ins,
                                              uint64_t *outputs, int32 num_outs) {
    (void)self;
    (void)inputs;
    (void)outputs;
    if (num_ins == 1 && num_outs == 1) {
        return kResultOk;
    }
    return kResultFalse;
}

static tresult processor_get_bus_arrangement(void *self, int32 dir, int32 index,
                                             uint64_t *arr) {
    (void)self;
    (void)dir;
    (void)index;
    *arr = 3; /* kStereo */
    return kResultOk;
}

static tresult processor_can_sample_size(void *self, int32 size) {
    (void)self;
    return size == 0 ? kResultOk : kResultFalse;
}

static uint32 processor_latency(void *self) {
    (void)self;
    return 0;
}

static tresult processor_setup(void *self, ProcessSetup *setup) {
    MockProcessor *p = (MockProcessor *)self;
    p->owner->sample_rate = setup->sampleRate;
    p->owner->max_frames = setup->maxSamplesPerBlock;
    return kResultOk;
}

static tresult processor_set_processing(void *self, uint8_t state) {
    (void)self;
    (void)state;
    return kResultOk;
}

static tresult processor_process(void *self, ProcessData *data) {
    MockProcessor *p = (MockProcessor *)self;
    MockComponent *c = p->owner;
    if (data->numSamples <= 0 || data->numOutputs < 1 || !data->outputs) {
        return kResultOk;
    }
    const float scale = (float)(1.0 - c->mix + c->mix * c->gain);
    for (int32 bus = 0; bus < data->numOutputs; bus++) {
        AudioBusBuffers *out = &data->outputs[bus];
        const AudioBusBuffers *in = (data->numInputs > bus) ? &data->inputs[bus] : NULL;
        for (int32 ch = 0; ch < out->numChannels; ch++) {
            float *dst = out->u.channelBuffers32[ch];
            const float *src = (in && ch < in->numChannels) ? in->u.channelBuffers32[ch] : NULL;
            for (int32 i = 0; i < data->numSamples; i++) {
                dst[i] = (src ? src[i] : 0.0f) * scale;
            }
        }
    }
    return kResultOk;
}

static uint32 processor_tail(void *self) {
    (void)self;
    return 0;
}

/* -------------------------------------------------------------- controller */

static tresult controller_query(void *self, const char *iid, void **obj) {
    MockController *c = (MockController *)self;
    if (uid_eq(iid, IID_FUnknown) || uid_eq(iid, IID_IPluginBase) ||
        uid_eq(iid, IID_IEditController)) {
        c->refcount++;
        *obj = c;
        return kResultOk;
    }
    *obj = NULL;
    return kNoInterface;
}

static uint32 controller_addref(void *self) {
    MockController *c = (MockController *)self;
    return (uint32)(++c->refcount);
}

static uint32 controller_release(void *self) {
    MockController *c = (MockController *)self;
    if (--c->refcount <= 0) {
        free(c);
        return 0;
    }
    return (uint32)c->refcount;
}

static tresult controller_initialize(void *self, void *context) {
    (void)self;
    (void)context;
    return kResultOk;
}

static tresult controller_terminate(void *self) {
    (void)self;
    return kResultOk;
}

static tresult controller_set_component_state(void *self, void *state) {
    (void)self;
    (void)state;
    return kResultFalse;
}

static tresult controller_set_state(void *self, void *state) {
    (void)self;
    (void)state;
    return kResultFalse;
}

static tresult controller_get_state(void *self, void *state) {
    (void)self;
    (void)state;
    return kResultFalse;
}

static int32 controller_param_count(void *self) {
    (void)self;
    return 2;
}

static tresult controller_param_info(void *self, int32 index, ParameterInfo *info) {
    (void)self;
    memset(info, 0, sizeof(*info));
    if (index == 0) {
        info->id = PARAM_GAIN_ID;
        copy_u16(info->title, "Gain");
        copy_u16(info->shortTitle, "Gain");
        copy_u16(info->units, "x");
        info->stepCount = 0;
        info->defaultNormalizedValue = 1.0;
        info->flags = PARAM_CAN_AUTOMATE;
        return kResultOk;
    }
    if (index == 1) {
        info->id = PARAM_MIX_ID;
        copy_u16(info->title, "Mix");
        copy_u16(info->shortTitle, "Mix");
        copy_u16(info->units, "%");
        info->stepCount = 1;
        info->defaultNormalizedValue = 1.0;
        info->flags = PARAM_CAN_AUTOMATE | PARAM_IS_LIST;
        return kResultOk;
    }
    return kResultFalse;
}

static tresult controller_param_string(void *self, uint32 id, double value, char16 *string) {
    (void)self;
    (void)id;
    (void)value;
    string[0] = 0;
    return kResultFalse;
}

static tresult controller_param_value(void *self, uint32 id, char16 *string, double *value) {
    (void)self;
    (void)id;
    (void)string;
    (void)value;
    return kResultFalse;
}

static double controller_norm_to_plain(void *self, uint32 id, double value) {
    (void)self;
    (void)id;
    return value;
}

static double controller_plain_to_norm(void *self, uint32 id, double value) {
    (void)self;
    (void)id;
    return value;
}

static double controller_get_norm(void *self, uint32 id) {
    MockController *c = (MockController *)self;
    return id == PARAM_GAIN_ID ? c->gain : c->mix;
}

static tresult controller_set_norm(void *self, uint32 id, double value) {
    MockController *c = (MockController *)self;
    if (id == PARAM_GAIN_ID) {
        c->gain = value;
    } else if (id == PARAM_MIX_ID) {
        c->mix = value;
    } else {
        return kResultFalse;
    }
    return kResultOk;
}

static tresult controller_set_handler(void *self, void *handler) {
    (void)self;
    (void)handler;
    return kResultOk;
}

static void *controller_create_view(void *self, const char *name) {
    (void)self;
    (void)name;
    return NULL;
}

/* ------------------------------------------------------------------ module */

static MockFactory g_factory = {&factory_vtbl, 1};

#ifdef __cplusplus
extern "C" {
#endif

bool InitDll(void) {
    return true;
}

bool ExitDll(void) {
    return true;
}
void *GetPluginFactory(void) {
    g_factory.refcount = 1;
    return &g_factory;
}

#ifdef __cplusplus
}
#endif
