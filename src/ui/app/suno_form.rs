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


/// Ett schema-objekt blir ett fält. Delad av den platta och den nästlade vägen.
fn field_from_spec(name: &str, spec: &Value) -> SunoField {
    let kind = spec.get("type").and_then(|t| t.as_str()).unwrap_or("string");
    if let Some(options) = spec.get("enum").and_then(|e| e.as_array()) {
        let options: Vec<String> = options
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        return SunoField::Choice { options, selected: 0 };
    }
    match kind {
        "boolean" => SunoField::Bool(false),
        "number" | "integer" => SunoField::Number {
            value: spec.get("default").and_then(|d| d.as_f64()).unwrap_or(0.5),
            min: spec.get("minimum").and_then(|d| d.as_f64()).unwrap_or(0.0),
            max: spec.get("maximum").and_then(|d| d.as_f64()).unwrap_or(1.0),
        },
        _ => SunoField::Text {
            value: String::new(),
            multiline: matches!(
                name.rsplit('.').next().unwrap_or(name),
                "lyrics" | "prompt" | "description" | "style"
            ),
        },
    }
}

/// Fälten i schemats ordning. Ett schema utan `properties` ger inga fält — panelen visar då bara
/// prompten, vilket är rätt svar på ett verktyg som inte beskriver sig.
pub fn fields_from_schema(schema: &Value) -> Vec<(String, SunoField)> {
    let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(props.len());
    for (name, spec) in props {
        // **Nästlade objekt packas upp.** Sunos `audio-create` lägger weirdness, style influence
        // och variety inuti `controls`; utan det här blev "More options" tom fast servern har
        // kontrollerna.
        if let Some(nested) = spec.get("properties").and_then(|p| p.as_object()) {
            for (sub, sub_spec) in nested {
                let dotted = format!("{name}.{sub}");
                let field = field_from_spec(&dotted, sub_spec);
                out.push((dotted, field));
            }
            continue;
        }
        let field = field_from_spec(name, spec);
        out.push((name.clone(), field));
    }
    out
}

/// The live schema's fields with the values already typed carried over by field name.
///
/// The schema decides which fields exist; a value typed before sign-in survives when the name
/// matches. A name the server does not describe (the fallback's own extras) is dropped.
pub fn merge_fields(
    typed: &[(String, SunoField)],
    live: Vec<(String, SunoField)>,
) -> Vec<(String, SunoField)> {
    live.into_iter()
        .map(|(name, field)| match typed.iter().find(|(old, _)| *old == name) {
            Some((_, old)) => (name, carry_value(old.clone(), field)),
            None => (name, field),
        })
        .collect()
}

