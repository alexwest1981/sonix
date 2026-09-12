//! Städar främmande metadata ur ljudfiler (8.5b).
//!
//! **Varför:** filer från generativa tjänster bär taggar som inte har med musiken
//! att göra — Suno skriver till exempel `comment = "Made with Suno; Created=<tid>;
//! id=<sträng>"` i mp3:er och motsvarande i RIFF-`INFO` i wav. Det är *din* fil,
//! och texten innehåller *din* prompt och deras anspråk. Att ta bort den är vanlig
//! metadatahygien — samma som varje seriös redigerare erbjuder.
//!
//! **Vad det här INTE är, och varför det sägs rakt ut:** det gör inte en fil
//! oigenkännlig för en AI-kontroll. Kontrollerna läser *ljudet* (SynthID är
//! inbäddat i samplen, C2PA är ett signerat manifest, och detektorer tittar på
//! spektrat) — inte taggen. Den som tror att en städad tagg betyder "ingen AI har
//! varit framme" tror fel, och den villfarelsen är farligare än skräpet.
//!
//! Det här är alltså: bort med deras text, in med ingenting (eller ditt eget).
//!
//! **Två format, samma regel.** mp3 bär ID3 (i början och/eller slutet), wav bär
//! RIFF-chunkar — `LIST`/`INFO` är där tjänsterna skriver sin text, och en wav kan
//! också ha en ID3-tagg inbäddad i en egen `id3 `-chunk. Städningen tar
//! metadata-chunkarna och lämnar allt annat byte för byte, som ID3-vägen gör.

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

/// Texterna i en `LIST`/`INFO`-chunk — en per underchunk (`ICMT`, `INAM`, `ISFT` …).
///
/// Utan det här visar rapporten chunkarnas **id:n** ("LIST", "INFO", "ICMT") i stället
/// för texten, och den som ska ta bort något ska se *vad* som tas bort. Mätt mot en
/// riktig fil: första versionen skrev ut `LISTV` och `INFOICMT%`.
fn riff_info_texts(chunk: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    if chunk.len() < 12 || &chunk[0..4] != b"LIST" || &chunk[8..12] != b"INFO" {
        return out;
    }
    let mut pos = 12usize;
    while pos + 8 <= chunk.len() {
        let size = u32::from_le_bytes([
            chunk[pos + 4],
            chunk[pos + 5],
            chunk[pos + 6],
            chunk[pos + 7],
        ]) as usize;
        let start = pos + 8;
        let Some(end) = start.checked_add(size) else {
            break;
        };
        if end > chunk.len() {
            break;
        }
        out.extend(printable_strings(&chunk[start..end], 4));
        let padded = end + (size & 1);
        if padded <= pos {
            break;
        }
        pos = padded;
    }
    out
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

/// Vad filen bär, läsbart nog att visa för en människa.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagReport {
    /// Byte som skulle försvinna om taggarna togs bort.
    pub removable_bytes: usize,
    /// ID3v2 i början.
    pub has_id3v2: bool,
    /// ID3v1 i slutet.
    pub has_id3v1: bool,
    /// RIFF-metadata (`LIST`/`INFO` eller en inbäddad `id3 `-chunk) i en wav.
    pub has_riff_metadata: bool,
    /// Utdrag ur taggtexten, för att kunna se VAD som står där.
    pub excerpts: Vec<String>,
}

/// Läser vad en fil bär utan att ändra något.
pub fn scan(path: &str) -> Result<TagReport, String> {
    let data = std::fs::read(path).map_err(|e| format!("kan inte läsa {path}: {e}"))?;
    let mut report = TagReport::default();
    let mut excerpts: Vec<String> = Vec::new();

    if let Some((start, end)) = id3v2_span(&data) {
        report.has_id3v2 = true;
        report.removable_bytes += end - start;
        // Läsbara strängar ur taggen räcker för att visa vad som står: vi ska visa
        // och ta bort, inte tolka varje frame.
        excerpts.extend(printable_strings(&data[start..end], 12));
    }
    if let Some(off) = id3v1_offset(&data) {
        report.has_id3v1 = true;
        report.removable_bytes += data.len() - off;
        excerpts.extend(printable_strings(&data[off..], 12));
    }
    // Wav: metadata ligger i chunkar inuti filen (Fas 8.5b). Texten där är ofta
    // kort ("Suno", "id=…"), så fönstret är mindre än för ID3 — filtreringen
    // nedan kastar ändå allt under fem tecken.
    let riff_spans = riff_metadata_spans(&data);
    if !riff_spans.is_empty() {
        report.has_riff_metadata = true;
        for (start, end) in &riff_spans {
            report.removable_bytes += end - start;
            let texts = riff_info_texts(&data[*start..*end]);
            if texts.is_empty() {
                // En inbäddad `id3 `-chunk har ingen INFO-struktur: läs den som en tagg.
                excerpts.extend(printable_strings(&data[*start..*end], 4));
            } else {
                excerpts.extend(texts);
            }
        }
    }
    excerpts.retain(|s| s.len() >= 5);
    excerpts.truncate(8);
    report.excerpts = excerpts;
    Ok(report)
}

