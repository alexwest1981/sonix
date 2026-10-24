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
4. Verktyg du får använda: set_tempo, set_key, write_drum_pattern, plan_sections, mute_track,
   set_channel_instrument. Inget annat.
5. Svara med JSON: {\"calls\": [{\"tool\": \"<namn>\", \"args\": {…}}]} och inget annat när du
   vill göra något. Vill du bara prata, svara med ren text.
";

/// Verktygen modellen får anropa. `mode` styr vilka som kräver bekräftelse.
pub fn tool_schema() -> Value {
    json!([
        {"type": "function", "function": {"name": "set_tempo", "description": "Sätter projektets tempo i BPM (40–240).", "parameters": {"type": "object", "properties": {"bpm": {"type": "number"}}, "required": ["bpm"]}}},
        {"type": "function", "function": {"name": "set_key", "description": "Sätter grundton och skala.", "parameters": {"type": "object", "properties": {"root": {"type": "string", "description": "C, C#, D … B"}, "scale": {"type": "string", "description": "dur eller moll"}}, "required": ["root"]}}},
        {"type": "function", "function": {"name": "write_drum_pattern", "description": "Skriver ett trumkomp i ett pattern. Stilen är en av de kända i DRUM_STYLES.", "parameters": {"type": "object", "properties": {"pattern": {"type": "integer"}, "style": {"type": "string"}, "bars": {"type": "integer", "description": "1–4"}}, "required": ["pattern", "style"]}}},
        {"type": "function", "function": {"name": "plan_sections", "description": "Föreslår en sektionsplan. Skriver inget — användaren bekräftar först.", "parameters": {"type": "object", "properties": {"sections": {"type": "array", "items": {"type": "object", "properties": {"name": {"type": "string"}, "bars": {"type": "integer"}}, "required": ["name", "bars"]}}}, "required": ["sections"]}}},
        {"type": "function", "function": {"name": "mute_track", "description": "Tystar eller slår på ett spår i arrangemanget.", "parameters": {"type": "object", "properties": {"track": {"type": "integer"}, "muted": {"type": "boolean"}}, "required": ["track", "muted"]}}},
        {"type": "function", "function": {"name": "set_channel_instrument", "description": "Byter ljud på en kanal i racket.", "parameters": {"type": "object", "properties": {"channel": {"type": "integer"}, "instrument": {"type": "string"}}, "required": ["channel", "instrument"]}}}
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
pub const DRUM_STYLES: [(&str, &str, &str, &str); 3] = [
    ("house", "x...x...x...x...", "....x.......x...", "..x...x...x...x."),
    ("hiphop", "x.....x...x.....", "....x.......x...", "x.x.x.x.x.x.x.x."),
    ("rock", "x.......x.x.....", "....x.......x...", "x.x.x.x.x.x.x.x."),
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
                "set_channel_instrument"
            ]
        );
        // Varje verktygsnamn ska också nämnas i prompten, annars vet modellen inte att det finns.
        for name in &names {
            assert!(SYSTEM_PROMPT.contains(name), "prompten nämner inte {name}");
        }
    }
}
