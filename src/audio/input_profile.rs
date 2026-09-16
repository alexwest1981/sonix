//! **Instrumentingångar som känns igen** (Sprint 1, punkt 1).
//!
//! Alex: *"Se till att bygga in stöd för t.ex. Rocksmiths kabel och andra såna sladdar, så det är
//! plugg and enjoy för användaren."*
//!
//! Problemet modulen löser är inte att läsa ljudet — `cpal` ser varje USB-ljudklass-enhet utan
//! drivrutin. Problemet är att **veta vad som blev inkopplat** och ställa in det därefter. I dag
//! gissar gränssnittet med `namn.contains("samson") || namn.contains("usb") || namn.contains("mic")`,
//! vilket ger en gitarrkabel etiketten "Rekommenderad mikrofon" och mikrofonnivåer på en
//! instrumentingång.
//!
//! Här bor kunskapen som en **tabell**, inte som gissningar i gränssnittet:
//!
//! - `12ba:00ff` — *Rocksmith Guitar Adapter* (Rocksmiths Real Tone Cable) — instrument, mono, 48 kHz.
//!   Verifierad i den lokala USB-databasen `/usr/share/hwdata/usb.ids` (2026-09-16).
//! - `0e41:4750` — Line 6 GuitarPort, `1ed8:0014` — Fender Mustang I, m.fl. Se `PROFILES`.
//! - Kort som sitter i datorn (`HDA Intel`, `ALC1220`) klassas som line — de är inte instrument.
//!
//! **Två saker modulen med flit inte gör:** den gissar aldrig ett USB-id (saknas sysfs-filerna blir
//! svaret `None`), och den säger hellre ifrån än monitorerar i fel tempo — en ingång som är låst till
//! 48 kHz mot en motor i 44,1 kHz spelas i fel hastighet, och det ska stå, inte höras.
//!
//! Status: byggs (Sprint 1, punkt 1) — tabell, klassning och sysfs-läsning klara och prövade.
//! Kvar: att kvittera på riktig hårdvara (Alex' kabel) och att låta valet minnas mellan starter.
//! Rör inte: `classify` är den enda vägen från ett enhetsnamn till en profil — lägg nya
//! enheter i `PROFILES`, aldrig som en ny `if` hos anroparen.

use std::path::{Path, PathBuf};

/// Vad som sitter i andra ändan av sladden.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind {
    /// Gitarr, bas, eller en synt som går in på instrumentnivå. Behöver gain och gate som en
    /// mikrofon inte vill ha, och vill höra sig själv.
    Instrument,
    /// Line-nivå: en extern synt, en mixer, ett ljudkort i datorn.
    Line,
    /// Mikrofon — den nivå appen redan är byggd kring.
    Microphone,
    /// Okänd. **Ingen gissning:** appen beter sig som förut.
    Unknown,
}

impl InputKind {
    /// Etiketten gränssnittet visar.
    pub fn badge(self) -> &'static str {
        match self {
            InputKind::Instrument => "🎸 instrument",
            InputKind::Line => "🎚 line",
            InputKind::Microphone => "🎤 mikrofon",
            InputKind::Unknown => "❔ okänd",
        }
    }
}

/// En känd enhet. `usb` är (vendor, product) som de står i USB-databasen; `names` är
/// gemener som matchas mot enhetsnamnet när USB-id:t inte går att läsa.
pub struct InputProfile {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: InputKind,
    pub usb: &'static [(u16, u16)],
    pub names: &'static [&'static str],
    /// Enhetens låsta samplingsfrekvens, om den har en. `None` = följer motorn.
    pub fixed_rate: Option<u32>,
    /// Vad enheten är, i klartext — det som står i gränssnittet.
    pub note: &'static str,
}

