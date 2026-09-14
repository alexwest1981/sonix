//! Tempot och sträckningen — tempokartan, styckena, cachenycklarna och regionerna till motorn.
//!
//! Ett klipp delas vid varje tempobyte **i takter** (\`stretch_pieces\`), och varje stycke bär
//! sitt eget \`bpm_here\` läst ur kartan. Räkna **inte** fram tempot ur \`ratio\`: \`ratio\` är
//! ut-sekunder per källsekund (projekt/källa), och att vända den är hur en smurf tar sig in.
//!
//! Status: byggs — sträckningen; cachenyckeln byts när motorn byts (motorns version i nyckeln).
//! Rör inte: \`tempo_map\` är enda källan till takt↔tid; gå inte förbi den med egen matematik.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StretchPiece {
    /// Första ut-sekund i stycket, räknat från projektets början.
    pub out_start_secs: f64,
    /// Antal ut-sekunder i stycket.
    pub out_secs: f64,
    /// Var i källan stycket börjar (källsekunder).
    pub source_offset_secs: f64,
    /// **Ut-sekunder per källsekund** (projektets tempo delat med källans).
    pub ratio: f64,
    /// Tempot i det här stycket, läst ur kartan i takten stycket börjar på. Det står här för att
    /// den som beställer en sträckning behöver **exakt** det talet som nyckeln byggdes med —
    /// räknar anroparen fram det ur `ratio` i stället uppstår den vända enheten på nytt.
    pub bpm_here: f32,
}

pub fn stretch_pieces(
    start_bar: f64,
    length_bars: f64,
    sample_offset_secs: f64,
    source_bpm: f32,
    tempo: &crate::audio::tempo::TempoMap,
) -> Vec<StretchPiece> {
    let from_bar = start_bar;
    let to_bar = start_bar + length_bars.max(0.0) as f64;
    if to_bar <= from_bar {
        return Vec::new();
    }

    // **Skären ligger i TAKTER, inte i sekunder.** Tempokartan är skriven i takter, och en
    // sekund som ligger en hårsmån från ett byte hamnar på fel sida om det: första versionen
    // frågade kartan om `bpm_at(bar_at_secs(gräns + 1e-9))` och fick tempot *före* bytet i
    // stället för efter — provet mätte 1,0 där 0,8 var rätt. I takter är gränsen exakt.
    //
    // Ändpunkterna är inte skär: ett klipp som börjar precis på ett byte hör till det bytet.
    let mut skär: Vec<f64> = tempo
        .points()
        .iter()
        .map(|p| p.start_bar as f64)
        .filter(|b| *b > from_bar + 1e-9 && *b < to_bar - 1e-9)
        .collect();
    skär.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));

    let mut pieces = Vec::with_capacity(skär.len() + 1);
    let mut piece_bar = from_bar;
    let mut source_offset = sample_offset_secs;
    for nästa_bar in skär.into_iter().chain(std::iter::once(to_bar)) {
        let out_start_secs = tempo.secs_at_bar(piece_bar);
        let out_secs = tempo.secs_at_bar(nästa_bar) - out_start_secs;
        if out_secs <= 1e-9 {
            piece_bar = nästa_bar;
            continue;
        }
        // Tempot **i det här stycket**, läst i takten stycket börjar på — inte vid klippets start.
        let ratio = stretch_ratio_for(source_bpm, tempo.bpm_at(piece_bar)) as f64;
        let bpm_here = tempo.bpm_at(piece_bar);
        pieces.push(StretchPiece {
            out_start_secs,
            out_secs,
            source_offset_secs: source_offset,
            ratio,
            bpm_here,
        });
        // Källan flyttar sig med **ut-tiden gånger faktorn**, eftersom faktorn är ut-sekunder
        // per källsekund: en ut-sekund gör av med `ratio` källsekunder. Med konventionen i
        // motsatt riktning vore det en division — och det felet hörs som en smurf i ena änden
        // och en långsam fil i den andra.
        source_offset += out_secs * ratio;
        piece_bar = nästa_bar;
    }
    pieces
}

pub fn stretch_ratio_for(source_bpm: f32, project_bpm: f32) -> f32 {
    if source_bpm <= 0.0 || project_bpm <= 0.0 {
        return 1.0;
    }
    (project_bpm / source_bpm).clamp(MIN_STRETCH_RATIO, MAX_STRETCH_RATIO)
}

/// Ett klipps inspelningstempo, ur det som går att **mäta** (Fas 8.10).
///
/// Projekt som sparades innan fältet fanns har inget inspelningstempo (0,0 =
/// okänt), och då rörs ljudet inte. Men oftast går det att avgöra: klossens längd i
/// takter byggdes en gång ur filens längd i sekunder (`takter × 240 / bpm`), så
/// `takter × 240 / sekunder` är det tempo klossen byggdes i. Stämmer det med
/// projektets eget tempo (inom en procent) är klippet importerat i projektets eget
/// tempo — **mätt, inte gissat**.
///
/// Stämmer det inte lämnas 0,0: då är klossen trimmad, eller så har tempot ändrats
/// sedan importen, och att gissa vore att hitta på data. Användaren kan sätta
/// tempot själv i stället (⏱ Tempokarta → "låt klippen följa tempot").
///
/// **Två källor kan säga samma sak med olika tal — och då vinner geometrin.**
/// Klossens mått (`takter × 240 / filens sekunder`) är det tempo klossen är byggd
/// med, och det är den **enda** siffran som gör att en sträckt fil fyller klossen:
/// filen blir `filsekunder × källa/projekt` lång medan klossen är
/// `takter × 240/projekt`, och de möts bara när `källa = takter × 240/filsekunder`.
/// En sparad siffra inom en procent av måttet är samma tempo, och geometrin är då
/// den sanna: 0,8 % fel låter rätt i första takten och ligger **2,3 sekunder fel**
/// efter fyra minuter. Mätt på Alex' Broken 2026-09-12: klossarna bar 120,98828,
/// filerna är 254,000 s och klossarna 127 takter = **120,0000** BPM — en fil som
/// blev 2,3 s för lång vid 110, alltså hörbart ur synk.
///
/// Ligger de längre ifrån varandra står den sparade siffran kvar: då är de inte
/// samma tempo, och att byta ut den vore att gissa.
pub(crate) fn source_bpm_from_region(region: &AudioRegion, source_secs: f32, project_bpm: f32) -> f32 {
    if source_secs <= 0.0 || project_bpm <= 0.0 || region.length_bars <= 0.0 {
        return region.source_bpm.max(0.0); // inget att mäta mot: behåll det vi vet
    }
    let derived = region.length_bars as f32 * 240.0 / source_secs;
    if region.source_bpm > 0.0 {
        return if ((derived - region.source_bpm) / region.source_bpm).abs() <= 0.01 {
            derived // samma tempo, olika tal: geometrin vinner (se docen)
        } else {
            region.source_bpm
        };
    }
    if ((derived - project_bpm) / project_bpm).abs() <= 0.01 {
        project_bpm
    } else {
        0.0
    }
}

