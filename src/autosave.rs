//! Autosave, versionsrotation och kraschåterställning (Fas 6.1).
//!
//! Tre regler styr allt i den här modulen:
//!
//! 1. **En avbruten skrivning får aldrig ersätta en hel fil.** Allt skrivs till
//!    en temp-fil i samma katalog och `rename`as på plats — `rename` är atomiskt
//!    på samma filsystem, så målet är antingen den gamla eller den nya filen,
//!    aldrig en halv.
//! 2. **Autosaven är ett skyddsnät, inte ett dokument.** Den bor i
//!    state-katalogen (`~/.local/state/sonix/autosave/`), inte i projektmappen —
//!    användaren ska inte behöva städa bland halvfärdiga kopior.
//! 3. **Att återställa får inte tysta nästa varning.** En förbrukad autosave
//!    *pensioneras* (döps om till `*.restored`) i stället för att raderas, så
//!    den försvinner ur listan men finns kvar om något gick fel.
//!
//! Modulen är medveten om filsystem men inte om projektformatet: den tar emot
//! färdiga bytes. Det gör hela logiken — rotation, namngivning, sortering och
//! "är autosaven nyare än den manuella sparningen" — testbar utan ljudmotor,
//! GUI eller klocka (tidsstämpeln skickas in).

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Antal versioner som behålls per projekt. Fler än så är städning, färre än så
/// gör att en trasig redigering kan ha skrivit över det enda användbara läget.
pub const KEEP_PER_PROJECT: usize = 5;

/// Hur ofta ett ändrat projekt autosparas, i sekunder.
pub const INTERVAL_SECS: f32 = 60.0;

/// Ändelsen som skiljer en autosave från en riktig projektfil.
pub const SUFFIX: &str = ".autosave.json";

/// Ändelsen en förbrukad autosave får (den erbjuds inte igen).
pub const RESTORED_SUFFIX: &str = ".restored";

/// Gör ett projektnamn till ett filnamnssäkert segment.
///
/// Endast `[a-z0-9_-]` behålls; allt annat blir `-`. Svenska tecken blir sin
/// ASCII-motsvarighet så att namnet går att läsa i en filhanterare, och
/// separatorer/kolon kan aldrig råka bygga en sökväg.
pub fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        let mapped = match ch {
            'å' | 'ä' | 'Å' | 'Ä' => 'a',
            'ö' | 'Ö' => 'o',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            _ => ch,
        };
        if mapped.is_ascii_alphanumeric() {
            out.push(mapped.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "projekt".to_string()
    } else {
        trimmed
    }
}

/// Filnamnet för en autosave: `<slug>.<tidsstämpel>.autosave.json`.
pub fn file_name(project: &str, stamp: u64) -> String {
    format!("{}.{}{}", slug(project), stamp, SUFFIX)
}

/// Tolkar ett autosave-filnamn tillbaka till `(projekt-slug, tidsstämpel)`.
pub fn parse_file_name(name: &str) -> Option<(String, u64)> {
    let base = name.strip_suffix(SUFFIX)?;
    let (project, stamp) = base.rsplit_once('.')?;
    let stamp: u64 = stamp.parse().ok()?;
    if project.is_empty() {
        return None;
    }
    Some((project.to_string(), stamp))
}

/// FNV-1a 64-bitars fingeravtryck över projektets bytes.
///
/// Används för att svara på "har något ändrats sedan förra sparningen?" utan att
/// jämföra hela strukturen. Ett ändrat fält ger ett annat värde; identiskt
/// innehåll ger alltid samma.
pub fn fingerprint(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Skriver `bytes` till `path` atomiskt: temp-fil i samma katalog + `rename`.
///
/// Katalogen skapas vid behov. Temp-filen städas bort om något går fel, så ett
/// avbrott lämnar aldrig skräp efter sig och aldrig en halv fil på plats.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;

    let stem = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "fil".to_string());
    let tmp = dir.join(format!(".{}.tmp", stem));

    std::fs::write(&tmp, bytes)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// En autosave på disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutosaveEntry {
    pub path: PathBuf,
    /// Projektets slug (från filnamnet). Det läsbara namnet finns i filens JSON.
    pub project: String,
    /// Unix-sekunder då autosaven skrevs.
    pub stamp: u64,
    pub bytes: u64,
}

impl AutosaveEntry {
    /// Är den här autosaven värd att erbjuda mot en manuell sparning?
    ///
    /// Ja när den manuella filen saknas (krasch före första sparningen) eller
    /// när autosaven skrevs efter den. Annars är autosaven en gammal kopia av
    /// något som redan finns sparat — då vore en dialog bara i vägen.
    pub fn worth_offering(&self, manual_modified: Option<SystemTime>) -> bool {
        match manual_modified {
            None => true,
            Some(t) => match t.duration_since(UNIX_EPOCH) {
                Ok(d) => self.stamp > d.as_secs(),
                // En tidsstämpel före epoch kan inte jämföras meningsfullt;
                // hellre visa dialogen en gång för mycket än tappa arbete.
                Err(_) => true,
            },
        }
    }
}

