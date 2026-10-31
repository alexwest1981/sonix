# Abletons DSP-karta — hur man hittar algoritmerna

`docs/ABLETON-BINARY-MAP.md` svarade på *vad* som finns i Ableton Live 12.4.5: 382 `O*Processor`-klasser
i RTTI, 6 087 klassnamn, en binär som **inte** är packad. Det här dokumentet svarar på *var* de sitter
och hur man kommer från ett enhetsnamn till dess kod.

Allt nedan är mätt på den uppackade installationen
(`Ableton Live 12 Suite.exe`, 148 197 832 B, PE32+ x86-64, image base `0x140000000`).
Skriptet som tog fram det ligger i `scripts/extract-ableton-registry.py` och kan köras om.

## Registreringstabellen

Varje processor registreras med **samma tre instruktioner**:

```asm
lea  rdx, [rip+disp32]   ; RTTI-typbeskrivning för klassen
lea  rcx, [rip+disp32]   ; namnet som C-sträng
call 0x143406320         ; registreringen
```

Fyndet kom från strängen `ORoarProcessor` (VA `0x146cfda58`), som refereras från tre ställen i
`.text`; kring det första (`0x1434e0802`) låg 227 sådana block efter varandra. Registret är alltså
**en enda sammanhängande tabell** från `0x1434dec36` till `0x1434e0976`, med ett block var ~0x20 byte.

Ett svep över `.text` (100 MB, ~45 s i Python med `struct.unpack_from`) ger **227 poster**:
`(namn, typ_info-adress, fabriksadress)`. Alla har samma fabrik (`0x143406320`) — det är
registreringsfunktionen, inte en konstruktor per enhet.

Ett urval ur tabellen, i den ordning Ableton lagrar dem (adress = `lea rcx`-instruktionen):

| Processor | Adress |
| :--- | :--- |
| `OAutoFilter2Processor` | `0x1434e08f0` |
| `OChorus2Processor` | `0x1434df0d6` |
| `ODelayProcessor` | `0x1434e0736` |
| `OEchoProcessor` | `0x1434df396` |
| `OErosion2Processor` | `0x1434e0934` |
| `OGlueCompressorProcessor` | `0x1434df5d6` |
| `OHybridProcessor` | `0x1434df616` |
| `OInstrumentWavetableProcessor` | `0x1434df7b4` |
| `OMeldProcessor` | `0x1434e0824` |
| `OPhaserNewProcessor` | `0x1434e0180` |
| `ORedux2Processor` | `0x1434e0758` |
| `OReverbProcessor` | `0x1434e02b2` |
| `ORoarProcessor` | `0x1434e0802` |
| `OShifterProcessor` | `0x1434e07be` |
| `OSineOscillatorProcessor` | `0x1434e046c` |
| `OSpectralTimeProcessor` | `0x1434e079c` |
| `OTransmuteProcessor` | `0x1434e077a` |

Utöver `O*`-klasserna finns byggstenar med egna namn i samma tabell — `AdsrFilter`, `FilterDelay`,
`FilterEQ3`, `FilterSatDecay`, `InstrumentOperatorFilter`, `InstrumentOperatorWavetable`,
`SimplerSubOsc`, `VoiceAllocator`, `MultibandDynamics`, `MidiArpeggiator`, `MidiChord`,
`MidiNoteLength`, `MidiRandom`, `MidiScale`, `MidiVelocity`, `MidiPitcher`. Det är Abletons **interna
moduluppdelning**: `ORoarProcessor` är alltså en armatur kring shapers, filter och feedback, inte en
enda stor funktion.

## Konstruktorn namnger vtable:n

Registreringens tredje fält pekar inte på RTTI utan på **konstruktorn** — koden på
`0x143600300` (för `ORoarProcessor`) är

```asm
48 89 4c 24 08        mov  [rsp+8], rcx      ; this
53                    push rbx
48 83 ec 20           sub  rsp, 0x20
48 8b d9              mov  rbx, rcx
e8 7e 17 8c fd        call <basklassens ctor>
48 8d 05 56 a2 70 03  lea  rax, [rip+0x370a256]   ; <- vtable
48 89 03              mov  [rbx], rax        ; this->vptr = vtable
```