/// Tempot en kloss är **byggd i**, räknat ur dess eget mått (Fas 8.10).
///
/// `takter × 240 / filens sekunder` är det tempo klossen **lades ut i** — och det är
/// *inte* alltid filens eget tempo. **Mätt 2026-09-13 på Alex' "Rock and Hard Place"**,
/// där Suno varken gav ett BPM i arkivnamnet eller i taggarna
/// (`comment=made with suno; created=…`): alla nio klipp ger
/// `129,64 × 240 / 259,28 = 120,000` — men **ljudet går i 140** (kammätning på
/// trumstämman: 2,658 vid 140 mot 0,748 vid 120, alltså 3,6× starkare). Klippen lades
/// alltså ut i 120 medan musiken går i 140, och då är måttet 120 — layouten, inte
/// musiken. Det är skälet att talet **visas i stället för att sättas i smyg**: rätt svar
/// beror på om klippet byggdes i musikens tempo, och det vet bara användaren.
///
/// Talet är ett **mått, inte en gissning** — men det vilar på att klippet är hela filen.
/// Har någon klippt i klippet är takterna färre och talet för lågt, och då blir
/// sträckningen fel när tempot rörs. Därför sätts det aldrig i smyg: det visas i
/// ⏱ Tempokarta med sina egna siffror, och användaren trycker.
///
/// Utanför ett rimligt tempo (samma spann som tempokontrollen) är svaret `None` —
/// hellre inget svar än ett orimligt.
pub fn geometry_source_bpm(length_bars: f32, source_secs: f32) -> Option<f32> {
    if length_bars <= 0.0 || source_secs <= 0.0 {
        return None;
    }
    let bpm = length_bars * 240.0 / source_secs;
    if (40.0..=280.0).contains(&bpm) {
        Some(bpm)
    } else {
        None
    }
}

/// Vad stämpeln sätter på ett klipp: måttets tempo när det går att räkna, annars
/// projektets (Fas 8.10).
///
/// Ett klipp som börjar en bit in i filen beskriver ett **utsnitt**, inte filen: då är
/// takterna inte filens tempo, och projektets tempo är det ärliga svaret (faktorn 1,0 —
/// ingenting hörs förrän man rör tempot).
pub(crate) fn stamped_tempo(length_bars: f32, offset_sec: f32, file_secs: Option<f32>, project_bpm: f32) -> f32 {
    if offset_sec > 0.001 {
        return project_bpm;
    }
    file_secs
        .and_then(|secs| geometry_source_bpm(length_bars, secs))
        .unwrap_or(project_bpm)
}

/// Vad statusraden ska säga när tempot har ändrats (Fas 8.10).
///
/// **Tyst bortfall är det som gör en sådan här sak osynlig.** Ett klipp utan känt
/// inspelningstempo rörs inte när tempot ändras — och då ser kontrollen ut att inte göra
/// något alls. Alex drog i tempot i ett projekt där **alla** klipp saknade måttet och fick
/// ingen återkoppling alls; det här är den raden han skulle fått i stället.
///
/// Ren funktion: regeln går att pröva utan fönster, och den räknar i stället för att gissa.
pub fn tempo_change_note(following: usize, stuck: usize, measured: Option<f32>) -> Option<String> {
    if stuck == 0 {
        return None; // alla klipp följer; inget att säga
    }
    // Går tempot att räkna ur klippen ska talet stå här, inte bara en hänvisning:
    // "öppna Tempokarta" är ett steg för mycket när svaret redan är uträknat.
    if let Some(bpm) = measured {
        if following == 0 {
            return Some(crate::tstatus!(
                "ℹ Tempot ändrat, men {} klipp står still: de saknar känt inspelningstempo. Filerna ger {:.1} BPM — öppna ⏱ Tempokarta för att sätta det.",
                stuck,
                bpm
            ));
        }
        return Some(crate::tstatus!(
            "🎚 Tempot ändrat: {} klipp följer, {} står still (okänt inspelningstempo — filerna ger {:.1} BPM, se ⏱ Tempokarta).",
            following,
            stuck,
            bpm
        ));
    }
    if following == 0 {
        // Det här är fallet som ser ut som en död kontroll.
        return Some(crate::tstatus!(
            "ℹ Tempot ändrat, men {} klipp står still: de saknar känt inspelningstempo. Öppna ⏱ Tempokarta för att låta dem följa — eller importera stämmorna på nytt.",
            stuck
        ));
    }
    Some(crate::tstatus!(
        "🎚 Tempot ändrat: {} klipp följer, {} står still (okänt inspelningstempo — se ⏱ Tempokarta).",
        following,
        stuck
    ))
}