/// Kända enheter, i den ordning de prövas. **Lägg nya här** — inte i gränssnittet.
pub const PROFILES: &[InputProfile] = &[
    InputProfile {
        id: "rocksmith-cable",
        label: "Rocksmith-kabel",
        kind: InputKind::Instrument,
        usb: &[(0x12ba, 0x00ff)],
        names: &["rocksmith", "real tone cable"],
        fixed_rate: Some(48_000),
        note: "Rocksmiths Real Tone Cable — mono instrumentingång, låst till 48 kHz",
    },
    InputProfile {
        id: "line6-guitarport",
        label: "Line 6 GuitarPort",
        kind: InputKind::Instrument,
        usb: &[(0x0e41, 0x4750)],
        names: &["guitarport"],
        fixed_rate: Some(48_000),
        note: "Line 6 GuitarPort — instrumentingång",
    },
    InputProfile {
        id: "fender-mustang",
        label: "Fender Mustang (USB-ljud)",
        kind: InputKind::Instrument,
        usb: &[(0x1ed8, 0x0014)],
        names: &["mustang"],
        fixed_rate: None,
        note: "Fender Mustang-förstärkare som ljudkort — instrumentingång",
    },
    InputProfile {
        id: "pcm290x",
        label: "PCM29xx-ljudkort",
        kind: InputKind::Line,
        usb: &[
            (0x08bb, 0x2900),
            (0x08bb, 0x2902),
            (0x08bb, 0x29b2),
            (0x08bb, 0x29c2),
        ],
        names: &["guitar link", "ucg102", "uca202"],
        fixed_rate: None,
        note: "TI PCM29xx — sitter i billiga USB-ljudkort och gitarrlänkar (line-nivå)",
    },
    InputProfile {
        id: "cmedia-adapter",
        label: "USB-ljudadapter",
        kind: InputKind::Line,
        usb: &[(0x0d8c, 0x000c), (0x0d8c, 0x000e), (0x0d8c, 0x0014)],
        names: &["usb audio adapter"],
        fixed_rate: None,
        note: "C-Media-adapter — samma krets sitter i billiga gitarrkablar, men nivån är line",
    },
    InputProfile {
        id: "samson-q2u",
        label: "Samson Q2U",
        kind: InputKind::Microphone,
        usb: &[(0x17a0, 0x0301)],
        names: &["q2u", "samson"],
        fixed_rate: None,
        note: "USB-mikrofon — XLR/USB, mikrofonnivå",
    },
    InputProfile {
        id: "xht-digital-mic",
        label: "XHT digitalmikrofon",
        kind: InputKind::Microphone,
        usb: &[(0x1bcd, 0xcb05)],
        names: &["wcam", "digital mic"],
        fixed_rate: None,
        note: "USB-mikrofon",
    },
];

/// Enheten som sitter i datorn (PCI/kortplats) — line, aldrig instrument.
const BUILTIN_NAMES: &[&str] = &[
    "hda intel",
    "alc1220",
    "alc897",
    "realtek",
    "pch",
    "built-in",
    "intern",
];

/// En ingång som appen kan visa och ställa in.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedInput {
    /// Enhetens namn så som `cpal` listar det (index i den listan om `device_index` är satt).
    pub device_name: String,
    pub device_index: Option<usize>,
    pub usb: Option<(u16, u16)>,
    pub profile_id: &'static str,
    pub label: &'static str,
    pub kind: InputKind,
    pub note: &'static str,
    pub fixed_rate: Option<u32>,
}

impl ResolvedInput {
    /// Är det här en ingång appen bör föreslå för ett instrument?
    pub fn is_instrument(&self) -> bool {
        self.kind == InputKind::Instrument
    }
}

/// Kortet som sitter i datorn eller en okänd USB-enhet.
fn fallback_profile() -> &'static InputProfile {
    &InputProfile {
        id: "unknown",
        label: "Okänd ingång",
        kind: InputKind::Unknown,
        usb: &[],
        names: &[],
        fixed_rate: None,
        note: "Ingen känd enhet — appen behåller standardinställningarna",
    }
}

fn builtin_profile() -> &'static InputProfile {
    &InputProfile {
        id: "builtin",
        label: "Inbyggt ljudkort",
        kind: InputKind::Line,
        usb: &[],
        names: BUILTIN_NAMES,
        fixed_rate: None,
        note: "Ljudkortet på moderkortet — line/mikrofon, ingen instrumentingång",
    }
}

/// **Klassningen.** USB-id går före namn: ett namn kan ändras av en firmware-uppdatering,
/// ett id kan inte. Hittas inget blir svaret `unknown` — och då rör appen ingenting.
pub fn classify(name: &str, usb: Option<(u16, u16)>) -> &'static InputProfile {
    let lowered = name.to_lowercase();
    if let Some((vid, pid)) = usb {
        for profile in PROFILES {
            if profile.usb.contains(&(vid, pid)) {
                return profile;
            }
        }
    }
    for profile in PROFILES {
        for pattern in profile.names {
            if !pattern.is_empty() && lowered.contains(pattern) {
                return profile;
            }
        }
    }
    for pattern in BUILTIN_NAMES {
        if lowered.contains(pattern) {
            return builtin_profile();
        }
    }
    fallback_profile()
}