/// Skriver en autosave. Om tidsstämpeln redan är tagen flyttas den framåt ett
/// steg i taget, så två sparningar inom samma sekund inte skriver över varandra.
pub fn save(dir: &Path, project: &str, bytes: &[u8], stamp: u64) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let mut stamp = stamp;
    let mut path = dir.join(file_name(project, stamp));
    while path.exists() {
        stamp += 1;
        path = dir.join(file_name(project, stamp));
    }
    write_atomic(&path, bytes)?;
    Ok(path)
}

/// Alla autosaves i `dir`, nyaste först. Trasiga filnamn hoppas över.
pub fn list(dir: &Path) -> Vec<AutosaveEntry> {
    let mut out = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some((project, stamp)) = parse_file_name(&name) else {
            continue;
        };
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        out.push(AutosaveEntry {
            path: entry.path(),
            project,
            stamp,
            bytes: meta.len(),
        });
    }
    out.sort_by(|a, b| b.stamp.cmp(&a.stamp).then_with(|| a.project.cmp(&b.project)));
    out
}

/// Den nyaste autosaven per projekt, nyaste projekt först.
pub fn latest_per_project(dir: &Path) -> Vec<AutosaveEntry> {
    let mut seen = Vec::new();
    let mut out: Vec<AutosaveEntry> = Vec::new();
    for entry in list(dir) {
        if seen.contains(&entry.project) {
            continue;
        }
        seen.push(entry.project.clone());
        out.push(entry);
    }
    out
}