/// Sätter inspelningstempo på klipp som saknar det (Fas 8.10).
///
/// Användarens egen handling: "klippen låter som de ska nu — låt dem följa tempot
/// härifrån". Tempot tas ur klippets **eget mått** när det går att räkna (se
/// [`geometry_source_bpm`]); annars projektets, som förut.
///
/// **Rättat 2026-09-13:** förut sattes alltid projektets tempo. Det är rätt bara när
/// projektet står i det tempo klippen byggdes i — och Alex' projekt stod i **200** medan
/// klippen var byggda i **120**. En stämpel med 200 hade sagt "spelad i 200", och en
/// sänkning till 100 hade då sträckt ljudet till **halva** hastigheten. Måttet ger
/// 120,000 för alla nio stämmorna.
///
/// Returen är `(antal stämplade, tempot som sattes)` — antalet så att anroparen kan
/// säga hur många, och tempot så att den kan säga **vilket** (och varna när det inte är
/// projektets: då sträcks klippen direkt).
pub(crate) fn stamp_source_tempo<'a>(
    regions: impl Iterator<Item = &'a mut AudioRegion>,
    file_secs: Option<f32>,
    project_bpm: f32,
) -> (usize, f32) {
    let mut stamped = 0;
    let mut used = project_bpm;
    for r in regions {
        if r.source_bpm <= 0.0 {
            let bpm = stamped_tempo(r.length_bars, r.sample_offset_sec, file_secs, project_bpm);
            r.source_bpm = bpm;
            used = bpm;
            stamped += 1;
        }
    }
    (stamped, used)
}

/// Hur många sampel av spårets ljud en region täcker (Fas 8.10).
///
/// Ett klipp som spelades in i ett lägre tempo än projektets rymmer **mer** ljud än
/// den tid det tar på tidslinjen, och ett inspelat i ett högre tempo mindre. Faktorn
/// är därför med: utan den ritar vågformen, och kopierar exporten, ett annat stycke
/// än det som hörs.
pub(crate) fn region_source_span_samples(region_secs: f32, rate: f32, sample_rate: u32) -> usize {
    (region_secs.max(0.0) * rate.max(0.05) * sample_rate.max(1) as f32).max(1.0) as usize
}

/// Vad ett klipp ska spelas med, givet beslutet och om filen redan finns
/// (Fas 8.10 steg 2).
///
/// **Ren funktion**, så att regeln går att pröva utan fönster och utan ljudmotor —
/// och så att tidslinjen, ritningen och exporten får samma svar. Faktorn är
/// källsekunder per utsekund, och är 1,0 både när inget ska hända och när klippet
/// spelar en färdigsträckt fil (den *är* redan i rätt tempo).
pub fn playback_for(
    decision: crate::audio::stretch::FollowDecision,
    stretched_ready: bool,
) -> (f32, bool) {
    match decision.mode {
        crate::audio::stretch::FollowMode::Stretch => (1.0, stretched_ready),
        _ => (decision.ratio, false),
    }
}

/// Var i källjudet ett klipp börjar när det spelas från en färdigsträckt fil
/// (Fas 8.10 steg 2).
///
/// Filen är källan skalad med källa/projekt: ett utsnitt som låg 12 s in i en källa
/// i 120 BPM ligger 12 × 0,8 = 9,6 s in i filen när projektet står i 150. Utan känt
/// tempo — eller utan en sträckt fil — är offsetten oförändrad, precis som före 8.10.
pub fn stretched_offset_secs(offset_sec: f32, source_bpm: f32, bpm_here: f32) -> f32 {
    if source_bpm <= 0.0 || bpm_here <= 0.0 {
        return offset_sec;
    }
    offset_sec * (source_bpm / bpm_here)
}

/// Hur långt in i klippets ljud "hitta första slaget" letar (Fas 8.14).
///
/// Ett anslag i början av en stämma ligger inom de första sekunderna. Talet är ett tak
/// för **arbetet**, inte en åsikt om musiken: hittas inget inom fönstret säger appen
/// det i stället för att leta vidare i en hel låt efter något att flytta.
pub const FIRST_BEAT_SEARCH_SECS: f32 = 20.0;

/// Gränserna för [`stretch_ratio_for`] — samma spann som sångstudiens reglage.
pub const MIN_STRETCH_RATIO: f32 = 0.25;

pub const MAX_STRETCH_RATIO: f32 = 4.0;

pub fn frozen_region(left: &[f32], sample_rate: u32) -> crate::audio::StemRegionPlayback {
    crate::audio::StemRegionPlayback {
        start_time_secs: 0.0,
        length_secs: left.len() as f32 / sample_rate.max(1) as f32,
        sample_offset_sec: 0.0,
        gain: 1.0,
        fade_in_sec: 0.0,
        fade_out_sec: 0.0,
        muted: false,
        is_reverse: false,
        loop_length_secs: 0.0,
        // En frysning är renderad i ett visst tempo och bär det i sitt digest:
        // ändras tempot visas den som inaktuell i stället för att sträckas hit
        // och dit (Fas 8.1). Att sträcka en gammal frysning vore att spela fel
        // ljud utan att säga det.
        stretch_ratio: 1.0,
        source_audio: None,
    }
}

impl SonixApp {
/// Regionerna ett spår spelar, i sekunder, som motorn vill ha dem.
///
/// Projektets tempokarta.
///
/// I dag en enda punkt: projektets tempo. Det är avsiktligt att allt som
/// räknar tid frågar den här i stället för att läsa `bpm` själv — annars
/// Sätter ett tempobyte (Fas 8.2).
///
/// `bpm` speglar kartans första punkt så länge en karta finns. Det är en
/// avbild av sanningen, inte en andra sanning: `tempo_map()` läser kartan.
/// Skälet att spegla är att resten av appen visar `bpm`, och en siffra som
/// visar något annat än det som hörs är en lögn.
pub(crate) fn set_tempo_point(&mut self, bar: u32, bpm: f32) {
    crate::audio::tempo::set_tempo_point(&mut self.tempo_points, bar, bpm);
    self.bpm = self.tempo_map().bpm_at(0.0);
    self.status_message = crate::tstatus!("⏱ Tempobyte: Takt {} = {:.1} BPM", bar + 1, bpm);
}
}

