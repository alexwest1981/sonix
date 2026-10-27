//! **AI:ns instruktioner och verktyg** (roadmapens AI-punkt).
//!
//! Modellen får inte röra appen fritt. Den får en fast ordlista (Sonix egna begrepp), ett
//! schema med namngivna verktyg, och appen utför anropet efter validering. Allt som går att
//! avgöra utan appen — vilka steg ett komp ska ha, om en sektionsplan är rimlig — ligger här och
//! är provbart utan vare sig nät eller GUI.

use serde_json::{json, Value};

/// Kick, virvel och hi-hat som tre stegrader.
pub type DrumLayers = ([bool; STEPS], [bool; STEPS], [bool; STEPS]);

/// Max antal steg i ett pattern (4 takter à 16 sextondelar).
pub const STEPS: usize = 64;

/// Systemprompten. Kort med flit: en modell som får tio sidor om Sonix glömmer reglerna längst ned.
pub const SYSTEM_PROMPT: &str = "\
Du är Sonix, en assisterande producent inbyggd i DAW:t Sonix Studio. Du samtalar på svenska och
skriver aldrig kod.

Sonix ordlista:
- kanal: en rad i kanalracket med ett instrument eller en trumma (index 0..n).
- pattern: 1–4 takter sextondelar (16 steg per takt, max 64). Varje kanal har `steps` (på/av) och
  `notes` (MIDI 0–127) per steg.
- sektion: ett antal takter i arrangemanget (intro, vers, refräng, brygga …).
- spår: en rad i arrangemanget som spelar ett pattern eller ljud.

Regler:
1. Föreslå sektioner som en plan först. Skriv inget i arrangemanget förrän användaren säger ja.
2. Skriv aldrig över ett klipp eller en kanal som redan har noter utan att säga det.
3. Rör aldrig masterns volym.
4. Verktyg du får använda: set_tempo, set_key, write_drum_pattern, write_pattern, write_notes,
   load_drum_kit, plan_sections, mute_track, set_channel_instrument. Inget annat.