/// Ingångens standardvärden. Instrument och mikrofon vill inte ha samma sak:
/// en gitarr behöver mer gain, en hårdare gate (strängljud och brum) och **monitor på** —
/// utan den hör man sig inte, och då är kabeln ingen "plugg and enjoy".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputDefaults {
    pub gain: f32,
    pub gate: f32,
    pub low_cut: bool,
    pub monitor: bool,
    pub autotune: bool,
}

/// Mikrofonens värden är appens nuvarande standard (gain 1,5 · gate 0,012 · monitor av).
pub const MIC_DEFAULTS: InputDefaults = InputDefaults {
    gain: 1.5,
    gate: 0.012,
    low_cut: true,
    monitor: false,
    autotune: false,
};

/// Instrumentets värden: samma gain-golv som mikrofonen (en gitarr är svagare, men
/// ingångsratten finns kvar för finlir), hårdare gate mot strängljud och brum, lågcut på
/// (muller under 80 Hz är inte musik på en gitarr) och **monitor på**.
pub const INSTRUMENT_DEFAULTS: InputDefaults = InputDefaults {
    gain: 1.8,
    gate: 0.020,
    low_cut: true,
    monitor: true,
    autotune: false,
};

pub fn defaults_for(kind: InputKind) -> InputDefaults {
    match kind {
        InputKind::Instrument => INSTRUMENT_DEFAULTS,
        _ => MIC_DEFAULTS,
    }
}

/// Vad appen ska göra med en ingång, som **ren data** (gränssnittet formulerar texten).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputPlan {
    /// Välj den här enheten.
    pub select: bool,
    /// Ställ motorn på den här frekvensen först (enheten är låst till den).
    pub set_rate: Option<u32>,
    pub defaults: InputDefaults,
    /// Monitorering får slås på. Falskt när ingång och motor går i olika tempo —
    /// då vore monitoreringen fel i tid, och det felet ska inte höras.
    pub monitor_allowed: bool,
}

/// Planen för en ingång, givet motorns nuvarande frekvens.
///
/// Är enheten låst till en frekvens motorn inte står på, säger planen två saker: byt frekvens,
/// och **låt bli att monitorera** om bytet inte går igenom. Sista ordet är motorns, eftersom det
/// är den som vet vad enheten klarade — därför returneras `set_rate` i stället för att utföras här.
pub fn plan_for(input: &ResolvedInput, engine_rate: u32) -> InputPlan {
    let defaults = defaults_for(input.kind);
    let mut plan = InputPlan {
        select: input.device_index.is_some() || !input.device_name.is_empty(),
        set_rate: None,
        defaults,
        monitor_allowed: true,
    };
    if let Some(fixed) = input.fixed_rate {
        if fixed != engine_rate {
            plan.set_rate = Some(fixed);
            // Om bytet inte går igenom står motorn kvar på en annan frekvens. Gränssnittet
            // stänger då av monitorn och säger varför; det beslutet tas efter `reconfigure`.
            plan.monitor_allowed = false;
        }
    }
    plan
}

/// Går ingången och motorn i samma tempo? (Sprint 1, punkt 6.)
///
/// Monitorns ring dräneras **en sample per ut-sample** (`synth/process.rs`), så vägen antar att
/// ingången och utgången har samma frekvens. Är de olika hörs ingången i fel hastighet — och det
/// felet ska stå i klartext, inte höras: samma regel som 8.5 (en väg som ger ljud får inte hitta
/// på ljudet). Resampling i monitorvägen är nästa steg, se `SPRINT.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateVerdict {
    Agree,
    Mismatch { engine: u32, input: u32 },
}

pub fn rate_verdict(engine_rate: u32, input_rate: u32) -> RateVerdict {
    if engine_rate == input_rate {
        RateVerdict::Agree
    } else {
        RateVerdict::Mismatch {
            engine: engine_rate,
            input: input_rate,
        }
    }
}

