# 🍊 SONIX STUDIO - Professionell Linux DAW

Ett modernt, blixtsnabbt grafiskt musikproduktionsprogram skapat i **Rust** och **egui** med en integrerad modulär synthesizer, linjär flerspårs-audioredigerare med Suno AI-stöd, 16-stegs Channel Rack, Piano Roll och Touch-instrument för Linux (PipeWire / Wayland / ALSA / JACK).

---

## 🚀 Starta appen

Starta direkt från applikationsmenyn i Omarchy / Linux Desktop, eller kör från terminalen:

```bash
sonix
```

Eller kompilera och kör från källkod:

```bash
cd ~/Projects/sonix
cargo run --release
```

---

## 🎨 Huvudfunktioner i Sonix Studio:

### 1. 🎛️ Metallic Top Toolbar & LCD Display:
* **Transport:** Taktila kontroller för `▶ PLAY`, `⏸ PAUSE`, `⏹ STOP`.
* **Digital LCD:** Visar BPM, speltid och exakt taktposition (`BAR 01 : 03 : 12`).
* **Live Oscilloscope:** Högupplöst realtids-vågformsvisare.
* **Taktila Rotary Knobs:** Vridreglage för Master VOL och Master PAN.

### 2. 🎼 Suno AI Stem Import & Linjär Audioredigering:
* **Direkt Zip/Folder-import:** Dra in eller öppna Suno Stem-arkiv/mappar (`vocals`, `drums`, `bass`, `guitar`, `keys`, `back_vocals`, etc.).
* **Realtids Vågformer:** Kontinuerliga ljudregioner med renderade toppvärden.
* **Klippverktyg (Slice ✂):** Klipp i ljudregioner var som helst på tidslinjen med millimeterprecision.
* **Verktygspalett:** Välj (Pointer), Rita (Paint), Klipp (Slice ✂), Radera (Trash 🗑), Mute (🔇).
* **Flerspårs Streaming:** Synkroniserad realtidsmixning direkt från ljudmotorn med noll latens.

### 3. 🥁 Sonix Channel Rack (16-Stegs Sequencer):
* **6 Dedikerade Spår:**
  1. 💥 `808 Kick Drum`
  2. 🥁 `909 Snare Drum`
  3. ⚡ `Crisp Closed Hi-Hat`
  4. 🌊 `Open Hi-Hat`
  5. 🎹 `303 Acid Synth Lead`
  6. 🎸 `Sub Bassline`
* **4-takt-stegknappar:** Taktila knappar grupperade i fyror med **lysande vita center-LEDs** vid aktivering.
* **Mute [M] & Solo [S]** med status-LEDs.
* **Mini VOL & PAN Knobs** för varje enskild kanal.

### 4. 🎛️ Sonix Analog Synthesizer:
* **Filter & Resonans:** `CUTOFF` (Hz) och `RESO` (Q).
* **ADSR Envelope:** `ATTACK`, `DECAY`, `SUSTAIN`, `RELEASE`.
* **Live Envelope Graf:** Ritar upp den exakta kurvan i realtid.
* **Oscillatorer:** `∿ Sinus`, `⩘ Sågtand`, `⊓ Fyrkant`, `⋀ Triangel`.

### 5. 🎹 Sonix Piano Roll & Touch Piano:
* **Interaktiv Noteditor:** Polyfonisk pianoroll med dynamiska notlängder och anslagsdynamik (velocity).
* **Touch Klaviatur:** Taktilt klaviatur med visuell neonfeedback vid spelning.