5. Svara med JSON: {\"calls\": [{\"tool\": \"<namn>\", \"args\": {…}}]} och inget annat när du
   vill göra något. Vill du bara prata, svara med ren text.

Skriva musik:
- `write_pattern` skriver en stämma på en kanal: `steps` är en sträng med ett tecken per
  sextondel, 16 för en takt, 32 för två, upp till 64. `x` = slag, `.` = vila. `note` är
  MIDI-noten slaget spelar (36 = C1). Trummor går på trumkanalerna, bas och synth på sina.
- `write_notes` skriver melodi/bas: `notes` är {step, pitch, len, vel} med step 0–63,
  pitch 0–127, len 1–16 steg, vel 1–127. Ett ackord är flera noter på samma step.
- `load_drum_kit` väljer samplingar ur användarens eget bibliotek för trumrollerna
  (kick, snare, clap, hat, openhat, crash) och lägger dem på trumkanalerna. Kalla den
  **före** write_pattern när användaren vill ha en stil (t.ex. trap) — då matchar ljuden.
- Beskriv stilen i ord (trap, drill, boom bap, house) så blir ljudvalet rätt: en trap-kick
  letar efter 808, en house-hat efter öppen hi-hat.
- Kanalerna du får adressera står i användarens meddelande (index och namn). Skriv bara på
  kanaler som finns; hitta inte på egna index.

Gränser du måste känna till — säg dem till användaren i stället för att låtsas:
- Rutnätet är **sextondelar**. Det finns ingen triolupplösning, så en trap-hi-hat med
  triolrullar går inte att skriva. Skriv raka sextondelar och säg att rullarna saknas.
- `write_pattern` ger **en not per steg och kanal**, utan anslag och utan notlängd. Vill du ha
  dynamik eller längre toner får det bli `write_notes` — men den skriver också i samma
  16-stegsrutnät, så legato över taktgräns går inte.
- Max 64 steg (fyra takter) per anrop, högst 64 noter per kanal och anrop.
- Ett `step` är ett **heltal**. `1.5` avvisas; avrunda själv och säg vad du gjorde.
";

/// Verktygen modellen får anropa. `mode` styr vilka som kräver bekräftelse.
pub fn tool_schema() -> Value {
    json!([
        {"type": "function", "function": {"name": "set_tempo", "description": "Sätter projektets tempo i BPM (40–240).", "parameters": {"type": "object", "properties": {"bpm": {"type": "number"}}, "required": ["bpm"]}}},
        {"type": "function", "function": {"name": "set_key", "description": "Sätter grundton och skala.", "parameters": {"type": "object", "properties": {"root": {"type": "string", "description": "C, C#, D … B"}, "scale": {"type": "string", "description": "dur eller moll"}}, "required": ["root"]}}},
        {"type": "function", "function": {"name": "write_drum_pattern", "description": "Skriver ett trumkomp i ett pattern. Stilen är en av de kända i DRUM_STYLES.", "parameters": {"type": "object", "properties": {"pattern": {"type": "integer"}, "style": {"type": "string"}, "bars": {"type": "integer", "description": "1–4"}}, "required": ["pattern", "style"]}}},
        {"type": "function", "function": {"name": "plan_sections", "description": "Föreslår en sektionsplan. Skriver inget — användaren bekräftar först.", "parameters": {"type": "object", "properties": {"sections": {"type": "array", "items": {"type": "object", "properties": {"name": {"type": "string"}, "bars": {"type": "integer"}}, "required": ["name", "bars"]}}}, "required": ["sections"]}}},
        {"type": "function", "function": {"name": "mute_track", "description": "Tystar eller slår på ett spår i arrangemanget.", "parameters": {"type": "object", "properties": {"track": {"type": "integer"}, "muted": {"type": "boolean"}}, "required": ["track", "muted"]}}},
        {"type": "function", "function": {"name": "set_channel_instrument", "description": "Byter ljud på en kanal i racket.", "parameters": {"type": "object", "properties": {"channel": {"type": "integer"}, "instrument": {"type": "string"}}, "required": ["channel", "instrument"]}}},
        {"type": "function", "function": {"name": "write_pattern", "description": "Skriver en stämma (trummor eller perc) på kanaler med ett eget stegmönster.", "parameters": {"type": "object", "properties": {"lanes": {"type": "array", "items": {"type": "object", "properties": {"channel": {"type": "integer"}, "steps": {"type": "string", "description": "16, 32, 48 eller 64 tecken: x = slag, . = vila"}, "note": {"type": "integer", "description": "MIDI-not 0–127, valfri"}}, "required": ["channel", "steps"]}}}, "required": ["lanes"]}}},
        {"type": "function", "function": {"name": "write_notes", "description": "Skriver melodi, bas eller ackord på en kanal.", "parameters": {"type": "object", "properties": {"channel": {"type": "integer"}, "notes": {"type": "array", "items": {"type": "object", "properties": {"step": {"type": "integer", "description": "0–63"}, "pitch": {"type": "integer", "description": "MIDI 0–127"}, "len": {"type": "integer", "description": "1–16 steg"}, "vel": {"type": "integer", "description": "1–127"}}, "required": ["step", "pitch"]}}}, "required": ["channel", "notes"]}}},
        {"type": "function", "function": {"name": "load_drum_kit", "description": "Väljer samplingar ur användarens bibliotek för trumrollerna och lägger dem på trumkanalerna.", "parameters": {"type": "object", "properties": {"genre": {"type": "string", "description": "stil, t.ex. trap, drill, house, boom bap"}}, "required": ["genre"]}}}
    ])
}

/// Ett anrop från modellen.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub tool: String,
    pub args: Value,
}