impl SonixApp {
/// Tar bort ett tempobyte. Takt 1 går inte — den är kartans början.
pub(crate) fn remove_tempo_point(&mut self, bar: u32) {
    if crate::audio::tempo::remove_tempo_point(&mut self.tempo_points, bar) {
        self.bpm = self.tempo_map().bpm_at(0.0);
        self.status_message = crate::tstatus!("🗑 Tempobyte i takt {} borttaget", bar + 1);
    } else if bar == 0 {
        self.status_message = crate::i18n::t(
            "Tempot i takt 1 går inte att ta bort — ändra det i stället.",
        )
        .to_string();
    }
}
}

impl SonixApp {
/// uppstår den andra sanningen om takter och sekunder, och den här gången
/// blir det ingen.
/// Tempokartan som lista (Fas 8.2).
///
/// Punkterna sätts med högerklick på taktlinjalen; här syns de och kan
/// ändras eller tas bort. En rad per byte är den enklaste form som går att
/// förstå utan att någon visat den.
pub(crate) fn render_tempo_modal(&mut self, ctx: &egui::Context) {
    if !self.show_tempo_modal {
        return;
    }
    let mut open = self.show_tempo_modal;
    let mut remove: Option<u32> = None;
    let mut set: Option<(u32, f32)> = None;
    let mut stamp = false;
    egui::Window::new(crate::i18n::t("⏱ Tempokarta"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(340.0, 220.0))
        .show(ctx, |ui| {
            // Klippens tempo (Fas 8.10) står FÖRE den tidiga returen: ett
            // projekt med ett enda tempo är det vanliga fallet, och det är
            // just där frågan "varför händer inget när jag ändrar tempot?"
            // uppstår.
            let following = self.clips_with_source_tempo();
            let unknown = self.clips_without_source_tempo();
            ui.label(egui::RichText::new(crate::tstatus!(
                "🎚 Klipp som följer tempot: {} (av kända {})",
                following,
                following + unknown
            ))
            .size(11.0)
            .color(if unknown == 0 { Theme::FL_GREEN } else { Theme::TEXT_BRIGHT }));
            if unknown > 0 {
                // Tempot räknas ur klippens eget mått när det går (Fas 8.10): takter
                // och filens längd. Det syns i knappen, för det är den siffra som
                // avgör hur ljudet sträcks när tempot sedan ändras.
                let measured = self.geometry_tempo_for_unknown_clips();
                let label = match measured {
                    Some(bpm) => crate::tstatus!(
                        "🎚 Låt de {} klippen följa tempot (byggda i {:.1} BPM — takter × 240 / filernas längd)",
                        unknown,
                        bpm
                    ),
                    None => crate::tstatus!(
                        "🎚 Låt de {} klippen följa tempot (inspelningstempo {:.1} BPM)",
                        unknown,
                        self.bpm
                    ),
                };
                if ui
                    .button(label)
                    .on_hover_text(crate::i18n::t(
                        "Stämpeln säger vilket tempo klippen byggdes i — räknat ur klippens takter och filernas längd när det går, annars projektets nuvarande tempo. Är det projektets tempo hörs ingenting förrän du rör tempot. Är det ett annat (klippen byggdes i 120 men projektet står i 200) sträcks de till projektets tempo direkt, och då säger statusraden det.",
                    ))
                    .clicked()
                {
                    stamp = true;
                }
                if let Some(bpm) = measured {
                    ui.label(
                        egui::RichText::new(crate::tstatus!(
                            "🧮 Räknat ur klippens takter och filernas längd: {:.1} BPM — det tempo klippen LADES UT i. Är ett klipp klippt i är talet för lågt; hörs musiken i ett annat tempo än projektet står i, sätt projektets tempo till musikens först.",
                            bpm
                        ))
                        .size(9.5)
                        .color(Theme::TEXT_MUTED),
                    );
                }
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Klipp utan känt inspelningstempo (importerade i ett äldre projekt, eller från biblioteket) står still när tempot ändras — de sträcks aldrig i smyg.",
                    ))
                    .size(9.5)
                    .color(Theme::TEXT_MUTED),
                );
            }

            // "Följ tempot" (Fas 8.10 steg 2): projektets enda val, och ingen
            // algoritm att välja. Att stänga av den betyder att inget klipp följer
            // tempot alls — varken sträckning eller bandspelare.
            ui.separator();
            let mut follow = self.follow_tempo;
            let pending = self.stretch_cache.pending_count();
            if ui
                .checkbox(&mut follow, crate::i18n::t("🎚 Följ tempot (bevara tonhöjden)"))
                .on_hover_text(crate::i18n::t(
                    "På: klipp med känt inspelningstempo sträcks när tempot ändras, med bevarad tonhöjd. Av: inget klipp följer tempot. Ett enstaka klipp kan i stället sättas i bandspelarläge (tonhöjden följer med) i klippmenyn.",
                ))
                .changed()
            {
                self.follow_tempo = follow;
                // Ett enda val ska slå igenom direkt: motorn får nya regioner.
                for t_idx in 0..self.playlist_tracks.len() {
                    self.sync_track_regions(t_idx);
                }
                self.status_message = if follow {
                    crate::i18n::t("🎚 Klippen följer tempot med bevarad tonhöjd").to_string()
                } else {
                    crate::i18n::t("➡ Klippen står still när tempot ändras (Följ tempot av)")
                        .to_string()
                };
            }
            if pending > 0 {
                // En kontroll som ingen läser är ingen funktion — och en väntan som
                // ingen säger något om ser ut som att ingenting händer.
                ui.label(
                    egui::RichText::new(crate::tstatus!("⏳ Sträcker {} klipp …", pending))
                        .size(9.5)
                        .color(Theme::TEXT_MUTED),
                );
                ui.add_space(6.0);
            }
            if self.tempo_points.is_empty() {
                ui.label(crate::tstatus!(
                    "Projektet har ett enda tempo: {:.1} BPM.",
                    self.bpm
                ));
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Högerklicka på taktlinjalen för att sätta ett byte.",
                    ))
                    .size(9.5)
                    .color(Theme::TEXT_MUTED),
                );
                return;
            }
            ui.label(crate::i18n::t("Ett byte gäller från sin takt och framåt:"));
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for point in &self.tempo_points {
                    let bar = point.start_bar;
                    ui.horizontal(|ui| {
                        ui.label(crate::tstatus!("Takt {}", bar + 1));
                        let mut bpm = point.bpm;
                        if ui
                            .add(
                                egui::DragValue::new(&mut bpm)
                                    .speed(0.5)
                                    .range(40.0..=260.0)
                                    .suffix(" BPM"),
                            )
                            .changed()
                        {
                            set = Some((bar, bpm));
                        }
                        if bar > 0 {
                            if ui
                                .button("🗑")
                                .on_hover_text(crate::i18n::t("Ta bort bytet"))
                                .clicked()
                            {
                                remove = Some(bar);
                            }
                        } else {
                            ui.label(
                                egui::RichText::new(crate::i18n::t("(start)"))
                                    .size(9.5)
                                    .color(Theme::TEXT_MUTED),
                            );
                        }
                    });
                }
            });
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(crate::i18n::t(
                    "Högerklicka på taktlinjalen för att lägga till eller ta bort.",
                ))
                .size(9.5)
                .color(Theme::TEXT_MUTED),
            );
        });
    self.show_tempo_modal = open;
    if stamp {
        let (n, bpm) = self.stamp_unknown_source_tempo();
        // Stämpeln kan ändra vad som SPELAS: klippen byggdes i ett annat tempo än
        // projektets just nu. Motorn får nya regioner direkt i stället för först vid
        // nästa tempoändring — annars står klippets siffra och ljudet inte i samma sak.
        for t_idx in 0..self.playlist_tracks.len() {
            self.sync_track_regions(t_idx);
        }
        let away = bpm > 0.0 && (bpm - self.bpm).abs() / bpm > 0.01;
        self.status_message = if away {
            crate::tstatus!(
                "🎚 {} klipp byggdes i {:.1} BPM och projektet står i {:.1} — de sträcks till projektets tempo. Sätt tempot till {:.1} för att höra dem som de spelades in.",
                n,
                bpm,
                self.bpm,
                bpm
            )
        } else {
            crate::tstatus!(
                "🎚 {} klipp följer nu tempot ({:.1} BPM som inspelningstempo). Ändra tempot och de följer med.",
                n,
                bpm
            )
        };
    }
    if let Some((bar, bpm)) = set {
        self.set_tempo_point(bar, bpm);
    }
    if let Some(bar) = remove {
        self.remove_tempo_point(bar);
    }
}
}

