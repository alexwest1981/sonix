//! **SoundFont 2 — instrument ur en `.sf2`-fil** (FL Studio: *Soundfont Player* / *DirectWave*).
//!
//! En `.sf2` är en RIFF-fil med två halvor: `sdta` (rå 16-bitars PCM) och `pdta`
//! (presets → instrument → zoner → sampel). Vi läser **instrumentlagret** och gör om varje
//! zon till en [`SampleZone`], alltså exakt samma modell som multi-samplern redan spelar:
//! `main`-kanalen behöver ingen ny DSP för att ett soundfont ska låta.
//!
//! **Vad som läses:** `phdr` (namn, för listan), `inst`/`ibag`/`igen` (zoner, `keyRange`,
//! `velRange`, `sampleID`, `overridingRootKey`), `shdr` (start/slut, samplingsfrekvens,
//! grundton) och `smpl` (själva ljudet).
//!
//! **Vad som inte läses, med flit:** presetlagrets egna zoner (bank/preset-val), `pmod`
//! (LFO/EG-modulation) och `pgen` utöver operator 41 (presetets instrument, för presetmenyn). Samplets **loop-punkter**
//! läses (0.9.66) och följer med zonen; saknar sampeln dem gäller kanalens egen `loop_mode`.
//! Instrumentet används
//! alltså direkt, utan preset-förval. Det är samma avgränsning som ett "load instrument"
//! i en sampler, inte en ofullständig preset-spelare.

use std::sync::Arc;

use super::keymap::SampleZone;

/// Ett instrument ur filen, med sina zoner i filens ordning (den sista matchande zonen vinner,
/// samma regel som keymappen).
#[derive(Debug, Clone, Default)]
pub struct SoundFontInstrument {
    pub name: String,
    pub zones: Vec<SampleZone>,
}

/// Hela filen: presetnamnen (för en meny), vilket instrument varje preset pekar på, och
/// instrumenten.
#[derive(Debug, Clone, Default)]
pub struct SoundFont {
    pub presets: Vec<String>,
    /// `presets[i]` → index i `instruments`, from the preset's own `pgen` operator 41
    /// (`instrument`). `None` when the preset names no instrument we could read.
    pub preset_instruments: Vec<Option<usize>>,
    pub instruments: Vec<SoundFontInstrument>,
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn name_at(b: &[u8], o: usize) -> String {
    let raw = &b[o..o + 20];
    let end = raw.iter().position(|&c| c == 0).unwrap_or(20);
    String::from_utf8_lossy(&raw[..end]).trim().to_string()
}

/// Letar upp ett chunk-id i en chunk-lista av formen `id(4) size(4) data…` och returnerar datan.
fn find_chunk<'a>(mut b: &'a [u8], want: &[u8; 4]) -> Option<&'a [u8]> {
    while b.len() >= 8 {
        let id = &b[0..4];
        let size = u32_at(b, 4) as usize;
        let end = (8 + size).min(b.len());
        if id == want {
            return Some(&b[8..end]);
        }
        // Chunkar är ordnade i 2-byte-gräns.
        let padded = 8 + size + (size & 1);
        if padded >= b.len() {
            return None;
        }
        b = &b[padded..];
    }
    None
}