/// Läser modellens svar. Tre former accepteras, eftersom leverantörerna skiljer sig:
/// `{"calls":[…]}` (vår egen), en bar array, och OpenAI:s `tool_calls` med `arguments` som sträng.
pub fn parse_calls(text: &str) -> Result<Vec<Call>, String> {
    let trimmed = text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let value: Value = serde_json::from_str(trimmed).map_err(|e| format!("inte JSON: {e}"))?;
    let raw = if let Some(list) = value.get("calls").and_then(|v| v.as_array()) {
        list.clone()
    } else if let Some(list) = value.get("tool_calls").and_then(|v| v.as_array()) {
        list.iter()
            .filter_map(|c| c.get("function"))
            .map(|f| {
                let mut obj = json!({"tool": f.get("name").cloned().unwrap_or(Value::Null)});
                let args = f
                    .get("arguments")
                    .and_then(|a| a.as_str())
                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                    .or_else(|| f.get("arguments").cloned())
                    .unwrap_or_else(|| json!({}));
                obj["args"] = args;
                obj
            })
            .collect()
    } else if let Some(list) = value.as_array() {
        list.clone()
    } else {
        return Err("svaret innehöll inga anrop".to_string());
    };

    let mut calls = Vec::new();
    for item in raw {
        let tool = item
            .get("tool")
            .or_else(|| item.get("name"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "anrop utan verktygsnamn".to_string())?
            .to_string();
        let args = item.get("args").cloned().unwrap_or_else(|| json!({}));
        calls.push(Call { tool, args });
    }
    Ok(calls)
}

/// En sektion i en föreslagen plan.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedSection {
    pub name: String,
    pub bars: usize,
}

/// Läser en sektionsplan ur ett `plan_sections`-anrop och kräver att den går att spela:
/// minst en sektion, 1–32 takter var, och ett namn som inte är tomt.
pub fn parse_section_plan(args: &Value) -> Result<Vec<PlannedSection>, String> {
    let list = args
        .get("sections")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "planen saknar 'sections'".to_string())?;
    let mut out = Vec::new();
    for item in list {
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() {
            return Err("en sektion saknar namn".to_string());
        }
        let bars = item.get("bars").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        if !(1..=32).contains(&bars) {
            return Err(format!("sektionen '{name}' har {bars} takter (1–32 tillåts)"));
        }
        out.push(PlannedSection { name, bars });
    }
    if out.is_empty() {
        return Err("planen är tom".to_string());
    }
    Ok(out)
}

/// Stilar kompanter kan skrivas i. `(kick, virvel, hi-hat)` som steg i en takt (16 sextondelar).
pub const DRUM_STYLES: [(&str, &str, &str, &str); 5] = [
    ("house", "x...x...x...x...", "....x.......x...", "..x...x...x...x."),
    ("hiphop", "x.....x...x.....", "....x.......x...", "x.x.x.x.x.x.x.x."),
    ("rock", "x.......x.x.....", "....x.......x...", "x.x.x.x.x.x.x.x."),
    // Trap: 808-kick på ettan och trean, snaren på trean i varje halvtakt, och hi-haten i
    // sextondelar med ett dubbelslag före taktbytet. Triolrullar går inte: rutnätet är
    // sextondelar, så en treslagsserie ligger utanför vad ett steg kan säga.
    ("trap", "x.......x.......", "....x.......x...", "xxxxxxxxxxxxxxxx"),
    ("drill", "x.............x.", "....x.......x...", "x.xxx.x.x.xxx.x."),
];

/// Lägger en plan efter varandra från `start_bar` — ren funktion, så att starttakterna går att
/// prova utan en app. Sektionerna rör inte varandra: nästa börjar där den förra slutar.
pub fn sections_from_plan(
    plan: &[PlannedSection],
    start_bar: usize,
) -> Vec<(String, usize, usize)> {
    let mut bar = start_bar;
    let mut out = Vec::new();
    for section in plan {
        out.push((section.name.clone(), bar, section.bars));
        bar += section.bars;
    }
    out
}

/// **AI:n får inte skriva över ett komp användaren redan har.** Regeln är ren och provbar:
/// alla tre kanalerna måste vara tomma, annars avvisas hela kommandot (inget halvskrivet komp).
pub fn ensure_channels_free(busy: &[bool], names: &[String]) -> Result<(), String> {
    if let Some(i) = busy.iter().position(|b| *b) {
        let name = names.get(i).cloned().unwrap_or_else(|| format!("kanal {}", i + 1));
        return Err(format!("kanalen '{name}' har redan steg — töm den först"));
    }
    Ok(())
}

