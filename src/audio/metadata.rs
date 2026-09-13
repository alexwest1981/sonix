//! Tar bort AI-/Sunohärkomst ur ljudfiler — och ingenting annat (8.5b).
//!
//! **Regeln, och varför den är smal (Alex 2026-09-13):** en stämma från Suno bär
//! mycket som *är* musiken — titeln, artisten, låttexten, omslaget. Det som ska
//! bort är det som pekar på **AI:n och deras plattform**: deras anspråk och spår
//! (`comment = "made with suno; created=…; id=…"`, `WOAS`-länken till `suno.com`,
//! och C2PA-manifestet i `GEOB`-ramen, som är plattformens signerade
//! härkomstbevis). Allt annat lämnas byte för byte.
//!
//! **Mätt på Alex' egna stämmor 2026-09-13** (Broken, Rock and Hard Place, A Box
//! of You), ur originalet i zip-arkiven: mp3-taggen bär `TIT2` (stämnamnet),
//! `TPE1` (artist), `USLT` (hela låttexten, 2 130 byte), `APIC` (omslaget,
//! 11–15 kB) och `GEOB` (C2PA, 13,7 kB) — utöver Sunos egen text. Den första
//! versionen tog **hela taggen**, alltså också texten och omslaget. Det var för
//! mycket, och därför läser den här filen **ramar** i stället för att kasta
//! behållaren.
//!
//! **Vad det här INTE är, och varför det sägs rakt ut:** det gör inte en fil
//! oigenkännlig för en AI-kontroll. Kontrollerna läser *ljudet* (SynthID är
//! inbäddat i samplen, C2PA är ett signerat manifest, och detektorer tittar på
//! spektrat) — inte taggen. Den som tror att en städad tagg betyder "ingen AI har
//! varit framme" tror fel, och den villfarelsen är farligare än skräpet.
//!
//! **Två format, en regel.** mp3 bär ID3 (i början och/eller slutet), wav bär
//! RIFF-chunkar — `LIST`/`INFO` är där tjänsterna skriver sin text, och en wav kan
//! också ha en ID3-tagg inbäddad i en egen `id3 `-chunk. Båda vägarna tar ramar
//! respektive **underchunkar**, inte behållare: en `LIST`/`INFO` med `INAM` (titel)
//! och `ICMT` (Sunos text) behåller titeln.
//!
//! **Hellre inget än en gissning.** Går taggen inte att läsa ram-säkert (annan
//! version än 2.3/2.4, osynkroniserad, eller en utökad header vi inte tolkar)
//! lämnas filen i fred — och det **sägs** i rapporten. En tystnad vore samma lögn
//! som de tysta klippen i 8.5.
//!
//! Status: stabil (8.5b) — bara härkomst tas; musik, text och omslag lämnas

/// Var en ID3v2-tagg börjar och slutar (i filens början), om det finns en.
///
/// Formatet är en 10-byte header: `ID3`, version (2 byte), flaggor (1 byte) och
/// en synchsafe storlek i fyra byte där högsta biten i varje byte är noll — alltså
/// 7 bitar per byte, inte 8. Den detaljen är hela anledningen att den här
/// funktionen finns som en egen, prövbar enhet.
pub fn id3v2_span(data: &[u8]) -> Option<(usize, usize)> {
    if data.len() < 10 || &data[0..3] != b"ID3" {
        return None;
    }
    // Synchsafe: bara de sju lägsta bitarna i varje byte räknas.
    let size = ((data[6] as usize & 0x7F) << 21)
        | ((data[7] as usize & 0x7F) << 14)
        | ((data[8] as usize & 0x7F) << 7)
        | (data[9] as usize & 0x7F);
    let end = 10 + size;
    if end > data.len() {
        // Trunkerad tagg: hellre inget än att klippa bort ljud.
        return None;
    }
    Some((0, end))
}

/// Var ID3v1-taggen börjar, om filen slutar med en. Den är alltid 128 byte och
/// börjar med `TAG` — samma tagg som gav mp3 sitt "128 byte i slutet"-problem.
pub fn id3v1_offset(data: &[u8]) -> Option<usize> {
    if data.len() < 128 {
        return None;
    }
    let start = data.len() - 128;
    if &data[start..start + 3] == b"TAG" {
        Some(start)
    } else {
        None
    }
}

/// Chunkarna i en RIFF/WAVE-fil som bär metadata: `LIST`/`INFO` och en inbäddad
/// `id3 `-chunk. Svaret är `(start, slut)` för varje chunk, padden inräknad.
///
/// **Vad som inte räknas, och varför det står här:** `fmt `, `data`, `fact`,
/// `cue `, `smpl` … är ljudet och musikens egna data. En parser som råkade ta
/// `smpl` skulle förstöra loopar, och `data` skulle ta hela ljudet. Bara chunkar
/// som är metadata till sin natur tas.
///
/// En trunkerad eller orimlig chunkstorlek stoppar läsningen i stället för att
/// gissa: hellre inget än att klippa bort ljud.
pub fn riff_metadata_spans(data: &[u8]) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return spans;
    }
    let mut pos = 12usize;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32::from_le_bytes([
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]) as usize;
        let body_start = pos + 8;
        let Some(body_end) = body_start.checked_add(size) else {
            break;
        };
        if body_end > data.len() {
            break;
        }
        // Udda chunkstorlek pad:as till jämn — padden hör till chunken och ska
        // bort med den.
        let padded_end = (body_end + (size & 1)).min(data.len());
        let is_info =
            id == b"LIST" && size >= 4 && &data[body_start..body_start + 4] == b"INFO";
        let is_id3 = id == b"id3 " || id == b"ID3 ";
        if is_info || is_id3 {
            spans.push((pos, padded_end));
        }
        pos = padded_end.max(pos + 8);
    }
    spans
}

/// Finns en `data`-chunk med innehåll kvar? Vakten före en skrivning: en wav utan
/// ljud är inte en wav, och då ska filen lämnas i fred.
pub fn riff_has_audio(data: &[u8]) -> bool {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return false;
    }
    let mut pos = 12usize;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32::from_le_bytes([
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]) as usize;
        if id == b"data" && size > 0 {
            return true;
        }
        let Some(next) = pos
            .checked_add(8)
            .and_then(|b| b.checked_add(size))
            .map(|e| e + (size & 1))
        else {
            return false;
        };
        if next <= pos || next > data.len() {
            return false;
        }
        pos = next;
    }
    false
}

/// Text som pekar på AI:n eller deras plattform — aldrig på musiken.
///
/// **En tabell, en plats.** Fler markörer läggs här och slår igenom i båda
/// formaten. `c2pa` täcker `application/c2pa`, `urn:c2pa:…` och C2PA-butiken som
/// Suno bäddar in i `GEOB`.
const PROVENANCE_MARKERS: [&str; 4] = ["made with suno", "suno studio", "suno.com", "c2pa"];

/// Bär den här bytessekvensen en härkomst-markör?
///
/// Tre avkodningar prövas: latin-1 (där varje byte är ett tecken) och UTF-16 läst
/// från båda pariteterna — den andra täcker både LE och BE, eftersom bokstäverna
/// i en UTF-16-sträng ligger med en nollbyte mellan sig och en rå sökning i
/// byteflödet hade missat varje sådan text. Mätt på Sunos filer: deras text är
/// ASCII, men en fil skriven av ett annat verktyg kan vara UTF-16, och en regel
/// som bara gäller det man råkade mäta är ingen regel.
pub fn carries_provenance(bytes: &[u8]) -> bool {
    matched_marker(bytes).is_some()
}

