//! **Suno-panelens formulär, ritat ur verktygets eget schema.**
//!
//! Sunos egen panel har vocal gender, duration, max mode, personalize, weirdness, style
//! influence, variety, titel och en sparplats. Att skriva in de namnen här hade låst oss till en
//! beta-yta vi inte har mätt: MCP-verktyget beskriver sina parametrar i `input_schema`, och det är
//! den beskrivningen som ska styra fälten. Då dyker varje option servern faktiskt har upp — även
//! de som tillkommer efter oss — och ett annat namn spelar ingen roll.
//!
//! Ren modul: schema in, fält ut; fält in, argument-JSON ut. Ingen UI, ingen tråd, inget nät.

use serde_json::{json, Map, Value};

/// Ett fält i panelen, med sitt värde. Ordningen är schemats, så panelen ser likadan ut varje gång.
#[derive(Clone, Debug, PartialEq)]
pub enum SunoField {
    /// Fritext. `multiline` för textarea (lyrics, prompt).
    Text { value: String, multiline: bool },
    /// Kryssruta (max mode, personalize).
    Bool(bool),
    /// Slider. `min`/`max` ur schemat när de finns, annars 0..1 — det är skalan Suno använder för
    /// weirdness och style influence.
    Number { value: f64, min: f64, max: f64 },
    /// Rullgardin ur `enum`.
    Choice { options: Vec<String>, selected: usize },
}

/// Fälten i schemats ordning. Ett schema utan `properties` ger inga fält — panelen visar då bara
/// prompten, vilket är rätt svar på ett verktyg som inte beskriver sig.
pub fn fields_from_schema(schema: &Value) -> Vec<(String, SunoField)> {
    let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(props.len());
    for (name, spec) in props {
        let kind = spec.get("type").and_then(|t| t.as_str()).unwrap_or("string");
        let field = if let Some(options) = spec.get("enum").and_then(|e| e.as_array()) {
            let options: Vec<String> = options
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            SunoField::Choice { options, selected: 0 }
        } else {
            match kind {
                "boolean" => SunoField::Bool(false),
                "number" | "integer" => SunoField::Number {
                    value: spec
                        .get("default")
                        .and_then(|d| d.as_f64())
                        .unwrap_or(0.5),
                    min: spec.get("minimum").and_then(|d| d.as_f64()).unwrap_or(0.0),
                    max: spec.get("maximum").and_then(|d| d.as_f64()).unwrap_or(1.0),
                },
                _ => SunoField::Text {
                    value: String::new(),
                    // Långa texter får en ruta man kan skriva i; `description`avgör inte det,
                    // fältnamnet gör det.
                    multiline: matches!(name.as_str(), "lyrics" | "prompt" | "description" | "style"),
                },
            }
        };
        out.push((name.clone(), field));
    }
    out
}

/// Argumenten som skickas till verktyget. Tomma texter och orörda standardval skickas **inte** —
/// ett fält användaren inte rört ska inte tvinga servern till ett värde.
pub fn args_from_form(fields: &[(String, SunoField)]) -> Value {
    let mut map = Map::new();
    for (name, field) in fields {
        let value = match field {
            SunoField::Text { value, .. } => {
                if value.trim().is_empty() {
                    continue;
                }
                json!(value)
            }
            SunoField::Bool(on) => json!(on),
            SunoField::Number { value, .. } => json!(value),
            SunoField::Choice { options, selected } => match options.get(*selected) {
                Some(choice) => json!(choice),
                None => continue,
            },
        };
        map.insert(name.clone(), value);
    }
    Value::Object(map)
}

/// Textfältet med det namnet, om schemat har ett. Panelen skriver in prompten där i stället för i
/// ett eget fält bredvid, så att ett verktyg som kallar den `description` också fungerar.
pub fn text_field_mut<'a>(
    fields: &'a mut [(String, SunoField)],
    names: &[&str],
) -> Option<&'a mut String> {
    for (name, field) in fields.iter_mut() {
        if names.contains(&name.as_str())
            && let SunoField::Text { value, .. } = field
        {
            return Some(value);
        }
    }
    None
}


/// **Var i formuläret Sunos egna arbetsflöde sitter.** Sunos skapa-vy har Lyrics, Styles,
/// Weirdness, Style Influence, Variety, Vocal Gender, Duration, Max Mode, Personalize och
/// Song Title. Vi vet inte vad beta-ytan kallar dem, så varje plats letas upp på nyckelord i
/// fältnamnet — och en plats som inte finns blir `None`, vilket panelen visar som "servern erbjuder
/// inte det här" i stället för en ratt som inte gör något.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SunoSlots {
    pub lyrics: Option<usize>,
    pub styles: Option<usize>,
    pub title: Option<usize>,
    pub weirdness: Option<usize>,
    pub style_influence: Option<usize>,
    pub variety: Option<usize>,
    pub vocal_gender: Option<usize>,
    pub duration: Option<usize>,
    pub max_mode: Option<usize>,
    pub personalize: Option<usize>,
    pub workspace: Option<usize>,
    pub model: Option<usize>,
}

fn find(fields: &[(String, SunoField)], needles: &[&str]) -> Option<usize> {
    fields.iter().position(|(name, _)| {
        let name = name.to_lowercase();
        needles.iter().any(|n| name.contains(n))
    })
}