/// A typed value wins; the schema decides the shape around it (options, bounds, multiline).
fn carry_value(old: SunoField, new: SunoField) -> SunoField {
    match (old, new) {
        (SunoField::Text { value, .. }, SunoField::Text { multiline, .. }) => {
            SunoField::Text { value, multiline }
        }
        (SunoField::Bool(on), SunoField::Bool(_)) => SunoField::Bool(on),
        (SunoField::Number { value, .. }, SunoField::Number { min, max, .. }) => {
            SunoField::Number { value, min, max }
        }
        // The schema's own list decides which choices exist; the index is kept if it still fits.
        (SunoField::Choice { selected, .. }, SunoField::Choice { options, .. }) => {
            SunoField::Choice {
                selected: selected.min(options.len().saturating_sub(1)),
                options,
            }
        }
        (_, new) => new,
    }
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
            // **Bara sant skickas.** `duration_guidance`/`duration_strict` är `false` som default
            // och servern svarar 400 på dem utan `duration`: "omit duration_guidance unless you
            // set duration". Samma sak för `instrumental`.
            SunoField::Bool(on) => {
                if !*on {
                    continue;
                }
                json!(true)
            }
            // **Ett tal utanför schemats intervall skickas inte alls.** `duration` har 15–360 och
            // en nolla där betyder "låt modellen välja"; servern svarade `invalid_request` på den.
            SunoField::Number { value, min, max } => {
                if *value < *min || *value > *max {
                    continue;
                }
                json!(value)
            }
            SunoField::Choice { options, selected } => match options.get(*selected) {
                Some(choice) => json!(choice),
                None => continue,
            },
        };
        // `controls.weirdness` blir `{"controls":{"weirdness":…}}` — serverns egen form.
        match name.split_once('.') {
            Some((outer, inner)) => {
                let entry = map
                    .entry(outer.to_string())
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Some(object) = entry.as_object_mut() {
                    object.insert(inner.to_string(), value);
                }
            }
            None => {
                map.insert(name.clone(), value);
            }
        }
    }
    // **Ett arbetsytenamn är inget id.** Servern svarar "workspace_id is not a workspace id" på
    // fritext; bara uuid-formade id (från `workspaces-list`) skickas.
    if let Some(id) = map.get("workspace_id").and_then(|v| v.as_str()) {
        let looks_like_id = id.len() >= 32 && id.chars().filter(|c| *c == '-').count() >= 4;
        if !looks_like_id {
            map.remove("workspace_id");
        }
    }
    // **Rösten vinner över `vocal_gender`.** En namngiven röst har redan ett kön; servern säger
    // uttryckligen att den tar precedence, så vi skickar inte båda.
    if map.get("voice_id").and_then(|v| v.as_str()).is_some_and(|v| !v.is_empty()) {
        map.remove("vocal_gender");
    }
    // **`audio_influence` kräver en röst.** Schemat: "Only valid alongside a `voice_id` — a
    // generation that conditions on no audio returns 400". Mätt: `audio_influence requires
    // conditioning audio — set a voice_id` utan röst.
    let has_voice = map
        .get("voice_id")
        .and_then(|v| v.as_str())
        .is_some_and(|v| !v.trim().is_empty());
    if !has_voice && let Some(controls) = map.get_mut("controls").and_then(|c| c.as_object_mut()) {
        controls.remove("audio_influence");
        if controls.is_empty() {
            map.remove("controls");
        }
    }
    // **`description` och `style` får inte skickas ihop.** Schemat säger: "Not valid with
    // `description`" — Simple-läget skickar beskrivningen, Custom-läget taggarna.
    if map
        .get("description")
        .and_then(|d| d.as_str())
        .is_some_and(|d| !d.trim().is_empty())
    {
        map.remove("style");
        map.remove("negative_style");
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
    pub description: Option<usize>,
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
    pub instrumental: Option<usize>,
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
        description: find(fields, &["description", "prompt"]),
        lyrics: find(fields, &["lyric"]),
        styles: find(fields, &["style", "tag", "genre"]),
        title: find(fields, &["title", "name"]),
        weirdness: find(fields, &["weird"]),
        style_influence: find(fields, &["style_influence", "style_strength", "influence"]),
        variety: find(fields, &["variety"]),
        vocal_gender: find(fields, &["vocal", "gender", "voice_type"]),
        duration: find(fields, &["duration", "length"]),
        max_mode: find(fields, &["max_mode", "max"]),
        personalize: find(fields, &["personalize", "personal"]),
        instrumental: find(fields, &["instrumental"]),
        workspace: find(fields, &["workspace", "save_to", "folder"]),
        model: find(fields, &["model", "version"]),
    }
}