/// Markören som gjorde att kroppen räknades som härkomst — för rapporten.
///
/// C2PA-manifestet är binärt och har ingen läsbar text, så utan den här raden
/// stod det bara "bifogad data (13724 byte)" om det som togs. Den som ska ta bort
/// något ska se *vad* det är, även när det inte går att läsa som text.
fn matched_marker(bytes: &[u8]) -> Option<&'static str> {
    fn hits(text: &str) -> Option<&'static str> {
        let lower = text.to_lowercase();
        PROVENANCE_MARKERS.iter().find(|m| lower.contains(**m)).copied()
    }

    let latin: String = bytes.iter().map(|&b| b as char).collect();
    if let Some(m) = hits(&latin) {
        return Some(m);
    }
    // Båda pariteterna: en textram har en kodningsbyte först, så texten kan börja
    // på udda index.
    for odd in [false, true] {
        let mut text = String::new();
        let mut i = usize::from(odd);
        while i + 1 < bytes.len() {
            let unit = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
            text.push(char::from_u32(unit as u32).unwrap_or('\u{fffd}'));
            i += 2;
        }
        if let Some(m) = hits(&text) {
            return Some(m);
        }
    }
    None
}

/// Vad en ram heter — rapporten ska gå att läsa, inte avkodas.
fn frame_name(id: &[u8; 4]) -> &'static str {
    match id {
        b"TIT2" => "titel",
        b"TPE1" => "artist",
        b"TALB" => "album",
        b"TCON" => "genre",
        b"TYER" | b"TDRC" => "år",
        b"USLT" | b"SYLT" => "låttext",
        b"APIC" => "omslag",
        b"TSSE" => "kodare",
        b"WOAS" => "källänk",
        b"COMM" => "kommentar",
        b"TXXX" => "egen text",
        b"GEOB" => "bifogad data",
        _ => "ram",
    }
}

/// Den läsbara texten i en ram, om det är en textram. Omslag och manifest har
/// ingen — där räcker namnet och storleken.
///
/// **Lärdomen från första versionen:** rapporten visade chunk- och ram-id:n i
/// stället för texten (`LISTV`, `INFOICMT%`), alltså precis det man inte behöver
/// veta. Den som ska ta bort något ska se *vad* som tas bort.
fn frame_text(id: &[u8; 4], body: &[u8]) -> String {
    let is_text = id[0] == b'T'
        || id == b"COMM"
        || id == b"USLT"
        || id == b"WOAS"
        || id == b"WXXX";
    if !is_text || body.len() < 2 {
        return String::new();
    }
    let enc = body[0];
    let rest = &body[1..];
    let decoded: String = if enc == 1 || enc == 2 {
        // UTF-16. Med BOM avgör den ordningen; annars gäller LE för 1 och BE för 2.
        let (le, skip) = match rest.starts_with(&[0xFF, 0xFE]) {
            true => (true, 2),
            false => match rest.starts_with(&[0xFE, 0xFF]) {
                true => (false, 2),
                false => (enc == 1, 0),
            },
        };
        let mut text = String::new();
        let mut i = skip;
        while i + 1 < rest.len() {
            let unit = if le {
                u16::from_le_bytes([rest[i], rest[i + 1]])
            } else {
                u16::from_be_bytes([rest[i], rest[i + 1]])
            };
            text.push(char::from_u32(unit as u32).unwrap_or('\u{fffd}'));
            i += 2;
        }
        text
    } else {
        rest.iter().map(|&b| b as char).collect()
    };
    decoded
        .chars()
        .filter(|c| !c.is_control() || *c == ' ')
        .collect::<String>()
        .trim()
        .to_string()
}

/// En kort, läsbar rad om en ram — för både det som tas och det som lämnas.
fn frame_label(frame: &Frame<'_>) -> String {
    let name = frame_name(&frame.id);
    let text = frame_text(&frame.id, frame.body);
    if !text.is_empty() {
        return format!("{name}: {}", text.chars().take(60).collect::<String>());
    }
    match matched_marker(frame.body) {
        Some(m) => format!("{name} ({} byte) — bär \"{m}\"", frame.body.len()),
        None => format!("{name} ({} byte)", frame.body.len()),
    }
}

/// En ID3v2-ram, lånad ur taggen den kom ifrån.
struct Frame<'a> {
    /// Ramens id, fyra tecken (`TIT2`, `APIC` …).
    id: [u8; 4],
    /// Hela ramen, header inräknad — den skrivs tillbaka ordagrant.
    raw: &'a [u8],
    /// Bara innehållet, för den som ska läsa vad den säger.
    body: &'a [u8],
}

/// Läser ramarna i en ID3v2.3/2.4-tagg. `None` = taggen går inte att läsa
/// ram-säkert, och då lämnas filen i fred (varför står i `notes`).
///
/// Trasiga storlekar stoppar läsningen i stället för att gissa: hellre inget än att
/// skriva tillbaka en tagg man inte förstod. En nolla där ett ram-id borde stå är
/// padden, och den avslutar läsningen utan att vara ett fel.
fn id3_frames<'a>(tag: &'a [u8], notes: &mut Vec<String>) -> Option<Vec<Frame<'a>>> {
    if tag.len() < 10 || &tag[0..3] != b"ID3" {
        return None;
    }
    let major = tag[3];
    if major != 3 && major != 4 {
        notes.push(format!(
            "ID3v2.{major} — ramar läses bara i 2.3 och 2.4, filen lämnas i fred"
        ));
        return None;
    }
    let flags = tag[5];
    if flags & 0x80 != 0 {
        notes.push("taggen är osynkroniserad — filen lämnas i fred".to_string());
        return None;
    }
    let size = ((tag[6] as usize & 0x7F) << 21)
        | ((tag[7] as usize & 0x7F) << 14)
        | ((tag[8] as usize & 0x7F) << 7)
        | (tag[9] as usize & 0x7F);
    let mut start = 10usize;
    if flags & 0x40 != 0 {
        // Utökad header. 2.4 räknar sin egen storlek i de fyra bytena, 2.3 gör det inte.
        if start + 4 > tag.len() {
            return None;
        }
        let ext = if major == 4 {
            ((tag[10] as usize & 0x7F) << 21)
                | ((tag[11] as usize & 0x7F) << 14)
                | ((tag[12] as usize & 0x7F) << 7)
                | (tag[13] as usize & 0x7F)
        } else {
            u32::from_be_bytes([tag[10], tag[11], tag[12], tag[13]]) as usize + 4
        };
        if ext < 4 {
            return None;
        }
        start += ext;
    }
    let mut end = 10 + size;
    // 2.4 kan ha en footer sist i taggen; den är 10 byte och räknas in i storleken.
    if major == 4 && flags & 0x10 != 0 {
        end = end.saturating_sub(10);
    }
    if end > tag.len() || start >= end {
        return None;
    }

    let mut frames = Vec::new();
    let mut pos = start;
    while pos + 10 <= end {
        let id = &tag[pos..pos + 4];
        if id[0] == 0 {
            break; // padden börjar här
        }
        if !id.iter().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
            notes.push("ett ram-id är inte läsbart — filen lämnas i fred".to_string());
            return None;
        }
        let fsz = if major == 4 {
            ((tag[pos + 4] as usize & 0x7F) << 21)
                | ((tag[pos + 5] as usize & 0x7F) << 14)
                | ((tag[pos + 6] as usize & 0x7F) << 7)
                | (tag[pos + 7] as usize & 0x7F)
        } else {
            u32::from_be_bytes([tag[pos + 4], tag[pos + 5], tag[pos + 6], tag[pos + 7]]) as usize
        };
        let body_start = pos + 10;
        let Some(body_end) = body_start.checked_add(fsz) else {
            return None;
        };
        if fsz == 0 || body_end > end {
            notes.push("en ramstorlek pekar utanför taggen — filen lämnas i fred".to_string());
            return None;
        }
        let mut id_arr = [0u8; 4];
        id_arr.copy_from_slice(id);
        frames.push(Frame {
            id: id_arr,
            raw: &tag[pos..body_end],
            body: &tag[body_start..body_end],
        });
        pos = body_end;
    }
    Some(frames)
}

