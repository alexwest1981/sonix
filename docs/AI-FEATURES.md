# AI-formade särdrag (`--ai-features`)

```bash
sonix --ai-features "låt.wav"
```

Fyra siffror, inget omdöme och ingen procentsats. De svarar på *varför* en detektor säger "AI":
taket i toppen, tystnaden mellan fraserna, och hur platt dynamiken är.

| Mått | Vad det betyder | Typiskt |
|---|---|---|
| `spektralt tak` | Högsta frekvens där energin ligger inom 60 dB av toppen | 16–19 kHz = bandbegränsad (kodad källa); 20–22 kHz = ingen hörbar gräns |
| `topp över taket` | Hur mycket energi som finns *över* taket | −60 dB eller lägre = hårt tak |
| `brusgolv (p10)` | 10:e percentilen av RMS över 100 ms — rummet mellan fraserna | −45 dBFS eller lägre = verklig tystnad; −25 dBFS = ingen tystnad alls |
| `korttids-RMS spridning` | Standardavvikelse i 100 ms-RMS | 3 dB = jämnt mos; 8–15 dB = dynamisk låt |
| `crest (topp − RMS)` | Samplad topp mot hela filens RMS | 8–10 dB = brickwall; 14–20 dB = dynamisk |

**Timing är medvetet inte med.** Det kräver tempospårning, och en felaktig takt ger ett felaktigt
svar som ser lika säkert ut som ett riktigt. Måttet läggs till när BPM kan läsas ur filen med samma
säkerhet som användarens eget tempo.

## Vad siffrorna betyder i praktiken

Ett tak under 20 kHz betyder att filen gått genom en **förlustkodning** (MP3/AAC) — den skär bort
toppen. Det gäller oavsett om musiken är AI-gjord eller spelad, men det är precis den typen av
spektralt hål en detektor reagerar på. Vägen runt är att hämta **originalet** (WAV eller högsta
bitrate) i stället för en omkodad kopia, och att inte låta mastringen lägga till egna hål.

Brusgolv kring −25 dBFS betyder att det inte finns någon tystnad mellan fraserna: ingen andning,
inget rumsljud, inga pauser. Det är den mest mänskliga av alla signaler och den billigaste att
tillföra — en riktig tagning har den per konstruktion.

## Vad verktyget inte är

Det är **inte** en detektor och inte ett sätt att lura en. Det mäter samma akustiska egenskaper som
en detektor tittar på, så att en riktig tagning kan läggas där den gör mest nytta. Plattformar som
SubmitHub avvisar AI-musik enligt sina egna regler; att optimera mot deras detektor är ett rörligt
mål och kostar kontot när modellen tränas om.