/// Tar bort de äldsta autosavarna så att varje projekt har högst `keep`.
/// Returnerar antalet borttagna filer.
pub fn prune(dir: &Path, keep: usize) -> io::Result<usize> {
    let mut per_project: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut removed = 0;
    // `list` är nyaste först, så räknaren går från nyast till äldst.
    for entry in list(dir) {
        let count = per_project.entry(entry.project.clone()).or_insert(0);
        *count += 1;
        if *count > keep {
            std::fs::remove_file(&entry.path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Pensionerar en autosave efter att den återställts: den döps om till
/// `*.restored`, så den erbjuds inte igen men går att gräva fram.
pub fn retire(path: &Path) -> io::Result<PathBuf> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "autosave".to_string());
    let target = path.with_file_name(format!("{}{}", name, RESTORED_SUFFIX));
    std::fs::rename(path, &target)?;
    Ok(target)
}

/// Unix-sekunder för "nu".
pub fn now_stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Mänsklig tidsangivelse för dialogen: "3 min sedan" / "2 h sedan".
///
/// Enheterna går genom i18n, så dialogens tidsangivelse följer språkvalet.
pub fn relative_age(stamp: u64, now: u64) -> String {
    let secs = now.saturating_sub(stamp);
    let (value, unit) = if secs < 60 {
        (secs, crate::i18n::t(" s sedan"))
    } else if secs < 3600 {
        (secs / 60, crate::i18n::t(" min sedan"))
    } else if secs < 86_400 {
        (secs / 3600, crate::i18n::t(" h sedan"))
    } else {
        (secs / 86_400, crate::i18n::t(" dygn sedan"))
    };
    format!("{}{}", value, unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sonix_autosave_test_{}_{}_{}",
            tag,
            std::process::id(),
            now_stamp()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tmpdir");
        dir
    }

    #[test]
    fn slug_is_filesystem_safe() {
        assert_eq!(slug("Vägen hit"), "vagen-hit");
        assert_eq!(slug("Demo / Låt: 2"), "demo-lat-2");
        assert!(!slug("../../etc/passwd").contains('/'));
        assert_eq!(slug("   "), "projekt");
        assert_eq!(slug("Låt 1"), "lat-1");
    }

    #[test]
    fn file_name_round_trips() {
        let name = file_name("Vägen hit", 1_700_000_000);
        assert_eq!(name, "vagen-hit.1700000000.autosave.json");
        assert_eq!(
            parse_file_name(&name),
            Some(("vagen-hit".to_string(), 1_700_000_000))
        );
        assert_eq!(parse_file_name("config.json"), None);
        assert_eq!(parse_file_name("vagen-hit.autosave.json"), None);
    }

    #[test]
    fn fingerprint_is_stable_and_content_sensitive() {
        assert_eq!(fingerprint(b"abc"), fingerprint(b"abc"));
        assert_ne!(fingerprint(b"abc"), fingerprint(b"abd"));
        assert_ne!(fingerprint(b"abc"), fingerprint(b"abcd"));
    }

    #[test]
    fn write_atomic_replaces_the_whole_file_and_leaves_no_temp() {
        let dir = tmpdir("atomic");
        let target = dir.join("projekt.sonix");

        write_atomic(&target, b"{\"first\":1}").expect("skriv");
        assert_eq!(std::fs::read(&target).unwrap(), b"{\"first\":1}");

        // Kortare innehåll: utan atomisk skrivning hade svansen från det gamla
        // innehållet kunnat ligga kvar.
        write_atomic(&target, b"{}").expect("skriv om");
        assert_eq!(std::fs::read(&target).unwrap(), b"{}");

        let leftovers: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp-filer kvar: {:?}", leftovers);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_never_overwrites_an_existing_stamp() {
        let dir = tmpdir("stamp");
        let a = save(&dir, "Demo", b"ett", 1000).expect("spara 1");
        let b = save(&dir, "Demo", "två".as_bytes(), 1000).expect("spara 2");
        assert_ne!(a, b, "samma sekund får inte skriva över förra versionen");
        assert_eq!(std::fs::read(&a).unwrap(), b"ett");
        assert_eq!(std::fs::read(&b).unwrap(), "två".as_bytes());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_and_latest_per_project_sort_newest_first() {
        let dir = tmpdir("list");
        save(&dir, "Alfa", b"a1", 100).unwrap();
        save(&dir, "Alfa", b"a2", 300).unwrap();
        save(&dir, "Beta", b"b1", 200).unwrap();
        std::fs::write(dir.join("skräp.txt"), b"x").unwrap();

        let all = list(&dir);
        assert_eq!(all.len(), 3, "trasiga namn ska ignoreras: {:?}", all);
        assert_eq!(all[0].stamp, 300, "nyaste först");

        let latest = latest_per_project(&dir);
        assert_eq!(latest.len(), 2, "ett kort per projekt");
        assert_eq!(latest[0].project, "alfa");
        assert_eq!(latest[0].stamp, 300);
        assert_eq!(latest[1].project, "beta");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_keeps_only_the_newest_versions_per_project() {
        let dir = tmpdir("prune");
        for i in 0..8u64 {
            save(&dir, "Alfa", format!("v{i}").as_bytes(), 1000 + i).unwrap();
        }
        save(&dir, "Beta", b"b", 5000).unwrap();

        let removed = prune(&dir, KEEP_PER_PROJECT).expect("prune");
        assert_eq!(removed, 8 - KEEP_PER_PROJECT);

        let alfa = list(&dir)
            .into_iter()
            .filter(|e| e.project == "alfa")
            .collect::<Vec<_>>();
        assert_eq!(alfa.len(), KEEP_PER_PROJECT);
        // De behållna är de nyaste — det äldsta läget är det som offras.
        assert_eq!(alfa[0].stamp, 1007);
        assert_eq!(alfa.last().unwrap().stamp, 1003);
        assert!(
            list(&dir).iter().any(|e| e.project == "beta"),
            "ett annat projekt ska inte påverkas av rotationen"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn worth_offering_compares_against_the_manual_save() {
        let entry = AutosaveEntry {
            path: PathBuf::from("/tmp/x"),
            project: "demo".to_string(),
            stamp: 2_000,
            bytes: 10,
        };
        // Ingen manuell fil: krasch före första sparningen → erbjud.
        assert!(entry.worth_offering(None));
        // Manuell fil skriven före autosaven → erbjud.
        assert!(entry.worth_offering(Some(UNIX_EPOCH + std::time::Duration::from_secs(1_500))));
        // Manuell fil skriven efter autosaven → inget att återställa.
        assert!(!entry.worth_offering(Some(UNIX_EPOCH + std::time::Duration::from_secs(2_500))));
    }

    #[test]
    fn retiring_a_consumed_autosave_hides_it_but_keeps_the_bytes() {
        let dir = tmpdir("retire");
        let path = save(&dir, "Demo", b"arbete", 900).unwrap();
        let retired = retire(&path).expect("pensionera");

        assert!(retired.to_string_lossy().ends_with(RESTORED_SUFFIX));
        assert!(!path.exists());
        assert_eq!(std::fs::read(&retired).unwrap(), b"arbete");
        assert!(
            list(&dir).is_empty(),
            "en pensionerad autosave ska inte erbjudas igen"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn age_is_rendered_in_readable_units() {
        // Enheterna går genom i18n, så testet kontrollerar siffran och var
        // gränserna går — inte den exakta ordalydelsen (som beror på språk).
        assert!(relative_age(1_000, 1_030).starts_with("30 "), "sekunder");
        assert!(relative_age(1_000, 1_600).starts_with("10 "), "minuter");
        assert!(relative_age(1_000, 1_000 + 7_200).starts_with("2 "), "timmar");
        assert!(
            relative_age(1_000, 1_000 + 172_800).starts_with("2 "),
            "dygn"
        );
        // Gränserna: 59 s räknas i sekunder, 60 s i minuter.
        assert_eq!(relative_age(0, 59).split(' ').next(), Some("59"));
        assert_eq!(relative_age(0, 60).split(' ').next(), Some("1"));
    }
}
