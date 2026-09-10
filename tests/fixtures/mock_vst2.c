/*
 * A minimal, real VST2 plugin used only to test Sonix's VST2 host.
 *
 * It implements the VST 2.4 C ABI by hand (no SDK dependency): an exported
 * `VSTPluginMain` returning an `AEffect` whose `dispatcher`, `setParameter`,
 * `getParameter` and `processReplacing` entry points are laid out exactly as
 * `aeffect.h` declares them. The effect is a stereo gain/delay with two
 * parameters ("Gain" and "Mix") and a fixed 8-frame latency, matching the CLAP
 * and VST3 mocks so all three backends can be exercised by the same tests.
 *
 * Build (done automatically by build.rs when `--features plugin-host`):
 *   cc -shared -fPIC -O2 -o libsonix_mock_vst2.so tests/fixtures/mock_vst2.c
 */

#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef int32_t VstInt32;
typedef intptr_t VstIntPtr;

typedef struct AEffect AEffect;

typedef VstIntPtr (*audioMasterCallback)(AEffect *, VstInt32, VstInt32,
                                         VstIntPtr, void *, float);
typedef VstIntPtr (*AEffectDispatcherProc)(AEffect *, VstInt32, VstInt32,
                                           VstIntPtr, void *, float);
typedef void (*AEffectProcessProc)(AEffect *, float **, float **, VstInt32);
typedef void (*AEffectProcessDoubleProc)(AEffect *, double **, double **,
                                         VstInt32);
typedef void (*AEffectSetParameterProc)(AEffect *, VstInt32, float);
typedef float (*AEffectGetParameterProc)(AEffect *, VstInt32);

struct AEffect {
    VstInt32 magic;
    AEffectDispatcherProc dispatcher;
    AEffectProcessProc process;
    AEffectSetParameterProc setParameter;
    AEffectGetParameterProc getParameter;
    VstInt32 numPrograms;
    VstInt32 numParams;
    VstInt32 numInputs;
    VstInt32 numOutputs;
    VstInt32 flags;
    VstIntPtr resvd1;
    VstIntPtr resvd2;
    VstInt32 initialDelay;
    VstInt32 realQualities;
    VstInt32 offQualities;
    float ioRatio;
    void *object;
    void *user;
    VstInt32 uniqueID;
    VstInt32 version;
    AEffectProcessProc processReplacing;
    AEffectProcessDoubleProc processDoubleReplacing;
    char future[56];
};

/* aeffect.h opcodes (VST 2.4). */
#define effOpen 0
#define effClose 1
#define effSetProgram 2
#define effGetProgram 3
#define effGetProgramName 5
#define effGetParamLabel 6
#define effGetParamDisplay 7
#define effGetParamName 8
#define effSetSampleRate 10
#define effSetBlockSize 11
#define effMainsChanged 12
#define effGetChunk 23
#define effSetChunk 24
#define effGetPlugCategory 35
#define effGetEffectName 45
#define effGetVendorString 47
#define effGetProductString 48
#define effGetVendorVersion 49
#define effCanDo 51
#define effGetTailSize 52
#define effGetParameterProperties 56
#define effGetVstVersion 58
#define effSetProcessPrecision 77

#define kEffectMagic 0x56737450

#define effFlagsCanReplacing 16
#define effFlagsProgramChunks 32

#define kPlugCategEffect 1

/* VstParameterProperties flags. */
#define kVstParameterIsSwitch 1

/* kVstMaxParamStrLen, kVstMaxEffectNameLen, kVstMaxVendorStrLen. */
#define kParamStrLen 8
#define kEffectNameLen 32
#define kVendorStrLen 64

#define LATENCY_SAMPLES 8
#define PARAM_GAIN 0
#define PARAM_MIX 1

/* Layout must match aeffectx.h's `VstParameterProperties` exactly. */
typedef struct VstParameterProperties {
    float stepFloat;
    float smallStepFloat;
    float largeStepFloat;
    char label[64];
    VstInt32 flags;
    VstInt32 minInteger;
    VstInt32 maxInteger;
    VstInt32 stepInteger;
    VstInt32 largeStepInteger;
    char shortLabel[8];
    int16_t displayIndex;
    int16_t category;
    int16_t numParametersInCategory;
    int16_t reserved;
    char categoryLabel[24];
    char future[16];
} VstParameterProperties;

typedef struct MockVst2 {
    AEffect effect;
    double gain;
    double mix;
    float delay_l[LATENCY_SAMPLES];
    float delay_r[LATENCY_SAMPLES];
    int delay_pos;
    double sample_rate;
    VstInt32 max_block;
    int active;
} MockVst2;

static void mock_set_parameter(AEffect *e, VstInt32 index, float value) {
    MockVst2 *m = (MockVst2 *)e;
    if (index == PARAM_GAIN) {
        m->gain = value;
    } else if (index == PARAM_MIX) {
        m->mix = value;
    }
}

static float mock_get_parameter(AEffect *e, VstInt32 index) {
    MockVst2 *m = (MockVst2 *)e;
    if (index == PARAM_GAIN) {
        return (float)m->gain;
    }
    if (index == PARAM_MIX) {
        return (float)m->mix;
    }
    return 0.0f;
}

