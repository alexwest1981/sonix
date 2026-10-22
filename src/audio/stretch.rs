//! Tempoföljning med bevarad tonhöjd (Fas 8.10, steg 2).
//!
//! **Problemet.** Steg 1 gjorde klippen till bandspelare: när projektets BPM ändras följer
//! tonhöjden med, och en sångröst går från Leonard Cohen till smurfarna. Det här modulen
//! räknar i stället om ljudet med **bevarad tonhöjd**.
//!
//! **Varför offline till en fil och inte per sample i ljudtråden.** Beslutet är fattat med
//! researchunderlag (ROADMAP § 8.10 och `references/daw-research/08`–`10`):
//!
//! - Ingen av de undersökta DAW:erna lägger tempo-stretch på en hostad plugin i
//!   uppspelningsvägen; mönstret är inbäddad licensierad DSP eller egen DSP, och Pro Tools'
//!   tyngsta läge (X-Form) är uttryckligen *Rendered Only*.
//! - Att stretcha per sample i ljudtråden är dyrt (WSOLA-sökning per grain), allokerar och
//!   kan ge xrun. Att räkna **en gång** och spela en vanlig fil ger ingen ny felkälla i
//!   uppspelningen alls — och resultatet går att **verifiera** (längd, ändliga sampel, inte
//!   tyst) innan det används.
//!
//! Filen skrivs **atomiskt** (temp + rename): en halvskriven cache får aldrig kunna läsas som
//! ett giltigt klipp.
//!
//! **Formanter är inte problemet.** En korrekt pitch-bevarande tidsskalning flyttar inte
//! formanterna — formant-reglage hör till *pitch-shift*. Det som återstår efter en korrekt
//! sträckning är motorns artefakter. Läs `references/daw-research/10` innan du byter motor.

use std::path::{Path, PathBuf};

/// Samma spann som sångstudiens reglage och `stretch_ratio_for` i `app.rs`.
pub const MIN_RATIO: f32 = 0.25;
pub const MAX_RATIO: f32 = 4.0;

/// **Taket för den automatiska följningen** — en faktor per klipp som tempot får följas med
/// utan att någon har bett om det.
///
/// Spannet är en oktav i tempo: 0,5× (projektet står i halva källans tempo) till 2,0×.
/// Innanför det ligger allt en människa menar med "samma låt i ett annat tempo" —
/// halv- och dubbeltempo inräknat. Utanför är det inte längre en tempoföljning utan en
/// omritning av musiken, och den ska vara ett **aktivt val** (sångstudiens reglage eller
/// bandspelarläget per klipp), inte något som händer för att ett reglage står fel.
///
/// **Mätt 2026-09-20, Alex' "Under Vintergatan"** — stämmorna byggda i 163 BPM, projektet i
/// 40 (= 4,075×, alltså precis utanför): appen sträckte då **varje** stämma till klämningen
/// 4,0×, skrev **486 MB per stämma** (de 21 filerna i den familjen ligger på **8,4 GB** i
/// `~/.cache/sonix/stretch` — hela cachen är 17 GB — och varje stämma blev 21 minuter i
/// stället för 5:17) och spelade det. Det som hörs är samma musik
/// fyra gånger långsammare — och ovanpå den en **kornrepeterad drill**: vågformen i den
/// sträckta filen har en topp i autokorrelationen på 42,7 ms (0,78 mot 0,02 för originalet),
/// alltså kornet upprepat i samma takt. Sträckningen är inte trasig; den är bara inte musik.
///
/// Klippet **spelar som inspelat** i stället (faktor 1,0), och [`crate::ui::app::stretch`]
/// säger varför i statusraden. Klämningen till [`MIN_RATIO`]–[`MAX_RATIO`] finns kvar för
/// den som sträcker något med flit.
pub const FOLLOW_MIN_RATIO: f32 = 0.5;
pub const FOLLOW_MAX_RATIO: f32 = 2.0;

/// Vad som ska räknas fram för ett klipp.
///
/// **Två faktorer, och de är varandras inverterade.** Det här är den fälla som
/// gjorde att planen först räknade filens längd åt fel håll: motorn mäter
/// *källsekunder per utsekund* (projekt/källa), medan filen mäts som *ut-tid
/// genom in-tid* (källa/projekt). Vid ett höjt tempo ska klippet bli **kortare**
/// — och då måste filen bli kortare, inte längre. En fil som blev längre och
/// ändå spelades med faktor 1,0 i ett kortare klipp vore en smurf, alltså precis
/// det 8.10 steg 2 finns för att ta bort.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StretchPlan {
    /// **Motorns faktor: källsekunder per utsekund = projekt/källa.**
    /// 1,25 = klippet spelas 25 % snabbare och tar 20 % kortare tid på tidslinjen.
    /// Samma tal som `stretch_ratio_for` ger motorn i bandspelarläget.
    pub playback_ratio: f32,
    /// **Filens faktor: ut-tid / in-tid = källa/projekt** = `1.0 / playback_ratio`.
    /// 0,8 = den sträckta filen blir 20 % kortare än källan — precis så lång som
    /// klippet är på tidslinjen vid det nya tempot.
    pub file_ratio: f32,
    /// Antal stereoframes den färdiga filen ska ha.
    pub out_frames: usize,
}

impl StretchPlan {
    /// Är planen en faktisk ändring? (Faktor 1,0 ska aldrig rendera något.)
    pub fn changes_anything(&self) -> bool {
        (self.playback_ratio - 1.0).abs() > 1e-4
    }
}

/// Räknar fram vad ett klipp behöver, eller `None` när inget ska göras.
///
/// **`None` är ett svar, inte ett fel.** Det betyder "rör inte ljudet", och det gäller:
///
/// - **okänt inspelningstempo** (`source_bpm <= 0`) — 8.10:s regel: en källa utan känt tempo
///   sträcks inte i smyg,
/// - **samma tempo** (`ratio ≈ 1,0`) — då är vägen bit-exakt som förut och ingen fil behövs,
/// - **tomt klipp** — det finns inget att sträcka.
///
/// Faktorn kläms till [`MIN_RATIO`]–[`MAX_RATIO`], samma spann som sångstudiens reglage.
pub fn plan(source_bpm: f32, project_bpm: f32, source_frames: usize) -> Option<StretchPlan> {
    if source_frames == 0 || source_bpm <= 0.0 || project_bpm <= 0.0 {
        return None;
    }
    let playback_ratio = (project_bpm / source_bpm).clamp(MIN_RATIO, MAX_RATIO);
    if (playback_ratio - 1.0).abs() <= 1e-4 {
        return None;
    }
    // Filen skalas ned när tempot går upp: klippet blir kortare, alltså måste
    // ljudet rymmas i mindre tid. Spannet är detsamma, bara inverterat.
    let file_ratio = 1.0 / playback_ratio;
    let out_frames = ((source_frames as f64) * (file_ratio as f64))
        .round()
        .max(1.0) as usize;
    Some(StretchPlan {
        playback_ratio,
        file_ratio,
        out_frames,
    })
}

/// Vad som ska hända med **ett** klipp när tempot rör sig.
///
/// Tre utfall, och bara tre — det är hela användarvända regeln bakom "ett dumhuvud ska klara
/// det". Ingen algoritm väljs manuellt; klippets typ och switchen avgör.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FollowMode {
    /// Ljudet rörs inte: okänt inspelningstempo, samma tempo, eller switchen av.
    Untouched,
    /// Bandspelaren: tonhöjden följer med. Ett **aktivt** val per klipp (den som vill ha
    /// effekten), aldrig standard — det är smurfen.
    Tape,
    /// Tonhöjdsbevarande sträckning till en fil, spelad med faktor 1,0.
    Stretch,
}

/// Beslutet för ett klipp, med faktorn när en sträckning behövs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FollowDecision {
    pub mode: FollowMode,
    /// Ut-tid / in-tid. 1,0 när inget ska hända.
    pub ratio: f32,
}

/// Regeln: **följ tempot, bevara tonhöjden** — utom där klippet självt säger tape.
///
/// Ordningen är vald så att inget kan hända av misstag:
/// 1. **Okänt inspelningstempo** (`source_bpm <= 0`) → rör inte ljudet (8.10:s regel).
/// 2. **Switchen av** → rör inte ljudet. "Följ tempot" är en switch, inte en halv.
/// 3. **Samma tempo** → inget att göra (faktor 1,0, bit-exakt väg).
/// 4. **Klippet vill bandspelare** (`per_clip_tape`) → [`FollowMode::Tape`], tonhöjden följer.
///    Det är ett aktivt val per klipp och **kostar ingenting** (ingen fil räknas fram), så
///    det kläms som förut i stället för att ha ett tak.
/// 5. **Mer än dubbelt från källans tempo** (utanför [`FOLLOW_MIN_RATIO`]–[`FOLLOW_MAX_RATIO`])
///    → rör inte ljudet. En 4×-sträckning är ett tekniskt genomförbart svar på en fråga ingen
///    ställt, och den kostar hundratals MB per klipp. Se konstantens doc.
/// 6. Annars → [`FollowMode::Stretch`] med projekt/källa.
pub fn decide(
    source_bpm: f32,
    project_bpm: f32,
    follow_tempo: bool,
    per_clip_tape: bool,
) -> FollowDecision {
    let untouched = FollowDecision {
        mode: FollowMode::Untouched,
        ratio: 1.0,
    };
    if source_bpm <= 0.0 || project_bpm <= 0.0 || !follow_tempo {
        return untouched;
    }
    let raw = (project_bpm / source_bpm).clamp(MIN_RATIO, MAX_RATIO);
    if (raw - 1.0).abs() <= 1e-4 {
        return untouched;
    }
    if per_clip_tape {
        return FollowDecision {
            mode: FollowMode::Tape,
            ratio: raw,
        };
    }
    // **Taket** (se [`FOLLOW_MIN_RATIO`]): ovanför det spelar klippet som inspelat.
    if beyond_follow_band(source_bpm, project_bpm) {
        return untouched;
    }
    FollowDecision {
        mode: FollowMode::Stretch,
        ratio: raw,
    }
}

/// Ligger tempot utanför taket för automatisk följning? (Se [`FOLLOW_MIN_RATIO`].)
///
/// **Ren funktion, och samma regel som [`decide`] dömer med** — appen frågar den för att
/// kunna säga *varför* ett klipp står still i stället för att gissa (statusraden, och
/// räknaren i ⏱ Tempokarta). Två ställen med samma regel skriver isär; därför är det här
/// den enda.
pub fn beyond_follow_band(source_bpm: f32, project_bpm: f32) -> bool {
    if source_bpm <= 0.0 || project_bpm <= 0.0 {
        return false; // okänt tempo är ett annat svar (rör inte ljudet, 8.10)
    }
    !(FOLLOW_MIN_RATIO..=FOLLOW_MAX_RATIO).contains(&(project_bpm / source_bpm))
}

/// **Följer klippet tempot?** — frågan som **räknarna** ställer: hänger klippet med när
/// tempot ändras (och sträcks eller bandas det då)?
///
/// Skillnaden mot [`decide`] är **ett** fall, och den är avsiktlig: ett klipp som redan ligger
/// i projektets tempo får `Untouched` av `decide` — det ska inte sträckas *nu* — men det
/// **följer** med så fort tempot rör sig. Att räkna "hur många klipp följer tempot" med
/// `decide` direkt kallar därför tolv stämmor i projektets eget tempo för stillastående, och
/// det var den sortens tal som sade "tolv klipp följer med" om ett projekt där inget gjorde
/// det. `follows_tempo_agrees_with_decide` håller de två svaren ihop.
///
/// Ett klipp i bandspelarläge följer alltid: tonhöjden får följa med, det är hela valet.
pub fn follows_tempo(
    source_bpm: f32,
    project_bpm: f32,
    follow_tempo: bool,
    per_clip_tape: bool,
) -> bool {
    if source_bpm <= 0.0 || project_bpm <= 0.0 || !follow_tempo {
        return false; // okänt tempo eller avstängd switch: ljudet rör sig inte (8.10)
    }
    if per_clip_tape {
        return true;
    }
    !beyond_follow_band(source_bpm, project_bpm)
}