impl SonixApp {
/// Ser till att spårets vågformscache finns och hör till dess nuvarande ljud.
///
/// Anropas en gång per spår och bildruta. Är cachen byggd ur samma buffert
/// som spåret har nu, kostar anropet en jämförelse av fyra tal.
/// Låter klippen följa tempot (Fas 8.10).
///
/// **Ett ställe, inte en per dörr.** Tempot ändras av reglaget, av TAP, av ett
/// tempobyte i kartan och av att ett projekt läses; en sync i varje väg vore
/// fyra chanser att glömma en. Jämförelsen kostar ett tal per bildruta.
///
/// Klipp med okänt inspelningstempo (`source_bpm == 0`) får faktorn 1,0 och
/// rörs inte — se [`stretch_ratio_for`].
pub fn sync_tempo_follow(&mut self) {
    if (self.bpm - self.stems_synced_bpm).abs() <= 0.0005 {
        return;
    }
    self.stems_synced_bpm = self.bpm;
    // Säg vad som hände — eller inte hände (Fas 8.10). Utan den här raden är ett
    // klipp utan känt tempo en kontroll som ser död ut i stället för en förklaring.
    if let Some(note) = tempo_change_note(
        self.clips_with_source_tempo(),
        self.clips_without_source_tempo(),
        self.geometry_tempo_for_unknown_clips(),
    ) {
        self.status_message = note;
    }
    for t_idx in 0..self.playlist_tracks.len() {
        let has_audio = self.playlist_tracks[t_idx].pcm_audio.is_some()
            || self.playlist_tracks[t_idx].frozen_pcm.is_some();
        if has_audio {
            self.sync_track_regions(t_idx);
        }
    }
}
}

impl SonixApp {
/// Hur många klipp som **har** ett känt inspelningstempo (och alltså följer
/// med i stället för att stå still).
pub fn clips_with_source_tempo(&self) -> usize {
    self.playlist_tracks
        .iter()
        .flat_map(|t| t.regions.iter())
        .filter(|r| r.source_bpm > 0.0)
        .count()
}
}

impl SonixApp {
/// Hur många klipp som saknar känt inspelningstempo — de som står still när
/// tempot ändras, och som användaren kan sätta ett tempo på.
pub fn clips_without_source_tempo(&self) -> usize {
    self.playlist_tracks
        .iter()
        .flat_map(|t| t.regions.iter())
        .filter(|r| r.source_bpm <= 0.0)
        .count()
}
}

impl SonixApp {
/// "Klippen låter som de ska nu — låt dem följa tempot härifrån" (Fas 8.10).
///
/// Tempot tas ur klippets eget mått när det går att räkna (filens längd och
/// klippets takter, se [`geometry_source_bpm`]) — det är det tempo klossen byggdes
/// i, och därmed det tal en tempoändring ska mätas mot. Saknas måttet sätts
/// projektets tempo, som förut.
///
/// Returen är `(antal klipp, tempot som sattes)` så att anroparen kan säga **vilket**
/// tempo som gavs i stället för att bara säga hur många.
pub fn stamp_unknown_source_tempo(&mut self) -> (usize, f32) {
    let fallback = self.bpm;
    let mut stamped = 0usize;
    let mut used = fallback;
    for t in self.playlist_tracks.iter_mut() {
        let file_secs = t
            .pcm_audio
            .as_ref()
            .map(|(l, _, sr)| l.len() as f32 / (*sr).max(1) as f32);
        let (n, bpm) = stamp_source_tempo(t.regions.iter_mut(), file_secs, fallback);
        stamped += n;
        if n > 0 {
            used = bpm;
        }
    }
    (stamped, used)
}
}