/// Läser en `.sf2`-fil.
pub fn parse(bytes: &[u8]) -> Result<SoundFont, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"sfbk" {
        return Err("inte en SoundFont 2-fil (RIFF/sfbk saknas)".to_string());
    }
    let mut pdta: Option<&[u8]> = None;
    let mut smpl: Option<&[u8]> = None;
    let mut b = &bytes[12..];
    while b.len() >= 8 {
        let id = &b[0..4];
        let size = u32_at(b, 4) as usize;
        let end = (8 + size).min(b.len());
        let data = &b[8..end];
        if id == b"LIST" && data.len() >= 4 {
            let kind = &data[0..4];
            if kind == b"pdta" {
                pdta = Some(&data[4..]);
            } else if kind == b"sdta" {
                smpl = find_chunk(&data[4..], b"smpl");
            }
        }
        let padded = 8 + size + (size & 1);
        if padded >= b.len() {
            break;
        }
        b = &b[padded..];
    }
    let pdta = pdta.ok_or("filen saknar pdta (instrumenttabellerna)")?;
    let smpl = smpl.ok_or("filen saknar sdta/smpl (ljudet)")?;

    let phdr = find_chunk(pdta, b"phdr").ok_or("pdta saknar phdr")?;
    let pbag = find_chunk(pdta, b"pbag").ok_or("pdta saknar pbag")?;
    let inst = find_chunk(pdta, b"inst").ok_or("pdta saknar inst")?;
    let ibag = find_chunk(pdta, b"ibag").ok_or("pdta saknar ibag")?;
    let igen = find_chunk(pdta, b"igen").ok_or("pdta saknar igen")?;
    let shdr = find_chunk(pdta, b"shdr").ok_or("pdta saknar shdr")?;

    let _ = pbag; // preset-zonernas generatorer läses inte utom operator 41 nedan.

    // --- Ljudet: 16-bitars mono i `smpl`, indexerat i *ramar* av shdr.
    if smpl.len() % 2 != 0 {
        return Err("smpl är inte jämn 16-bitars data".to_string());
    }
    let pcm: Vec<f32> = smpl
        .as_chunks::<2>().0.iter()
        .map(|p| i16::from_le_bytes([p[0], p[1]]) as f32 / 32_768.0)
        .collect();

    struct RawSample {
        start: usize,
        end: usize,
        rate: u32,
        root: u8,
        /// `shdr`-loopens start/slut i *filens* frames. SF2 skriver ofta 0/0 när sampeln
        /// inte loopar.
        loop_frames: (usize, usize),
    }
    // shdr-post = 46 byte; sista posten är EOP.
    let sh_count = shdr.len() / 46;
    if sh_count == 0 {
        return Err("shdr är tom".to_string());
    }
    let mut samples: Vec<RawSample> = Vec::with_capacity(sh_count.saturating_sub(1));
    for i in 0..sh_count.saturating_sub(1) {
        let o = i * 46;
        samples.push(RawSample {
            start: u32_at(shdr, o + 20) as usize,
            end: u32_at(shdr, o + 24) as usize,
            rate: u32_at(shdr, o + 36),
            root: shdr[o + 40],
            loop_frames: (
                u32_at(shdr, o + 28) as usize,
                u32_at(shdr, o + 32) as usize,
            ),
        });
    }

    // --- Presetnamnen (för en meny).
    let mut presets = Vec::new();
    let ph_count = phdr.len() / 38;
    for i in 0..ph_count.saturating_sub(1) {
        presets.push(name_at(phdr, i * 38));
    }

    // --- Preset → instrument, so a preset picker imports the chosen preset and not just the
    // first instrument in the file. The link only exists in the preset zone's generator 41;
    // the first zone that names an instrument wins (a layered preset names one per zone).
    let pgen = find_chunk(pdta, b"pgen").unwrap_or(&[]);
    let mut preset_instruments = Vec::with_capacity(ph_count.saturating_sub(1));
    for i in 0..ph_count.saturating_sub(1) {
        let o = i * 38;
        let bag_first = u16_at(phdr, o + 24) as usize;
        let bag_last = u16_at(phdr, o + 38 + 24) as usize; // next record's bag index
        let mut instrument = None;
        for bag in bag_first..bag_last.min(pbag.len() / 4) {
            // The range ends at the *next* bag's generator index.
            if bag * 4 + 6 > pbag.len() {
                break;
            }
            let gen_first = u16_at(pbag, bag * 4) as usize;
            let gen_last = u16_at(pbag, bag * 4 + 4) as usize;
            if let Some(g) =
                (gen_first..gen_last.min(pgen.len() / 4)).find(|&g| u16_at(pgen, g * 4) == 41)
            {
                instrument = Some(u16_at(pgen, g * 4 + 2) as usize);
                break;
            }
        }
        preset_instruments.push(instrument);
    }

    // --- Instrumenten: bag-intervallen ligger i `inst`, generatorerna i `igen`.
    let inst_count = inst.len() / 22;
    let mut instruments = Vec::new();
    for i in 0..inst_count.saturating_sub(1) {
        let o = i * 22;
        let name = name_at(inst, o);
        let bag_first = u16_at(inst, o + 20) as usize;
        let bag_last = u16_at(inst, o + 22 + 20) as usize; // nästa posts bag-index (posten är 22 byte)
        let mut zones = Vec::new();
        for bag in bag_first..bag_last.min(ibag.len() / 4) {
            let gen_first = u16_at(ibag, bag * 4) as usize;
            let gen_last = u16_at(ibag, bag * 4 + 4) as usize;
            let mut sample_id: Option<usize> = None;
            let mut key_low = 0u8;
            let mut key_high = 127u8;
            let mut vel_low = 0.0f32;
            let mut vel_high = 1.0f32;
            let mut root_override: Option<u8> = None;
            for g in gen_first..gen_last.min(igen.len() / 4) {
                let go = g * 4;
                let oper = u16_at(igen, go);
                let lo = igen[go + 2];
                let hi = igen[go + 3];
                match oper {
                    43 => {
                        // keyRange
                        key_low = lo;
                        key_high = hi;
                    }
                    44 => {
                        // velRange, 0–127 → kanalens 0,0–1,0
                        vel_low = lo as f32 / 127.0;
                        vel_high = hi as f32 / 127.0;
                    }
                    53 => sample_id = Some(u16_at(igen, go + 2) as usize),
                    58 => root_override = Some(lo),
                    _ => {}
                }
            }
            // En bag utan `sampleID` är instrumentets *globala* zon (inställningar för alla
            // zoner). Vi hoppar över den: den bär inget ljud, och att slå ihop den med varje
            // zon hade krävt en andra tabell för samma sak.
            let Some(id) = sample_id else { continue };
            let Some(s) = samples.get(id) else { continue };
            let (start, end) = (s.start.min(pcm.len()), s.end.min(pcm.len()));
            if end <= start {
                continue;
            }
            let mono = Arc::new(pcm[start..end].to_vec());
            zones.push(SampleZone {
                sample_path: None,
                pcm: Some((mono.clone(), mono, s.rate.max(1))),
                // Loopen räknas om till zonens eget ljud (start..end), annars pekar den
                // utanför när flera sampel delar `smpl`.
                loop_frames: {
                    let (ls, le) = s.loop_frames;
                    (le > ls && ls >= start && le <= end)
                        .then_some(((ls - start) as u32, (le - start) as u32))
                },
                root: root_override.unwrap_or(s.root),
                key_low,
                key_high,
                vel_low,
                vel_high,
                seq: None,
            });
        }
        instruments.push(SoundFontInstrument { name, zones });
    }

    Ok(SoundFont {
        presets,
        preset_instruments,
        instruments,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Bygger en minimal men giltig `.sf2` i minnet: två zoner, två sampel.
    pub(crate) fn tiny_sf2() -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut v = Vec::new();
            v.extend_from_slice(id);
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(data);
            if data.len() % 2 == 1 {
                v.push(0);
            }
            v
        }
        fn name20(s: &str) -> Vec<u8> {
            let mut v = s.as_bytes().to_vec();
            v.resize(20, 0);
            v
        }

        // 16 ramar: 0..7 (sampel 0) blir 0,0 … 7/32768; 8..15 (sampel 1) blir 100/32768 …
        let mut pcm = Vec::new();
        for i in 0..8i16 {
            pcm.extend_from_slice(&i.to_le_bytes());
        }
        for i in 0..8i16 {
            pcm.extend_from_slice(&(100 + i).to_le_bytes());
        }

        // shdr: två sampel + EOP.
        let mut shdr = Vec::new();
        for (n, (start, end)) in [("A", (0u32, 8u32)), ("B", (8, 16))] {
            shdr.extend_from_slice(&name20(n));
            shdr.extend_from_slice(&start.to_le_bytes());
            shdr.extend_from_slice(&end.to_le_bytes());
            shdr.extend_from_slice(&start.to_le_bytes()); // loopstart
            shdr.extend_from_slice(&end.to_le_bytes()); // loopslut
            shdr.extend_from_slice(&48_000u32.to_le_bytes());
            shdr.push(60); // original_pitch
            shdr.push(0); // pitch_correction
            shdr.extend_from_slice(&0u16.to_le_bytes()); // sample_link
            shdr.extend_from_slice(&1u16.to_le_bytes()); // mono
        }
        shdr.extend_from_slice(&name20("EOP"));
        shdr.resize(shdr.len() + 26, 0);

        // igen: zon0 = keyRange 0–59 + sampleID 0; zon1 = keyRange 60–127 + sampleID 1.
        let mut igen = Vec::new();
        for (oper, lo, hi) in [(43u16, 0u8, 59u8), (53, 0, 0), (43, 60, 127), (53, 1, 0)] {
            igen.extend_from_slice(&oper.to_le_bytes());
            igen.push(lo);
            igen.push(hi);
        }
        igen.extend_from_slice(&0u16.to_le_bytes()); // EOP-gen

        // ibag: bag0 → gen 0, bag1 → gen 2, EOP → 4.
        let mut ibag = Vec::new();
        for (gen_ndx, m) in [(0u16, 0u16), (2, 0), (4, 0)] {
            ibag.extend_from_slice(&gen_ndx.to_le_bytes());
            ibag.extend_from_slice(&m.to_le_bytes());
        }

        // inst: ett instrument med bag 0, plus EOP.
        let mut inst = Vec::new();
        inst.extend_from_slice(&name20("TestInst"));
        inst.extend_from_slice(&0u16.to_le_bytes());
        inst.extend_from_slice(&name20("EOP"));
        inst.extend_from_slice(&2u16.to_le_bytes());

        // phdr: en preset + EOP (läses bara som namn).
        let mut phdr = Vec::new();
        phdr.extend_from_slice(&name20("TestPreset"));
        phdr.extend_from_slice(&0u16.to_le_bytes()); // preset
        phdr.extend_from_slice(&0u16.to_le_bytes()); // bank
        phdr.extend_from_slice(&0u16.to_le_bytes()); // bag
        phdr.extend_from_slice(&0u32.to_le_bytes());
        phdr.extend_from_slice(&0u32.to_le_bytes());
        phdr.extend_from_slice(&0u32.to_le_bytes());
        phdr.extend_from_slice(&name20("EOP"));
        phdr.resize(phdr.len() + 18, 0);
        // The EOP's bag index ends the preset's zone range: bag 1, so the preset owns bag 0.
        let eop_bag = phdr.len() - 18 + 4;
        phdr[eop_bag..eop_bag + 2].copy_from_slice(&1u16.to_le_bytes());

        // pbag: the preset's one zone spans generator 0..1, EOP at 1.
        let mut pbag = Vec::new();
        for gen_ndx in [0u16, 1, 1] {
            pbag.extend_from_slice(&gen_ndx.to_le_bytes());
            pbag.extend_from_slice(&0u16.to_le_bytes());
        }

        let mut pdta = Vec::new();
        pdta.extend_from_slice(&chunk(b"phdr", &phdr));
        pdta.extend_from_slice(&chunk(b"pbag", &pbag));
        pdta.extend_from_slice(&chunk(b"pmod", &[0u8; 10]));
        // pgen: operator 41 (`instrument`) → instrument 0, the link a preset picker follows.
        let mut pgen = Vec::new();
        pgen.extend_from_slice(&41u16.to_le_bytes());
        pgen.extend_from_slice(&0u16.to_le_bytes());
        pdta.extend_from_slice(&chunk(b"pgen", &pgen));
        pdta.extend_from_slice(&chunk(b"inst", &inst));
        pdta.extend_from_slice(&chunk(b"ibag", &ibag));
        pdta.extend_from_slice(&chunk(b"imod", &[0u8; 10]));
        pdta.extend_from_slice(&chunk(b"igen", &igen));
        pdta.extend_from_slice(&chunk(b"shdr", &shdr));

        let mut sdta = Vec::new();
        sdta.extend_from_slice(&chunk(b"smpl", &pcm));

        let mut body = Vec::new();
        let mut list = |kind: &[u8; 4], inner: &[u8]| {
            let mut d = Vec::new();
            d.extend_from_slice(kind);
            d.extend_from_slice(inner);
            body.extend_from_slice(&chunk(b"LIST", &d));
        };
        list(b"sdta", &sdta);
        list(b"pdta", &pdta);

        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&((4 + body.len()) as u32).to_le_bytes());
        out.extend_from_slice(b"sfbk");
        out.extend_from_slice(&body);
        out
    }

    #[test]
    fn a_soundfont_becomes_zones_with_the_right_ranges() {
        let sf = parse(&tiny_sf2()).expect("sf2 ska läsas");
        assert_eq!(sf.presets, vec!["TestPreset".to_string()]);
        assert_eq!(sf.instruments.len(), 1, "ett instrument");
        let inst = &sf.instruments[0];
        assert_eq!(inst.name, "TestInst");
        assert_eq!(inst.zones.len(), 2, "två zoner (en global zon ska hoppas över)");
        assert_eq!((inst.zones[0].key_low, inst.zones[0].key_high), (0, 59));
        assert_eq!((inst.zones[1].key_low, inst.zones[1].key_high), (60, 127));
        assert_eq!(inst.zones[0].root, 60);
        // **Loopen följer med zonen** och är omräknad till zonens eget ljud: i testfilen
        // täcker `shdr`-loopen hela samplet, alltså 0..längden.
        let (loop_l, _, _) = inst.zones[0].pcm.as_ref().unwrap();
        assert_eq!(
            inst.zones[0].loop_frames,
            Some((0, loop_l.len() as u32)),
            "SF2-loopens punkter ska in i zonen"
        );
        // Ljudet är filens ramar, inte nollor: 8 ramar per zon, värdet 0/32768 … 7/32768.
        let (l, r, rate) = inst.zones[0].pcm.as_ref().expect("zonen har ljud");
        assert_eq!(rate, &48_000);
        assert_eq!(l.len(), 8);
        assert_eq!(r.len(), 8);
        assert!((l[7] - 7.0 / 32_768.0).abs() < 1e-9);
        let (l1, _, _) = inst.zones[1].pcm.as_ref().unwrap();
        assert!((l1[0] - 100.0 / 32_768.0).abs() < 1e-9);
    }

    #[test]
    fn a_file_that_is_not_a_soundfont_says_so() {
        assert!(parse(b"not a soundfont at all").is_err());
        assert!(parse(b"RIFF\x10\0\0\0WAVEfmt ").is_err());
    }
}