/// **Signalsmith Stretch som renderingsmotor** (Fas 8.10 steg 2).
///
/// Vår egen WSOLA lägger till transienter som inte finns i källan. Mätt på Alex' Broken-
/// stämmor 2026-09-13 (120 → 110 BPM, det tydliga provet): **+9,8 % / +9,5 % / +13,6 % /
/// +11,0 %** för sång, bakgrundssång, trummor och bas. Det är de falska anslagen som hörs
/// som skorr. Två försök att laga vår egen sökning blev **sämre** och rullades tillbaka —
/// därför prövas i stället `signalsmith-stretch` (MIT), motorn branschen använder för det
/// här, och **samma prov avgör**: blir det inte bättre behåller vi vår egen.
///
/// **Bara renderingen.** Realtidsvägen i Sångstudion ligger kvar på vår egen WSOLA — den
/// sträcker strömmande ljud, där en fil som räknas en gång inte är ett alternativ.
///
/// **Kanalkartan:** biblioteket tar **interleaved** in och ut (L,R,L,R …), så kanalerna
/// läggs i varandra och plockas isär igen. Att skicka två monolistor som om de vore en
/// kanal är den klassiska förväxlingen — stereobilden skulle kollapsa till mono.
///
/// Vid faktor 1,0 returneras ingången oförändrad: ingen process, ingen förlust.
///
/// **Test-kod med avsikt:** motorn förlorade domen (se roadmapens 8.10) och får inte ligga
/// i produktionsvägen — men A/B:et ska kunna göras om. Provet är `the_two_engines_on_the_same_stems`.
#[cfg(test)]
pub fn stretch_signalsmith(
    left: &[f32],
    right: &[f32],
    ratio: f32,
    sample_rate: f32,
) -> (Vec<f32>, Vec<f32>) {
    stretch_signalsmith_with(
        left,
        right,
        ratio,
        sample_rate,
        SIGNALSMITH_BLOCK_MS,
        SIGNALSMITH_INTERVAL_MS,
    )
}

/// Blocklängd och intervall för Signalsmith (millisekunder) — **mätt fram**, inte gissat.
///
/// Bibliotekets förval är 120/30 ms, och det är gjort för musik i största allmänhet. På en
/// **klickföljd** smetade det ut 24 anslag till 52 (skyddsprovet
/// `the_engine_does_not_invent_transients` fällde det direkt): en lång FFT-ruta med glest
/// intervall hinner inte följa en transient. Svepet som valde talen står i
/// `the_block_and_interval_sweep` — korta intervall köper transienter, långa rutor köper
/// tonal jämnhet, och tabellen avgjorde avvägningen. 40/10 ms är vad svepet lämnade:
/// förvalet 120/30 smetade ut 24 klick till 44 anslag, 120/10 till 91, medan **40/10 gav
/// 24 — noll överskott** — och 20/5 gav samma sak till lägre tonal marginal.
#[cfg(test)]
pub const SIGNALSMITH_BLOCK_MS: f32 = 40.0;
#[cfg(test)]
pub const SIGNALSMITH_INTERVAL_MS: f32 = 10.0;

/// Som [`stretch_signalsmith`], men med givna block- och intervalltal (för mätningen).
#[cfg(test)]
pub fn stretch_signalsmith_with(
    left: &[f32],
    right: &[f32],
    ratio: f32,
    sample_rate: f32,
    block_ms: f32,
    interval_ms: f32,
) -> (Vec<f32>, Vec<f32>) {
    let frames = left.len().min(right.len());
    if frames == 0 || sample_rate <= 0.0 {
        return (Vec::new(), Vec::new());
    }
    let ratio = ratio.clamp(MIN_RATIO, MAX_RATIO);
    if (ratio - 1.0).abs() <= 1e-4 {
        return (left[..frames].to_vec(), right[..frames].to_vec());
    }
    let out_frames = ((frames as f64) * (ratio as f64)).round().max(1.0) as usize;

    let mut interleaved = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        interleaved.push(left[i]);
        interleaved.push(right[i]);
    }
    let mut out = vec![0.0f32; out_frames * 2];

    let block = ((block_ms / 1000.0) * sample_rate).round().max(4.0) as usize;
    let interval = ((interval_ms / 1000.0) * sample_rate).round().max(1.0) as usize;
    let interval = interval.min(block.saturating_sub(1)).max(1);
    let mut stretch = signalsmith_stretch::Stretch::new(2, block, interval);
    if !stretch.exact(&interleaved, &mut out) {
        // 8.5-regeln: hellre tomt än påhittat. `check_rendered` avvisar det, och appen
        // säger ifrån i stället för att spela något som inte är filen.
        return (Vec::new(), Vec::new());
    }

    let mut out_l = Vec::with_capacity(out_frames);
    let mut out_r = Vec::with_capacity(out_frames);
    for i in 0..out_frames {
        out_l.push(out[i * 2]);
        out_r.push(out[i * 2 + 1]);
    }
    (out_l, out_r)
}

/// Sträcker ett stereopar med bevarad tonhöjd — **produktionsvägen**.
///
/// Kanalerna går genom **samma** WSOLA-instans, så grainsökningen (som avgör var varje korn
/// läggs) görs en gång för båda kanalerna. Att köra två separata instanser skulle kunna välja
/// olika skiften för vänster och höger och därmed sprida ut stereobilden — samma fälla som ett
/// separat monoklipp per kanal.
///
/// Vid faktor 1,0 returneras ingången oförändrad: ingen grain-process, ingen förlust.
///
/// **Motorn är vald av en mätning, inte av en inställning** (Fas 8.10 steg 2, 2026-09-13):
/// `signalsmith-stretch` prövades och förlorade på transienterna — se
/// [`stretch_signalsmith`] och roadmapens 8.10. Vår egen WSOLA står kvar.
/// Standardvägen (`Complex`) — kvar för proven, som mäter mot den kända
/// kvaliteten. Produktionen går alltid genom [`stretch_stereo_mode`].
#[cfg(test)]
pub fn stretch_stereo(
    left: &[f32],
    right: &[f32],
    ratio: f32,
    sample_rate: f32,
) -> (Vec<f32>, Vec<f32>) {
    stretch_stereo_mode(left, right, ratio, sample_rate, crate::audio::warp::WarpMode::Complex)
}

/// Sträcker med ett valt warp-läge.
///
/// Läget ändrar **kornets längd och sökvidden**, inget annat: kort korn följer transienter,
/// långt korn håller tonhöjd och övertoner. Standard är `Complex`, alltså exakt den väg
/// [`stretch_stereo`] alltid har tagit.
pub fn stretch_stereo_mode(
    left: &[f32],
    right: &[f32],
    ratio: f32,
    sample_rate: f32,
    mode: crate::audio::warp::WarpMode,
) -> (Vec<f32>, Vec<f32>) {
    if (ratio - 1.0).abs() < 1e-6 {
        return (left.to_vec(), right.to_vec());
    }
    if mode == crate::audio::warp::WarpMode::Transient {
        // Anslagen kopieras orörda och bara kropparna sträcks: transienterna kan då inte
        // smetas ut, vilket de kornbaserade lägena inte kan lova.
        return stretch_stereo_transient(left, right, ratio, sample_rate);
    }
    let (frame, hop, search) = mode.wsola_params(sample_rate);
    let mut wsola = super::vocal_harmonizer::Wsola::with_hann(frame, hop, search);
    stretch_stereo_with(left, right, ratio, sample_rate, &mut wsola)
}

/// Sträcker med **anslagen orörda**: attacken kopieras rakt av, kroppen sträcks.
///
/// Det här är den enkla vägen till transientbevarande, och den mäter bättre än att
/// försöka styra kornen mot anslagen. En transient som ligger i fyra överlappande korn
/// delas upp och tappar topp, hur kornet än placeras (mätt: 0,450 mot 0,900). Kopieras
/// attacken i stället blir toppen exakt källans, och bara den del som ska förlängas
/// sträcks.
///
/// Segmenten skärs vid de anslag detektorn hittar (profilen som mätte bäst mot Abletons
/// egen analys: 1,5 × omgivningen och 90 ms mellan slagen). Varje segment blir
/// `attack_len` sampel orörd attack, sedan en 1 ms övergång in i den sträckta kroppen.
/// Segment som är kortare än attacken kopieras rakt av.
pub fn stretch_stereo_transient(
    left: &[f32],
    right: &[f32],
    ratio: f32,
    sample_rate: f32,
) -> (Vec<f32>, Vec<f32>) {
    let frames = left.len().min(right.len());
    if frames == 0 || sample_rate <= 0.0 {
        return (Vec::new(), Vec::new());
    }
    let ratio = ratio.clamp(MIN_RATIO, MAX_RATIO);
    if (ratio - 1.0).abs() <= 1e-4 {
        return (left[..frames].to_vec(), right[..frames].to_vec());
    }
    let out_frames = ((frames as f64) * (ratio as f64)).round().max(1.0) as usize;

    let mono: Vec<f32> = left[..frames]
        .iter()
        .zip(right[..frames].iter())
        .map(|(l, r)| 0.5 * (l + r))
        .collect();
    let params = crate::audio::onset::OnsetParams {
        sensitivity: 1.5,
        min_gap_ms: 90.0,
        window_ms: 60.0,
    };
    let attack = ((0.005 * sample_rate) as usize).max(1);
    let cross = ((0.001 * sample_rate) as usize).max(2);

    // Segmentgränser: filens början, anslagen, filens slut. Anslag som ligger närmare
    // varandra än en attack slås ihop, annars blir det bara attack och ingen kropp.
    let mut bounds: Vec<usize> = vec![0];
    for onset in crate::audio::onset::detect_onsets(&mono, sample_rate, &params) {
        if onset > *bounds.last().unwrap_or(&0) + attack + cross {
            bounds.push(onset);
        }
    }
    if *bounds.last().unwrap_or(&0) + 1 < frames {
        bounds.push(frames);
    }

    let (frame, hop, search) = crate::audio::warp::WarpMode::Complex.wsola_params(sample_rate);
    let mut out_l: Vec<f32> = Vec::with_capacity(out_frames);
    let mut out_r: Vec<f32> = Vec::with_capacity(out_frames);

    for pair in bounds.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let seg = b.saturating_sub(a);
        if seg == 0 {
            continue;
        }
        let want = ((seg as f64) * (ratio as f64)).round().max(1.0) as usize;
        if seg <= attack + cross {
            // Kortare än attacken: kopiera rakt av, ingen kropp att sträcka.
            let take = seg.min(want.max(1)).min(seg);
            out_l.extend_from_slice(&left[a..a + take]);
            out_r.extend_from_slice(&right[a..a + take]);
            continue;
        }
        // Attacken kopieras orörd — det är den som bär transienten.
        let head = attack.min(seg).min(want);
        out_l.extend_from_slice(&left[a..a + head]);
        out_r.extend_from_slice(&right[a..a + head]);

        let body_src = seg - head;
        // Kroppen får `n` extra sampel: de första `n` konsumeras av övergången, så att
        // segmentet ändå blir exakt `want` långt. Annars tappar varje segment `n` sampel
        // och anslagen glider bakåt genom filen.
        let n = cross.min(head);
        let body_out = want.saturating_sub(head) + n;
        if body_out == 0 {
            continue;
        }
        let (mut bl, mut br) = if body_src > frame {
            let mut wsola = super::vocal_harmonizer::Wsola::with_hann(frame, hop, search);
            stretch_stereo_with(
                &left[a + head..b],
                &right[a + head..b],
                body_out as f32 / body_src as f32,
                sample_rate,
                &mut wsola,
            )
        } else {
            // Kroppen är kortare än ett korn: linjär omsampling duger, och tonhöjden får
            // följa med (det hörs inte på ett anslag som redan är över).
            linear_stretch(&left[a + head..b], &right[a + head..b], body_out)
        };
        bl.resize(body_out, 0.0);
        br.resize(body_out, 0.0);
        // 1 ms övergång mellan den orörda attacken och den sträckta kroppen.
        let n = n.min(body_out);
        let base_l = out_l.len() - n;
        let base_r = out_r.len() - n;
        for i in 0..n {
            let t = (i + 1) as f32 / (n + 1) as f32;
            out_l[base_l + i] = out_l[base_l + i] * (1.0 - t) + bl[i] * t;
            out_r[base_r + i] = out_r[base_r + i] * (1.0 - t) + br[i] * t;
        }
        out_l.extend_from_slice(&bl[n..]);
        out_r.extend_from_slice(&br[n..]);
    }

    // Exakt längd: filen ska bli `ratio` gånger källan, annars avvisas den av
    // `check_rendered` och originalet spelas i stället.
    out_l.resize(out_frames, 0.0);
    out_r.resize(out_frames, 0.0);
    out_l.truncate(out_frames);
    out_r.truncate(out_frames);
    (out_l, out_r)
}

/// Linjär omsampling, för kroppar som är kortare än ett korn.
fn linear_stretch(left: &[f32], right: &[f32], want: usize) -> (Vec<f32>, Vec<f32>) {
    let n = left.len().min(right.len());
    let mut out_l = Vec::with_capacity(want);
    let mut out_r = Vec::with_capacity(want);
    if n == 0 || want == 0 {
        return (vec![0.0; want], vec![0.0; want]);
    }
    for i in 0..want {
        let pos = i as f64 * (n as f64 - 1.0) / (want as f64 - 1.0).max(1.0);
        let i0 = pos.floor() as usize;
        let i1 = (i0 + 1).min(n - 1);
        let frac = (pos - i0 as f64) as f32;
        out_l.push(left[i0] + (left[i1] - left[i0]) * frac);
        out_r.push(right[i0] + (right[i1] - right[i0]) * frac);
    }
    (out_l, out_r)
}