impl SonixApp {
/// Måttets tempo för klippen som saknar inspelningstempo (Fas 8.10).
///
/// Ett tal bara när klippen **säger samma sak**: stämmorna ur samma låt är byggda i
/// samma tempo, så nio klipp som ger 120,000 är ett mått (Alex' Rock and Hard Place).
/// Ger de olika tal är projektet blandat — eller något klipp klippt — och då visas
/// inget: hellre ingen siffra än en siffra som stämmer på en del av klippen.
pub fn geometry_tempo_for_unknown_clips(&self) -> Option<f32> {
    let mut values: Vec<f32> = Vec::new();
    for t in &self.playlist_tracks {
        let Some((l, _, sr)) = t.pcm_audio.as_ref() else {
            continue;
        };
        let file_secs = l.len() as f32 / (*sr).max(1) as f32;
        for r in &t.regions {
            if r.source_bpm > 0.0 || r.sample_offset_sec > 0.001 {
                continue;
            }
            if let Some(bpm) = geometry_source_bpm(r.length_bars, file_secs) {
                values.push(bpm);
            }
        }
    }
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = values[values.len() / 2];
    if values.iter().all(|v| (v - median).abs() / median <= 0.01) {
        Some(median)
    } else {
        None
    }
}
}

impl SonixApp {
/// Vad motorn ska spela för ett klipp just nu (Fas 8.10 steg 2).
///
/// **Ett ställe.** Tidslinjen, ritningen, "spara region som sample" och exporten
/// frågar samma funktion — annars kan filen och högtalarna säga olika saker, och
/// den fällan har kostat tid två gånger redan (8.5 och sidokedjan).
///
/// Tre svar, och bara tre (se [`crate::audio::stretch::decide`]):
/// - **Untouched** — okänt tempo eller switchen av: faktorn är 1,0.
/// - **Tape** — klippet vill ha bandspelarljudet: projekt/källa, tonhöjden följer.
/// - **Stretch** — en färdigsträckt fil spelas med faktor 1,0. Är den inte klar
///   (eller avvisades) spelas originalet i **sitt eget tempo**: ingen smurf uppstår
///   av misstag, och statusraden säger vad som väntar.
fn region_playback(
    &self,
    r: &AudioRegion,
    bpm_here: f32,
) -> (f32, Option<crate::audio::stretch::StretchedAudio>) {
    let decision =
        crate::audio::stretch::decide(r.source_bpm, bpm_here, self.follow_tempo, r.tape);
    let audio = if decision.mode == crate::audio::stretch::FollowMode::Stretch {
        self.stretched_audio_for(r, bpm_here)
    } else {
        None
    };
    let (rate, _) = playback_for(decision, audio.is_some());
    (rate, audio)
}
}

impl SonixApp {
/// Nyckeln en sträckning har i cachen: källans sökväg **och båda tempon**.
///
/// Ett enda ställe som bygger den, så att uppslagningen och beställningen inte
/// kan glida ifrån varandra.
fn stretch_key_for(r: &AudioRegion, bpm_here: f32) -> Option<String> {
    let path = r.source_path.as_deref()?;
    if path.is_empty() {
        return None; // ingen källa på disk: det finns inget att sträcka
    }
    Some(crate::audio::stretch::cache_key(path, r.source_bpm, bpm_here))
}
}

impl SonixApp {
/// Den färdigsträckta filen för ett klipp, om den finns och godkändes.
fn stretched_audio_for(
    &self,
    r: &AudioRegion,
    bpm_here: f32,
) -> Option<crate::audio::stretch::StretchedAudio> {
    let key = Self::stretch_key_for(r, bpm_here)?;
    self.stretch_cache.get(&key).cloned()
}
}

impl SonixApp {
/// Beställer sträckningen av de klipp som behöver den (Fas 8.10 steg 2).
///
/// Anropas en gång per bildruta. Tempot måste ha stått still en stund först:
/// reglaget byter värde varje bildruta medan det dras, och varje värde är en
/// egen fil att räkna fram — arbete ingen hann höra.
pub fn ensure_stretched(&mut self) {
    // Först: ta emot vad arbetstråden blivit klar med. Ett klipp som blivit
    // sträckt ska spelas om direkt, annars hörs det först nästa gång tempot rörs.
    let finished = self.stretch_cache.poll();
    if !finished.is_empty() {
        let failed: Vec<String> = finished
            .iter()
            .filter_map(|k| self.stretch_cache.reason(k).map(|r| r.to_string()))
            .collect();
        for t_idx in 0..self.playlist_tracks.len() {
            self.sync_track_regions(t_idx);
        }
        // **Mätt i motorn, inte antaget** (Fas 8.10c): hur många spår spelar
        // sträckt ljud *efter* omsynken? Kommandot kan ha fallit bort på vägen —
        // och då ska det sägas högt, för symptomet ("ingen skillnad på tempot")
        // är detsamma som när allt fungerar.
        let sent = self
            .playlist_tracks
            .iter()
            .filter(|t| {
                self.stem_regions_for(t)
                    .iter()
                    .any(|r| r.source_audio.is_some())
            })
            .count() as u32;
        let engine_play = self.engine.stretched_track_count();
        self.status_message = if !failed.is_empty() {
            // 8.5-regeln: säg vad som gick fel i stället för att tiga. Originalet
            // spelar under tiden — ett besked, inte en tystnad.
            crate::tstatus!(
                "⚠ Kunde inte sträcka {} klipp: {} — originalet spelar.",
                failed.len(),
                failed[0]
            )
        } else if engine_play < sent {
            crate::tstatus!(
                "⚠ Sträckningen nådde inte motorn: {} av {} spår spelar sträckt ljud — originalet spelar för resten.",
                engine_play,
                sent
            )
        } else {
            crate::tstatus!(
                "🎚 {} klipp sträckta till {:.1} BPM (tonhöjden bevarad).",
                finished.len(),
                self.bpm
            )
        };
    }

    if !self
        .tempo_settle
        .tick(self.bpm, crate::audio::stretch::TEMPO_SETTLE_FRAMES)
    {
        return;
    }

    // Vilka klipp behöver en sträckning, och vilka svar saknas?
    let mut wanted: Vec<(String, String, f32, f32)> = Vec::new();
    for t in &self.playlist_tracks {
        for r in &t.regions {
            // **Ett stycke per tempovärde** (8.10 punkt 1): ett klipp som spänner över ett byte
            // behöver en sträckning per avsnitt, inte en för hela spannet. För ett klipp inom
            // ett enda tempo blir det **exakt ett** stycke med dagens nyckel — alltså ingen
            // ändring, ingen extra rendering och inget arbete ingen hör.
            for piece in stretch_pieces(
                r.start_bar as f64,
                r.length_bars as f64,
                r.sample_offset_sec as f64,
                r.source_bpm,
                &self.tempo_map(),
            ) {
                let bpm_here = piece.bpm_here;
                let decision = crate::audio::stretch::decide(
                    r.source_bpm,
                    bpm_here,
                    self.follow_tempo,
                    r.tape,
                );
                if decision.mode != crate::audio::stretch::FollowMode::Stretch {
                    continue;
                }
                let Some(key) = Self::stretch_key_for(r, bpm_here) else {
                    continue;
                };
                if self.stretch_cache.knows(&key) {
                    continue;
                }
                wanted.push((
                    key,
                    r.source_path.clone().unwrap_or_default(),
                    r.source_bpm,
                    bpm_here,
                ));
            }
        }
    }
    if wanted.is_empty() {
        return;
    }
    let dir = crate::paths::paths().stretch_cache_dir();
    let count = wanted.len();
    for (key, source_path, source_bpm, project_bpm) in wanted {
        self.stretch_cache
            .request(&dir, key, source_path, source_bpm, project_bpm);
    }
    self.status_message = crate::tstatus!(
        "⏳ Sträcker {} klipp till {:.1} BPM — originalet spelar under tiden.",
        count,
        self.bpm
    );
}
}