/// Ett ljudkort som kärnan ser det, med det som går att läsa ur sysfs.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundCard {
    /// `card0`, `card3`, …
    pub name: String,
    /// Kortets id ur sysfs (`PCH`, `Microphone`, …).
    pub id: String,
    pub usb: Option<(u16, u16)>,
    /// USB-produktens namn, om enheten är USB.
    pub product: String,
    pub manufacturer: String,
}

/// Läser ett `cardN`-träd. Saknade filer ger tomma fält — **aldrig en gissning**.
pub fn read_card(dir: &Path) -> Option<SoundCard> {
    let name = dir.file_name()?.to_string_lossy().to_string();
    if !name.starts_with("card") {
        return None;
    }
    let read = |p: PathBuf| {
        std::fs::read_to_string(p)
            .ok()
            .map(|s| s.trim().to_string())
    };
    let id = read(dir.join("id")).unwrap_or_default();
    // USB-enheter hänger under device/../ (själva USB-gränssnittet), PCI-kort saknar idVendor.
    let usb_root = dir.join("device").join("..");
    let vid = read(usb_root.join("idVendor"))
        .and_then(|s| u16::from_str_radix(s.trim_start_matches("0x"), 16).ok());
    let pid = read(usb_root.join("idProduct"))
        .and_then(|s| u16::from_str_radix(s.trim_start_matches("0x"), 16).ok());
    let usb = match (vid, pid) {
        (Some(v), Some(p)) => Some((v, p)),
        _ => None,
    };
    Some(SoundCard {
        name,
        id,
        usb,
        product: read(usb_root.join("product")).unwrap_or_default(),
        manufacturer: read(usb_root.join("manufacturer")).unwrap_or_default(),
    })
}

/// Alla kort under en rot ( `/sys/class/sound` i drift, en fixtur i proven).
pub fn read_cards(root: &Path) -> Vec<SoundCard> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut cards: Vec<SoundCard> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| read_card(&e.path()))
        .collect();
    cards.sort_by(|a, b| a.name.cmp(&b.name));
    cards
}

/// Korten som kärnan faktiskt har just nu.
pub fn system_cards() -> Vec<SoundCard> {
    read_cards(Path::new("/sys/class/sound"))
}

/// Kopplar en lista enhetsnamn (som `cpal` ger dem) till korten och deras profiler.
///
/// Matchningen är med flit enkel och sträng: kortets produktnamn, tillverkare eller id måste
/// finnas i enhetsnamnet (gemener). Blir det ingen träff är profilen `unknown` — att gissa
/// "nära nog" vore att ställa in fel nivåer på fel ingång.
pub fn resolve_inputs(device_names: &[String], cards: &[SoundCard]) -> Vec<ResolvedInput> {
    device_names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let lowered = name.to_lowercase();
            let mut matched: Option<&SoundCard> = None;
            for card in cards {
                let mut keys = vec![card.id.to_lowercase(), card.product.to_lowercase()];
                if !card.manufacturer.is_empty() {
                    keys.push(card.manufacturer.to_lowercase());
                }
                if keys
                    .iter()
                    .any(|k| k.len() >= 3 && lowered.contains(k.as_str()))
                {
                    matched = Some(card);
                    break;
                }
            }
            let usb = matched.and_then(|c| c.usb);
            let profile = classify(name, usb);
            ResolvedInput {
                device_name: name.clone(),
                device_index: Some(index),
                usb,
                profile_id: profile.id,
                label: profile.label,
                kind: profile.kind,
                note: profile.note,
                fixed_rate: profile.fixed_rate,
            }
        })
        .collect()
}