/// Tar bort ID3v2 i början och ID3v1 i slutet. Ljudet rörs inte: byte för byte
/// av det som ligger däremellan skrivs tillbaka. Returnerar antalet borttagna byte.
///
/// Skrivningen sker till en temp-fil i samma katalog och flyttas på plats — samma
/// regel som resten av repot, så en avbruten körning aldrig lämnar en halv fil.
pub fn strip_tags(path: &str) -> Result<usize, String> {
    let data = std::fs::read(path).map_err(|e| format!("kan inte läsa {path}: {e}"))?;
    // En RIFF-fil skrivs om som chunkar: en ID3-tagg kan ligga inbäddad i en egen
    // `id3 `-chunk, och då är det chunk-vägen som gäller — inte en klippning i
    // filens ändar.
    let riff_spans = riff_metadata_spans(&data);
    if !riff_spans.is_empty() {
        return strip_riff_chunks(path, &data, &riff_spans);
    }
    let head = id3v2_span(&data).map(|(_, end)| end).unwrap_or(0);
    let tail = id3v1_offset(&data).unwrap_or(data.len());
    if head == 0 && tail == data.len() {
        return Ok(0);
    }
    if tail < head {
        return Err("taggen överlappar ljudet — rör inte filen".to_string());
    }
    let body = &data[head..tail];
    if body.len() < 16 {
        return Err("inget ljud kvar efter taggen — rör inte filen".to_string());
    }
    let p = std::path::Path::new(path);
    let dir = p.parent().unwrap_or_else(|| std::path::Path::new("."));
    let tmp = dir.join(format!(
        ".sonix_tagtmp_{}_{}",
        std::process::id(),
        p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
    ));
    std::fs::write(&tmp, body).map_err(|e| format!("kan inte skriva temp-fil: {e}"))?;
    std::fs::rename(&tmp, p).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("kan inte flytta temp-filen på plats: {e}")
    })?;
    Ok(data.len() - body.len())
}