/// En stämma ur ett `write_pattern`-anrop: vilken kanal, vilka steg, och vilken not slaget spelar.
#[derive(Clone, Debug, PartialEq)]
pub struct Lane {
    pub channel: usize,
    pub steps: [bool; STEPS],
    pub note: Option<u8>,
}

/// Läser en stegsträng: `x`/`X` är slag, allt annat är vila. Längden måste vara 16, 32, 48
/// eller 64 — en sextondel per tecken, en takt per 16. Ett mönster som är 15 eller 20 tecken
/// långt är ett räknfel hos modellen, inte ett kort komp, och ska sägas tillbaka till den.
pub fn parse_steps(text: &str) -> Result<[bool; STEPS], String> {
    let chars: Vec<char> = text.trim().chars().filter(|c| !c.is_whitespace()).collect();
    if !matches!(chars.len(), 16 | 32 | 48 | 64) {
        return Err(format!(
            "steget är {} tecken långt — 16, 32, 48 eller 64 krävs (ett tecken per sextondel)",
            chars.len()
        ));
    }
    let mut steps = [false; STEPS];
    for (i, c) in chars.iter().enumerate() {
        steps[i] = matches!(c, 'x' | 'X' | 'o' | 'O');
    }
    Ok(steps)
}

/// Läser `lanes` ur ett `write_pattern`-anrop. En stämma per kanal, och **samma kanal två
/// gånger avvisas**: det andra anropet hade tyst skrivit över det första.
pub fn parse_lanes(args: &Value) -> Result<Vec<Lane>, String> {
    let list = args
        .get("lanes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "'lanes' saknas".to_string())?;
    if list.is_empty() {
        return Err("inga stämmor angavs".to_string());
    }
    if list.len() > 8 {
        return Err(format!("{} stämmor — högst 8 per anrop", list.len()));
    }
    let mut out: Vec<Lane> = Vec::new();
    for item in list {
        let channel = item.get("channel").and_then(|v| v.as_u64()).unwrap_or(u64::MAX) as usize;
        if channel >= 64 {
            return Err("kanalindex saknas eller är orimligt".to_string());
        }
        if out.iter().any(|l| l.channel == channel) {
            return Err(format!("kanal {} finns två gånger i samma anrop", channel));
        }
        let text = item.get("steps").and_then(|v| v.as_str()).unwrap_or("");
        let steps = parse_steps(text).map_err(|e| format!("kanal {channel}: {e}"))?;
        let note = item
            .get("note")
            .and_then(|v| v.as_u64())
            .map(|n| n.min(127) as u8);
        out.push(Lane { channel, steps, note });
    }
    Ok(out)
}

/// En not ur ett `write_notes`-anrop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteHit {
    pub step: usize,
    pub pitch: u8,
    pub length: u8,
    pub velocity: u8,
}