Alltså: **konstruktorn skriver vtable-pekaren i objektets första ord**, och `lea`-adressen är
vtable:n. Mätt för två enheter:

| Enhet | Konstruktor | Vtable | Slot 0 | Slot 1 |
| :--- | :--- | :--- | :--- | :--- |
| `ORoarProcessor` | `0x143600300` | `0x146d0a580` | `0x143422160` | `0x140ec1860` |
| `OReverbProcessor` | `0x1435ed780` | `0x146d08ff0` | `0x143422160` | `0x140db6aa0` |

Slot 0 är samma funktion för båda (`0x143422160`) — det är destruktorn, som väntat i MSVC-layouten.
Slot 1 skiljer sig, så det är en virtuell metod i basklassen. `OSineOscillatorProcessor` gav ingen
`lea`-träff inom 64 byte, så mönstret gäller inte alla klasser (vissa konstruktorer sätter vtable:n
via en delad hjälpfunktion).

**Vad som fortfarande fattas:** att avgöra *vilken* slot som är `process()` och läsa den. Det går
inte att gissa ur ordningen — MSVC lägger metoderna i deklarationsordning och vi har ingen header.
Nästa steg är att disassemblera slot 1..n och se vilken som refererar sample-rate-medlemmen och
skriver till utbufferten; det är den enda säkra kännetecknen.

## Vägen vidare till koden

Registreringen ger `typ_info`-adressen per klass. Därifrån går kedjan:

1. `typ_info` → dess `_type_info`-vtable (`??_7type_info@@6B@`) → `type_info::vftable`.
2. `type_info` bär `name()` som pekar på den manglade dekorationen (`.?AVORoarProcessor@@`), vilket
   bekräftar att typen är rätt och inte en namne.
3. Klassens **vtable** hittas via `RTTICompleteObjectLocator` i `.rdata`: sök efter en post vars
   `TypeDescriptor`-RVA pekar på `typ_info`.
4. Vtable-slots är `process()`/`processBlock()`, `prepareToPlay()`, `parameter`-getters osv. Slot 0–3
   är nästan alltid destruktor och `prepare`, medan själva DSP:n ligger i den slot som anropas per
   block — den syns genom att den refererar sample-rate-medlemmen och skriver till `rdx`/`r8`.

Det steget är **inte gjort än**: ingen vtable har följts och ingen DSP-funktion är läst. Det som är
mätt är tabellen ovan. Att gå vidare är mekaniskt (skriptet kan byggas ut med steg 1–3), men det är
`L`-arbete *per enhet* — därav att backloggen säger "L per enhet" och inte "L".

## Vad det här **inte** ger

- Inga **kurvor**. Shaper-tabeller och filterkoefficienter ligger i `.rdata` som flyttalstabeller utan
  namn; att hitta dem kräver att man först hittar den funktion som läser dem (steg 4).
- Inga **parameterenheter**. Parametrarnas intervall står i `.adv`-presets (se
  `ABLETON-BINARY-MAP.md` §3) och i `Parameter`-objekten, inte i `typ_info`.
- Ingen **licens**. Att läsa för att förstå är en sak; att kopiera kod är en annan. Rubber Band-valet
  i 0.9.220 är vårt svar på samma behov utan den frågan.

## Reproduktion

```sh
innoextract -s -d ableton ~/Downloads/"Ableton Live 12 Suite Setup.exe"   # engångs
python3 scripts/extract-ableton-registry.py \
    ableton/app/Program/"Ableton Live 12 Suite.exe" ableton_registry.tsv
wc -l ableton_registry.tsv      # 11141 rader: hela .text, inte bara registret
```

De 11 141 raderna är **alla** ställen med samma instruktionsmönster, inklusive
assertionsmeddelanden (`lea rdx,<typ>; lea rcx,<påstående>; call`). Enhetsregistret är det
sammanhängande kluster som innehåller `O*Processor`-namn — 227 poster vid `0x1434dec36`. Klustringen
görs genom att sortera på adress och hugga där avståndet överstiger `0x80`; sju kluster har minst fem
`O*Processor`-poster.