static void mock_process_replacing(AEffect *e, float **inputs, float **outputs,
                                   VstInt32 frames) {
    MockVst2 *m = (MockVst2 *)e;
    const float mix = (float)m->mix;
    const float gain = (float)m->gain;
    for (VstInt32 i = 0; i < frames; i++) {
        const float in_l = inputs[0][i];
        const float in_r = inputs[1][i];
        const float wet_l = m->delay_l[m->delay_pos] * gain;
        const float wet_r = m->delay_r[m->delay_pos] * gain;
        m->delay_l[m->delay_pos] = in_l;
        m->delay_r[m->delay_pos] = in_r;
        m->delay_pos = (m->delay_pos + 1) % LATENCY_SAMPLES;
        outputs[0][i] = in_l * (1.0f - mix) + wet_l * mix;
        outputs[1][i] = in_r * (1.0f - mix) + wet_r * mix;
    }
}

static void mock_process_noop(AEffect *e, float **inputs, float **outputs,
                              VstInt32 frames) {
    (void)e;
    (void)inputs;
    (void)outputs;
    (void)frames;
}

static VstIntPtr mock_dispatcher(AEffect *e, VstInt32 opcode, VstInt32 index,
                                 VstIntPtr value, void *ptr, float opt) {
    MockVst2 *m = (MockVst2 *)e;
    switch (opcode) {
    case effOpen:
        return 0;
    case effClose:
        free(m);
        return 0;
    case effSetProgram:
    case effGetProgram:
        return 0;
    case effGetProgramName:
        if (ptr) {
            strncpy((char *)ptr, "Default", kParamStrLen);
            ((char *)ptr)[kParamStrLen - 1] = 0;
        }
        return 0;
    case effGetParamLabel:
        if (ptr) {
            ((char *)ptr)[0] = 0;
        }
        return 0;
    case effGetParamDisplay:
        if (ptr) {
            const double v = (index == PARAM_GAIN) ? m->gain : m->mix;
            snprintf((char *)ptr, kParamStrLen, "%.2f", v);
        }
        return 0;
    case effGetParamName:
        if (ptr && index >= 0 && index < 2) {
            const char *name = (index == PARAM_GAIN) ? "Gain" : "Mix";
            strncpy((char *)ptr, name, kParamStrLen);
            ((char *)ptr)[kParamStrLen - 1] = 0;
        }
        return 0;
    case effGetParameterProperties:
        if (index == PARAM_MIX && ptr) {
            VstParameterProperties *p = (VstParameterProperties *)ptr;
            memset(p, 0, sizeof(*p));
            p->flags = kVstParameterIsSwitch;
            strncpy(p->shortLabel, "Mix", sizeof(p->shortLabel));
            p->shortLabel[sizeof(p->shortLabel) - 1] = 0;
            return 1;
        }
        return 0;
    case effSetSampleRate:
        m->sample_rate = (double)opt;
        return 0;
    case effSetBlockSize:
        m->max_block = (VstInt32)value;
        return 0;
    case effMainsChanged:
        m->active = (value != 0);
        if (value) {
            memset(m->delay_l, 0, sizeof(m->delay_l));
            memset(m->delay_r, 0, sizeof(m->delay_r));
            m->delay_pos = 0;
        }
        return 0;
    case effGetEffectName:
        if (ptr) {
            strncpy((char *)ptr, "Sonix Mock VST2", kEffectNameLen);
            ((char *)ptr)[kEffectNameLen - 1] = 0;
        }
        return 0;
    case effGetVendorString:
        if (ptr) {
            strncpy((char *)ptr, "Sonix Test", kVendorStrLen);
            ((char *)ptr)[kVendorStrLen - 1] = 0;
        }
        return 0;
    case effGetProductString:
        if (ptr) {
            strncpy((char *)ptr, "Sonix Mock VST2", kVendorStrLen);
            ((char *)ptr)[kVendorStrLen - 1] = 0;
        }
        return 0;
    case effGetVendorVersion:
        return 100;
    case effGetPlugCategory:
        return kPlugCategEffect;
    case effGetVstVersion:
        return 2400;
    case effSetProcessPrecision:
        return 1;
    case effGetTailSize:
        return LATENCY_SAMPLES;
    case effCanDo:
        return 0;
    case effGetChunk:
        if (index == 1) {
            double *buf = (double *)malloc(2 * sizeof(double));
            if (!buf) {
                return 0;
            }
            buf[0] = m->gain;
            buf[1] = m->mix;
            if (ptr) {
                *(void **)ptr = buf;
            }
            return (VstIntPtr)(2 * sizeof(double));
        }
        return 0;
    case effSetChunk:
        if (index == 1 && ptr && value >= (VstIntPtr)(2 * sizeof(double))) {
            const double *buf = (const double *)ptr;
            m->gain = buf[0];
            m->mix = buf[1];
            return 1;
        }
        return 0;
    default:
        return 0;
    }
}

AEffect *VSTPluginMain(audioMasterCallback audioMaster) {
    (void)audioMaster;
    MockVst2 *m = (MockVst2 *)calloc(1, sizeof(MockVst2));
    if (!m) {
        return NULL;
    }
    m->gain = 1.0;
    m->mix = 1.0;
    m->sample_rate = 44100.0;
    m->max_block = 512;

    AEffect *e = &m->effect;
    e->magic = kEffectMagic;
    e->dispatcher = mock_dispatcher;
    e->process = mock_process_noop;
    e->setParameter = mock_set_parameter;
    e->getParameter = mock_get_parameter;
    e->numPrograms = 1;
    e->numParams = 2;
    e->numInputs = 2;
    e->numOutputs = 2;
    e->flags = effFlagsCanReplacing | effFlagsProgramChunks;
    e->initialDelay = LATENCY_SAMPLES;
    e->uniqueID = 0x534E5832; /* 'SNX2' */
    e->version = 100;
    e->processReplacing = mock_process_replacing;
    e->object = m;
    return e;
}