/// Läser melodi/bas. Två noter på samma steg **och samma tonhöjd** är en dubblering och
/// avvisas; samma steg med olika tonhöjd är ett ackord och går igenom.
pub fn parse_notes(args: &Value) -> Result<Vec<NoteHit>, String> {
    let list = args
        .get("notes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "'notes' saknas".to_string())?;
    if list.is_empty() {
        return Err("inga noter angavs".to_string());
    }
    if list.len() > 64 {
        return Err(format!("{} noter — högst 64 per kanal", list.len()));
    }
    let mut out: Vec<NoteHit> = Vec::new();
    for item in list {
        // **Ett bråktal är ett räkfel, inte en triol.** Rutnätet är sextondelar, så `1.5` kan
        // inte sparas — och tidigare föll det igenom som "steget -1 ligger utanför" och lät som
        // ett internt fel. Nu står det vad som gäller.
        let step = match item.get("step") {
            Some(v) if v.is_f64() && v.as_f64().is_some_and(|f| f.fract() != 0.0) => {
                return Err(format!(
                    "steget {} är inte ett heltal — rutnätet är sextondelar, avrunda till ett steg",
                    item.get("step").and_then(|v| v.as_f64()).unwrap_or(0.0)
                ));
            }
            // `2.0` är ett heltal skrivet som JSON-bråktal — `as_i64()` ger None för det, så
            // talet måste läsas som f64 först. Annars blir ett korrekt `2.0` till "steget -1".
            Some(v) => v
                .as_i64()
                .or_else(|| v.as_f64().map(|f| f.round() as i64))
                .unwrap_or(-1),
            None => -1,
        };
        if !(0..STEPS as i64).contains(&step) {
            return Err(format!("steget {step} ligger utanför 0–{}", STEPS - 1));
        }
        let pitch = item.get("pitch").and_then(|v| v.as_i64()).unwrap_or(-1);
        if !(0..=127).contains(&pitch) {
            return Err(format!("tonhöjden {pitch} ligger utanför 0–127"));
        }
        let length = item.get("len").and_then(|v| v.as_i64()).unwrap_or(1).clamp(1, 16) as u8;
        let velocity = item.get("vel").and_then(|v| v.as_i64()).unwrap_or(100).clamp(1, 127) as u8;
        let hit = NoteHit { step: step as usize, pitch: pitch as u8, length, velocity };
        if out.iter().any(|h| h.step == hit.step && h.pitch == hit.pitch) {
            return Err(format!("noten {} på steg {} finns två gånger", hit.pitch, hit.step));
        }
        out.push(hit);
    }
    out.sort_by_key(|h| h.step);
    Ok(out)
}

/// Ord som känns igen i ett samplebiblioteks filnamn, per roll. Ett bibliotek är filnamn —
/// `BD_808_Hard.wav`, `CHH_trap_02.wav` — så matchningen är text, inte metadata.
const ROLE_WORDS: &[(&str, &[&str])] = &[
    ("kick", &["kick", "bassdrum", "bass drum", "bd0", "bd_", "808"]),
    ("snare", &["snare", "snr", "sd_"]),
    ("clap", &["clap", "handclap", "cp_"]),
    ("hat", &["closed hat", "chh", "hat closed", "hihat", "hi-hat", "hh_", "hat"]),
    ("openhat", &["open hat", "ohh", "hat open", "ohat"]),
    ("crash", &["crash", "cymbal", "ride"]),
    ("perc", &["perc", "conga", "rim", "shaker", "tom", "cowbell"]),
    ("bass", &["bass", "sub"]),
    ("synth", &["lead", "pluck", "arp", "keys", "pad", "synth"]),
];

/// Vilken roll en **kanal** har, av dess namn: "808 Kick" → kick, "Open Hat" → openhat.
/// Rollistan är ordnad så att det mest specifika ordet vinner: en kanal som heter "Open Hat"
/// matchar både `hat` och `openhat`, och då ska den vara en öppen hi-hat.
pub fn role_of(channel_name: &str) -> Option<&'static str> {
    let hay = channel_name.to_lowercase();
    let mut best: Option<(usize, &'static str)> = None;
    for (role, words) in ROLE_WORDS {
        if let Some(pos) = words.iter().position(|w| hay.contains(w)) {
            if best.map_or(true, |(p, _)| pos < p) {
                best = Some((pos, role));
            }
        }
    }
    best.map(|(_, role)| role)
}

/// Väljer det sample som bäst matchar en roll och en stil. Ren funktion: `items` är
/// `(namn, kategori)` och returen är index, så valet går att prova utan ett bibliotek på disk.
///
/// Rollordet väger tyngst; stilen är en knuff, inte ett krav — en trap-kick letar efter 808 men
/// en vanlig kick duger när biblioteket inte har någon 808. Redan använda index hoppas över, så
/// kick och snare inte blir samma fil.
pub fn pick_sample(
    role: &str,
    genre: &str,
    items: &[(String, String)],
    used: &[usize],
) -> Option<usize> {
    let role_words = ROLE_WORDS
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(role.trim()))?
        .1;
    let genre = genre.trim().to_lowercase();
    let mut best: Option<(i32, usize)> = None;
    for (i, (name, category)) in items.iter().enumerate() {
        if used.contains(&i) {
            continue;
        }
        let hay = format!("{} {}", name.to_lowercase(), category.to_lowercase());
        let Some(role_score) = role_words.iter().position(|w| hay.contains(w)) else {
            continue;
        };
        // Första ordet i listan är det mest specifika: 808 före kick.
        let mut score = 10 - role_score as i32;
        if !genre.is_empty() && hay.contains(&genre) {
            score += 6;
        }
        if best.map_or(true, |(s, _)| score > s) {
            best = Some((score, i));
        }
    }
    best.map(|(_, i)| i)
}