impl SonixApp {
pub(crate) fn tempo_map(&self) -> crate::audio::tempo::TempoMap {
    if self.tempo_points.is_empty() {
        return crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
    }
    crate::audio::tempo::TempoMap::from_points(self.tempo_points.clone())
}
}

impl SonixApp {
/// **Ett ställe för omräkningen takter→sekunder.** Tre funktioner gjorde
/// samma sak förut (två synkvägar och exporten), vilket är hur två svar på
/// samma fråga uppstår. Nu går de genom tempokartan (Fas 8.2), så att en
/// framtida tempokarta slår igenom i uppspelning och export samtidigt.
///
/// Ett fruset spår är ETT långt ljud från början; ett ljudspår har sina egna
/// regioner. Längder räknas som längder (inte som en differens av två
/// positioner), eftersom en kloss kan sträcka sig över ett tempobyte.
pub(crate) fn stem_regions_for(&self, t: &PlaylistTrack) -> Vec<StemRegionPlayback> {
    let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
    let mut regions: Vec<StemRegionPlayback> = Vec::new();
    if let Some((l, _r, sr)) = t.frozen_pcm.as_ref() {
        regions.push(frozen_region(l, *sr));
    }
    regions.extend(t.regions.iter().map(|r| {
        let from = r.start_bar as f64;
        let bpm_here = tempo.bpm_at(from);
        // Faktorn och källjudet kommer från SAMMA beslut (Fas 8.10 steg 2): en
        // färdigsträckt fil spelas med faktor 1,0, bandspelarläget med faktorn,
        // och okänt tempo med 1,0. Ritningen och exporten frågar samma funktion.
        let (rate, source_audio) = self.region_playback(r, bpm_here);
        // Är filen sträckt ligger utsnittet på en annan sekund i den: filen är
        // källan skalad med källa/projekt. Utan en sträckt fil är offsetten
        // oförändrad — precis som före 8.10.
        let sample_offset_sec = if source_audio.is_some() {
            stretched_offset_secs(r.sample_offset_sec, r.source_bpm, bpm_here)
        } else {
            r.sample_offset_sec
        };
        StemRegionPlayback {
            start_time_secs: tempo.secs_at_bar(from) as f32,
            length_secs: tempo.secs_for_bars_at(from, r.length_bars as f64) as f32,
            sample_offset_sec,
            source_audio,
            gain: r.volume,
            fade_in_sec: tempo.secs_for_bars_at(from, r.fade_in_bars as f64) as f32,
            fade_out_sec: tempo.secs_for_bars_at(from, r.fade_out_bars as f64) as f32,
            muted: r.muted,
            is_reverse: r.is_reverse,
            loop_length_secs: tempo.secs_for_bars_at(from, r.loop_length_bars as f64) as f32,
            // Faktorn kommer från tempot vid klossens START: en kloss som
            // sträcker sig över ett tempobyte får en faktor, inte en kurva —
            // kvar att lösa (8.10).
            stretch_ratio: rate,
        }
    }));
    regions
}
}

impl SonixApp {
pub fn sync_track_regions(&mut self, track_idx: usize) {
    if track_idx < self.playlist_tracks.len() {
        let region_playbacks = self.stem_regions_for(&self.playlist_tracks[track_idx]);
        let _ = self.engine.send_command(AudioCommand::SetStemTrackRegions {
            track_index: track_idx,
            regions: region_playbacks,
        });
    }
}
}