/// Bygger om taggen i början av `data`: härkomst-ramarna tas, resten står kvar.
///
/// `Some((ny tagg, slutet på den gamla))` när något togs; `None` när inget togs
/// eller taggen inte gick att läsa ram-säkert.
fn plan_id3v2(
    data: &[u8],
    removed: &mut Vec<String>,
    kept: &mut Vec<String>,
    notes: &mut Vec<String>,
) -> Option<(Vec<u8>, usize)> {
    let (_, end) = id3v2_span(data)?;
    let tag = &data[..end];
    let frames = id3_frames(tag, notes)?;
    let mut body: Vec<u8> = Vec::new();
    let mut took = 0usize;
    for frame in &frames {
        if carries_provenance(frame.body) {
            removed.push(frame_label(frame));
            took += 1;
        } else {
            kept.push(frame_label(frame));
            body.extend_from_slice(frame.raw);
        }
    }
    if took == 0 {
        return None;
    }
    if body.is_empty() {
        // Allt i taggen var härkomst: då går hela taggen, inte en tom tio-byte-header
        // kvar som "ID3\x04\x00\x00\x00\x00\x00\x00".
        return Some((Vec::new(), end));
    }
    let mut out = Vec::with_capacity(body.len() + 10);
    out.extend_from_slice(b"ID3");
    out.push(tag[3]);
    out.push(0);
    // Inga flaggor: ingen osynk, ingen utökad header och ingen footer kvar.
    out.push(0);
    let size = body.len();
    out.push(((size >> 21) & 0x7F) as u8);
    out.push(((size >> 14) & 0x7F) as u8);
    out.push(((size >> 7) & 0x7F) as u8);
    out.push((size & 0x7F) as u8);
    out.extend_from_slice(&body);
    Some((out, end))
}

/// Ska ID3v1-taggen i slutet bort? Den bär bara musik — titel, artist, album, år,
/// kommentar och genre — så den tas bara när **texten i den** är härkomst. Sunos
/// kommentar hamnar där i en del kodare, och då är hela de 128 byten deras.
fn plan_id3v1(data: &[u8], removed: &mut Vec<String>, kept: &mut Vec<String>) -> Option<usize> {
    let off = id3v1_offset(data)?;
    let tag = &data[off..];
    let hit = [3..33, 33..63, 63..93, 97..127]
        .iter()
        .any(|r| carries_provenance(&tag[r.clone()]));
    if hit {
        removed.push("ID3v1: härkomst i texten (128 byte)".to_string());
        Some(off)
    } else {
        kept.push("ID3v1: titel/artist/album/år/genre".to_string());
        None
    }
}

/// En chunk med sin storlek och sin RIFF-pad — som en riktig skrivare gör.
fn push_chunk(out: &mut Vec<u8>, id: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        out.push(0);
    }
}

/// Bygger om en `LIST`/`INFO`-payload: varje underchunk som inte bär härkomst
/// behålls. `(ny payload, antal tagna, antal kvar)`.
fn plan_info_chunk(
    payload: &[u8],
    removed: &mut Vec<String>,
    kept: &mut Vec<String>,
) -> (Vec<u8>, usize, usize) {
    let mut out = b"INFO".to_vec();
    let mut took = 0usize;
    let mut left = 0usize;
    let mut broken = false;
    let mut pos = 4usize;
    while pos + 8 <= payload.len() {
        let id = &payload[pos..pos + 4];
        let size = u32::from_le_bytes([
            payload[pos + 4],
            payload[pos + 5],
            payload[pos + 6],
            payload[pos + 7],
        ]) as usize;
        let Some(end) = pos
            .checked_add(8)
            .and_then(|e| e.checked_add(size))
            .map(|e| e + (size & 1))
        else {
            broken = true;
            break;
        };
        if end > payload.len() {
            broken = true;
            break;
        }
        let text = &payload[pos + 8..pos + 8 + size];
        let readable = printable_strings(text, 4).join(" ");
        let shown = if readable.is_empty() {
            matched_marker(text)
                .map(|m| format!("bär \"{m}\""))
                .unwrap_or_default()
        } else {
            readable
        };
        let label = format!(
            "INFO {}: {}",
            String::from_utf8_lossy(id),
            shown.chars().take(60).collect::<String>()
        );
        if carries_provenance(text) {
            removed.push(label);
            took += 1;
        } else {
            kept.push(label);
            out.extend_from_slice(&payload[pos..end]);
            left += 1;
        }
        pos = end;
    }
    if broken {
        // En storlek pekade utanför chunken: då är inget säkert, och då tas inget.
        return (payload.to_vec(), 0, 1);
    }
    if pos < payload.len() {
        // En svans kortare än ett chunkhuvud kopieras som den är — hellre det än
        // att tappa byte man inte förstod.
        out.extend_from_slice(&payload[pos..]);
    }
    (out, took, left)
}

/// Städar en RIFF/WAVE-fil: bara de `INFO`-underchunkar och den `id3 `-tagg som
/// bär härkomst tas. Titel, artist, kommentar och allt annat står kvar byte för
/// byte, och `data`-chunken rörs aldrig.
fn plan_riff(
    data: &[u8],
    removed: &mut Vec<String>,
    kept: &mut Vec<String>,
    notes: &mut Vec<String>,
) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[0..12]);
    let mut pos = 12usize;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32::from_le_bytes([
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]) as usize;
        let Some(end) = pos
            .checked_add(8)
            .and_then(|e| e.checked_add(size))
            .map(|e| e + (size & 1))
        else {
            break;
        };
        if end > data.len() {
            // Trasig chunkstorlek: hellre inget än att klippa bort ljud.
            break;
        }
        let payload = &data[pos + 8..pos + 8 + size];
        let chunk = &data[pos..end];
        if id == b"LIST" && payload.len() >= 4 && &payload[0..4] == b"INFO" {
            let (rebuilt, took, left) = plan_info_chunk(payload, removed, kept);
            if took > 0 && left == 0 {
                // Hela LIST/INFO var härkomst: chunken försvinner.
            } else if took > 0 {
                push_chunk(&mut out, b"LIST", &rebuilt);
            } else {
                out.extend_from_slice(chunk);
            }
        } else if id == b"id3 " || id == b"ID3 " {
            let mut sub_removed: Vec<String> = Vec::new();
            let mut sub_kept: Vec<String> = Vec::new();
            match plan_id3v2(payload, &mut sub_removed, &mut sub_kept, notes) {
                Some((new_tag, tag_end)) => {
                    removed.extend(sub_removed);
                    kept.extend(sub_kept);
                    let mut new_payload = new_tag;
                    if tag_end < payload.len() {
                        new_payload.extend_from_slice(&payload[tag_end..]);
                    }
                    if new_payload.is_empty() {
                        // Hela den inbäddade taggen var härkomst — ingen tom chunk kvar.
                    } else {
                        push_chunk(&mut out, b"id3 ", &new_payload);
                    }
                }
                None => out.extend_from_slice(chunk),
            }
        } else {
            out.extend_from_slice(chunk);
        }
        pos = end;
    }
    if pos < data.len() {
        out.extend_from_slice(&data[pos..]);
    }
    let len = out.len();
    out[4..8].copy_from_slice(&((len - 8) as u32).to_le_bytes());
    out
}