/// Som [`stretch_stereo`], men med en given WSOLA.
///
/// Finns för **mätningen**: samma källfil genom två parameteruppsättningar, så att
/// artefakterna kan jämföras i siffror i stället för att beskrivas. Produktionsvägen
/// skickar alltid offlineläget.
pub fn stretch_stereo_with(
    left: &[f32],
    right: &[f32],
    ratio: f32,
    sample_rate: f32,
    wsola: &mut super::vocal_harmonizer::Wsola,
) -> (Vec<f32>, Vec<f32>) {
    let frames = left.len().min(right.len());
    if frames == 0 || sample_rate <= 0.0 {
        return (Vec::new(), Vec::new());
    }
    let ratio = ratio.clamp(MIN_RATIO, MAX_RATIO);
    if (ratio - 1.0).abs() <= 1e-4 {
        return (left[..frames].to_vec(), right[..frames].to_vec());
    }
    let out_frames = ((frames as f64) * (ratio as f64)).round().max(1.0) as usize;

    wsola.set_ratio(ratio);
    let mut out_l = Vec::with_capacity(out_frames);
    let mut out_r = Vec::with_capacity(out_frames);
    while out_l.len() < out_frames {
        let (l, r) = wsola.next_resampled(left, right, false, 1.0);
        // WSOLA:n är slut och har inget mer att ge: avbryt i stället för att snurra. Ett
        // avbrott här syns i valideringen efteråt, och en för kort fil används aldrig.
        if wsola.available() <= 0.0 && wsola.is_finished() {
            break;
        }
        out_l.push(l);
        out_r.push(r);
    }
    // Svansen: det sista kornet kan sakna källa, och då blir filen några sampel för kort —
    // och en för kort fil **avvisas** av `check_rendered` (alltså spelas originalet i
    // stället). En kort svans (högst 50 ms) fylls därför ut med tystnad; är filen kortare
    // än så är något fel, och då får den vara kort så att valideringen säger ifrån.
    let short = out_frames.saturating_sub(out_l.len());
    if short > 0 && short <= (sample_rate * 0.05) as usize {
        out_l.resize(out_frames, 0.0);
        out_r.resize(out_frames, 0.0);
    }
    (out_l, out_r)
}

/// Prövar en renderad sträckning **innan** den får användas.
///
/// Det här är 8.5-regeln i siffror: en fil som är för kort, innehåller NaN/Inf eller är tyst
/// ska avvisas med ett skäl i klartext — aldrig spelas. En trasig sträckning som spelas är
/// exakt "en möjlig spricka i rustningen".
pub fn check_rendered(left: &[f32], right: &[f32], expected_frames: usize) -> Result<(), String> {
    if left.is_empty() || right.is_empty() {
        return Err(crate::i18n::t("sträckningen blev tom").to_string());
    }
    if left.len() != right.len() {
        return Err(crate::tstatus!(
            "kanalerna har olika längd ({} mot {})",
            left.len(),
            right.len()
        ));
    }
    // Rundningen i `plan` ger som mest en frame fel; två frames är marginal för WSOLA:ns sista
    // korn. Mer än så betyder att något gick fel på vägen.
    let diff = left.len().abs_diff(expected_frames);
    if diff > 2 {
        return Err(crate::tstatus!(
            "fel längd: {} frames i stället för {}",
            left.len(),
            expected_frames
        ));
    }
    if !left.iter().chain(right.iter()).all(|s| s.is_finite()) {
        return Err(crate::i18n::t("sträckningen innehåller ogiltiga sampel (NaN)").to_string());
    }
    let peak = left
        .iter()
        .chain(right.iter())
        .fold(0.0f32, |m, s| m.max(s.abs()));
    if peak <= 1e-6 {
        return Err(crate::i18n::t("sträckningen blev tyst").to_string());
    }
    Ok(())
}

/// Filnamnet för en sträckning: källans namn **och båda tempon**.
///
/// Båda tempon måste med, annars kan en gammal cache från ett annat tempo läsas som giltig —
/// samma sorts föråldrade sammanfattning som `visual_peaks_from`-fällan.
/// **Motorns version i cachenyckeln.** Höj när sträckningen ändras.
///
/// Utan den är nyckeln bara källans namn och de två tempona — och en fil räknad med den
/// gamla motorn läses som giltig efter ett motorbyte. Det hände 2026-09-13: nio cachar
/// räknade med ±2,7 ms sökfönster låg kvar och hade spelats upp trots att motorn bytts —
/// ändringen var gjord i koden men hördes inte i ljudet.
///
/// - 2: WSOLA med ±12 ms sökfönster (8.10c) — **den motor som gäller**.
/// - 3 var ämnad för Signalsmith Stretch (8.10 steg 2), men den motorn förlorade domen och
///   ligger kvar som mät-dörr. Numret är därför inte taget i bruk: produktionsvägen är
///   oförändrad, och hans cachar ska fortsätta träffa. **Om motorn byts en dag: höj den då** —
///   annars spelas gamla cachar upp och bytet hörs inte (det hände 2026-09-13).
pub const STRETCH_ENGINE_VERSION: u32 = 2;

pub fn cache_key(source: &str, source_bpm: f32, project_bpm: f32) -> String {
    let stem = Path::new(source)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "klipp".to_string());
    format!(
        "{}-{:.2}-till-{:.2}-v{}",
        crate::autosave::slug(&stem),
        source_bpm,
        project_bpm,
        STRETCH_ENGINE_VERSION
    )
}

/// Sökvägen en sträckning har (eller ska få) i cachen.
///
/// **En enda plats för namnet**, så uppslagningen och skrivningen inte kan glida ifrån
/// varandra — en cache som skrivs under ett namn och letas under ett annat är en cache som
/// aldrig träffar, och då räknas allt om varje gång utan att någon märker varför.
pub fn cache_path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{}.wav", crate::autosave::slug(key)))
}

/// Sträcker, prövar och skriver en sträckt version av ett klipp. Lämnar sökvägen.
///
/// Skrivningen är **atomisk**: först en tempfil, sedan `rename`. En avbruten skrivning lämnar
/// aldrig en fil som ser ut som en giltig cache.
pub fn render_to_file(
    dir: &Path,
    key: &str,
    left: &[f32],
    right: &[f32],
    ratio: f32,
    sample_rate: f32,
    mode: crate::audio::warp::WarpMode,
) -> Result<String, String> {
    let frames = left.len().min(right.len());
    let out_frames = ((frames as f64) * (ratio.clamp(MIN_RATIO, MAX_RATIO) as f64))
        .round()
        .max(1.0) as usize;
    let (out_l, out_r) = stretch_stereo_mode(left, right, ratio, sample_rate, mode);
    check_rendered(&out_l, &out_r, out_frames)?;

    std::fs::create_dir_all(dir)
        .map_err(|e| crate::tstatus!("kunde inte skapa '{}': {}", dir.display(), e))?;
    let final_path: PathBuf = cache_path(dir, key);
    let temp_path = dir.join(format!("{}.wav.tmp", crate::autosave::slug(key)));
    let temp_str = temp_path.to_string_lossy().to_string();
    super::exporter::write_stem_wav(&temp_str, &out_l, &out_r, sample_rate as u32)?;
    std::fs::rename(&temp_path, &final_path).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        crate::tstatus!(
            "kunde inte flytta den färdiga sträckningen till '{}': {}",
            final_path.display(),
            e
        )
    })?;
    Ok(final_path.to_string_lossy().into_owned())
}

/// En färdigsträckt stereobuffert: vänster, höger, samplingsfrekvens.
pub type StretchedAudio = (std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, f32);

/// Ett arbete åt arbetstråden.
#[derive(Clone)]
struct RenderJob {
    key: String,
    source_path: String,
    source_bpm: f32,
    project_bpm: f32,
    mode: crate::audio::warp::WarpMode,
    dir: PathBuf,
}

/// Vad ett arbete gav.
struct RenderDone {
    key: String,
    /// `Some` = ljudet, `None` = avvisat (då bär `error` skälet).
    source: Option<StretchedAudio>,
    error: Option<String>,
}

/// Cachen av färdigsträckta filer, med **en** arbetstråd.
///
/// Appen frågar [`StretchCache::get`] varje bildruta. Finns svaret inte och ingen
/// rendering är på väg beställs en. Nyckeln bär **både** källans och projektets
/// tempo (se [`cache_key`]), så en fil från ett annat tempo kan aldrig läsas som
/// giltig — det var precis den sortens föråldrade sammanfattning som gav pixlarna
/// i vågformen.
///
/// Arbetstråden gör hela arbetet utanför ljudtråden: avkoda källan, sträcka,
/// pröva, skriva atomiskt, läsa tillbaka. Ljudtråden får en färdig buffert och
/// spelar den som vilken fil som helst — **ingen ny felkälla i uppspelningen**.
pub struct StretchCache {
    /// Färdiga svar. `None` = avvisad, och då står skälet i `reasons`.
    ready: std::collections::HashMap<String, Option<StretchedAudio>>,
    /// Skälet en avvisad rendering fick — det som ska stå i statusraden.
    reasons: std::collections::HashMap<String, String>,
    /// Nycklar en tråd arbetar med just nu.
    pending: std::collections::HashSet<String>,
    tx: Option<std::sync::mpsc::Sender<RenderJob>>,
    rx: Option<std::sync::mpsc::Receiver<RenderDone>>,
}

impl Default for StretchCache {
    fn default() -> Self {
        Self::new()
    }
}

impl StretchCache {
    pub fn new() -> Self {
        Self {
            ready: std::collections::HashMap::new(),
            reasons: std::collections::HashMap::new(),
            pending: std::collections::HashSet::new(),
            tx: None,
            rx: None,
        }
    }

    /// Ljudet för en nyckel — bara om det finns **och** godkändes.
    pub fn get(&self, key: &str) -> Option<&StretchedAudio> {
        self.ready.get(key).and_then(|a| a.as_ref())
    }

    /// Finns ett svar, är ett på väg, eller har det redan avvisats? Då ska inget
    /// beställas igen: en avvisad sträckning ska inte försöka om varje bildruta.
    pub fn knows(&self, key: &str) -> bool {
        self.ready.contains_key(key) || self.pending.contains(key)
    }

    /// Skälet en avvisad rendering fick, om något.
    pub fn reason(&self, key: &str) -> Option<&str> {
        self.reasons.get(key).map(|s| s.as_str())
    }

    /// Hur många renderingar som är på väg.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Beställ en sträckning — om svaret inte redan finns eller är på väg.
    ///
    /// Tråden skapas vid första beställningen (samma grepp som vågformscachen),
    /// så att en app som aldrig ändrar tempot aldrig startar den.
    pub fn request(
        &mut self,
        dir: &Path,
        key: String,
        source_path: String,
        source_bpm: f32,
        project_bpm: f32,
        mode: crate::audio::warp::WarpMode,
    ) {
        if self.knows(&key) {
            return;
        }
        if self.tx.is_none() {
            let (tx, rx) = std::sync::mpsc::channel::<RenderJob>();
            let (done_tx, done_rx) = std::sync::mpsc::channel::<RenderDone>();
            std::thread::spawn(move || worker(rx, done_tx));
            self.tx = Some(tx);
            self.rx = Some(done_rx);
        }
        let job = RenderJob {
            key: key.clone(),
            source_path,
            source_bpm,
            project_bpm,
            mode,
            dir: dir.to_path_buf(),
        };
        let sent = matches!(self.tx.as_ref().map(|tx| tx.send(job)), Some(Ok(())));
        if sent {
            self.pending.insert(key);
        }
    }

    /// Tar emot färdiga renderingar. Anropas en gång per bildruta.
    ///
    /// Returnerar nycklarna som blev klara, så att appen kan säga till motorn om
    /// dem — en stämma som blivit sträckt ska spelas om, inte vänta på nästa
    /// tempoändring.
    pub fn poll(&mut self) -> Vec<String> {
        let mut done = Vec::new();
        if let Some(rx) = self.rx.as_ref() {
            while let Ok(d) = rx.try_recv() {
                self.pending.remove(&d.key);
                match d.source {
                    Some(audio) => {
                        self.reasons.remove(&d.key);
                        self.ready.insert(d.key.clone(), Some(audio));
                    }
                    None => {
                        if let Some(e) = d.error {
                            self.reasons.insert(d.key.clone(), e);
                        }
                        self.ready.insert(d.key.clone(), None);
                    }
                }
                done.push(d.key);
            }
        }
        done
    }
}