/// Första ingången som är ett instrument, om någon finns.
pub fn first_instrument(inputs: &[ResolvedInput]) -> Option<&ResolvedInput> {
    inputs.iter().find(|i| i.is_instrument())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("sonix-input-profile-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// **Rocksmith-kabeln känns igen på sitt USB-id** — id:t är verifierat i
    /// `/usr/share/hwdata/usb.ids` (`12ba:00ff`), inte gissat ur minnet.
    #[test]
    fn the_rocksmith_cable_is_an_instrument() {
        let p = classify("USB Audio Device", Some((0x12ba, 0x00ff)));
        assert_eq!(p.id, "rocksmith-cable");
        assert_eq!(p.kind, InputKind::Instrument);
        assert_eq!(p.fixed_rate, Some(48_000));
    }

    /// Namnet räcker när USB-id:t inte går att läsa (t.ex. en annan plattform).
    #[test]
    fn the_name_is_enough_when_the_id_is_missing() {
        let p = classify("Rocksmith Guitar Adapter: USB Audio (hw:4,0)", None);
        assert_eq!(p.id, "rocksmith-cable");
        assert_eq!(p.kind, InputKind::Instrument);
    }

    /// **Id:t går före namnet.** En kabel som bytt namn ska fortfarande bli rätt.
    #[test]
    fn the_usb_id_beats_a_misleading_name() {
        let p = classify("USB Audio", Some((0x0e41, 0x4750)));
        assert_eq!(p.id, "line6-guitarport");
    }

    /// Mikrofonen på den här maskinen (verifierad i sysfs 2026-09-16).
    #[test]
    fn the_samson_microphone_stays_a_microphone() {
        let p = classify(
            "Samson Technologies Corp. Q2U handheld microphone with XLR",
            Some((0x17a0, 0x0301)),
        );
        assert_eq!(p.id, "samson-q2u");
        assert_eq!(p.kind, InputKind::Microphone);
    }

    /// Ljudkortet i datorn är line — inte ett instrument, hur mycket "ljud" det än heter.
    #[test]
    fn the_builtin_card_is_line_level() {
        let p = classify("HDA Intel PCH: ALC1220 Analog (hw:0,0)", None);
        assert_eq!(p.id, "builtin");
        assert_eq!(p.kind, InputKind::Line);
    }

    /// **Okänt förblir okänt.** Ingen gissning, ingen påhittad profil.
    #[test]
    fn an_unknown_device_is_not_guessed() {
        let p = classify("Något helt annat: Audio (hw:9,0)", None);
        assert_eq!(p.id, "unknown");
        assert_eq!(p.kind, InputKind::Unknown);
    }

    /// Instrument och mikrofon får inte samma standard: den ena hörs inte utan monitor.
    #[test]
    fn instruments_and_microphones_get_different_defaults() {
        let inst = defaults_for(InputKind::Instrument);
        let mic = defaults_for(InputKind::Microphone);
        assert!(
            inst.monitor,
            "ett instrument måste höras när det kopplas in"
        );
        assert!(
            !mic.monitor,
            "en mikrofon ska inte öppna för rundgång av sig själv"
        );
        assert!(inst.gate > mic.gate, "instrumentet behöver hårdare gate");
        assert_eq!(defaults_for(InputKind::Line).gate, mic.gate);
    }

    /// En kabel låst till 48 kHz mot en motor i 44,1 kHz: byt frekvens, och **stäng av monitorn
    /// tills bytet gått igenom** — annars hörs fel tempo.
    #[test]
    fn a_fixed_rate_cable_asks_for_the_rate_and_holds_the_monitor() {
        let input = ResolvedInput {
            device_name: "Rocksmith".into(),
            device_index: Some(2),
            usb: Some((0x12ba, 0x00ff)),
            profile_id: "rocksmith-cable",
            label: "Rocksmith-kabel",
            kind: InputKind::Instrument,
            note: "",
            fixed_rate: Some(48_000),
        };
        let plan = plan_for(&input, 44_100);
        assert_eq!(plan.set_rate, Some(48_000));
        assert!(!plan.monitor_allowed, "monitorn får inte gå i fel tempo");
        assert!(plan.select);
        assert!(
            plan.defaults.monitor,
            "instrumentets standard vill ha monitor"
        );

        let already_right = plan_for(&input, 48_000);
        assert_eq!(already_right.set_rate, None);
        assert!(already_right.monitor_allowed);
    }

    /// Sysfs-läsningen: USB-kort ger id, PCI-kort ger `None` — och en halv fil ger `None`.
    #[test]
    fn the_card_reader_takes_the_id_from_sysfs_or_says_nothing() {
        let root = tmpdir("cards");

        // USB-kort, som Samson-mikrofonen ser ut i sysfs.
        let usb = root.join("card3");
        fs::create_dir_all(usb.join("device").join("..")).unwrap();
        fs::write(usb.join("id"), "Microphone\n").unwrap();
        let usb_attrs = usb.join("device").join("..");
        fs::write(usb_attrs.join("idVendor"), "17a0\n").unwrap();
        fs::write(usb_attrs.join("idProduct"), "0301\n").unwrap();
        fs::write(
            usb_attrs.join("product"),
            "Q2U handheld microphone with XLR\n",
        )
        .unwrap();
        fs::write(
            usb_attrs.join("manufacturer"),
            "Samson Technologies Corp.\n",
        )
        .unwrap();

        // PCI-kort utan USB-attribut.
        let pci = root.join("card0");
        fs::create_dir_all(pci.join("device")).unwrap();
        fs::write(pci.join("id"), "PCH\n").unwrap();

        // Ett kort med bara idVendor (halv information) → inget id.
        let half = root.join("card7");
        fs::create_dir_all(half.join("device").join("..")).unwrap();
        fs::write(half.join("id"), "Half\n").unwrap();
        fs::write(half.join("device").join("..").join("idVendor"), "1234\n").unwrap();

        let cards = read_cards(&root);
        assert_eq!(cards.len(), 3, "tre kort ska läsas: {cards:?}");
        let mic = cards.iter().find(|c| c.name == "card3").unwrap();
        assert_eq!(mic.usb, Some((0x17a0, 0x0301)));
        assert_eq!(mic.product, "Q2U handheld microphone with XLR");
        let pci_card = cards.iter().find(|c| c.name == "card0").unwrap();
        assert_eq!(pci_card.usb, None);
        assert_eq!(pci_card.id, "PCH");
        let half_card = cards.iter().find(|c| c.name == "card7").unwrap();
        assert_eq!(half_card.usb, None, "halv information är ingen information");

        let _ = fs::remove_dir_all(&root);
    }

    /// Enhetsnamnen från cpal kopplas till rätt kort och därmed rätt profil.
    #[test]
    fn device_names_find_their_card_and_profile() {
        let cards = vec![
            SoundCard {
                name: "card0".into(),
                id: "PCH".into(),
                usb: None,
                product: String::new(),
                manufacturer: String::new(),
            },
            SoundCard {
                name: "card3".into(),
                id: "Microphone".into(),
                usb: Some((0x17a0, 0x0301)),
                product: "Q2U handheld microphone with XLR".into(),
                manufacturer: "Samson Technologies Corp.".into(),
            },
            SoundCard {
                name: "card4".into(),
                id: "USB".into(),
                usb: Some((0x12ba, 0x00ff)),
                product: "Rocksmith Guitar Adapter".into(),
                manufacturer: "Licensed by Sony Computer Entertainment America".into(),
            },
        ];
        let names = vec![
            "HDA Intel PCH: ALC1220 Analog (hw:0,0)".to_string(),
            "Samson Technologies Corp. Q2U handheld microphone with XLR: USB Audio (hw:3,0)"
                .to_string(),
            "Rocksmith Guitar Adapter: USB Audio (hw:4,0)".to_string(),
        ];
        let resolved = resolve_inputs(&names, &cards);
        assert_eq!(resolved.len(), 3);
        assert_eq!(resolved[0].kind, InputKind::Line);
        assert_eq!(resolved[1].kind, InputKind::Microphone);
        assert_eq!(resolved[2].usb, Some((0x12ba, 0x00ff)));
        assert!(resolved[2].is_instrument());
        assert_eq!(
            first_instrument(&resolved).unwrap().label,
            "Rocksmith-kabel"
        );
    }

    /// **Samma tempo tiger, olika tempo talar.** Monitorn får bara gå när frekvenserna är lika;
    /// annars skulle ingången höras i fel hastighet utan att något sade till.
    #[test]
    fn the_rate_verdict_speaks_only_when_the_rates_differ() {
        assert_eq!(rate_verdict(48_000, 48_000), RateVerdict::Agree);
        assert_eq!(
            rate_verdict(48_000, 44_100),
            RateVerdict::Mismatch {
                engine: 48_000,
                input: 44_100
            }
        );
        assert_eq!(
            rate_verdict(44_100, 48_000),
            RateVerdict::Mismatch {
                engine: 44_100,
                input: 48_000
            }
        );
    }

    /// Saknas enhetslistan blir svaret tomt — ingen panik, ingen påhittad ingång.
    #[test]
    fn no_devices_is_an_empty_answer() {
        assert!(resolve_inputs(&[], &[]).is_empty());
        assert!(first_instrument(&[]).is_none());
    }
}
