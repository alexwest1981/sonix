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

/// Vad filen bär, läsbart nog att visa för en människa.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagReport {
    /// Byte som skulle försvinna om taggarna togs bort.
    pub removable_bytes: usize,
    /// ID3v2 i början.
    pub has_id3v2: bool,
    /// ID3v1 i slutet.
    pub has_id3v1: bool,
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