/// Arbetstråden: ett arbete i taget, i tur och ordning.
fn worker(rx: std::sync::mpsc::Receiver<RenderJob>, tx: std::sync::mpsc::Sender<RenderDone>) {
    while let Ok(job) = rx.recv() {
        let done = render_one(&job);
        if tx.send(done).is_err() {
            return; // appen är borta; inget mer att göra
        }
    }
}

/// Sträcker ett klipp till en fil och läser tillbaka den.
///
/// **Läser tillbaka filen vi just skrev.** Den buffert tidslinjen spelar är då
/// filens innehåll, bokstavligen — filen och högtalarna kan inte säga olika saker,
/// och en skrivning som inte gick att läsa upptäcks här i stället för i örat.
fn render_one(job: &RenderJob) -> RenderDone {
    let key = job.key.clone();
    let fail = |e: String| RenderDone {
        key: key.clone(),
        source: None,
        error: Some(e),
    };
    let ok = |l: Vec<f32>, r: Vec<f32>, sr: u32| RenderDone {
        key: key.clone(),
        source: Some((std::sync::Arc::new(l), std::sync::Arc::new(r), sr as f32)),
        error: None,
    };

    // 1. Ligger den redan i cachen? Då är det bara att läsa den.
    let path = cache_path(&job.dir, &job.key);
    let path_str = path.to_string_lossy().to_string();
    if path.exists() {
        match crate::audio::load_wav_pcm(&path_str) {
            Ok((l, r, sr)) => return ok(l, r, sr),
            // En trasig fil i cachen ska inte göra klippet ospelbart: den räknas om.
            // (Filen skrivs atomiskt, så det här är sista utvägen, inte första.)
            Err(_) => {}
        }
    }

    // 2. Avkoda källan, sträck, pröva, skriv.
    let (l, r, sr) = match crate::audio::load_wav_pcm(&job.source_path) {
        Ok(x) => x,
        Err(e) => return fail(e),
    };
    let Some(plan) = plan(job.source_bpm, job.project_bpm, l.len()) else {
        return fail(crate::i18n::t("inget att sträcka — faktorn är 1,0").to_string());
    };
    // Skulle någon beställa en sträckning med faktor 1,0 ändå ska vi inte skriva en
    // kopia av källan och kalla den sträckt.
    if !plan.changes_anything() {
        return fail(crate::i18n::t("inget att sträcka — faktorn är 1,0").to_string());
    }
    match render_to_file(&job.dir, &job.key, &l, &r, plan.file_ratio, sr as f32, job.mode) {
        Ok(written) => match crate::audio::load_wav_pcm(&written) {
            Ok((sl, sr2, ssr)) => ok(sl, sr2, ssr),
            Err(e) => fail(e),
        },
        Err(e) => fail(e),
    }
}

/// Väntar på att tempot ska stå still innan en rendering beställs.
///
/// Reglaget byter värde varje bildruta medan det dras, och varje värde är en egen
/// fil. Utan den här spärren blir en dragning från 120 till 150 i praktiken en
/// beställning per bildruta — arbete ingen bad om, på en fil som ingen hann spela.
#[derive(Clone, Copy, Debug)]
pub struct TempoSettle {
    last: f32,
    frames: u32,
}

impl TempoSettle {
    pub fn new(bpm: f32) -> Self {
        Self {
            last: bpm,
            frames: 0,
        }
    }

    /// Räknar en bildruta. `true` = tempot har stått still `needed` bildrutor.
    pub fn tick(&mut self, bpm: f32, needed: u32) -> bool {
        if (bpm - self.last).abs() > 0.0005 {
            self.last = bpm;
            self.frames = 0;
            return false;
        }
        self.frames = self.frames.saturating_add(1);
        self.frames >= needed
    }
}