/// mp3-vägen: ID3v2 i början, ID3v1 i slutet, ljudet däremellan orört.
fn plan_id3_file(
    data: &[u8],
    removed: &mut Vec<String>,
    kept: &mut Vec<String>,
    notes: &mut Vec<String>,
) -> Vec<u8> {
    let mut head: Vec<u8> = Vec::new();
    let mut body_start = 0usize;
    if let Some((new_tag, end)) = plan_id3v2(data, removed, kept, notes) {
        head = new_tag;
        body_start = end;
    }
    let body_end = match plan_id3v1(data, removed, kept) {
        Some(off) if off >= body_start => off,
        _ => data.len(),
    };
    let mut out = head;
    out.extend_from_slice(&data[body_start..body_end]);
    out
}

/// Städningens plan för en fil — den nya filen, vad som tas och vad som lämnas.
///
/// **En väg, inte två.** `scan` och `strip_tags` bygger båda på den här, så
/// rapporten och skrivningen kan inte glida ifrån varandra. Det är repots egen
/// läxa: två lager som mäter samma sak driver isär.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CleanPlan {
    /// Filen som den ser ut efter städningen (identisk med originalet om inget tas).
    pub out: Vec<u8>,
    /// Härkomsten som tas, läsbar.
    pub removed: Vec<String>,
    /// Musiken som lämnas, läsbar.
    pub kept: Vec<String>,
    /// Det som inte kunde avgöras — sägs rakt ut i stället för att tigas.
    pub notes: Vec<String>,
}

impl CleanPlan {
    /// Byte som försvinner — räknat ur den nya filen, inte gissat.
    pub fn removed_bytes(&self, original_len: usize) -> usize {
        original_len.saturating_sub(self.out.len())
    }

    /// Bytes som tas bort ur filen. Noll betyder att filen är orörd.
    pub fn removes_anything(&self) -> bool {
        !self.removed.is_empty()
    }
}

/// Planerar städningen av en fil — utan att skriva något.
///
/// Vakten är densamma som 8.5 vilar på: blir det inget ljud kvar backar hela
/// planen och filen lämnas i fred. En wav utan ljud är inte en wav.
pub fn plan_clean(data: &[u8]) -> CleanPlan {
    let mut plan = CleanPlan {
        out: data.to_vec(),
        ..CleanPlan::default()
    };
    let is_riff = data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WAVE";
    let candidate = if is_riff {
        plan_riff(data, &mut plan.removed, &mut plan.kept, &mut plan.notes)
    } else {
        plan_id3_file(data, &mut plan.removed, &mut plan.kept, &mut plan.notes)
    };

    if !plan.removes_anything() {
        plan.out = data.to_vec();
        return plan;
    }
    let has_audio = if is_riff {
        riff_has_audio(&candidate)
    } else {
        candidate.len() >= 16
    };
    if !has_audio {
        plan.notes
            .push("inget ljud kvar efter städningen — filen lämnas i fred".to_string());
        plan.removed.clear();
        plan.out = data.to_vec();
        return plan;
    }
    plan.out = candidate;
    plan
}

/// Vad filen bär, läsbart nog att visa för en människa.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagReport {
    /// Byte som skulle försvinna om härkomsten togs bort (0 = inget att ta).
    pub removable_bytes: usize,
    /// ID3v2 i början.
    pub has_id3v2: bool,
    /// ID3v1 i slutet.
    pub has_id3v1: bool,
    /// RIFF-metadata (`LIST`/`INFO` eller en inbäddad `id3 `-chunk) i en wav.
    pub has_riff_metadata: bool,
    /// Härkomsten som tas, läsbar — det man tar bort ska gå att se.
    pub excerpts: Vec<String>,
    /// Musiken som lämnas (titel, artist, låttext, omslag …).
    pub kept: Vec<String>,
    /// Det som inte kunde avgöras (t.ex. en taggversion vi inte läser).
    pub notes: Vec<String>,
}

/// Läser vad en fil bär utan att ändra något.
pub fn scan(path: &str) -> Result<TagReport, String> {
    let data = std::fs::read(path).map_err(|e| format!("kan inte läsa {path}: {e}"))?;
    let plan = plan_clean(&data);
    Ok(TagReport {
        removable_bytes: plan.removed_bytes(data.len()),
        has_id3v2: id3v2_span(&data).is_some(),
        has_id3v1: id3v1_offset(&data).is_some(),
        has_riff_metadata: !riff_metadata_spans(&data).is_empty(),
        excerpts: plan.removed,
        kept: plan.kept,
        notes: plan.notes,
    })
}

/// Tar bort härkomsten ur filen. Ljudet rörs inte: allt utom det som togs skrivs
/// tillbaka byte för byte. Returnerar antalet borttagna byte.
///
/// Skrivningen sker till en temp-fil i samma katalog och flyttas på plats — samma
/// regel som resten av repot, så en avbruten körning aldrig lämnar en halv fil.
pub fn strip_tags(path: &str) -> Result<usize, String> {
    let data = std::fs::read(path).map_err(|e| format!("kan inte läsa {path}: {e}"))?;
    let plan = plan_clean(&data);
    let removed = plan.removed_bytes(data.len());
    if removed == 0 {
        return Ok(0);
    }
    let p = std::path::Path::new(path);
    let dir = p.parent().unwrap_or_else(|| std::path::Path::new("."));
    let tmp = dir.join(format!(
        ".sonix_tagtmp_{}_{}",
        std::process::id(),
        p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
    ));
    std::fs::write(&tmp, &plan.out).map_err(|e| format!("kan inte skriva temp-fil: {e}"))?;
    std::fs::rename(&tmp, p).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("kan inte flytta temp-filen på plats: {e}")
    })?;
    Ok(removed)
}