/// Skriver om en RIFF/WAVE-fil utan metadata-chunkarna.
///
/// Ljudet rörs inte: varje annan chunk kopieras byte för byte, och RIFF-huvudets
/// storlek räknas om (den är allt efter de första åtta bytena). Skrivningen går
/// till en temp-fil i samma katalog och flyttas på plats, som ID3-vägen.
///
/// **Vakten:** finns ingen `data`-chunk med innehåll kvar lämnas filen i fred. En
/// wav utan ljud är inte en wav, och det är samma regel som 8.5 vilar på.
fn strip_riff_chunks(path: &str, data: &[u8], spans: &[(usize, usize)]) -> Result<usize, String> {
    let mut out: Vec<u8> = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[0..12]);
    let mut cursor = 12usize;
    let mut removed = 0usize;
    for &(start, end) in spans {
        if start > cursor {
            out.extend_from_slice(&data[cursor..start]);
        }
        removed += end.saturating_sub(start.max(cursor));
        cursor = end.max(cursor);
    }
    if cursor < data.len() {
        out.extend_from_slice(&data[cursor..]);
    }
    if removed == 0 {
        return Ok(0);
    }
    if !riff_has_audio(&out) {
        return Err("inget ljud kvar efter städningen — rör inte filen".to_string());
    }
    let riff_size = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&riff_size.to_le_bytes());

    let p = std::path::Path::new(path);
    let dir = p.parent().unwrap_or_else(|| std::path::Path::new("."));
    let tmp = dir.join(format!(
        ".sonix_tagtmp_{}_{}",
        std::process::id(),
        p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
    ));
    std::fs::write(&tmp, &out).map_err(|e| format!("kan inte skriva temp-fil: {e}"))?;
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

    fn info_chunk(text: &[u8]) -> (&'static [u8; 4], Vec<u8>) {
        let mut payload = b"INFO".to_vec();
        payload.extend_from_slice(b"ICMT");
        payload.extend_from_slice(&(text.len() as u32 + 1).to_le_bytes());
        payload.extend_from_slice(text);
        payload.push(0);
        (b"LIST", payload)
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

    /// Läser RIFF-filens egen storlekssiffra (allt efter de första åtta bytena).
    fn riff_size(data: &[u8]) -> u32 {
        u32::from_le_bytes([data[4], data[5], data[6], data[7]])
    }

    /// En wav med `LIST`/`INFO` blir ren — och allt annat står byte för byte kvar.
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
        assert!(
            report.excerpts.iter().any(|s| s.contains("Made with Suno")),
            "texten ska gå att SE innan den tas bort: {:?}",
            report.excerpts
        );
        // Och rapporten ska visa TEXTEN, inte chunkarnas id:n.
        assert!(
            !report.excerpts.iter().any(|s| s.contains("ICMT") || s.contains("LIST")),
            "id:n ska inte läsas som text: {:?}",
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
            "INFO ska vara borta"
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

    /// Udda chunkstorlek: RIFF-padden hör till chunken och ska bort med den —
    /// annars flyttar varje följande chunk ett steg och filen blir trasig.
    #[test]
    fn an_odd_sized_metadata_chunk_takes_its_pad_byte_with_it() {
        // INFO-texten är udda (31 byte inklusive avslutande nolla).
        let mut payload = b"INFO".to_vec();
        let text: &[u8] = b"Odd"; // 3 + 1 = 4... gör den udda med flit nedan
        payload.extend_from_slice(b"ICMT");
        payload.extend_from_slice(&(text.len() as u32 + 1).to_le_bytes());
        payload.extend_from_slice(text);
        payload.push(0);
        while payload.len() % 2 == 0 {
            payload.push(0);
        }
        let file = riff_file(&[fmt_chunk(), (b"LIST", payload.clone()), data_chunk(64)]);
        let spans = riff_metadata_spans(&file);
        assert_eq!(spans.len(), 1, "en metadata-chunk");
        let (start, end) = spans[0];
        assert_eq!(
            end - start,
            8 + payload.len() + 1,
            "spannet ska rymma chunken OCH padden"
        );
        // Och efter städningen ligger `data` kvar utan förskjutning.
        let stripped: Vec<u8> = file[..start].iter().chain(file[end..].iter()).copied().collect();
        assert!(stripped.windows(4).any(|w| w == b"data"));
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
        let file = riff_file(&[fmt_chunk(), smpl, cue.clone(), info_chunk(b"x"), data_chunk(16)]);
        let spans = riff_metadata_spans(&file);
        assert_eq!(spans.len(), 1, "bara LIST/INFO ska pekas ut");
        let (start, end) = spans[0];
        let removed = &file[start..end];
        assert!(
            !removed.starts_with(b"smpl") && !removed.starts_with(b"cue "),
            "en musikchunk pekades ut för borttagning"
        );
        let _ = cue;
    }

    /// Hela vägen med appens EGEN skrivare och läsare: en wav som Sonix själv har
    /// skrivit (den bär LIST/INFO) städas, och ljudet låter exakt som förut.
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
            "Sonix egna export bär LIST/INFO: {:?}",
            report
        );
        assert!(
            report.excerpts.iter().any(|s| s.contains("Made with Suno")),
            "texten ska synas: {:?}",
            report.excerpts
        );

        let (before_l, _before_r, before_sr) =
            crate::audio::load_audio_pcm(&path_str).expect("fick läsas före");
        assert_eq!(before_sr, sr);
        let removed = strip_tags(&path_str).expect("städningen ska lyckas");
        assert!(removed > 0, "något ska ha tagits bort");
        assert!(
            !scan(&path_str).unwrap().has_riff_metadata,
            "metadata ska vara borta"
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

    /// Hela vägen: en fil med tagg i båda ändar blir ren, och ljudbyten är exakt
    /// de som låg däremellan. Ingenting annat får ändras.
    #[test]
    fn stripping_removes_both_tags_and_keeps_the_audio_bytes() {
        let dir = std::env::temp_dir().join("sonix_tag_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("prov.mp3");
        let path_str = path.to_string_lossy().to_string();

        let audio: Vec<u8> = (0..2000u32).map(|i| (i % 251) as u8).collect();
        // Taggen är EXAKT så stor som texten — siffrorna härleds, inte handräknas.
        // Första försöket deklarerade 30 byte och skrev 45, och då låg 15 byte text
        // kvar som "ljud". Testet hade fel, inte koden.
        let tag_text: &[u8] = b"Made with Suno; Created=1234";
        let mut file = id3v2(tag_text.len());
        file.extend_from_slice(tag_text);
        file.extend_from_slice(&audio);
        file.extend_from_slice(b"TAG");
        file.extend_from_slice(&vec![0u8; 125]);
        let expected_removed = 10 + tag_text.len() + 128;
        std::fs::write(&path, &file).unwrap();

        let report = scan(&path_str).expect("scanning ska lyckas");
        assert!(report.has_id3v2 && report.has_id3v1);
        assert!(
            report.excerpts.iter().any(|s| s.contains("Made with Suno")),
            "texten ska gå att SE innan den tas bort — annars vet man inte vad man tar bort"
        );

        let removed = strip_tags(&path_str).expect("städningen ska lyckas");
        assert_eq!(removed, expected_removed, "header + tagg + v1");

        let after = std::fs::read(&path).unwrap();
        assert_eq!(after.len(), audio.len(), "bara taggarna ska försvinna");
        assert_eq!(after, audio, "ljudet ska vara byte för byte detsamma");
        assert_eq!(strip_tags(&path_str).unwrap(), 0, "idempotent: inget kvar att ta");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