/// **Fälten som den riktiga `audio-create` beskriver, mätta 1 nov 2026.** Används innan
/// anslutningen hunnit hämta schemat, så att ytan går att skriva i direkt; vid anslutning ersätts
/// listan av serverns egen. Ordningen är Sunos: det man skriver först, options sist.
pub fn fallback_fields() -> Vec<(String, SunoField)> {
    let text = |name: &str, multiline: bool| {
        (
            name.to_string(),
            SunoField::Text {
                value: String::new(),
                multiline,
            },
        )
    };
    vec![
        text("description", true),
        text("lyrics", true),
        text("style", false),
        text("title", false),
        text("negative_style", false),
        (
            "instrumental".to_string(),
            SunoField::Bool(false),
        ),
        (
            "vocal_gender".to_string(),
            SunoField::Choice {
                options: vec!["female".to_string(), "male".to_string()],
                selected: 0,
            },
        ),
        // 0 = "modellen väljer" och faller utanför 15–360, så den skickas aldrig.
        (
            "duration".to_string(),
            SunoField::Number {
                value: 0.0,
                min: 15.0,
                max: 360.0,
            },
        ),
        (
            "model".to_string(),
            // Ingen förvald modell: användaren väljer ur kontots `allowed_models`.
            SunoField::Text {
                value: String::new(),
                multiline: false,
            },
        ),
        text("workspace_id", false),
        (
            "controls.weirdness".to_string(),
            SunoField::Number {
                value: 0.5,
                min: 0.0,
                max: 1.0,
            },
        ),
        (
            "controls.style_influence".to_string(),
            SunoField::Number {
                value: 0.5,
                min: 0.0,
                max: 1.0,
            },
        ),
    ]
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

    /// Reservschemat fyller Sunos egna kontroller innan anslutningen hunnit svara.
    #[test]
    fn the_fallback_schema_fills_the_workflow() {
        let fields = fallback_fields();
        let found = slots(&fields);
        println!("reserv: {} fält, platser {found:?}", fields.len());
        assert!(found.lyrics.is_some());
        assert!(found.styles.is_some());
        assert!(found.title.is_some());
        assert!(found.weirdness.is_some());
        assert!(found.style_influence.is_some());
        assert!(found.vocal_gender.is_some());
        assert!(found.duration.is_some());
        assert!(found.model.is_some());
        assert!(found.workspace.is_some());
        // Variation finns inte i audio-create — platsen ska vara tom, inte påhittad.
        assert!(found.variety.is_none());
        let args = args_from_form(&fields);
        println!("reserv-argument: {args}");
        assert!(args.get("controls").and_then(|c| c.get("weirdness")).is_some());
    }

    /// Ett `false` i ett kryssruta-fält skickas inte: servern kallar det "invalid request body".
    #[test]
    fn a_false_toggle_is_not_sent() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "description": { "type": "string" },
                "duration_guidance": { "type": "boolean" },
                "duration_strict": { "type": "boolean" },
                "instrumental": { "type": "boolean" }
            }
        });
        let mut fields = fields_from_schema(&schema);
        if let Some((_, SunoField::Text { value, .. })) = fields.first_mut() {
            *value = "short atmospheric synthwave".to_string();
        }
        let args = args_from_form(&fields);
        println!("argument: {args}");
        assert!(args.get("duration_guidance").is_none());
        assert!(args.get("duration_strict").is_none());
        assert!(args.get("instrumental").is_none());
        assert_eq!(args.get("description").and_then(|d| d.as_str()), Some("short atmospheric synthwave"));
    }

    /// **Fälten ur det riktiga `audio-create`-schemat**, med bara en beskrivning ifylld, ger den
    /// kropp panelen skickar. Skrivs ut så den kan skickas till servern som den är.
    #[test]
    fn the_live_schema_body_is_printed() {
        let path = "/mnt/fastnvme/hermes/cache/scratch/create_schema.json";
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        let schema: Value = serde_json::from_str(&text).expect("schema");
        let mut fields = fields_from_schema(&schema);
        if let Some((_, SunoField::Text { value, .. })) = fields
            .iter_mut()
            .find(|(name, _)| name == "description")
        {
            *value = "short atmospheric synthwave, 20 seconds".to_string();
        }
        let args = args_from_form(&fields);
        println!("live-body: {args}");
    }

    /// Typing before sign-in is not thrown away: the live schema keeps the typed values by name
    /// and drops the fallback's extra names.
    #[test]
    fn a_typed_form_survives_the_live_schema() {
        let mut typed = fallback_fields();
        for (name, value) in [
            ("description", "pagan metal with an irish harp"),
            ("lyrics", "[verse]\nlong ships on a grey sea"),
            ("style", "pagan metal, irish harp"),
            ("negative_style", "no autotune"),
        ] {
            if let Some((_, SunoField::Text { value: slot, .. })) =
                typed.iter_mut().find(|(old, _)| old == name)
            {
                *slot = value.to_string();
            }
        }
        // The server's schema does not describe `negative_style`: it must not survive.
        let live = fields_from_schema(&serde_json::json!({
            "type": "object",
            "properties": {
                "description": { "type": "string" },
                "lyrics": { "type": "string" },
                "style": { "type": "string" },
                "title": { "type": "string" }
            }
        }));
        let merged = merge_fields(&typed, live);
        println!("merged fields: {merged:?}");
        let get = |name: &str| {
            merged
                .iter()
                .find(|(old, _)| old == name)
                .map(|(_, field)| field.clone())
        };
        assert!(matches!(get("description"), Some(SunoField::Text { ref value, .. }) if value == "pagan metal with an irish harp"));
        assert!(matches!(get("lyrics"), Some(SunoField::Text { ref value, .. }) if value.contains("long ships")));
        assert!(matches!(get("style"), Some(SunoField::Text { ref value, .. }) if value == "pagan metal, irish harp"));
        assert!(matches!(get("title"), Some(SunoField::Text { ref value, .. }) if value.is_empty()));
        println!("field names after the merge: {:?}", merged.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>());
        assert_eq!(merged.len(), 4, "only the server's own names survive");
        assert!(merged.iter().all(|(name, _)| name != "negative_style"));
    }

    /// Ett arbetsytenamn är inget id: servern svarar "workspace_id is not a workspace id".
    #[test]
    fn a_workspace_name_is_not_sent_as_an_id() {
        let make = |value: &str| {
            let mut fields = fallback_fields();
            if let Some((_, SunoField::Text { value: v, .. })) =
                fields.iter_mut().find(|(name, _)| name == "workspace_id")
            {
                *v = value.to_string();
            }
            args_from_form(&fields)
        };
        let named = make("Satfläsk");
        println!("namn: {named}");
        assert!(named.get("workspace_id").is_none());
        let uuid = make("6f1c1b7e-2a44-4a9e-9d3f-1b2c3d4e5f60");
        println!("id: {uuid}");
        assert!(uuid.get("workspace_id").is_some());
    }
}