/// Kompets tre lager för sig — kick, virvel, hi-hat — så appen kan skriva dem på tre kanaler.
pub fn drum_layers(style: &str, bars: usize) -> Result<DrumLayers, String> {
    let (_, kick, snare, hat) = DRUM_STYLES
        .iter()
        .find(|(name, ..)| name.eq_ignore_ascii_case(style.trim()))
        .ok_or_else(|| format!("okänd trumstil '{style}'"))?;
    let bars = bars.clamp(1, 4);
    let mut layers = ([false; STEPS], [false; STEPS], [false; STEPS]);
    for (text, target) in [(kick, &mut layers.0), (snare, &mut layers.1), (hat, &mut layers.2)] {
        for bar in 0..bars {
            for (i, ch) in text.chars().enumerate().take(16) {
                if ch == 'x' {
                    target[bar * 16 + i] = true;
                }
            }
        }
    }
    Ok(layers)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Alla tre svarsformerna ger samma anrop — annars tappas halva leverantörsmatrisen.
    #[test]
    fn three_response_shapes_parse_the_same_way() {
        let ours = r#"{"calls":[{"tool":"set_tempo","args":{"bpm":128}}]}"#;
        let bare = r#"[{"tool":"set_tempo","args":{"bpm":128}}]"#;
        let openai = r#"{"tool_calls":[{"function":{"name":"set_tempo","arguments":"{\"bpm\":128}"}}]}"#;
        let expected = vec![Call { tool: "set_tempo".into(), args: json!({"bpm": 128}) }];
        for text in [ours, bare, openai] {
            assert_eq!(parse_calls(text).unwrap(), expected, "{text}");
        }
    }

    /// Ett svar med bara text (ingen avsikt) ska ge ett tydligt fel, inte en tom lista som
    /// appen tolkar som "inget att göra".
    #[test]
    fn prose_without_calls_is_an_error() {
        assert!(parse_calls("Visst! Jag föreslår en vers på 8 takter.").is_err());
    }

    /// Kompet hamnar i rätt takter och stilen avgör mönstret.
    #[test]
    fn a_drum_pattern_lands_in_the_right_bars() {
        let (kick, snare, hat) = drum_layers("house", 1).unwrap();
        assert!(kick[0] && kick[4] && kick[8] && kick[12], "fyra slag på golvet");
        assert!(!kick[16], "takt 2 ska vara tom vid 1 takt");
        assert!(snare[4] && hat[2]);

        let (kick2, ..) = drum_layers("house", 2).unwrap();
        assert!(kick2[16] && kick2[20], "kompet upprepas i takt 2");

        assert!(drum_layers("polka", 1).is_err());
        // 9 takter kläms till 4 — patternet rymmer inte mer.
        let (long, ..) = drum_layers("rock", 9).unwrap();
        let (four, ..) = drum_layers("rock", 4).unwrap();
        assert_eq!(long.iter().filter(|s| **s).count(), four.iter().filter(|s| **s).count());
    }

    /// Sektionerna läggs efter varandra utan glapp eller överlapp.
    #[test]
    fn sections_follow_each_other_from_the_start_bar() {
        let plan = vec![
            PlannedSection { name: "Intro".into(), bars: 4 },
            PlannedSection { name: "Vers".into(), bars: 8 },
            PlannedSection { name: "Refräng".into(), bars: 8 },
        ];
        let out = sections_from_plan(&plan, 0);
        assert_eq!(out[0], ("Intro".to_string(), 0, 4));
        assert_eq!(out[1], ("Vers".to_string(), 4, 8));
        assert_eq!(out[2], ("Refräng".to_string(), 12, 8));
    }

    /// Överskrivningsskyddet: en enda upptagen kanal stoppar hela kommandot.
    #[test]
    fn a_drum_write_is_refused_when_any_channel_has_steps() {
        let names = vec!["Kick".to_string(), "Snare".to_string(), "Hat".to_string()];
        assert!(ensure_channels_free(&[false, false, false], &names).is_ok());
        let err = ensure_channels_free(&[false, true, false], &names).unwrap_err();
        assert!(err.contains("Snare"), "{err}");
    }

    /// En plan med tomt namn eller omöjlig längd avvisas före den når appen.
    #[test]
    fn a_section_plan_is_validated_before_it_is_applied() {
        let ok = parse_section_plan(&json!({"sections": [
            {"name": "Intro", "bars": 4}, {"name": "Vers", "bars": 8}
        ]}))
        .unwrap();
        assert_eq!(ok.len(), 2);
        assert_eq!(ok[1].bars, 8);

        assert!(parse_section_plan(&json!({"sections": []})).is_err());
        assert!(parse_section_plan(&json!({"sections": [{"name": "  ", "bars": 4}]})).is_err());
        assert!(parse_section_plan(&json!({"sections": [{"name": "Vers", "bars": 0}]})).is_err());
        assert!(parse_section_plan(&json!({"sections": [{"name": "Vers", "bars": 99}]})).is_err());
    }

/// En stegsträng är ett räknestycke: 16, 32, 48 eller 64 tecken, och allt annat avvisas.
    #[test]
    fn a_step_string_must_be_whole_bars() {
        let steps = parse_steps("x...x...x...x...").unwrap();
        assert!(steps[0] && steps[4] && steps[12]);
        assert!(!steps[1]);
        assert_eq!(steps.iter().filter(|s| **s).count(), 4);

        // Två takter hamnar i takt 2, inte i takt 1.
        let two = parse_steps(&"x...............".repeat(2)).unwrap();
        assert!(two[16]);

        for bad in ["x...", "", "x".repeat(15).as_str(), "x".repeat(20).as_str()] {
            assert!(parse_steps(bad).is_err(), "längd {} borde avvisas", bad.len());
        }
        // Mellanslag räknas inte som steg, så en radbrytning i svaret fäller inget komp.
        assert!(parse_steps("x... x... x... x...").is_ok());
    }

    /// Samma kanal två gånger i ett anrop hade tyst skrivit över den första stämman.
    #[test]
    fn lanes_refuse_the_same_channel_twice() {
        let ok = parse_lanes(&json!({"lanes": [
            {"channel": 0, "steps": "x...x...x...x...", "note": 36},
            {"channel": 3, "steps": "x.x.x.x.x.x.x.x."}
        ]}))
        .unwrap();
        assert_eq!(ok.len(), 2);
        assert_eq!(ok[0].note, Some(36));
        assert_eq!(ok[1].note, None);

        assert!(parse_lanes(&json!({"lanes": []})).is_err());
        assert!(parse_lanes(&json!({})).is_err());
        assert!(parse_lanes(&json!({"lanes": [{"channel": 2, "steps": "x..."}]})).is_err());
        assert!(parse_lanes(&json!({"lanes": [
            {"channel": 1, "steps": "x...x...x...x..."},
            {"channel": 1, "steps": "x...x...x...x..."}
        ]}))
        .is_err());
        assert!(parse_lanes(&json!({"lanes": [{"steps": "x...x...x...x..."}]})).is_err());
    }

    /// Ackord går igenom (samma steg, olika toner); en dubblett av samma ton gör det inte.
    #[test]
    /// Ett bråktal i `step` ska säga vad som gäller — inte falla igenom som "steget -1".
    #[test]
    fn a_fractional_step_says_the_grid_is_sixteenths() {
        let args = serde_json::json!({"notes": [{"step": 1.5, "pitch": 60}]});
        let err = parse_notes(&args).unwrap_err();
        assert!(err.contains("inte ett heltal"), "{err}");
        assert!(err.contains("sextondelar"), "{err}");
        // Heltal som JSON-bråktal (2.0) är fortfarande ett heltal.
        let args = serde_json::json!({"notes": [{"step": 2.0, "pitch": 60}]});
        assert_eq!(parse_notes(&args).unwrap()[0].step, 2);
    }

    fn notes_allow_chords_but_not_double_hits() {
        let chord = parse_notes(&json!({"notes": [
            {"step": 0, "pitch": 36}, {"step": 0, "pitch": 43}, {"step": 8, "pitch": 48}
        ]}))
        .unwrap();
        assert_eq!(chord.len(), 3);
        assert_eq!(chord[0].step, 0);

        assert!(parse_notes(&json!({"notes": [{"step": 0, "pitch": 36}, {"step": 0, "pitch": 36}]})).is_err());
        assert!(parse_notes(&json!({"notes": [{"step": 64, "pitch": 36}]})).is_err());
        assert!(parse_notes(&json!({"notes": [{"step": 0, "pitch": 200}]})).is_err());
        assert!(parse_notes(&json!({"notes": []})).is_err());
        // Längd och anslag kläms, inte avvisas: de är nyanser, inte räknfel.
        let clamped = parse_notes(&json!({"notes": [{"step": 4, "pitch": 60, "len": 99, "vel": 0}]})).unwrap();
        assert_eq!(clamped[0].length, 16);
        assert_eq!(clamped[0].velocity, 1);
    }

    /// Ljudvalet: rollen väger tyngst, stilen knuffar, och en redan använd fil tas inte igen.
    #[test]
    fn sample_picking_prefers_the_role_then_the_genre() {
        let items = vec![
            ("BD_909_Kick".to_string(), "drums".to_string()),
            ("BD_808_trap_Hard".to_string(), "808".to_string()),
            ("CHH_trap_02".to_string(), "hats".to_string()),
            ("Clap_909".to_string(), "drums".to_string()),
        ];
        // Trap-kicken ska bli 808:an, inte 909:an.
        assert_eq!(pick_sample("kick", "trap", &items, &[]), Some(1));
        assert_eq!(pick_sample("kick", "house", &items, &[]), Some(0));
        // Upptagen fil hoppas över.
        assert_eq!(pick_sample("kick", "trap", &items, &[1]), Some(0));
        // Ingen hi-hat i listan efter att den använts → inget val, inte en kick.
        assert_eq!(pick_sample("hat", "trap", &items, &[2]), None);
        assert_eq!(pick_sample("trumpet", "", &items, &[]), None);
    }

    /// Kanalens roll läses ur namnet — annars vet `load_drum_kit` inte vad den ska leta efter.
    #[test]
    fn a_channel_role_comes_from_its_name() {
        assert_eq!(role_of("808 Kick"), Some("kick"));
        assert_eq!(role_of("909 Snare"), Some("snare"));
        assert_eq!(role_of("Crisp Hat"), Some("hat"));
        assert_eq!(role_of("Open Hat"), Some("openhat"));
        assert_eq!(role_of("Electro Clap"), Some("clap"));
        assert_eq!(role_of("Sub Bass"), Some("bass"));
        assert_eq!(role_of("303 Lead"), Some("synth"));
        assert_eq!(role_of("Mickey"), None);
    }

    /// Schemat ska räknas upp exakt, så prompten och koden inte glider isär.
    #[test]
    fn the_schema_lists_every_tool_the_prompt_promises() {
        let schema = tool_schema();
        let names: Vec<&str> = schema
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            vec![
                "set_tempo",
                "set_key",
                "write_drum_pattern",
                "plan_sections",
                "mute_track",
                "set_channel_instrument",
                "write_pattern",
                "write_notes",
                "load_drum_kit"
            ]
        );
        // Varje verktygsnamn ska också nämnas i prompten, annars vet modellen inte att det finns.
        for name in &names {
            assert!(SYSTEM_PROMPT.contains(name), "prompten nämner inte {name}");
        }
    }
}