/// Hur många bildrutor tempot ska stå still innan en sträckning beställs (~0,3 s).
pub const TEMPO_SETTLE_FRAMES: u32 = 20;

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44_100.0;

    /// Offlineläget får inte råka bli realtidsläget igen (Fas 8.10).
    ///
    /// Felet som gav Alex "artefaktljud": sökfönstret var **±2,7 ms** — mindre än en
    /// period av en 150 Hz-ton — så skarvarna hamnade ur fas. Mätt på hans egen stämma
    /// (140 → 120 BPM): F0-fladdret föll från 132,6 till 39,9 cent när fönstret breddades,
    /// medan kornstorleken och därmed diskanten stod kvar. Det här provet håller talet kvar.
    #[test]
    fn the_offline_stretcher_searches_wide_and_the_realtime_one_does_not() {
        let sr = 48_000.0;
        let (frame, hop, search) = crate::audio::vocal_harmonizer::Wsola::offline(sr).params();
        assert_eq!(hop, frame / 2, "50 % överlapp är nivåexakt (mätt på ren ton)");
        assert!(
            search as f32 / sr >= 0.010,
            "sökfönstret ska rymma en hel period ned till {:.0} Hz, inte {:.1} ms",
            sr / search as f32,
            1000.0 * search as f32 / sr
        );
        let (_, _, realtime) = crate::audio::vocal_harmonizer::Wsola::new(sr).params();
        assert!(
            realtime < search / 2,
            "realtidsläget ska vara det billiga: {realtime} mot {search} sampel"
        );
    }

    /// Cachenyckeln bär motorns version (Fas 8.10).
    ///
    /// Utan versionen läses en fil räknad med den **gamla** motorn som giltig efter ett
    /// motorbyte — och då hörs inte fixen. Mätt 2026-09-13: nio cachar räknade med
    /// ±2,7 ms sökfönster låg kvar och spelades upp.
    #[test]
    fn the_cache_key_carries_the_engine_version() {
        let key = cache_key("/tmp/x.wav", 140.0, 120.0);
        assert!(
            key.ends_with(&format!("-v{STRETCH_ENGINE_VERSION}")),
            "nyckeln ska bära motorns version: {key}"
        );
        assert_ne!(
            cache_key("/tmp/x.wav", 140.0, 120.0),
            cache_key("/tmp/x.wav", 140.0, 121.0),
            "och tempot ska fortfarande avgöra"
        );
    }

    /// **Läser appen tillbaka filen den själv skrev?** (Fas 8.10c, fältfynd 2026-09-13.)
    ///
    /// `render_one` skriver filen och läser den sedan med `load_wav_pcm`. Misslyckas
    /// läsningen blir svaret `None` — filen ligger kvar på disk, motorn får originalet,
    /// och Alex hör "ingen skillnad på tempot" medan cachen ser fullt frisk ut.
    /// Det här provet går mot hans **riktiga** cachefil, inte en syntetisk.
    #[test]
    #[ignore]
    fn the_cached_render_can_be_read_back() {
        let key = std::env::var("SONIX_CACHE_KEY")
            .unwrap_or_else(|_| "rock-and-hard-place-drums-140-00-till-100-00-v2".to_string());
        let path = cache_path(&crate::paths::paths().stretch_cache_dir(), &key);
        assert!(path.exists(), "cachen ska finnas: {}", path.display());
        match crate::audio::load_wav_pcm(&path.to_string_lossy()) {
            Ok((l, r, sr)) => {
                eprintln!(
                    "OK: {} frames, sr {sr}, {:.2} s",
                    l.len(),
                    l.len() as f32 / sr as f32
                );
                assert!(l.len() == r.len() && !l.is_empty());
            }
            Err(e) => panic!("KUNDE INTE LÄSA TILLBAKA: {e}"),
        }
    }

    /// **Mätning på riktig fil, inte på en sinuston** (Fas 8.10).
    ///
    /// Skriver samma källa genom två parameteruppsättningar — harmonizerns realtidsläge
    /// och offlineläget — som rå f32-PCM, så att artefakterna kan mätas i siffror i
    /// stället för att beskrivas. Körs bara på begäran:
    ///
    /// ```text
    /// SONIX_AB_SRC=~/imported_stems/.../x.wav \
    ///   cargo test --locked --bin sonix render_stretch_variants -- --ignored --nocapture
    /// ```
    ///
    /// (Filerna hamnar i `/tmp/sonix_stretch_ab/`.)
    #[test]
    #[ignore]
    fn render_stretch_variants_for_measurement() {
        let src = std::env::var("SONIX_AB_SRC").unwrap_or_default();
        let ratio: f32 = std::env::var("SONIX_AB_RATIO")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(140.0 / 120.0);
        let Ok((l, r, sr)) = crate::audio::load_audio_pcm(&src) else {
            eprintln!("kunde inte läsa '{src}' (sätt SONIX_AB_SRC)");
            return;
        };
        let dir = std::path::Path::new("/tmp/sonix_stretch_ab");
        let _ = std::fs::create_dir_all(dir);
        let interleave = |a: &[f32], b: &[f32]| -> Vec<u8> {
            let mut out = Vec::with_capacity(a.len() * 8);
            for i in 0..a.len() {
                out.extend_from_slice(&a[i].to_le_bytes());
                out.extend_from_slice(&b[i].to_le_bytes());
            }
            out
        };
        // Tre varianter, för att kunna välja på siffror i stället för på känsla:
        // realtidsläget (som var i drift), offlineläget, och en mellanväg som bara breddar
        // sökfönstret och behåller de korta kornen (korta korn bevarar transienter och
        // diskant bättre; bred sökning är det som tar bort fasstegen i skarvarna).
        let sr_f = sr as f32;
        let mut variants: Vec<(&str, crate::audio::vocal_harmonizer::Wsola)> = vec![
            ("gammal", crate::audio::vocal_harmonizer::Wsola::new(sr_f)),
            (
                "ny",
                crate::audio::vocal_harmonizer::Wsola::with_hann(
                    ((0.060 * sr_f) as usize / 4 * 4).max(1024),
                    (((0.060 * sr_f) as usize / 4 * 4).max(1024)) / 4,
                    ((0.012 * sr_f) as usize).max(128),
                ),
            ),
            (
                "sok",
                crate::audio::vocal_harmonizer::Wsola::with_hann(
                    1024,
                    512,
                    ((0.012 * sr_f) as usize).max(128),
                ),
            ),
        ];
        for (name, wsola) in variants.iter_mut() {
            let (ol, or) = stretch_stereo_with(&l, &r, ratio, sr_f, wsola);
            let path = dir.join(format!("{name}.f32"));
            std::fs::write(&path, interleave(&ol, &or)).unwrap();
            // Nivån hör till mätningen: en sträckning får inte ändra den.
            let peak_src = l.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            let peak_out = ol.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            println!(
                "{name}: {} frames (väntat {}), topp källa {:.4} → ut {:.4} ({:+.2} dB) → {}",
                ol.len(),
                (l.len() as f64 * ratio as f64).round() as usize,
                peak_src,
                peak_out,
                20.0 * (peak_out.max(1e-9) / peak_src.max(1e-9)).log10(),
                path.display()
            );
        }
    }

    fn tone(freq: f32, secs: f32, amp: f32) -> Vec<f32> {
        let n = (SR * secs) as usize;
        (0..n)
            .map(|i| (std::f32::consts::TAU * freq * i as f32 / SR).sin() * amp)
            .collect()
    }

    /// Energin vid en frekvens (Goertzel) — används för att bevisa att tonhöjden står still.
    fn energy(buf: &[f32], freq: f32) -> f32 {
        let w = std::f32::consts::TAU * freq / SR;
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &x in buf {
            let s0 = x + 2.0 * w.cos() * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        ((s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2).max(0.0)).sqrt() / buf.len() as f32
    }

    #[test]
    fn the_plan_is_none_when_nothing_should_change() {
        // Okänt inspelningstempo (8.10:s regel): rör inte ljudet.
        assert_eq!(plan(0.0, 128.0, 44_100), None);
        // Samma tempo: bit-exakt väg, ingen fil behövs.
        assert_eq!(plan(120.0, 120.0, 44_100), None);
        // Tomt klipp.
        assert_eq!(plan(120.0, 140.0, 0), None);
        // Och ett projekt utan tempo är inget att sträcka mot.
        assert_eq!(plan(120.0, 0.0, 44_100), None);
    }

    /// Faktorerna, och att de är varandras inverterade.
    ///
    /// Fysiken: fyra takter i 120 BPM är 8 sekunder, i 150 BPM 6,4 sekunder. Ska
    /// samma fyra takter låta lika högt i det högre tempot måste filen bli
    /// **kortare** (6,4 s), inte längre. Motorn får däremot 1,25 som faktor — den
    /// räknar källsekunder per utsekund.
    #[test]
    fn a_higher_tempo_gives_a_shorter_file_and_a_faster_playback_ratio() {
        let up = plan(120.0, 150.0, 44_100).expect("150 mot 120 ska sträckas");
        assert!(
            (up.playback_ratio - 1.25).abs() < 1e-6,
            "motorn: {}",
            up.playback_ratio
        );
        assert!(
            (up.file_ratio - 0.8).abs() < 1e-6,
            "filen: {}",
            up.file_ratio
        );
        assert_eq!(
            up.out_frames, 35_280,
            "0,8 × 44 100 frames — kortare, inte längre"
        );
        assert!(up.changes_anything());

        let down = plan(150.0, 120.0, 44_100).expect("120 mot 150 ska sträckas ned");
        assert!((down.playback_ratio - 0.8).abs() < 1e-6);
        assert!((down.file_ratio - 1.25).abs() < 1e-6);
        assert_eq!(down.out_frames, 55_125, "1,25 × 44 100 frames");

        // Och de två hör ihop: filens faktor gånger motorns är 1,0.
        for (src, dst) in [(120.0, 150.0), (150.0, 120.0), (90.0, 174.0)] {
            let p = plan(src, dst, 44_100).expect("en plan finns");
            assert!(
                (p.file_ratio * p.playback_ratio - 1.0).abs() < 1e-5,
                "{src}→{dst}: {} × {} ska bli 1,0",
                p.file_ratio,
                p.playback_ratio
            );
        }
    }

    #[test]
    fn the_plan_clamps_extreme_tempos_instead_of_producing_a_monster() {
        let fast = plan(120.0, 10_000.0, 1_000).expect("en plan finns");
        assert!((fast.playback_ratio - MAX_RATIO).abs() < 1e-6);
        assert!(
            fast.out_frames >= 1,
            "en klämd plan får aldrig bli noll frames"
        );
        let slow = plan(120.0, 1.0, 1_000).expect("en plan finns");
        assert!((slow.playback_ratio - MIN_RATIO).abs() < 1e-6);
        assert!(slow.out_frames >= 1);
    }

    /// **Acceptanskriterium 1:** vid faktor 1,0 händer ingenting alls — bit-exakt.
    #[test]
    fn a_ratio_of_one_returns_the_input_untouched() {
        let l = tone(220.0, 0.2, 0.7);
        let r: Vec<f32> = l.iter().map(|s| s * 0.5).collect();
        let (out_l, out_r) = stretch_stereo_mode(&l, &r, 1.0, SR, crate::audio::warp::WarpMode::Complex);
        assert_eq!(out_l, l, "vänster ska vara oförändrad");
        assert_eq!(out_r, r, "höger ska vara oförändrad");
    }

    /// **Acceptanskriterium 2:** tonhöjden står still när tempot ändras, och längden följer
    /// faktorn. Mätt med Goertzel vid grundtonen — och vid den frekvens en smurf hade hamnat på.
    #[test]
    fn the_pitch_stays_put_while_the_length_changes() {
        let l = tone(220.0, 0.5, 0.7);
        let r = l.clone();
        let (out_l, _out_r) = stretch_stereo_mode(&l, &r, 1.5, SR, crate::audio::warp::WarpMode::Complex);

        let expected = (l.len() as f64 * 1.5).round() as usize;
        assert!(
            out_l.len().abs_diff(expected) <= 2,
            "längden ska följa faktorn: {} mot {}",
            out_l.len(),
            expected
        );

        let mid = &out_l[out_l.len() / 4..out_l.len() * 3 / 4];
        let at_pitch = energy(mid, 220.0);
        let at_smurf = energy(mid, 220.0 * 1.5);
        assert!(
            at_pitch > at_smurf * 4.0,
            "grundtonen ska dominera (220 Hz: {at_pitch:.5} mot smurfen 330 Hz: {at_smurf:.5})"
        );
    }

    /// Och nedåt: en sänkning får inte heller flytta tonhöjden.
    #[test]
    fn slowing_down_does_not_deepen_the_voice() {
        let l = tone(300.0, 0.5, 0.6);
        let (out_l, _) = stretch_stereo_mode(&l, &l.clone(), 0.75, SR, crate::audio::warp::WarpMode::Complex);
        let expected = (l.len() as f64 * 0.75).round() as usize;
        assert!(
            out_l.len().abs_diff(expected) <= 2,
            "längden: {}",
            out_l.len()
        );
        let mid = &out_l[out_l.len() / 4..out_l.len() * 3 / 4];
        assert!(
            energy(mid, 300.0) > energy(mid, 300.0 * 0.75) * 4.0,
            "grundtonen ska stå kvar"
        );
    }

    /// **Stereobilden:** körs båda kanalerna genom samma grainsökning ska förhållandet mellan
    /// dem stå kvar. Här är höger exakt hälften av vänster, och det ska den vara efteråt också.
    #[test]
    fn both_channels_go_through_the_same_grains_so_the_stereo_image_survives() {
        let l = tone(220.0, 0.4, 0.8);
        let r: Vec<f32> = l.iter().map(|s| s * 0.5).collect();
        let (out_l, out_r) = stretch_stereo_mode(&l, &r, 1.25, SR, crate::audio::warp::WarpMode::Complex);

        let mut worst = 0.0f32;
        for (a, b) in out_l.iter().zip(out_r.iter()) {
            if a.abs() > 0.05 {
                worst = worst.max((b / a - 0.5).abs());
            }
        }
        assert!(
            worst < 0.05,
            "kanalerna ska hänga ihop, största avvikelse {worst}"
        );
    }

    /// **Acceptanskriterium 3:** en trasig sträckning avvisas — tom, tyst, NaN eller fel längd.
    #[test]
    fn a_broken_stretch_is_refused_and_never_played() {
        let good = tone(220.0, 0.1, 0.5);
        assert!(check_rendered(&good, &good, good.len()).is_ok());

        let empty: Vec<f32> = Vec::new();
        assert!(
            check_rendered(&empty, &empty, 0).is_err(),
            "tom ska avvisas"
        );

        let silent = vec![0.0f32; 4_410];
        assert!(
            check_rendered(&silent, &silent, 4_410).is_err(),
            "tyst ska avvisas"
        );

        let mut nan = good.clone();
        nan[10] = f32::NAN;
        assert!(
            check_rendered(&nan, &good, good.len()).is_err(),
            "NaN ska avvisas"
        );

        assert!(
            check_rendered(&good, &good, good.len() * 2).is_err(),
            "fel längd ska avvisas"
        );
        assert!(
            check_rendered(&good, &good[..good.len() - 1], good.len()).is_err(),
            "olika långa kanaler ska avvisas"
        );
    }

    /// Hela vägen: räkna, pröva, skriv — och läs tillbaka med appens egen avkodare.
    /// **Vad en sträckning kostar** (mätning, körs manuellt):
    /// `cargo test --release --bin sonix the_render_cost -- --ignored --nocapture`
    ///
    /// En fyraminutersstämma är det mått vågformscachen mättes med (1 min/4 min/10 min),
    /// så siffrorna går att jämföra: priset för att tidslinjen ska kunna spela en vanlig
    /// fil är att någon räknar en gång per (källa, tempo).
    #[test]
    #[ignore]
    fn the_render_cost() {
        for minutes in [1u32, 4] {
            let secs = (minutes * 60) as f32;
            let src = tone(220.0, secs, 0.5);
            let t0 = std::time::Instant::now();
            let (out, _) = stretch_stereo_mode(&src, &src, 1.25, SR, crate::audio::warp::WarpMode::Complex);
            let stretch_ms = t0.elapsed().as_millis();

            let dir = std::env::temp_dir().join(format!("sonix_stretch_cost_{minutes}"));
            let _ = std::fs::remove_dir_all(&dir);
            let t1 = std::time::Instant::now();
            let written = render_to_file(&dir, "kostnad", &src, &src, 1.25, SR, crate::audio::warp::WarpMode::Complex)
                .expect("renderingen ska lyckas");
            let total_ms = t1.elapsed().as_millis();
            let size_mb = std::fs::metadata(&written).map(|m| m.len()).unwrap_or(0) as f64 / 1e6;
            println!(
                "{minutes} min stereo: sträckning {stretch_ms} ms, hela vägen (sträck + prövning + skrivning + tillbakaläsning) {total_ms} ms, fil {size_mb:.1} MB, {} frames",
                out.len()
            );
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// **Hur mycket skorrar sträckningen?** (mätning mot riktiga stämmor, kör manuellt):
    /// `cargo test --release --bin sonix the_stretch_artefacts -- --ignored --nocapture`
    ///
    /// Alex hörde "vissa toner gick helt ur fas och skorrade" vid 110 BPM. Det går att
    /// mäta: **ligger anslagen där de ska, och blir de fler?** WSOLA dubblar korn och
    /// fasar — ett anslag som flyttat sig syns som ett tidsfel mot den förväntade
    /// platsen (`källans tid × faktorn`), och ett dubbelt anslag som en träff källan
    /// inte hade. Repots egen slagletning är validerad ("en jämn ton ger noll slag"), så
    /// den får vara instrumentet. Minsta avstånd sätts till 5 ms: en dubblering ligger
    /// typiskt ett korn isär, och 30 ms hade smält ihop den med originalet.
    /// **Motorn får inte hitta på anslag** — den enkla halvan av kravet.
    ///
    /// En klickföljd med känt antal slag sträcks och antalet slag i renderingen får inte
    /// överstiga källans. Vakten är syntetisk med flit: Alex' stämmor ligger i
    /// `~/imported_stems` och finns inte i CI, och en mätning som bara kan köras på en
    /// maskin är ingen vakt.
    ///
    /// **Vad den inte bevisar:** att överskottet i tät musik är borta. Mätt 2026-09-13
    /// klarar en isolerad klickföljd sig med **0** överskott både före och efter varje
    /// försök att styra kornplaceringen vid anslag — så testet fångar en grov regression,
    /// inte den fina. Det fina mäts av `the_stretch_artefacts_on_a_real_stem` (ignorerad,
    /// körs mot Alex' stämmor).
    #[test]
    fn the_engine_does_not_invent_transients() {
        let sr = 44_100.0f32;
        let clicks = 24usize;
        let gap_secs = 0.4f32;
        let total = (sr * (gap_secs * clicks as f32 + 1.0)) as usize;
        let mut left = vec![0.0f32; total];
        let mut right = vec![0.0f32; total];
        let mut seed = 0x1234_5678u32;
        let burst = (sr * 0.001) as usize; // 1 ms: ett klick, inte en ton
        for k in 0..clicks {
            let at = (k as f32 * gap_secs * sr) as usize;
            for i in 0..burst {
                if at + i >= total {
                    break;
                }
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let v = ((seed >> 9) as f32 / 8_388_608.0) - 1.0;
                left[at + i] = v * 0.8;
                right[at + i] = v * 0.8;
            }
        }

        let params = crate::audio::onset::OnsetParams {
            sensitivity: 2.0,
            min_gap_ms: 20.0,
            window_ms: 80.0,
        };
        let ratio = 120.0f32 / 110.0;
        let src = crate::audio::onset::detect_onsets(&left, sr, &params).len();
        let (out_l, _) = stretch_stereo(&left, &right, ratio, sr);
        let out = crate::audio::onset::detect_onsets(&out_l, sr, &params).len();

        println!("klickföljd vid {ratio:.4}×: källan {src} slag, renderingen {out}");
        assert_eq!(src, clicks, "detektorn ska hitta alla klick i källan");
        assert!(
            out <= src,
            "motorn hittade på {} anslag ({out} mot {src})",
            out.saturating_sub(src)
        );
    }

    #[test]
    #[ignore]
    fn the_stretch_artefacts_on_a_real_stem() {
        // Två inställningar: den täta fångar kornkanterna (där skorret sitter), den
        // tydliga räknar musikaliska anslag. Utan båda kan "överskottet" vara
        // detektorns eget brus i stället för sträckningens.
        let dense = crate::audio::onset::OnsetParams {
            min_gap_ms: 5.0,
            ..Default::default()
        };
        let clear = crate::audio::onset::OnsetParams {
            sensitivity: 2.5,
            min_gap_ms: 20.0,
            window_ms: 80.0,
        };
        let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
        let dir = std::env::temp_dir().join("sonix_stretch_artefacts");
        let _ = std::fs::remove_dir_all(&dir);
        let project = 110.0f32;
        let file_ratio = 120.0 / project;

        for name in [
            "Broken (Vocals).wav",
            "Broken (Backing Vocals).wav",
            "Broken (Drums).wav",
            "Broken (Bass).wav",
        ] {
            let path = home.join("imported_stems/Broken").join(name);
            if !path.exists() {
                println!("hoppar: {name} finns inte");
                continue;
            }
            let Ok((l, r, sr)) = crate::audio::load_wav_pcm(&path.to_string_lossy()) else {
                println!("hoppar: {name} gick inte att läsa");
                continue;
            };
            let src_dense = crate::audio::onset::detect_onsets(&l, sr as f32, &dense);
            let src_clear = crate::audio::onset::detect_onsets(&l, sr as f32, &clear);
            let written = render_to_file(&dir, name, &l, &r, file_ratio, sr as f32, crate::audio::warp::WarpMode::Complex)
                .expect("renderingen ska lyckas");
            let (rl, _rr, rsr) = crate::audio::load_wav_pcm(&written).expect("läs tillbaka");
            let out_dense = crate::audio::onset::detect_onsets(&rl, rsr as f32, &dense);
            let out_clear = crate::audio::onset::detect_onsets(&rl, rsr as f32, &clear);
            let out_secs: Vec<f32> = out_dense.iter().map(|&i| i as f32 / rsr as f32).collect();

            let mut errors: Vec<f32> = Vec::new();
            let mut missing = 0usize;
            for &o in &src_dense {
                let expected = o as f32 / sr as f32 * file_ratio;
                let nearest = out_secs
                    .iter()
                    .map(|&t| (t - expected).abs())
                    .fold(f32::INFINITY, f32::min);
                if nearest <= 0.030 {
                    errors.push(nearest);
                } else {
                    missing += 1;
                }
            }
            errors.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = errors.get(errors.len() / 2).copied().unwrap_or(0.0);
            let p95 = errors.get(errors.len() * 95 / 100).copied().unwrap_or(0.0);
            println!(
                "{name}\n   tätt:   källan {} anslag, renderingen {} (överskott {}), flyttade/saknade >30 ms: \
                 {missing}, felet median {:.1} ms / p95 {:.1} ms\n   tydligt: källan {} anslag, renderingen {} \
                 ({:+} %, musikaliska anslag)",
                src_dense.len(),
                out_dense.len(),
                out_dense.len() as i64 - src_dense.len() as i64,
                median * 1000.0,
                p95 * 1000.0,
                src_clear.len(),
                out_clear.len(),
                if src_clear.is_empty() { 0.0 } else {
                    100.0 * (out_clear.len() as f32 - src_clear.len() as f32) / src_clear.len() as f32
                }
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **Mätning mot en riktig stämma** (körs manuellt, hoppar tyst om filen inte finns):
    /// `cargo test --release --bin sonix the_stretch_fills -- --ignored --nocapture`
    ///
    /// Alex' Broken-stämmor är 254,000 s och klippen 127 takter, alltså 120,0000 BPM.
    /// Frågan provet svarar på: **blir den sträckta filen exakt så lång som klossen är
    /// på tidslinjen?** Gör den inte det glider ljudet ur takt, mer ju längre låten
    /// spelar — det var felet han hörde 2026-09-12 (0,82 % fel = 2,3 s över fyra
    /// minuter). Provet jämför den gamla siffran (120,98828) med geometrins (120,0).
    #[test]
    #[ignore]
    fn the_stretch_fills_the_clip_on_a_real_stem() {
        let stem = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join("imported_stems/Broken/Broken (Drums).wav");
        if !stem.exists() {
            println!("hoppar: {} finns inte", stem.display());
            return;
        }
        let Ok((l, r, sr)) = crate::audio::load_wav_pcm(&stem.to_string_lossy()) else {
            println!("hoppar: kunde inte läsa {}", stem.display());
            return;
        };
        let file_secs = l.len() as f32 / sr as f32;
        let bars = 127.0f32;
        let geometry = bars * 240.0 / file_secs;
        println!(
            "källan: {file_secs:.3} s, {bars} takter -> geometrin {geometry:.4} BPM \
             (klippet bar 120,98828)"
        );

        let dir = std::env::temp_dir().join("sonix_stretch_real");
        let _ = std::fs::remove_dir_all(&dir);
        for (label, source_bpm) in [("gammal siffra", 120.98828f32), ("geometrin", geometry)] {
            for project in [120.0f32, 110.0] {
                let file_ratio = source_bpm / project;
                let written = render_to_file(
                    &dir,
                    &format!("{label} {project}"),
                    &l,
                    &r,
                    file_ratio,
                    sr as f32,
                    crate::audio::warp::WarpMode::Complex,
                )
                .expect("renderingen ska lyckas");
                let out_secs = std::fs::metadata(&written).map(|m| m.len()).unwrap_or(0) as f32;
                let clip_secs = bars * 240.0 / project;
                // 48 kHz, 2 kanaler, 32-bitars float + header.
                let rendered_secs = out_secs / (sr as f32 * 2.0 * 4.0);
                println!(
                    "  {label:14} vid {project:5.1} BPM: filen {rendered_secs:8.2} s, \
                     klossen {clip_secs:8.2} s, fel {:+6.2} s",
                    rendered_secs - clip_secs
                );
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **Acceptanskriterium 2, hela vägen:** en ton i ett klipp har samma
    /// grundfrekvens efter en tempoändring — och det tidslinjen spelar är filen
    /// cachen skrev.
    ///
    /// Provet går genom `StretchCache` (beställning → arbetstråd → fil → tillbaka),
    /// alltså samma väg appen tar. Att bara pröva `stretch_stereo` skulle inte säga
    /// något om att filen blir rätt, eller att den som skrevs är den som läses.
    #[test]
    fn the_cache_renders_a_file_and_the_tone_keeps_its_pitch() {
        let dir = std::env::temp_dir().join(format!("sonix_stretch_cache_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("testmappen ska gå att skapa");
        let src_path = dir.join("kalla.wav");
        let src_str = src_path.to_string_lossy().to_string();
        let tone = tone(220.0, 1.0, 0.6); // 1,0 s vid SR
        crate::audio::exporter::write_stem_wav(&src_str, &tone, &tone, SR as u32)
            .expect("källfilen ska gå att skriva");

        let mut cache = StretchCache::new();
        let key = cache_key(&src_str, 120.0, 180.0);
        cache.request(&dir, key.clone(), src_str.clone(), 120.0, 180.0, crate::audio::warp::WarpMode::Complex);

        // Arbetstråden renderar utanför testet: vänta in den (med ett tak).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while cache.get(&key).is_none() && std::time::Instant::now() < deadline {
            cache.poll();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let (left, right, _sr) = cache
            .get(&key)
            .expect("sträckningen ska bli klar inom taket")
            .clone();
        assert_eq!(left.len(), right.len(), "kanalerna ska vara lika långa");

        // 1,0 s källa i 120 BPM, projektet i 180: filen blir 120/180 = 0,667 s.
        let expected = (SR * (120.0 / 180.0)).round() as usize;
        assert!(
            left.len().abs_diff(expected) <= 2,
            "längden ska bli kortare: {} mot {}",
            left.len(),
            expected
        );

        // Tonhöjden står still: 220 Hz dominerar över den smurf 1,5× hade gett (330 Hz).
        let mid = &left[left.len() / 4..left.len() * 3 / 4];
        let at_pitch = energy(mid, 220.0);
        let at_smurf = energy(mid, 330.0);
        assert!(
            at_pitch > at_smurf * 4.0,
            "grundtonen ska dominera efter en tempoändring (220 Hz: {at_pitch:.5} mot 330 Hz: {at_smurf:.5})"
        );

        // Och filen ligger kvar i cachen: en ny beställning ska hitta den.
        assert!(cache_path(&dir, &key).exists(), "filen ska finnas i cachen");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rendered_stretch_lands_on_disk_and_reads_back() {
        let dir = std::env::temp_dir().join(format!("sonix_stretch_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let l = tone(220.0, 0.3, 0.6);
        let r: Vec<f32> = l.iter().map(|s| -s * 0.8).collect();
        let key = cache_key("/tmp/Min Låt (Suno).mp3", 120.0, 150.0);
        let path = render_to_file(&dir, &key, &l, &r, 1.25, SR, crate::audio::warp::WarpMode::Complex).expect("skrivningen ska lyckas");

        assert!(Path::new(&path).is_file(), "filen ska finnas: {path}");
        let (read_l, read_r, sr) = crate::audio::wav_reader::load_wav_pcm(&path)
            .expect("den skrivna filen ska gå att läsa");
        assert_eq!(sr, 44_100);
        let expected = (l.len() as f64 * 1.25).round() as usize;
        assert!(
            read_l.len().abs_diff(expected) <= 2,
            "längden i filen: {} mot {}",
            read_l.len(),
            expected
        );
        assert!(read_l.iter().all(|s| s.is_finite()));
        assert!(
            read_l.iter().fold(0.0f32, |m, s| m.max(s.abs())) > 0.05,
            "och den ska ha ljud"
        );
        // Osymmetrin mellan kanalerna ska bevaras (höger är inverterad och lägre).
        let mid = read_l.len() / 2;
        assert!(
            read_l[mid] * read_r[mid] <= 0.0,
            "kanalerna ska inte blandas ihop"
        );

        // Namnet innehåller BÅDA tempon, så en cache från ett annat tempo inte kan läsas.
        assert!(
            path.contains("120") && path.contains("150"),
            "nyckeln: {path}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// En plan på ett tomt klipp får inte skriva en tom fil.
    #[test]
    fn an_empty_clip_never_produces_a_file() {
        let dir = std::env::temp_dir().join(format!("sonix_stretch_empty_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let err = render_to_file(&dir, "tomt", &[], &[], 1.5, SR, crate::audio::warp::WarpMode::Complex).expect_err("ska avvisas");
        assert!(!err.is_empty(), "felet ska förklara sig: {err}");
        assert!(!dir.exists(), "ingen katalog ska ha skapats i onödan");
    }

    /// **Policyn**, i en tabell: vad som händer med ett klipp i varje läge.
    #[test]
    fn the_policy_never_surprises_the_user() {
        let untouched = FollowDecision {
            mode: FollowMode::Untouched,
            ratio: 1.0,
        };

        // 1. Okänt inspelningstempo (8.10) — även med switchen på.
        assert_eq!(decide(0.0, 160.0, true, false), untouched);
        // 2. Switchen av: ingen följning alls, varken sträckning eller bandspelare.
        assert_eq!(decide(120.0, 160.0, false, false), untouched);
        assert_eq!(decide(120.0, 160.0, false, true), untouched);
        // 3. Klippet vill ha bandspelaren: tonhöjden följer, och det är ett aktivt val.
        let tape = decide(120.0, 150.0, true, true);
        assert_eq!(tape.mode, FollowMode::Tape);
        assert!((tape.ratio - 1.25).abs() < 1e-6);
        // 4. Samma tempo: ingenting att göra.
        assert_eq!(decide(128.0, 128.0, true, false), untouched);
        // 5. Standardvägen: sträck, bevara tonhöjden.
        let stretch = decide(120.0, 150.0, true, false);
        assert_eq!(stretch.mode, FollowMode::Stretch);
        assert!((stretch.ratio - 1.25).abs() < 1e-6);
        // Och nedåt lika självklart.
        let down = decide(150.0, 120.0, true, false);
        assert_eq!(down.mode, FollowMode::Stretch);
        assert!((down.ratio - 0.8).abs() < 1e-6);
    }

    /// **Taket för den automatiska följningen** (mätt 2026-09-20).
    ///
    /// Alex' "Under Vintergatan": stämmorna är byggda i **163 BPM**, projektet stod i **40**
    /// — alltså 4,075× — och appen sträckte då **varje** stämma till klämningen 4,0× och
    /// skrev **486 MB per stämma** (5,8 GB på tre minuter). Provet håller taket: innanför
    /// 0,5–2,0 följer klippet tempot, utanför spelar det **som inspelat** i stället.
    #[test]
    fn a_tempo_more_than_double_from_the_source_is_not_stretched() {
        let untouched = FollowDecision {
            mode: FollowMode::Untouched,
            ratio: 1.0,
        };
        // Alex' fall: 163 → 40 = 4,075×. Det får inte bli en sträckning.
        assert_eq!(
            decide(163.0, 40.0, true, false),
            untouched,
            "4× från källans tempo ska spela originalet, inte en sträckt version"
        );
        // Exakt 2× (163 → 326) och exakt 0,5× (163 → 81,5) är innanför: de följer.
        let double = decide(163.0, 326.0, true, false);
        assert_eq!(double.mode, FollowMode::Stretch);
        assert!((double.ratio - 2.0).abs() < 1e-4);
        let half = decide(163.0, 81.5, true, false);
        assert_eq!(half.mode, FollowMode::Stretch);
        assert!((half.ratio - 0.5).abs() < 1e-4);
        // Ett vanligt tempodrag följer förstås fortfarande.
        assert_eq!(decide(163.0, 160.0, true, false).mode, FollowMode::Stretch);
        // Och samma regel svarar appen med när den ska säga VARFÖR ett klipp står still.
        assert!(beyond_follow_band(163.0, 40.0));
        assert!(!beyond_follow_band(163.0, 160.0));
        assert!(!beyond_follow_band(0.0, 40.0), "okänt tempo är ett annat svar");
    }

    /// **De två svaren får bara skilja i ett fall** (mätt 2026-09-20).
    ///
    /// `decide` svarar "vad ska göras just nu" och `follows_tempo` svarar "hänger klippet med
    /// när tempot ändras". De är samma regel utom för klippet som **redan ligger rätt**: det
    /// ska inte sträckas nu, men det följer med nästa gång tempot rör sig. Glider de isär
    /// någon annanstans faller det här provet — och då är det räknarna i gränssnittet som
    /// ljuger för Alex, inte ljudet.
    #[test]
    fn follows_tempo_agrees_with_decide() {
        for &source in &[0.0, 163.0, 120.0, 400.0] {
            for &project in &[40.0, 120.0, 163.0, 240.0] {
                for &switch in &[true, false] {
                    for &tape in &[true, false] {
                        let d = decide(source, project, switch, tape);
                        let already_right = source > 0.0
                            && switch
                            && ((project / source).clamp(MIN_RATIO, MAX_RATIO) - 1.0).abs() <= 1e-4;
                        assert_eq!(
                            follows_tempo(source, project, switch, tape),
                            d.mode != FollowMode::Untouched || already_right,
                            "källa {source}, projekt {project}, switch {switch}, band {tape}"
                        );
                    }
                }
            }
        }
    }

    /// **Bandspelaren är ett aktivt val och kläms som förut** — det är följningen
    /// (sträckningen) som har ett tak. Klämningen till sångstudiens spann bor i
    /// `ui::app::stretch_ratio_for` och i renderaren, och prövas där.
    #[test]
    fn the_policy_clamps_like_the_vocal_studio_does() {
        let tape = decide(120.0, 10_000.0, true, true);
        assert_eq!(tape.mode, FollowMode::Tape);
        assert!((tape.ratio - MAX_RATIO).abs() < 1e-6);
        let tape_down = decide(120.0, 1.0, true, true);
        assert_eq!(tape_down.mode, FollowMode::Tape);
        assert!((tape_down.ratio - MIN_RATIO).abs() < 1e-6);
        // Utan bandspelarläget är samma två fall **orörda**: klippet spelar som inspelat.
        assert_eq!(decide(120.0, 10_000.0, true, false).ratio, 1.0);
        assert_eq!(decide(120.0, 1.0, true, false).ratio, 1.0);
    }


    /// **Diskantmått**: RMS av första differensen delat med RMS — en billig, längdoberoende
    /// proxy för hur mycket av de höga frekvenserna som finns kvar. En tonhöjdsbevarande
    /// sträckning ska lämna den *oförändrad*; faller den har motorn ätit diskant, stiger den
    /// har den lagt till brus. Samma sorts fråga som 8.10c avgjorde med −45 % för 60 ms-kornet.
    fn brightness(x: &[f32]) -> f32 {
        if x.len() < 3 {
            return 0.0;
        }
        let mut num = 0.0f64;
        let mut den = 0.0f64;
        for i in 1..x.len() {
            let d = (x[i] - x[i - 1]) as f64;
            num += d * d;
            let v = x[i] as f64;
            den += v * v;
        }
        if den <= 0.0 {
            return 0.0;
        }
        ((num / den) as f32).sqrt()
    }

    /// **Motorerna mot varandra** (körs manuellt):
    /// `cargo test --release --bin sonix the_two_engines_on_the_same_stems -- --ignored --nocapture`
    ///
    /// Samma källfiler, samma slagletning, två motorer — siffrorna är domen, inte intrycket.
    /// Provet finns för att domen ska gå att göra **om**: byter någon motor, ändras
    /// överlappet eller kommer en ny version av biblioteket, avgör samma tabell igen.
    ///
    /// Det **tydliga** provet räknar musikaliska anslag (den siffra som hörs); det **täta**
    /// fångar kornkanterna. Båda behövs — utan det täta kan "överskottet" vara detektorns
    /// eget brus i stället för sträckningens. Längden skrivs ut också: en motor som ger fel
    /// antal sampel glider ur takt, hur fina anslagen än är.
    #[test]
    #[ignore]
    fn the_two_engines_on_the_same_stems() {
        let dense = crate::audio::onset::OnsetParams {
            min_gap_ms: 5.0,
            ..Default::default()
        };
        let clear = crate::audio::onset::OnsetParams {
            sensitivity: 2.5,
            min_gap_ms: 20.0,
            window_ms: 80.0,
        };
        let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
        let project = 110.0f32;
        let file_ratio = 120.0 / project;
        let expected = |frames: usize| ((frames as f64) * (file_ratio as f64)).round() as usize;

        for name in [
            "Broken (Vocals).wav",
            "Broken (Backing Vocals).wav",
            "Broken (Drums).wav",
            "Broken (Bass).wav",
        ] {
            let path = home.join("imported_stems/Broken").join(name);
            if !path.exists() {
                println!("hoppar: {name} finns inte");
                continue;
            }
            let Ok((l, r, sr)) = crate::audio::load_wav_pcm(&path.to_string_lossy()) else {
                println!("hoppar: {name} gick inte att läsa");
                continue;
            };
            let src_dense = crate::audio::onset::detect_onsets(&l, sr as f32, &dense).len();
            let src_clear = crate::audio::onset::detect_onsets(&l, sr as f32, &clear).len();
            let src_bright = brightness(&l);
            println!("{name}\n   källan: {src_dense} täta, {src_clear} tydliga anslag, diskant {src_bright:.4}");
            for (engine, out) in [
                ("WSOLA (vår)", stretch_stereo(&l, &r, file_ratio, sr as f32)),
                ("Signalsmith", stretch_signalsmith(&l, &r, file_ratio, sr as f32)),
            ] {
                let out_dense = crate::audio::onset::detect_onsets(&out.0, sr as f32, &dense).len();
                let out_clear = crate::audio::onset::detect_onsets(&out.0, sr as f32, &clear).len();
                let pct = if src_clear == 0 {
                    0.0
                } else {
                    100.0 * (out_clear as f32 - src_clear as f32) / src_clear as f32
                };
                let bright = brightness(&out.0);
                let bright_pct = 100.0 * (bright - src_bright) / src_bright;
                println!(
                    "   {engine:12} täta {out_dense:>6} ({:+}), tydliga {out_clear:>5} ({pct:+.2} %), \
                     diskant {bright:.4} ({bright_pct:+.1} %), längd {} sampel (väntat {})",
                    out_dense as i64 - src_dense as i64,
                    out.0.len(),
                    expected(l.len())
                );
            }
        }
    }

    /// **Svepet som valde block och intervall** (körs manuellt):
    /// `cargo test --release --bin sonix the_block_and_interval_sweep -- --ignored --nocapture`
    ///
    /// Två domare, för de drar åt olika håll. **Klickföljden** (syntetisk, härledd ur
    /// `the_engine_does_not_invent_transients`) straffar smetning: en lång FFT-ruta med
    /// glest intervall hinner inte följa en transient och delar den i flera. **Trummorna**
    /// (Alex' egen stämma) straffar det motsatta: för korta rutor tappar tonhöjd och kropp.
    /// Ett par som vinner på båda är det som gäller — vinner ingen, står förvalet kvar och
    /// siffrorna visar varför.
    #[test]
    #[ignore]
    fn the_block_and_interval_sweep() {
        let sr = 44_100.0f32;
        let dense = crate::audio::onset::OnsetParams {
            min_gap_ms: 5.0,
            ..Default::default()
        };
        let clear = crate::audio::onset::OnsetParams {
            sensitivity: 2.5,
            min_gap_ms: 20.0,
            window_ms: 80.0,
        };

        // Klickföljden: 24 klick, 4 per sekund i 6 s.
        let clicks = 24usize;
        let mut click_buf = vec![0.0f32; (sr * 6.0) as usize];
        for i in 0..clicks {
            let at = (i as f32 * sr / 4.0) as usize;
            if at + 1 < click_buf.len() {
                click_buf[at] = 0.9;
                click_buf[at + 1] = -0.7;
            }
        }
        let click_src = crate::audio::onset::detect_onsets(&click_buf, sr, &dense).len();

        let stem = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join("imported_stems/Broken/Broken (Drums).wav");
        let drums = crate::audio::load_wav_pcm(&stem.to_string_lossy()).ok();
        let (src_dense, src_clear) = match &drums {
            Some((l, _, s)) => (
                crate::audio::onset::detect_onsets(l, *s as f32, &dense).len(),
                crate::audio::onset::detect_onsets(l, *s as f32, &clear).len(),
            ),
            None => (0, 0),
        };
        println!("klickföljden: {click_src} anslag i källan — 0 överskott är kravet");
        if drums.is_some() {
            println!("trummorna:   {src_dense} täta / {src_clear} tydliga i källan");
        }

        for (block_ms, interval_ms) in [
            (120.0, 30.0), // bibliotekets förval — musik, inte transienter
            (120.0, 10.0),
            (80.0, 10.0),
            (40.0, 10.0),
            (20.0, 5.0),
        ] {
            let (cl, _cr) = stretch_signalsmith_with(&click_buf, &click_buf, 120.0 / 110.0, sr, block_ms, interval_ms);
            let click_out = crate::audio::onset::detect_onsets(&cl, sr, &dense).len();
            let mut line = format!(
                "   block {block_ms:>5.0} / intervall {interval_ms:>4.0} ms: klick {click_out:>3} ({:+})",
                click_out as i64 - click_src as i64
            );
            if let Some((l, r, s)) = &drums {
                let (dl, _dr) = stretch_signalsmith_with(l, r, 120.0 / 110.0, *s as f32, block_ms, interval_ms);
                let d_dense = crate::audio::onset::detect_onsets(&dl, *s as f32, &dense).len();
                let d_clear = crate::audio::onset::detect_onsets(&dl, *s as f32, &clear).len();
                let pct = if src_clear == 0 {
                    0.0
                } else {
                    100.0 * (d_clear as f32 - src_clear as f32) / src_clear as f32
                };
                line.push_str(&format!(
                    " | trummor täta {d_dense:>6} ({:+}), tydliga {d_clear:>5} ({pct:+.2} %) | längd {}",
                    d_dense as i64 - src_dense as i64,
                    dl.len()
                ));
            }
            println!("{line}");
        }
    }

    /// **Lyssningsprovet** (körs manuellt): samma 20 sekunder genom **båda** motorerna, så att
    /// örat får döma där siffrorna går isär. Mätt 2026-09-13 (Broken, 120 → 110 BPM):
    ///
    /// | | diskant | falska anslag (sång) |
    /// | :-- | --: | --: |
    /// | Vår WSOLA | −9,6 % | +9,8 % |
    /// | Signalsmith | **−2,3 %** | +16,0 % |
    ///
    /// Ingen av dem vinner båda. Provet skriver två wav-filer och skriver ut sökvägarna —
    /// sedan är det öronen som avgör, inte fler tabeller.
    #[test]
    #[ignore]
    fn the_two_engines_for_the_ear() {
        let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
        let path = home.join("imported_stems/Broken/Broken (Vocals).wav");
        if !path.exists() {
            println!("hoppar: {} finns inte", path.display());
            return;
        }
        let Ok((l, r, sr)) = crate::audio::load_wav_pcm(&path.to_string_lossy()) else {
            println!("hoppar: kunde inte läsa {}", path.display());
            return;
        };
        let ratio = 120.0f32 / 110.0;
        let out_frames = ((20.0 * sr as f32) as usize).max(1);
        let take = ((out_frames as f32) / ratio) as usize;
        let take = take.min(l.len());
        let (l, r) = (l[..take].to_vec(), r[..take].to_vec());

        let dir = std::env::temp_dir().join("sonix_engine_ear");
        let _ = std::fs::create_dir_all(&dir);
        for (name, out) in [
            ("vara-20s", stretch_stereo(&l, &r, ratio, sr as f32)),
            ("signalsmith-20s", stretch_signalsmith(&l, &r, ratio, sr as f32)),
        ] {
            let file = dir.join(format!("{name}.wav"));
            match crate::audio::exporter::write_stem_wav(
                &file.to_string_lossy(),
                &out.0,
                &out.1,
                sr,
            ) {
                Ok(()) => println!("{}", file.display()),
                Err(e) => println!("kunde inte skriva {}: {e}", file.display()),
            }
        }
    }
}

/// **Warp-lägena gör något, och det här är vad de gör** — mätt, inte antaget.
///
/// Klickrad sträckt 2× (klick var 250 ms, 40 sampel långa) och en ren 220 Hz-ton
/// sträckt 2×. Tre tal per läge: topp, crest (topp/medel) och **förhandsläckage** —
/// energin i de 5 ms *före* varje klick, alltså den utsmetning som hörs som en svans
/// bakåt i tiden.
#[test]
fn the_warp_modes_differ_in_measured_smearing() {
    use crate::audio::warp::WarpMode;
    let sr = 48_000.0f32;
    let mut click = vec![0.0f32; (sr * 2.0) as usize];
    for k in 0..8 {
        let at = (k as f32 * 0.25 * sr) as usize;
        if at + 40 < click.len() {
            for i in 0..40 {
                let env = (-(i as f32) / 6.0).exp();
                click[at + i] = if i % 2 == 0 { 0.9 * env } else { -0.9 * env };
            }
        }
    }
    let tone: Vec<f32> = (0..(sr * 2.0) as usize)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / sr).sin())
        .collect();

    let mut rows = Vec::new();
    for mode in WarpMode::ALL {
        let (out, _) = stretch_stereo_mode(&click, &click, 2.0, sr, mode);
        let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let rms = (out.iter().map(|v| v * v).sum::<f32>() / out.len() as f32).sqrt();
        // Förhandsläckage: energi strax före varje klick (klicket ligger på 0,5 s intervall
        // i den sträckta filen, 2× källans intervall).
        // Läckaget mäts före **den faktiska toppen**, inte före den förväntade positionen:
        // annars mäter man avrundningsfel i stället för utsmetning.
        let before = (sr * 0.005) as usize;
        let window = (sr * 0.020) as usize;
        let mut pre = 0.0f64;
        for k in 1..8 {
            let at = (k as f32 * 0.5 * sr) as usize;
            let lo = at.saturating_sub(window);
            let hi = (at + window).min(out.len());
            if lo >= before && hi > lo {
                let mut peak_at = lo;
                for i in lo..hi {
                    if out[i].abs() > out[peak_at].abs() {
                        peak_at = i;
                    }
                }
                pre += out[peak_at - before..peak_at]
                    .iter()
                    .map(|v| (*v as f64) * (*v as f64))
                    .sum::<f64>();
            }
        }
        let (tone_out, _) = stretch_stereo_mode(&tone, &tone, 2.0, sr, mode);
        let tone_rms = (tone_out.iter().map(|v| v * v).sum::<f32>() / tone_out.len() as f32).sqrt();
        rows.push((mode, peak, peak / rms.max(1e-9), pre.sqrt(), tone_rms));
    }
    for (mode, peak, crest, pre, tone_rms) in &rows {
        println!(
            "{:8} topp {peak:.3}  crest {crest:6.2}  förhandsläckage {pre:.4}  ton-RMS {tone_rms:.4}",
            mode.label()
        );
    }
    // Alla lägen ska höras och hålla tonen — det är inte kosmetiska skillnader.
    for (mode, peak, _, _, tone_rms) in &rows {
        assert!(*peak > 0.05, "{:?} gav tyst klickutdata", mode);
        assert!(*tone_rms > 0.2, "{:?} tappade tonen ({tone_rms:.3})", mode);
    }
    // Lägena får inte vara samma sak: minst ett av talen skiljer sig klart mellan
    // det kortaste och det längsta kornet.
    let fast = &rows[0];
    let smooth = &rows[2];
    let differs = (fast.2 - smooth.2).abs() > 1.0
        || (fast.3 - smooth.3).abs() > 0.001
        || (fast.4 - smooth.4).abs() > 0.002;
    assert!(differs, "Fast och Smooth gav samma resultat — då är läget ingen inställning");
    // Kort korn ska ha **mer** förhandsläckage än långt: det är mätningen, och det är
    // därför vi inte kallar det kortaste kornet för "Beats".
    assert!(
        fast.3 > smooth.3,
        "kort korn läcker mer bakåt än långt: {:.4} mot {:.4}",
        fast.3,
        smooth.3
    );
}

/// **Anslagskopieringen mätt mot standardvägen.** Samma klickrad som lägesprovet, sträckt
/// 2×: attacken ska behålla sin topp, och det ska inte ligga energi före anslaget.
#[test]
fn copying_the_attack_keeps_the_transient_that_stretching_smears() {
    let sr = 48_000.0f32;
    let mut click = vec![0.0f32; (sr * 2.0) as usize];
    for k in 0..8 {
        let at = (k as f32 * 0.25 * sr) as usize;
        if at + 40 < click.len() {
            for i in 0..40 {
                let env = (-(i as f32) / 6.0).exp();
                click[at + i] = if i % 2 == 0 { 0.9 * env } else { -0.9 * env };
            }
        }
    }
    // Läckaget mäts **före den faktiska toppen**, inte före den förväntade positionen:
    // annars mäter man några sampels avrundningsfel i stället för utsmetning.
    let stats = |out: &[f32]| {
        let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let rms = (out.iter().map(|v| v * v).sum::<f32>() / out.len() as f32).sqrt();
        let before = (sr * 0.005) as usize;
        let window = (sr * 0.020) as usize;
        let mut pre = 0.0f64;
        for k in 1..8 {
            let at = (k as f32 * 0.5 * sr) as usize;
            let lo = at.saturating_sub(window);
            let hi = (at + window).min(out.len());
            if lo >= before && hi > lo {
                let mut best = lo;
                for i in lo..hi {
                    if out[i].abs() > out[best].abs() {
                        best = i;
                    }
                }
                if best > before {
                    pre += out[best - before..best]
                        .iter()
                        .map(|v| (*v as f64) * (*v as f64))
                        .sum::<f64>();
                }
            }
        }
        (peak, peak / rms.max(1e-9), pre.sqrt())
    };

    let (plain, _) = stretch_stereo_mode(&click, &click, 2.0, sr, crate::audio::warp::WarpMode::Complex);
    let (copied, _) = stretch_stereo_transient(&click, &click, 2.0, sr);
    let (pk_plain, _cr_plain, pre_plain) = stats(&plain);
    let (pk_copy, _cr_copy, pre_copy) = stats(&copied);
    println!(
        "topp: kopia {pk_copy:.3} mot standard {pk_plain:.3}  förhandsläckage: kopia {pre_copy:.4} mot {pre_plain:.4}"
    );

    // Trumloop: kagge på slagen, hi-hat på åttondelarna, 120 BPM i två takter.
    let beat = (sr * 0.5) as usize;
    let mut loop_src = vec![0.0f32; beat * 4];
    for (i, at) in (0..loop_src.len()).step_by(beat / 2).enumerate() {
        let len = if i % 2 == 0 { 900 } else { 300 };
        let amp = if i % 2 == 0 { 0.9 } else { 0.35 };
        for j in 0..len.min(loop_src.len() - at) {
            let env = (-(j as f32) / (len as f32 / 4.0)).exp();
            let t = j as f32 / sr;
            let body = (std::f32::consts::TAU * 60.0 * t).sin();
            loop_src[at + j] += amp * env * if i % 2 == 0 { body } else { 0.5 };
        }
    }
    let params = crate::audio::onset::OnsetParams::default();
    let ratio = 1.35f32;
    let (mr, _) = stretch_stereo_mode(&loop_src, &loop_src, ratio, sr, crate::audio::warp::WarpMode::Complex);
    let (mt, _) = stretch_stereo_transient(&loop_src, &loop_src, ratio, sr);
    let n_src = crate::audio::onset::detect_onsets(&loop_src, sr, &params).len();
    let n_r = crate::audio::onset::detect_onsets(&mr, sr, &params).len();
    let n_t = crate::audio::onset::detect_onsets(&mt, sr, &params).len();
    let pk = |v: &[f32]| v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    println!(
        "trumloop 1,35x: anslag källa {n_src}, Complex {n_r}, Transient {n_t}; topp källa {:.3}, Complex {:.3}, Transient {:.3}",
        pk(&loop_src), pk(&mr), pk(&mt)
    );

    assert_eq!(copied.len(), click.len() * 2, "längden ska vara exakt ratio");
    assert!(pk_copy >= pk_plain, "kopian ska vara minst lika stark: {pk_copy:.3} mot {pk_plain:.3}");
    assert!(
        pre_copy <= pre_plain + 1e-6,
        "kopian får inte läcka mer bakåt: {pre_copy:.4} mot {pre_plain:.4}"
    );

    // **Beviset, inte statistiken:** varje anslag i källan finns ordagrant i resultatet.
    // Attacken kopieras, så den kan inte smetas ut — och det är något kornbaserade lägen
    // inte kan lova. Provet kräver också att standardvägen **inte** klarar samma sak,
    // annars mäter det ingenting.
    // Jämför de första 2 ms av attacken: resten av de 5 ms korsar över i den sträckta
    // kroppen, och då är den med flit inte längre ordagrann.
    let attack = ((0.002 * sr) as usize).max(1);
    let onsets = crate::audio::onset::detect_onsets(&loop_src, sr, &params);
    assert!(onsets.len() >= 4, "provet behöver anslag att jämföra");
    let mut exact = 0usize;
    let mut plain_exact = 0usize;
    for onset in &onsets {
        let expected = (*onset as f64 * ratio as f64) as usize;
        let lo = expected.saturating_sub(64);
        let hi = (expected + 64).min(mt.len().saturating_sub(attack));
        if hi <= lo {
            continue;
        }
        // Hitta var attacken faktiskt ligger (avrundningen flyttar den några sampel).
        let mut best = lo;
        let mut best_err = f32::INFINITY;
        for cand in lo..hi {
            let err = (*onset..*onset + 8)
                .zip(cand..cand + 8)
                .map(|(a, b)| (loop_src[a] - mt[b]).abs())
                .fold(0.0f32, f32::max);
            if err < best_err {
                best_err = err;
                best = cand;
            }
        }
        let diff = (*onset..*onset + attack)
            .zip(best..best + attack)
            .map(|(a, b)| (loop_src[a] - mt[b]).abs())
            .fold(0.0f32, f32::max);
        if diff < 1e-6 {
            exact += 1;
        }
        let plain_diff = (*onset..*onset + attack)
            .zip(best..best + attack)
            .map(|(a, b)| (loop_src[a] - plain[b.min(plain.len() - 1)]).abs())
            .fold(0.0f32, f32::max);
        if plain_diff < 1e-6 {
            plain_exact += 1;
        }
    }
    assert_eq!(exact, onsets.len(), "varje attack ska finnas ordagrant: {exact} av {}", onsets.len());
    assert!(
        plain_exact < onsets.len(),
        "standardvägen klarade också alla attacker ({plain_exact} av {}) — då mäter provet ingenting",
        onsets.len()
    );
}

/// **På riktiga loopar.** Samma jämförelse som den syntetiska, men på Abletons egna
/// trumloopar: hur många anslag finns kvar efter sträckning 1,35× jämfört med källan.
///
/// **Resultatet var inte det väntade:** Transientläget är inte bättre på det här måttet
/// (avvikelse 9,31 anslag mot Complex 8,97 på 39 loopar, alltså samma inom bruset). Det
/// som skiljer är attackens trohet, inte antalet anslag — läget kopierar attacken ordagrant
/// och är därför rätt för trummor, men det är ingen universalbättring. Provet låser att
/// skillnaden är liten, så att ingen senare tror att läget löser något det inte gör.
///
/// Körs bara med `SONIX_ASD_DIR`.
#[test]
fn real_drum_loops_get_about_the_same_onset_count_in_both_modes_if_available() {
    use crate::audio::warp::WarpMode;
    let Ok(root) = std::env::var("SONIX_ASD_DIR") else {
        return;
    };
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.to_string_lossy().ends_with(".wav.asd") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(std::path::Path::new(&root), &mut files);
    files.sort();
    // Bara trumloopar: de har tydliga anslag, som är det läget handlar om.
    files.retain(|p| p.to_string_lossy().contains("/Drums/"));

    let params = crate::audio::onset::OnsetParams::default();
    let (mut d_complex, mut d_transient, mut n_files) = (0i64, 0i64, 0usize);
    for asd in files.iter().take(40) {
        let wav = asd.with_extension("");
        let Ok((left, right, sr)) = crate::audio::wav_reader::load_wav_pcm(&wav.to_string_lossy())
        else {
            continue;
        };
        if left.len() < (sr as usize) {
            continue;
        }
        let src = crate::audio::onset::detect_onsets(&left, sr as f32, &params).len();
        if src < 4 {
            continue;
        }
        let (cl, _) = stretch_stereo_mode(&left, &right, 1.35, sr as f32, WarpMode::Complex);
        let (tl, _) = stretch_stereo_mode(&left, &right, 1.35, sr as f32, WarpMode::Transient);
        let c = crate::audio::onset::detect_onsets(&cl, sr as f32, &params).len();
        let t = crate::audio::onset::detect_onsets(&tl, sr as f32, &params).len();
        let expected = (src as f32 * 1.35) as i64;
        d_complex += (c as i64 - expected).abs();
        d_transient += (t as i64 - expected).abs();
        n_files += 1;
    }
    assert!(n_files >= 5, "för få loopar att mäta på: {n_files}");
    let complex = d_complex as f32 / n_files as f32;
    let transient = d_transient as f32 / n_files as f32;
    println!(
        "{n_files} riktiga trumloopar, avvikelse i antal anslag mot väntat: Complex {complex:.2}, Transient {transient:.2}"
    );
    assert!(
        (complex - transient).abs() <= complex.max(1.0) * 0.5,
        "lägena ska ligga nära varandra på det här måttet: {complex:.2} mot {transient:.2}"
    );
}