impl SonixApp {
pub fn sync_track_stem_to_engine(&mut self, track_idx: usize) {
    if track_idx < self.playlist_tracks.len() {
        let t = &self.playlist_tracks[track_idx];
        // Ett fruset spår har sitt ljud i frozen_pcm och sin region ur
        // bufferten. Utan det här skulle den här funktionen — som körs vid
        // varje uppspelningsstart — skicka en TOM regionlista för spåret och
        // tysta frysningen.
        let frozen_pcm = t.frozen_pcm.clone();
        if let Some((l, r, sr)) = frozen_pcm.as_ref().or(t.pcm_audio.as_ref()) {
            let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
                track_index: track_idx,
                left: l.clone(),
                right: r.clone(),
                sample_rate: *sr as f32,
                volume: t.volume,
                pan: t.pan,
                start_time_secs: 0.0,
            });
        }
        let stem_regions = self.stem_regions_for(t);
        let _ = self.engine.send_command(AudioCommand::SetStemTrackRegions {
            track_index: track_idx,
            regions: stem_regions,
        });
        let _ = self.engine.send_command(AudioCommand::SetStemTrackState {
            track_index: track_idx,
            volume: t.volume,
            pan: t.pan,
            muted: t.muted,
            solo: t.solo,
        });
        let _ = self.engine.send_command(AudioCommand::SetStemTrackRouting {
            track_index: track_idx,
            bus: t.bus,
            vca: t.vca,
        });
        let _ = self.engine.send_command(AudioCommand::SetStemTrackSidechain {
            track_index: track_idx,
            from: t.sidechain_from,
            amount_db: t.sidechain_amount_db,
            threshold_db: t.sidechain_threshold_db,
        });
        // Sends (Fas 8.13): samma ställe som sidokedjan, av samma skäl —
        // en omsynk efter ett spårbyte eller en ångring ska höras.
        let _ = self.engine.send_command(AudioCommand::SetStemTrackSends {
            track_index: track_idx,
            sends: t.sends.clone(),
        });
        let _ = self.engine.send_command(AudioCommand::SetTrackEq {
            track_index: track_idx,
            settings: t.eq.to_settings(),
        });
        let _ = self.engine.send_command(AudioCommand::SetTrackMix {
            track_index: track_idx,
            comp_threshold_db: t.comp_threshold_db,
            comp_ratio: t.comp_ratio,
            reverb_send: t.reverb_send,
            delay_send: t.delay_send,
            pitch_semitones: t.pitch_semitones,
        });
    }
}
}

impl SonixApp {
pub fn sync_all_stems_to_engine(&mut self) {
    for idx in 0..self.playlist_tracks.len() {
        self.sync_track_stem_to_engine(idx);
    }
}
}

impl SonixApp {
/// Re-render one stem through the pitch-preserving time-stretch and reload
/// it into the engine. Runs from the original (un-stretched) audio so the
/// SPEED knob can be adjusted repeatedly without compounding artefacts.
pub fn apply_stem_time_stretch(&mut self, idx: usize) {
    let sr = self.stem_project.sample_rate as f32;
    if idx >= self.stem_project.stems.len()
        || idx >= self.stem_project.stem_audio.len()
        || idx >= self.stem_project.stem_audio_base.len()
    {
        return;
    }
    let ratio = self.stem_project.stems[idx].time_stretch;
    let base = &self.stem_project.stem_audio_base[idx];
    let left = crate::audio::vocal_harmonizer::time_stretch(&base.left, sr, ratio);
    let right = crate::audio::vocal_harmonizer::time_stretch(&base.right, sr, ratio);
    self.stem_project.stem_audio[idx] = crate::audio::stem_separator::StemAudio { left, right };

    let audio = self.stem_project.stem_audio[idx].clone();
    let ch = &self.stem_project.stems[idx];
    let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
        track_index: idx,
        left: std::sync::Arc::new(audio.left),
        right: std::sync::Arc::new(audio.right),
        sample_rate: sr,
        volume: ch.volume,
        pan: ch.pan,
        start_time_secs: 0.0,
    });
    let _ = self.engine.send_command(AudioCommand::SetStemTrackState {
        track_index: idx,
        volume: ch.volume,
        pan: ch.pan,
        muted: ch.muted,
        solo: ch.solo,
    });
}
}

impl SonixApp {
pub fn tap_tempo(&mut self) {
    let now = Instant::now();
    self.tap_tempo_times.retain(|&t| now.duration_since(t).as_secs_f32() < 3.0);
    self.tap_tempo_times.push(now);
    if self.tap_tempo_times.len() >= 2 {
        let n = self.tap_tempo_times.len() - 1;
        let total_time: f32 = (0..n)
            .map(|i| self.tap_tempo_times[i + 1].duration_since(self.tap_tempo_times[i]).as_secs_f32())
            .sum();
        let avg_interval = total_time / n as f32;
        if avg_interval > 0.15 && avg_interval < 2.5 {
            let calculated_bpm = (60.0 / avg_interval).clamp(40.0, 260.0);
            self.bpm = (calculated_bpm * 10.0).round() / 10.0;
            self.status_message = crate::tstatus!("🎯 Tap Tempo: {:.1} BPM ({} tryck)", self.bpm, self.tap_tempo_times.len());
        }
    }
}
}

impl SonixApp {
/// Re-sends all persistent synth/mixer state after the output stream has
/// been rebuilt (e.g. a sample-rate change), because `reconfigure()`
/// recreates the `SynthEngine` from scratch.
pub(crate) fn resync_engine_after_reconfigure(&mut self) {
    self.monitor_ring_sent = None;
    self.monitor_level_sent = None;
    let _ = self.engine.send_command(AudioCommand::SetWaveform(self.waveform));
    let _ = self.engine.send_command(AudioCommand::SetAdsr(self.adsr));
    let _ = self.engine.send_command(AudioCommand::SetFilter(self.filter));
    let _ = self.engine.send_command(AudioCommand::SetFilterEnv {
        amount: self.filter_env_amount,
        adsr: self.filter_env,
    });
    let _ = self.engine.send_command(AudioCommand::SetDelay(self.delay));
    let _ = self.engine.send_command(AudioCommand::SetReverb(self.reverb));
    let _ = self.engine.send_command(AudioCommand::SetDrive(self.drive));
    let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
    let _ = self.engine.send_command(AudioCommand::SetRemixFx { mode: 0, bpm: self.bpm });
    self.last_patcher_spec = None;
    self.sync_patcher_graph();
    let _ = self.engine.send_command(AudioCommand::SetPatcherEnabled(self.patcher_enabled));
    self.is_playing = false;
    if self.view_mode == ViewMode::StemSeparator && !self.stem_project.stem_audio.is_empty() {
        self.load_separated_stems_to_engine();
    } else {
        self.sync_all_stems_to_engine();
    }
}
}

