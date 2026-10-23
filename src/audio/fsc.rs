//! **FL Studio-score (`.fsc`)** — fraserna i FL:s egen bank.
//!
//! Formatet är FL:s, inte SMF: filen börjar med `FLhd` i stället för `MThd`, och noterna
//! ligger som fasta poster i stället för som händelser. Fältordningen är mätt ur FL:s egna
//! filer (609 stycken i `Data/Patches/Scores`) och stämmer med den dokumentation som finns:
//!
//! ```text
//! FLhd  u32 längd  u16 okänt  u16 kanaler  u16 tidsbas (96 = pulser per fjärdedel)
//! FLdt  u32 längd  u16 okänt  version\0  u32 okänt  varint(bytarlängd)  poster
//! ```
//!
//! Posten finns i två storlekar: **20 byte** (FL 3.x/4.x) och **24 byte** (äldre). Båda bär
//! tid vid 0, längd vid 8, tonhöjd vid 12 och pan/anslag/x/y i de fyra sista byten. Att
//! storleken skiljer sig är därför inget problem: resten av fälten läses ur samma positioner.
//!
//! Status: stabil — formatet är mätt, inte gissat; nya fält läggs till med ett prov.

/// En not ur en `.fsc`: tid och längd i filens tidsbas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FscNote {
    pub time: u32,
    pub length: u32,
    pub key: u8,
    pub velocity: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FscScore {
    /// Pulser per fjärdedel (FL:s `Timebase`, 96 i de filer som mätts).
    pub timebase: u16,
    pub notes: Vec<FscNote>,
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// LEB128: sju bitar per byte, högsta biten = fortsättning, **låg ordning först**.
///
/// FL skriver längden så — inte som MIDI:s variabla längd, som lägger de höga bitarna
/// först. Att läsa den som MIDI ger 12 291 i stället för 480 på samma två byte.
fn leb128(b: &[u8], mut at: usize) -> Option<(usize, usize)> {
    let mut value = 0usize;
    let mut shift = 0;
    let mut used = 0;
    loop {
        let byte = *b.get(at)?;
        at += 1;
        used += 1;
        value |= ((byte & 0x7F) as usize) << shift;
        if byte & 0x80 == 0 {
            return Some((value, used));
        }
        shift += 7;
        if shift > 28 {
            return None;
        }
    }
}

/// Läser en `.fsc`. Felmeddelandet namnger det som inte stämde, så en trasig fil inte
/// förväxlas med en tom.
pub fn parse_fsc(bytes: &[u8]) -> Result<FscScore, String> {
    if bytes.get(0..4) != Some(b"FLhd") {
        return Err("inte en FL-score (FLhd saknas)".to_string());
    }
    let timebase = u16::from_le_bytes(
        bytes
            .get(12..14)
            .ok_or("filen är för kort för rubriken")?
            .try_into()
            .map_err(|_| "trasig rubrik")?,
    )
    .max(1);

    let dt = bytes
        .windows(4)
        .position(|w| w == b"FLdt")
        .ok_or("FLdt-posten saknas")?;
    let data_len = u32_at(bytes, dt + 4).ok_or("FLdt saknar längd")? as usize;
    let data = bytes
        .get(dt + 8..dt + 8 + data_len)
        .ok_or("FLdt är kortare än sin egen längd")?;

    // 2 byte okänt, sedan versionen som nollterminerad sträng.
    let v_start = 2.min(data.len());
    let v_end = data[v_start..]
        .iter()
        .position(|&b| b == 0)
        .map(|p| v_start + p)
        .ok_or("versionssträngen är inte avslutad")?;
    let after = v_end + 1;
    let version = String::from_utf8_lossy(&data[v_start..v_end]).to_string();
    let major: u32 = version
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Efter versionen följer 4–13 byte okänt (FL 20 har fler), men alltid markören
    // `41 00 00 e0` före posterlängden. Att leta efter den i stället för att räkna byte
    // är skillnaden mellan att läsa 609 filer och 213.
    let marker = [0x41u8, 0x00, 0x00, 0xE0];
    let scan_end = (after + 24).min(data.len());
    let marker_at = data[after..scan_end]
        .windows(4)
        .position(|w| w == marker)
        .map(|p| after + p)
        .ok_or("postmarkören (41 00 00 e0) saknas")?;
    let (payload_len, used) = leb128(data, marker_at + 4).ok_or("postlängden saknas")?;
    let start = marker_at + 4 + used;
    let payload = data
        .get(start..start + payload_len)
        .ok_or("posterna är kortare än den angivna längden")?;

    // FL 3 och 4 skriver 20-byte-poster; 8, 11 och 20 skriver 24. Ordningen måste följa
    // versionen — annars läses en 24-byte-fil som 20-byte-poster och ger tider på två
    // miljarder tick. Den andra storleken prövas bara som reserv.
    let order: [usize; 2] = if major == 3 || major == 4 {
        [20, 24]
    } else {
        [24, 20]
    };
    let notes = order
        .into_iter()
        .find_map(|size| parse_records(payload, size))
        .ok_or_else(|| {
            format!(
                "posterna passar inte som 20 eller 24 byte ({} byte)",
                payload.len()
            )
        })?;

    Ok(FscScore { timebase, notes })
}

/// Till appens MIDI-noter: filens tidsbas skalas till appens `ppq` (480).
///
/// En not utan längd (0) får ett sextondelssteg — FL:s score-poster kan bära 0, och en
/// not som aldrig tar slut är värre än en som tar slut för tidigt.
///
/// Noter med anslag 0 hoppas över: i FL:s score är de **pauser** (arfraser skrivs med
/// varannan post på 0), och en paus som infogas som en tyst not tar plats i rutnätet utan
/// att låta.
pub fn to_midi_notes(score: &FscScore, ppq: u16) -> Vec<crate::audio::smf::MidiNote> {
    let scale = ppq as f32 / score.timebase.max(1) as f32;
    let default_len = ((ppq as f32 / 4.0).round() as u32).max(1);
    score
        .notes
        .iter()
        .filter(|n| n.velocity > 0)
        .map(|n| crate::audio::smf::MidiNote {
            start: (n.time as f32 * scale).round() as u32,
            length: if n.length == 0 {
                default_len
            } else {
                ((n.length as f32 * scale).round() as u32).max(1)
            },
            channel: 0,
            key: n.key.min(127),
            velocity: ((n.velocity as f32) * 127.0 / 128.0)
                .round()
                .clamp(1.0, 127.0) as u8,
        })
        .collect()
}

fn parse_records(payload: &[u8], size: usize) -> Option<Vec<FscNote>> {
    if payload.is_empty() || payload.len() % size != 0 {
        return None;
    }
    let mut notes = Vec::with_capacity(payload.len() / size);
    let mut last_time = 0;
    for rec in payload.chunks_exact(size) {
        let time = u32_at(rec, 0)?;
        let length = u32_at(rec, 8)?;
        let key = *rec.get(12)?;
        let velocity = *rec.get(size - 3)?;
        // Tonhöjden går till B10 (131) i FL; allt över det är inte en notpost. Tiden måste
        // också vara rimlig: `Scores`-mappen innehåller **skaldefinitioner** (pentatonik,
        // egyptiska skalor) som har samma behållare men andra poster — lästa som noter får
        // de tider på två miljarder tick. En riktig fras är aldrig längre än ~2 600 takter.
        if time < last_time || key > 131 || time > 1_000_000 {
            return None;
        }
        last_time = time;
        notes.push(FscNote {
            time,
            length,
            key,
            velocity,
        });
    }
    Some(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bygger en fil med samma rubrik som FL:s egna (FLhd/FLdt, version, varint-längd).
    fn build_with(version: &str, timebase: u16, records: &[Vec<u8>]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"FLhd");
        out.extend_from_slice(&6u32.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&timebase.to_le_bytes());
        out.extend_from_slice(b"FLdt");
        let mut body = Vec::new();
        body.extend_from_slice(&[0xC7, 0x06]);
        body.extend_from_slice(version.as_bytes());
        body.push(0);
        body.extend_from_slice(&[0x41, 0x00, 0x00, 0xE0]);
        // LEB128, låg ordning först — som FL skriver den.
        let mut len = records.len() * records.first().map_or(20, Vec::len);
        loop {
            let byte = (len & 0x7F) as u8;
            len >>= 7;
            if len == 0 {
                body.push(byte);
                break;
            }
            body.push(byte | 0x80);
        }
        for r in records {
            body.extend_from_slice(r);
        }
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    fn build(records: &[[u8; 20]], timebase: u16) -> Vec<u8> {
        let owned: Vec<Vec<u8>> = records.iter().map(|r| r.to_vec()).collect();
        build_with("4.5.2", timebase, &owned)
    }

    fn record(time: u32, length: u32, key: u8, velocity: u8) -> [u8; 20] {
        let mut r = [0u8; 20];
        r[0..4].copy_from_slice(&time.to_le_bytes());
        r[4..8].copy_from_slice(&0x4000u32.to_le_bytes());
        r[8..12].copy_from_slice(&length.to_le_bytes());
        r[12] = key;
        r[13] = 120; // fine pitch, mitten
        r[16] = 64; // pan, mitten
        r[17] = velocity;
        r[18] = 128;
        r[19] = 128;
        r
    }

    #[test]
    fn a_score_is_read_as_time_length_key_and_velocity() {
        let file = build(
            &[record(0, 24, 60, 100), record(24, 24, 62, 80)],
            96,
        );
        let score = parse_fsc(&file).expect("filen ska gå att läsa");
        assert_eq!(score.timebase, 96);
        assert_eq!(
            score.notes,
            vec![
                FscNote { time: 0, length: 24, key: 60, velocity: 100 },
                FscNote { time: 24, length: 24, key: 62, velocity: 80 },
            ]
        );
    }

    #[test]
    fn a_file_with_the_wrong_magic_is_rejected() {
        assert!(parse_fsc(b"MThd\x00\x00\x00\x06").is_err());
        assert!(parse_fsc(&[]).is_err());
    }

    #[test]
    fn a_truncated_payload_is_rejected_instead_of_half_read() {
        let mut file = build(&[record(0, 24, 60, 100)], 96);
        file.truncate(file.len() - 5);
        assert!(parse_fsc(&file).is_err());
    }

    #[test]
    fn score_ticks_are_scaled_to_the_apps_ppq() {
        let score = FscScore {
            timebase: 96,
            notes: vec![FscNote { time: 24, length: 24, key: 60, velocity: 128 }],
        };
        let notes = to_midi_notes(&score, 480);
        assert_eq!(notes[0].start, 120);
        assert_eq!(notes[0].length, 120);
        assert_eq!(notes[0].velocity, 127);

        // Utan längd blir noten ett sextondelssteg i stället för oändlig.
        let zero = FscScore {
            timebase: 96,
            notes: vec![FscNote { time: 0, length: 0, key: 60, velocity: 100 }],
        };
        assert_eq!(to_midi_notes(&zero, 480)[0].length, 120);
    }

    fn record24(time: u32, length: u32, key: u8, velocity: u8) -> Vec<u8> {
        let mut r = vec![0u8; 24];
        r[0..4].copy_from_slice(&time.to_le_bytes());
        r[8..12].copy_from_slice(&length.to_le_bytes());
        r[12] = key;
        r[13] = 120;
        r[20] = 64; // pan, mitten
        r[21] = velocity;
        r[22] = 128;
        r[23] = 128;
        r
    }

    /// En fil från FL 11 har **24-byte-poster**, även när längden också går jämnt ut i 20.
    /// Läses den som 20 får noterna tider på två miljarder tick — det var felet som gjorde
    /// att skaldefinitionerna såg ut som trasiga fraser.
    #[test]
    fn a_file_from_fl_11_is_read_with_24_byte_records() {
        let recs: Vec<Vec<u8>> = [60u8, 62, 64, 67, 69]
            .iter()
            .map(|key| record24(0, 384, *key, 100))
            .collect();
        let file = build_with("11.0.3", 96, &recs);
        let score = parse_fsc(&file).expect("24-byte-poster ska läsas");
        assert_eq!(score.notes.len(), 5);
        assert_eq!(
            score.notes.iter().map(|n| n.key).collect::<Vec<_>>(),
            vec![60, 62, 64, 67, 69]
        );
        assert!(score.notes.iter().all(|n| n.time == 0));
    }

    /// Anslag 0 är en **paus** i FL:s score (arfraser skrivs med varannan post på 0). Den
    /// finns kvar i scoren men infogas inte som en tyst not.
    #[test]
    fn a_note_with_zero_velocity_is_a_rest_and_is_not_inserted() {
        let file = build(&[record(0, 24, 60, 100), record(24, 24, 72, 0)], 96);
        let score = parse_fsc(&file).expect("filen ska gå att läsa");
        assert_eq!(score.notes.len(), 2, "resten finns kvar i scoren");
        let notes = to_midi_notes(&score, 480);
        assert_eq!(notes.len(), 1, "men den infogas inte");
        assert_eq!(notes[0].key, 60);
    }
}