/// Läsbara strängar av minst `min` tecken — nog för att se vad en tagg säger.
fn printable_strings(bytes: &[u8], min: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for &b in bytes {
        if (0x20..0x7F).contains(&b) || b == 0x0A {
            cur.push(b as char);
        } else {
            if cur.len() >= min {
                out.push(cur.trim().to_string());
            }
            cur.clear();
        }
    }
    if cur.len() >= min {
        out.push(cur.trim().to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id3v2(bytes: usize) -> Vec<u8> {
        // Synchsafe storlek: 7 bitar per byte.
        let mut v = b"ID3\x04\x00\x00".to_vec();
        v.push(((bytes >> 21) & 0x7F) as u8);
        v.push(((bytes >> 14) & 0x7F) as u8);
        v.push(((bytes >> 7) & 0x7F) as u8);
        v.push((bytes & 0x7F) as u8);
        v
    }

    /// En ID3v2.4-ram: id, synchsafe storlek, två flaggbyte och kroppen.
    fn frame(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let n = body.len();
        let mut v = Vec::with_capacity(n + 10);
        v.extend_from_slice(id);
        v.push(((n >> 21) & 0x7F) as u8);
        v.push(((n >> 14) & 0x7F) as u8);
        v.push(((n >> 7) & 0x7F) as u8);
        v.push((n & 0x7F) as u8);
        v.extend_from_slice(&[0, 0]);
        v.extend_from_slice(body);
        v
    }

    /// En textram med latin-1-kodning — formen Suno använder.
    fn text_frame(id: &[u8; 4], text: &str) -> Vec<u8> {
        let mut body = vec![0u8];
        body.extend_from_slice(text.as_bytes());
        frame(id, &body)
    }

    /// Ljudet i testerna: en känd följd, så att "orört" går att mäta och inte bara
    /// påstå.
    fn audio(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    /// Taggen en Suno-stämma bär, med musiken OCH härkomsten: titel, artist,
    /// kodare, låttext, omslag, och sedan Sunos egen text, länken och C2PA.
    fn suno_tag_body() -> Vec<u8> {
        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(&text_frame(b"TIT2", "Broken (Vocals)"));
        body.extend_from_slice(&text_frame(b"TPE1", "alexwest"));
        body.extend_from_slice(&text_frame(b"TSSE", "Lavf60.16.100"));
        let mut lyrics = vec![0u8];
        lyrics.extend_from_slice(b"eng");
        lyrics.extend_from_slice(&[0, 0, 0]);
        lyrics.extend_from_slice(b"[Verse] My back locks up getting out of bed");
        body.extend_from_slice(&frame(b"USLT", &lyrics));
        let mut cover = vec![0u8];
        cover.extend_from_slice(b"image/jpeg\x00\x03Cover\x00");
        cover.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10]);
        cover.extend_from_slice(b"JFIF-bilddata");
        body.extend_from_slice(&frame(b"APIC", &cover));
        let mut txxx = vec![0u8];
        txxx.extend_from_slice(
            b"comment\x00made with suno; created=2026-09-11T06:34:15Z; id=072b968b",
        );
        body.extend_from_slice(&frame(b"TXXX", &txxx));
        body.extend_from_slice(&text_frame(
            b"WOAS",
            "https://suno.com/song/072b968b-28c6-4a8c-8987-aa10e0db9eed",
        ));
        let mut geob = vec![0u8];
        geob.extend_from_slice(b"\x00application/c2pa\x00manifest store\x00urn:c2pa:87e1fd67");
        geob.extend_from_slice(&[0xAA; 64]);
        body.extend_from_slice(&frame(b"GEOB", &geob));
        body
    }

    /// En minimal RIFF/WAVE-fil: chunkarna i den ordning de ges, med RIFF-padden
    /// utskriven för udda storlekar (precis som en riktig skrivare gör).
    fn riff_file(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut body = Vec::new();
        for (id, payload) in chunks {
            body.extend_from_slice(*id);
            body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            body.extend_from_slice(payload);
            if payload.len() % 2 == 1 {
                body.push(0);
            }
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(&body);
        out
    }

    /// En `LIST`/`INFO` med valfria underchunkar (`INAM`, `IART`, `ICMT`, `ISFT` …).
    fn info_with(subs: &[(&[u8; 4], &[u8])]) -> (&'static [u8; 4], Vec<u8>) {
        let mut payload = b"INFO".to_vec();
        for (id, text) in subs {
            payload.extend_from_slice(*id);
            payload.extend_from_slice(&(text.len() as u32).to_le_bytes());
            payload.extend_from_slice(text);
            if text.len() % 2 == 1 {
                payload.push(0);
            }
        }
        (b"LIST", payload)
    }

    fn info_chunk(text: &[u8]) -> (&'static [u8; 4], Vec<u8>) {
        info_with(&[(b"ICMT", text)])
    }

    fn fmt_chunk() -> (&'static [u8; 4], Vec<u8>) {
        // 16 byte PCM-format, 44100 Hz, 2 kanaler, 16 bitar.
        let mut p = Vec::new();
        p.extend_from_slice(&1u16.to_le_bytes());
        p.extend_from_slice(&2u16.to_le_bytes());
        p.extend_from_slice(&44_100u32.to_le_bytes());
        p.extend_from_slice(&176_400u32.to_le_bytes());
        p.extend_from_slice(&4u16.to_le_bytes());
        p.extend_from_slice(&16u16.to_le_bytes());
        (b"fmt ", p)
    }

    fn data_chunk(bytes: usize) -> (&'static [u8; 4], Vec<u8>) {
        (b"data", (0..bytes).map(|i| (i % 251) as u8).collect())
    }

    /// En wav med en Suno-stämma: titel och artist i `INFO`, Sunos text bredvid,
    /// och kodarens rad. Bara Sunos text ska bort.
    fn suno_wav() -> (Vec<u8>, Vec<u8>) {
        let data = data_chunk(1000);
        let info = info_with(&[
            (b"INAM", b"Broken (Vocals)"),
            (b"IART", b"alexwest"),
            (b"ICMT", b"made with suno; created=2026-09-11T06:38:01Z; id=3a860808"),
            (b"ISFT", b"Lavf60.16.100"),
        ]);
        (riff_file(&[fmt_chunk(), info, data.clone()]), data.1)
    }

    /// Läser RIFF-filens egen storlekssiffra (allt efter de första åtta bytena).
    fn riff_size(data: &[u8]) -> u32 {
        u32::from_le_bytes([data[4], data[5], data[6], data[7]])
    }

    /// Fällan i den gamla regeln, som ett test: HELA taggen togs — alltså också
    /// låttexten och omslaget. Det är precis vad Alex' invändning 2026-09-13
    /// handlade om, och det här testet är vakten mot att det kommer tillbaka.
    #[test]
    fn the_provenance_goes_and_the_music_info_stays() {
        let dir = std::env::temp_dir().join("sonix_metadata_narrow_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("stem.mp3");
        let path_str = path.to_string_lossy().to_string();

        let sound = audio(2000);
        let tag_body = suno_tag_body();
        let mut file = id3v2(tag_body.len());
        file.extend_from_slice(&tag_body);
        file.extend_from_slice(&sound);
        // ID3v1 i slutet, med Sunos kommentar i kommentarsfältet (97..127).
        let mut v1 = vec![0u8; 128];
        v1[0..3].copy_from_slice(b"TAG");
        v1[3..16].copy_from_slice(b"Broken Vocals");
        v1[97..117].copy_from_slice(b"made with suno; id=X");
        file.extend_from_slice(&v1);
        std::fs::write(&path, &file).unwrap();

        let report = scan(&path_str).expect("scanning ska lyckas");
        let removed = report.excerpts.join(" | ").to_lowercase();
        assert!(
            removed.contains("made with suno"),
            "Sunos text ska synas innan den tas: {:?}",
            report.excerpts
        );
        assert!(removed.contains("suno.com"), "länken ska tas: {:?}", report.excerpts);
        assert!(removed.contains("c2pa"), "C2PA ska tas: {:?}", report.excerpts);
        let kept = report.kept.join(" | ");
        assert!(kept.contains("titel"), "titeln ska lämnas: {:?}", report.kept);
        assert!(kept.contains("artist"), "artisten ska lämnas: {:?}", report.kept);
        assert!(kept.contains("låttext"), "låttexten ska lämnas: {:?}", report.kept);
        assert!(kept.contains("omslag"), "omslaget ska lämnas: {:?}", report.kept);
        assert!(kept.contains("kodare"), "kodarraden ska lämnas: {:?}", report.kept);
        assert!(report.notes.is_empty(), "inget oklart i en vanlig Suno-fil: {:?}", report.notes);

        let reported = report.removable_bytes;
        let actually = strip_tags(&path_str).expect("städningen ska lyckas");
        assert_eq!(reported, actually, "rapporten och skrivningen ska vara samma väg");

        let after = std::fs::read(&path).unwrap();
        assert!(after.ends_with(&sound), "ljudet ska stå kvar orört, sist i filen");
        assert!(after.windows(4).any(|w| w == b"TIT2"), "titelramen ska stå kvar");
        assert!(after.windows(4).any(|w| w == b"USLT"), "låttextramen ska stå kvar");
        assert!(after.windows(4).any(|w| w == b"APIC"), "omslagsramen ska stå kvar");
        assert!(after.windows(4).any(|w| w == b"TSSE"), "kodarraden ska stå kvar");
        let text = String::from_utf8_lossy(&after).to_lowercase();
        assert!(text.contains("broken (vocals)"), "titeln ska läsas ur filen");
        assert!(text.contains("my back locks up"), "låttexten ska läsas ur filen");
        assert!(!text.contains("made with suno"), "Sunos text ska vara borta");
        assert!(!text.contains("suno.com"), "länken ska vara borta");
        assert!(!text.contains("c2pa"), "C2PA-manifestet ska vara borta");
        assert!(!after.windows(3).any(|w| w == b"TAG"), "ID3v1 med Sunos text ska borta");
        assert_eq!(strip_tags(&path_str).unwrap(), 0, "idempotent: inget kvar att ta");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// En tagg utan härkomst lämnas i fred — byte för byte, och det står att den
    /// gör det. Den som aldrig städade något ska inte få sin fil omskriven.
    #[test]
    fn a_tag_without_provenance_is_left_alone() {
        let dir = std::env::temp_dir().join("sonix_metadata_keep_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("ren.mp3");
        let path_str = path.to_string_lossy().to_string();

        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(&text_frame(b"TIT2", "Min egen tagning"));
        body.extend_from_slice(&text_frame(b"TPE1", "Alex"));
        body.extend_from_slice(&text_frame(b"TSSE", "Lavf60.16.100"));
        let mut file = id3v2(body.len());
        file.extend_from_slice(&body);
        file.extend_from_slice(&audio(500));
        std::fs::write(&path, &file).unwrap();

        let report = scan(&path_str).expect("scanning");
        assert_eq!(
            report.removable_bytes, 0,
            "inget härkomst att ta: {:?}",
            report.excerpts
        );
        assert!(report.kept.iter().any(|s| s.contains("titel")), "{:?}", report.kept);
        assert_eq!(strip_tags(&path_str).unwrap(), 0, "filen ska lämnas i fred");
        assert_eq!(std::fs::read(&path).unwrap(), file, "orörd byte för byte");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ID3v1 bär bara musik — titel, artist, album, år, genre — så den tas bara
    /// när texten i den är härkomst.
    #[test]
    fn a_plain_v1_tag_stays_but_a_suno_one_goes() {
        let sound = audio(400);
        let mut plain = sound.clone();
        plain.extend_from_slice(b"TAG");
        plain.extend_from_slice(&vec![0u8; 125]);
        assert!(
            !plan_clean(&plain).removes_anything(),
            "en v1-tagg utan härkomst ska lämnas"
        );

        let mut suno = sound.clone();
        suno.extend_from_slice(b"TAG");
        let mut rest = vec![0u8; 125];
        // Kommentarsfältet ligger 97..127 i taggen, alltså 94..124 i resten.
        rest[94..114].copy_from_slice(b"made with suno; id=1");
        suno.extend_from_slice(&rest);
        let plan = plan_clean(&suno);
        assert!(plan.removes_anything(), "Sunos kommentar i v1 ska bort");
        assert_eq!(plan.out, sound, "bara de 128 byten försvinner");
    }

    /// Osynkroniserad tagg: ramarna går inte att flytta säkert, och då lämnas
    /// filen — men det SKA stå varför. Tystnad är samma lögn som de tysta klippen.
    #[test]
    fn an_unsynchronised_tag_says_so_and_is_left_alone() {
        let sound = audio(300);
        let mut body = vec![0u8];
        body.extend_from_slice(b"comment\x00made with suno; id=1");
        let f = frame(b"TXXX", &body);
        let mut head = id3v2(f.len());
        head[5] = 0x80;
        let mut file = head;
        file.extend_from_slice(&f);
        file.extend_from_slice(&sound);

        let plan = plan_clean(&file);
        assert!(!plan.removes_anything(), "osynk: filen ska lämnas i fred");
        assert_eq!(plan.out, file, "orörd");
        assert!(
            plan.notes.iter().any(|n| n.contains("osynkroniserad")),
            "det ska stå varför: {:?}",
            plan.notes
        );
    }

    /// En taggversion vi inte läser (2.2 eller äldre) lämnas i fred — och säger det.
    #[test]
    fn another_id3_version_is_left_alone_and_says_so() {
        let mut file = b"ID3\x02\x00\x00".to_vec();
        file.extend_from_slice(&[0, 0, 0, 40]);
        file.extend_from_slice(&vec![0u8; 40]);
        file.extend_from_slice(&audio(300));
        let plan = plan_clean(&file);
        assert!(!plan.removes_anything());
        assert!(plan.notes.iter().any(|n| n.contains("2.2")), "{:?}", plan.notes);
    }

    /// UTF-16-text: bokstäverna ligger med en nollbyte mellan sig, så en rå
    /// sökning i byteflödet hade missat varje sådan text.
    #[test]
    fn a_utf16_text_is_read_as_text() {
        let mut body = vec![1u8];
        for unit in "Made with Suno".encode_utf16() {
            body.extend_from_slice(&unit.to_le_bytes());
        }
        assert!(carries_provenance(&body), "UTF-16 ska läsas som text");
        let mut clean = vec![1u8];
        for unit in "Min egen text".encode_utf16() {
            clean.extend_from_slice(&unit.to_le_bytes());
        }
        assert!(!carries_provenance(&clean));
    }

    /// C2PA är härkomst, men ett omslag och en kodarrad är det inte.
    #[test]
    fn c2pa_is_provenance_but_an_image_and_an_encoder_are_not() {
        assert!(carries_provenance(b"\x00application/c2pa\x00manifest store"));
        assert!(carries_provenance(b"urn:c2pa:87e1fd67-0c4"));
        assert!(!carries_provenance(b"image/jpeg\x00\x03Cover"));
        assert!(!carries_provenance(b"Lavf60.16.100"));
    }

    /// Wav: `LIST`/`INFO` behålls med titel och artist kvar — bara Sunos text går.
    #[test]
    fn wav_info_keeps_the_title_and_takes_only_the_suno_text() {
        let dir = std::env::temp_dir().join("sonix_wav_narrow_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("stem.wav");
        let path_str = path.to_string_lossy().to_string();
        let (file, sound) = suno_wav();
        std::fs::write(&path, &file).unwrap();

        let report = scan(&path_str).expect("scanning");
        assert!(report.has_riff_metadata, "LIST/INFO ska hittas");
        assert!(
            report.excerpts.iter().any(|s| s.to_lowercase().contains("made with suno")),
            "Sunos text ska synas: {:?}",
            report.excerpts
        );
        assert!(
            report.kept.iter().any(|s| s.contains("INAM")),
            "titeln ska rapporteras som kvar: {:?}",
            report.kept
        );
        assert!(
            report.kept.iter().any(|s| s.contains("ISFT")),
            "kodarraden är inte härkomst: {:?}",
            report.kept
        );

        let reported = report.removable_bytes;
        let actually = strip_tags(&path_str).expect("städningen ska lyckas");
        assert_eq!(reported, actually, "rapporten och skrivningen ska vara samma väg");

        let after = std::fs::read(&path).unwrap();
        assert!(after.windows(4).any(|w| w == b"LIST"), "behållaren ska stå kvar");
        assert!(after.windows(4).any(|w| w == b"INAM"), "titeln ska stå kvar");
        assert!(!after.windows(4).any(|w| w == b"ICMT"), "Sunos text ska vara borta");
        assert!(after.windows(4).any(|w| w == b"ISFT"), "kodarraden ska stå kvar");
        assert!(
            after.windows(sound.len()).any(|w| w == sound.as_slice()),
            "data-chunken ska vara orörd"
        );
        assert_eq!(riff_size(&after) as usize, after.len() - 8, "RIFF-huvudet räknas om");
        assert_eq!(strip_tags(&path_str).unwrap(), 0, "idempotent");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Synchsafe-fällan: en tagg på 200 byte ska läsas som 200, inte som 200 med
    /// den högsta biten inräknad. Läser man 8 bitar per byte blir allt fel.
    #[test]
    fn the_synchsafe_size_is_read_seven_bits_at_a_time() {
        let mut data = id3v2(200);
        data.extend_from_slice(&vec![0u8; 200]);
        data.extend_from_slice(b"AUDIO-AUDIO-AUDIO");
        let (start, end) = id3v2_span(&data).expect("taggen ska hittas");
        assert_eq!((start, end), (0, 210), "10 byte header + 200 byte tagg");
        assert_eq!(&data[end..end + 5], b"AUDIO");
    }

    /// En trunkerad tagg ska INTE klippas bort — hellre inget än att förlora ljud.
    #[test]
    fn a_truncated_tag_is_left_alone() {
        let mut data = id3v2(5000);
        data.extend_from_slice(b"bara lite ljud");
        assert_eq!(id3v2_span(&data), None);
    }

    /// ID3v1 i slutet: 128 byte, börjar med TAG. Den ska hittas utan att ljudet rörs.
    #[test]
    fn the_trailing_v1_tag_is_found_at_the_exact_128_bytes() {
        let mut data = vec![7u8; 500];
        assert_eq!(id3v1_offset(&data), None);
        data.extend_from_slice(b"TAG");
        data.extend_from_slice(&vec![0u8; 125]);
        assert_eq!(id3v1_offset(&data), Some(500));
    }

    /// En wav vars ENDA metadata är Sunos text blir ren — och allt annat står
    /// kvar byte för byte, med RIFF-huvudet omräknat.
    #[test]
    fn riff_metadata_is_found_removed_and_the_audio_is_untouched() {
        let dir = std::env::temp_dir().join("sonix_wav_tag_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("prov.wav");
        let path_str = path.to_string_lossy().to_string();

        let text: &[u8] = b"Made with Suno; id=abc123";
        let fmt = fmt_chunk();
        let data = data_chunk(1000);
        let file = riff_file(&[fmt.clone(), info_chunk(text), data.clone()]);
        std::fs::write(&path, &file).unwrap();

        let report = scan(&path_str).expect("scanning ska lyckas");
        assert!(report.has_riff_metadata, "LIST/INFO ska hittas");
        assert!(report.removable_bytes > 0);
        let shown = report.excerpts.join(" | ").to_lowercase();
        assert!(
            shown.contains("made with suno"),
            "texten ska gå att SE innan den tas bort: {:?}",
            report.excerpts
        );
        // Och rapporten ska visa TEXTEN, inte bara id:na — den som ska ta bort
        // något ska se vad det är.
        assert!(
            report.excerpts.iter().any(|s| s.len() > 20),
            "raden ska bära texten, inte bara chunk-id:t: {:?}",
            report.excerpts
        );

        let removed = strip_tags(&path_str).expect("städningen ska lyckas");
        let after = std::fs::read(&path).unwrap();

        // Ljudet: exakt samma byte, på exakt samma plats i chunklistan.
        assert!(
            after.windows(data.1.len()).any(|w| w == data.1.as_slice()),
            "data-chunken ska vara orörd"
        );
        assert!(after.windows(4).any(|w| w == b"fmt "), "fmt ska vara kvar");
        assert!(
            !after.windows(4).any(|w| w == b"INFO"),
            "INFO ska vara borta när allt i den var härkomst"
        );
        assert_eq!(
            riff_size(&after) as usize,
            after.len() - 8,
            "RIFF-huvudet ska räknas om"
        );
        assert_eq!(
            removed,
            file.len() - after.len(),
            "rapporterad storlek ska vara det som faktiskt försvann"
        );
        assert_eq!(strip_tags(&path_str).unwrap(), 0, "idempotent");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Padden hör till sin chunk. För en `LIST`/`INFO` betyder det att en
    /// underchunk med udda storlek tar sin pad med sig — annars flyttar allt efter
    /// ett steg, och filen blir oläsbar.
    ///
    /// Det första testet här byggde en chunkyta som **ingen skrivare gör** och
    /// prövade därför ingenting; det här läser den städade filen som en RIFF-läsare
    /// gör och letar upp `data` på nytt, byte för byte.
    #[test]
    fn a_rebuilt_info_chunk_keeps_everything_after_it_aligned() {
        let data = data_chunk(64);
        let file = riff_file(&[
            fmt_chunk(),
            info_with(&[
                (b"INAM", b"Broken (Vocals)"),
                // 16 tecken + avslutande nolla = 17 byte: UDDA, alltså med pad.
                (b"ICMT", b"made with suno!!"),
            ]),
            data.clone(),
        ]);
        let spans = riff_metadata_spans(&file);
        assert_eq!(spans.len(), 1, "en metadata-chunk");

        let plan = plan_clean(&file);
        assert!(plan.removes_anything(), "Sunos text ska tas");
        let out = &plan.out;

        // Läs den städade filen chunk för chunk.
        assert_eq!(&out[0..4], b"RIFF");
        let mut pos = 12usize;
        let mut found: Option<(usize, usize)> = None;
        while pos + 8 <= out.len() {
            let id = &out[pos..pos + 4];
            let size = u32::from_le_bytes([
                out[pos + 4],
                out[pos + 5],
                out[pos + 6],
                out[pos + 7],
            ]) as usize;
            if id == b"data" {
                found = Some((pos + 8, size));
                break;
            }
            pos += 8 + size + (size & 1);
        }
        let (start, size) = found.expect("data-chunken ska gå att hitta igen efter städningen");
        assert_eq!(size, data.1.len(), "samma längd som före");
        assert_eq!(&out[start..start + size], data.1.as_slice(), "samma byte");
        assert!(out.windows(4).any(|w| w == b"INAM"), "titeln ska stå kvar");
        assert!(!out.windows(4).any(|w| w == b"ICMT"), "Sunos text ska borta");
        assert_eq!(riff_size(out) as usize, out.len() - 8, "RIFF-huvudet räknas om");
    }

    /// En mp3 är inte en RIFF-fil: wav-vägen ska inte röra den.
    #[test]
    fn a_non_riff_file_has_no_riff_spans() {
        let mut mp3 = id3v2(64);
        mp3.extend_from_slice(&vec![9u8; 400]);
        assert!(riff_metadata_spans(&mp3).is_empty());
        assert!(!riff_has_audio(&mp3));
    }

    /// En trunkerad chunkstorlek stoppar läsningen — hellre inget än att klippa
    /// bort ljud på en gissning.
    #[test]
    fn a_truncated_chunk_stops_the_reading_without_removing_anything() {
        let mut file = riff_file(&[fmt_chunk(), data_chunk(32)]);
        // Säg att sista chunken är mycket större än filen.
        let n = file.len();
        file[n - 8..n - 4].copy_from_slice(&9_999u32.to_le_bytes());
        assert!(
            riff_metadata_spans(&file).is_empty(),
            "ingen metadata ska rapporteras ur en trasig fil"
        );
    }

    /// `smpl` bär loop-punkter och `cue ` markörer — musik, inte metadata. De får
    /// aldrig tas, hur mycket de än liknar en chunk.
    #[test]
    fn loop_and_cue_data_is_never_touched() {
        let smpl = (b"smpl", vec![7u8; 36]);
        let cue = (b"cue ", vec![3u8; 12]);
        let file = riff_file(&[
            fmt_chunk(),
            smpl,
            cue.clone(),
            info_chunk(b"made with suno; id=x"),
            data_chunk(16),
        ]);
        let spans = riff_metadata_spans(&file);
        assert_eq!(spans.len(), 1, "bara LIST/INFO ska pekas ut");
        let (start, end) = spans[0];
        let removed = &file[start..end];
        assert!(
            !removed.starts_with(b"smpl") && !removed.starts_with(b"cue "),
            "en musikchunk pekades ut för borttagning"
        );
        let plan = plan_clean(&file);
        let out = &plan.out;
        assert!(
            out.windows(4).any(|w| w == b"smpl") && out.windows(4).any(|w| w == b"cue "),
            "loop- och markördata ska stå kvar efter städningen"
        );
        let _ = cue;
    }

    /// Hela vägen med appens EGEN skrivare och läsare: en wav som Sonix själv har
    /// skrivit städas från Sunos text — men **Sonix egen kodarrad lämnas**.
    /// Överstädning är samma fel som städning av fel sak.
    #[test]
    fn a_wav_written_by_sonix_is_cleaned_and_still_plays_back_the_same() {
        let dir = std::env::temp_dir().join("sonix_wav_roundtrip_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("export.wav");
        let path_str = path.to_string_lossy().to_string();

        let sr = 44_100u32;
        let samples: Vec<f32> = (0..2_000).map(|i| ((i as f32) * 0.001).sin() * 0.5).collect();
        let meta = crate::audio::exporter::ExportMeta {
            title: "Prov".to_string(),
            artist: String::new(),
            album: String::new(),
            genre: String::new(),
            year: String::new(),
            comment: "Made with Suno; id=abc123".to_string(),
        };
        crate::audio::exporter::write_wav(
            &path_str,
            &samples,
            sr,
            32,
            true,
            &meta,
            crate::audio::exporter::DitherSettings::default(),
        )
        .expect("skrivningen ska lyckas");

        let report = scan(&path_str).expect("scanning");
        assert!(
            report.has_riff_metadata,
            "Sonix egna export bär LIST/INFO: {report:?}"
        );
        assert!(
            report.excerpts.iter().any(|s| s.to_lowercase().contains("made with suno")),
            "texten ska synas: {:?}",
            report.excerpts
        );

        let (before_l, _before_r, before_sr) =
            crate::audio::load_audio_pcm(&path_str).expect("fick läsas före");
        assert_eq!(before_sr, sr);
        let removed = strip_tags(&path_str).expect("städningen ska lyckas");
        assert!(removed > 0, "något ska ha tagits bort");

        let after_report = scan(&path_str).expect("scanning efter");
        assert!(
            after_report.excerpts.is_empty(),
            "Sunos text ska vara borta: {:?}",
            after_report.excerpts
        );
        assert!(
            after_report.has_riff_metadata,
            "Sonix egen kodarrad ska stå kvar — exporten ska inte tvättas ren från sin skapare"
        );

        let (after_l, _after_r, after_sr) =
            crate::audio::load_audio_pcm(&path_str).expect("fick läsas efter");
        assert_eq!(after_sr, sr);
        assert_eq!(
            after_l.len(),
            before_l.len(),
            "lika många sampel efter som före"
        );
        assert!(
            after_l.iter().zip(before_l.iter()).all(|(a, b)| (a - b).abs() < 1e-6),
            "ljudet ska vara detsamma"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Hela vägen: en fil med härkomst i båda ändar blir ren, och ljudbyten är
    /// exakt de som låg däremellan. Ingenting annat får ändras.
    #[test]
    fn stripping_removes_both_tags_and_keeps_the_audio_bytes() {
        let dir = std::env::temp_dir().join("sonix_tag_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("prov.mp3");
        let path_str = path.to_string_lossy().to_string();

        let sound = audio(2000);
        // Ramarna är EXAKT så stora som de säger — siffrorna härleds, inte
        // handräknas. Första försöket deklarerade 30 byte och skrev 45, och då låg
        // 15 byte text kvar som "ljud". Testet hade fel, inte koden.
        let mut body = vec![0u8];
        body.extend_from_slice(b"comment\x00made with suno; created=1234");
        let f = frame(b"TXXX", &body);
        let mut file = id3v2(f.len());
        file.extend_from_slice(&f);
        file.extend_from_slice(&sound);
        file.extend_from_slice(b"TAG");
        let mut v1 = vec![0u8; 125];
        v1[94..114].copy_from_slice(b"made with suno; id=1");
        file.extend_from_slice(&v1);
        let expected_removed = 10 + f.len() + 128;
        std::fs::write(&path, &file).unwrap();

        let report = scan(&path_str).expect("scanning ska lyckas");
        assert!(report.has_id3v2 && report.has_id3v1);
        assert!(
            report.excerpts.iter().any(|s| s.to_lowercase().contains("made with suno")),
            "texten ska gå att SE innan den tas bort — annars vet man inte vad man tar bort"
        );

        let removed = strip_tags(&path_str).expect("städningen ska lyckas");
        assert_eq!(removed, expected_removed, "header + ram + v1");

        let after = std::fs::read(&path).unwrap();
        assert_eq!(after.len(), sound.len(), "bara taggarna ska försvinna");
        assert_eq!(after, sound, "ljudet ska vara byte för byte detsamma");
        assert_eq!(strip_tags(&path_str).unwrap(), 0, "idempotent: inget kvar att ta");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