/// Platserna, i den ordning Suno visar dem.
pub fn slots(fields: &[(String, SunoField)]) -> SunoSlots {
    SunoSlots {
        lyrics: find(fields, &["lyric", "text"]),
        styles: find(fields, &["style", "tag", "genre"]),
        title: find(fields, &["title", "name"]),
        weirdness: find(fields, &["weird"]),
        style_influence: find(fields, &["style_influence", "style_strength", "influence"]),
        variety: find(fields, &["variety"]),
        vocal_gender: find(fields, &["vocal", "gender", "voice_type"]),
        duration: find(fields, &["duration", "length"]),
        max_mode: find(fields, &["max_mode", "max"]),
        personalize: find(fields, &["personalize", "personal"]),
        workspace: find(fields, &["workspace", "save_to", "folder"]),
        model: find(fields, &["model", "version"]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ett schema av den form Sunos panel antyder: enums, tal, bool och text.
    fn reference_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string" },
                "title": { "type": "string" },
                "vocal_gender": { "type": "string", "enum": ["female", "male"] },
                "duration": { "type": "string", "enum": ["auto", "custom"] },
                "weirdness": { "type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.5 },
                "style_influence": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                "variety": { "type": "number" },
                "max_mode": { "type": "boolean" },
                "personalize": { "type": "boolean" }
            }
        })
    }

    #[test]
    fn the_schema_decides_the_fields() {
        let fields = fields_from_schema(&reference_schema());
        for (name, field) in &fields {
            println!("{name}: {field:?}");
        }
        assert_eq!(fields.len(), 9);
        let by = |n: &str| fields.iter().find(|(name, _)| name == n).map(|(_, f)| f.clone());
        assert!(matches!(by("vocal_gender"), Some(SunoField::Choice { options, .. }) if options == vec!["female", "male"]));
        assert!(matches!(by("weirdness"), Some(SunoField::Number { value, .. }) if (value - 0.5).abs() < 1e-9));
        assert!(matches!(by("max_mode"), Some(SunoField::Bool(false))));
        assert!(matches!(by("prompt"), Some(SunoField::Text { multiline: true, .. })));
        assert!(matches!(by("title"), Some(SunoField::Text { multiline: false, .. })));
    }

    #[test]
    fn the_form_becomes_the_arguments() {
        let mut fields = fields_from_schema(&reference_schema());
        if let Some(prompt) = text_field_mut(&mut fields, &["prompt", "description"]) {
            prompt.push_str("dark synthwave");
        }
        for (name, field) in fields.iter_mut() {
            match (name.as_str(), field) {
                ("vocal_gender", SunoField::Choice { selected, .. }) => *selected = 1,
                ("weirdness", SunoField::Number { value, .. }) => *value = 0.7,
                ("max_mode", SunoField::Bool(on)) => *on = true,
                _ => {}
            }
        }
        let args = args_from_form(&fields);
        println!("args: {args}");
        assert_eq!(args["prompt"], "dark synthwave");
        assert_eq!(args["vocal_gender"], "male");
        assert_eq!(args["weirdness"], 0.7);
        assert_eq!(args["max_mode"], true);
        // Orörda textfält skickas inte alls — tom titel ska inte tvinga servern till "".
        assert!(args.get("title").is_none(), "tom titel skickades: {args}");
    }

    #[test]
    fn a_schema_without_properties_gives_no_fields() {
        let fields = fields_from_schema(&json!({ "type": "object" }));
        println!("fält: {fields:?}");
        assert!(fields.is_empty());
        assert_eq!(args_from_form(&fields), json!({}));
    }

    /// Sunos egna tio kontroller hittar sina platser i ett schema som beskriver dem.
    #[test]
    fn the_workflow_finds_its_slots() {
        let schema = json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string" },
                "lyrics": { "type": "string" },
                "style": { "type": "string" },
                "song_title": { "type": "string" },
                "weirdness": { "type": "number" },
                "style_influence": { "type": "number" },
                "variety": { "type": "number" },
                "vocal_gender": { "type": "string", "enum": ["female", "male"] },
                "duration": { "type": "string", "enum": ["auto", "custom"] },
                "max_mode": { "type": "boolean" },
                "personalize": { "type": "boolean" },
                "save_to": { "type": "string" },
                "model": { "type": "string" }
            }
        });
        let fields = fields_from_schema(&schema);
        let found = slots(&fields);
        println!("platser: {found:?}");
        for (label, slot) in [
            ("lyrics", found.lyrics),
            ("styles", found.styles),
            ("title", found.title),
            ("weirdness", found.weirdness),
            ("style_influence", found.style_influence),
            ("variety", found.variety),
            ("vocal_gender", found.vocal_gender),
            ("duration", found.duration),
            ("max_mode", found.max_mode),
            ("personalize", found.personalize),
            ("workspace", found.workspace),
            ("model", found.model),
        ] {
            assert!(slot.is_some(), "{label} hittades inte");
        }
        // Ett schema utan kontrollerna ger tomma platser, inte påhittade.
        let bare = slots(&fields_from_schema(&json!({ "type": "object" })));
        assert_eq!(bare, SunoSlots::default());
        println!("utan schema: {bare:?}");
    }
}