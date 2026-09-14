//! Onset-detektering och slicekarta (Fas 8.7).
//!
//! **Vad problemet är.** Choppern har haft **en** trim-ruta per kanal
//! (`sample_start`/`sample_end` i procent av filen) och en knapp märkt
//! "Transient" som i själva verket bara satte slutpunkten till 18 % av filen.
//! Ett etablerat chop-arbetsflöde (FL Studio, Ableton, Reaper, Bitwig) gör tre
//! saker: hittar **transients/onsets**, mappar varje slice till en not/pad, och
//! låter användaren nudga, fadea och trigga slicen från sin egen start. Den här
//! modulen gör det första: hitta slagen och räkna fram en slicekarta.
//!
//! **Varför tidsdomän och ingen FFT.** Repot har ingen FFT (inga fft-beroenden i
//! `Cargo.toml`), och för det choppern ska göra — hitta slagen i en trumloop — är
//! den halvvågslikriktade förstadifferensen en beprövad väg: en derivation är ett
//! högpassfilter, så den betonar precis den höga frekvensenergi som ett anslag
//! har. Det är samma idé som **HFC** ("high frequency content") — den
//! onset-funktion aubio defaultar till och kallar effektiv för perkussiva onsets.
//! Spektral flux med FFT (librosa, SuperFlux) är starkare på melodiöst och
//! vibrato-rikt material; det står i `ROADMAP.md` som ett senare steg, inte som
//! något den här modulen låtsas om.
//!
//! **Tröskeln är lokal, inte global.** Varje sampel jämförs med medelvärdet i ett
//! fönster runt sig (60 ms som standard). En global tröskel hade missat slaget
//! efter ett starkt parti och hittat brus i det svaga — det var det felet
//! "Transient"-knappen gjorde i sin enklaste form.
//! Status: stabil — slagletning och slicekarta (8.7 steg 1 + 2)
//! Rör inte: mät på en jämn ton först: en detektor är en mätning, och 4 ms enpolsfilter + centrerad tröskel är den enda variant som ger noll falska slag

/// Hur känslig detekteringen ska vara och hur tätt slag får ligga.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OnsetParams {
    /// Hur mycket en topp måste sticka över sin omgivning (multiplikator på
    /// medelvärdet i fönstret). Lågt = känsligare: 1,3 hittar svaga slag,
    /// 2,0 hittar bara de tydliga.
    pub sensitivity: f32,
    /// Kortaste avstånd mellan två slag i millisekunder. Två träffar närmare
    /// varandra än så blir **ett** slag — annars blir en enda trumma till fem
    /// slicar.
    pub min_gap_ms: f32,
    /// Fönstret som toppen jämförs med, i millisekunder (hälften före, hälften
    /// efter).
    pub window_ms: f32,
}

impl Default for OnsetParams {
    fn default() -> Self {
        // Standardvärdena är valda efter vad en trumloop brukar se ut: 30 ms
        // minsta avstånd döljer en enda trummas dubbelslag, och 1,5× omgivningen
        // kräver att anslaget syns tydligt i höljet.
        Self { sensitivity: 1.5, min_gap_ms: 30.0, window_ms: 60.0 }
    }
}

/// En slice i **källans sampleframes** (`end` är exklusiv).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slice {
    pub start: usize,
    pub end: usize,
}

/// Anslags-höljet: halvvågslikriktad förstadifferens, utjämnad över ~2 ms.
///
/// Skillnaden `x[i] − x[i−1]` är ett högpassfilter (derivation), och att bara
/// behålla de positiva ändringarna gör att höljet stiger vid ett anslag och inte
/// vid en utklingning. Utjämningen tar bort enstaka sampeltoppar som annars hade
/// blivit egna "slag".
pub fn onset_envelope(samples: &[f32], sample_rate: f32) -> Vec<f32> {
    let n = samples.len();
    let mut diff = vec![0.0f32; n];
    for i in 1..n {
        let d = samples[i] - samples[i - 1];
        diff[i] = if d > 0.0 { d } else { 0.0 };
    }
    // Utjämning med ett **envpolsfilter**, inte ett boxfilter — och med en
    // tidskonstant som är vald ur en mätning, inte ur magkänsla.
    //
    // Mätt på en jämn 220 Hz-ton (0,8 i amplitud): höljet `max(Δx, 0)` har en
    // rippel på grundfrekvensen med ~0,8 av medelvärdets amplitud. Vid 2 ms
    // dämpas 220 Hz bara till 0,23, så rippeln blir ±18 % — och med en tröskel
    // på 1,5 × medelvärdet gav en **jämn ton 32 slag** (uppmätt: kvoten
    // env/tröskel gick över 1,5 om och om igen). Vid 4 ms dämpas 220 Hz till
    // 0,18, rippeln blir ±14 %, och kvoten stannar under tröskeln. Två klick som
    // ligger 12 ms isär är fortfarande två maxima, så känsligheten räcker.
    one_pole(&diff, sample_rate, 4.0)
}

/// Enpolsfilter (RC-lågpass) med tidskonstanten `tau_ms`.
fn one_pole(x: &[f32], sample_rate: f32, tau_ms: f32) -> Vec<f32> {
    let n = x.len();
    let tau_frames = (sample_rate.max(1.0) * tau_ms / 1000.0).max(1.0);
    let alpha = 1.0 - (-1.0 / tau_frames).exp();
    let mut out = vec![0.0f32; n];
    let mut acc = 0.0f32;
    for i in 0..n {
        acc += (x[i] - acc) * alpha;
        out[i] = acc;
    }
    out
}

/// Hittar slagen i `samples` och lämnar deras samplepositioner i stigande ordning.
///
/// Tre steg, i den ordning de etablerade verktygen gör dem:
/// 1. **Lokal tröskel** — en topp måste ligga över `sensitivity` × medelvärdet i
///    sitt fönster.
/// 2. **Topptest** — sampeln ska vara max både i sin vänstra och sin högra
///    halva av fönstret (annars är den bara en sluttning).
/// 3. **Backtrack** — varje slag flyttas bakåt till det närmaste föregående
///    energiminimumet, inom högst 20 ms. Det är samma sak som librossas
///    `onset_detect(backtrack=True)` gör, och det är vad som gör att slicen börjar
///    **strax före** anslaget i stället för på det. En slice som börjar mitt i
///    attacken klickar, och en som börjar efter den tappar anslagets första kant.
pub fn detect_onsets(samples: &[f32], sample_rate: f32, params: &OnsetParams) -> Vec<usize> {
    if samples.len() < 8 || sample_rate <= 0.0 {
        return Vec::new();
    }
    let env = onset_envelope(samples, sample_rate);
    let ms = |v: f32| ((sample_rate * v / 1000.0).round() as usize).max(1);
    let min_gap = ms(params.min_gap_ms);
    let max_back = ms(20.0);

    // Tröskeln är **lokal och centrerad**: medelvärdet av höljet i ett fönster
    // runt varje sampel. Ett fast tal vore fel (ett starkt parti skulle döva
    // detekteringen och ett svagt skulle ge brus för slag), och en *släpande*
    // tröskellinje vore också fel — mätt: den börjar på noll, så de första
    // ~100 ms låg höljet över 1,5 × tröskeln och en jämn ton fick fem slag
    // innan linjen hade kommit ikapp. Ett centrerat fönster ser framåt och har
    // inget sådant uppvaknande.
    let half = ((sample_rate.max(1.0) * params.window_ms.max(1.0) / 1000.0).round() as usize)
        .div_ceil(2)
        .max(1);
    let n = env.len();
    let mut prefix = vec![0.0f32; n + 1];
    for i in 0..n {
        prefix[i + 1] = prefix[i] + env[i];
    }
    let threshold_at = |i: usize| -> f32 {
        let lo = i.saturating_sub(half);
        let hi = (i + half + 1).min(n);
        (prefix[hi] - prefix[lo]) / (hi - lo).max(1) as f32
    };
    let sensitivity = params.sensitivity.max(1.0);

    // Kandidater: **lokala maxima** i höljet som ligger över tröskellinjen.
    //
    // Mätt, inte gissat: den första varianten tog maxvärdet i varje sammanhängande
    // sträcka över tröskeln. Då blev en dubbeltrumma ett enda slag (sträckan var
    // sammanhängande) och en jämn ton kunde ge en lång rad slag när rippeln i
    // höljet råkade ligga över linjen. Ett lokalt maximum är den punkt där
    // anslaget faktiskt är, och minsta avståndet avgör sedan om två tätt liggande
    // maxima är ett eller två slag.
    let mut peaks: Vec<usize> = Vec::new();
    for i in 1..n.saturating_sub(1) {
        if env[i] <= threshold_at(i) * sensitivity {
            continue;
        }
        if env[i] < env[i - 1] || env[i] <= env[i + 1] {
            continue;
        }
        if let Some(&prev) = peaks.last()
            && i - prev < min_gap
        {
            continue;
        }
        peaks.push(i);
    }

    // Backtrack till energiminimum före toppen (högst 20 ms bakåt).
    for p in peaks.iter_mut() {
        let lo = p.saturating_sub(max_back);
        let mut best = *p;
        let mut best_val = env[*p];
        let mut i = *p;
        while i > lo {
            i -= 1;
            if env[i] < best_val {
                best_val = env[i];
                best = i;
            } else if env[i] > best_val * 4.0 + 1e-9 {
                // Höljet har börjat stiga igen: minimumet är passerat.
                break;
            }
        }
        *p = best;
    }
    peaks.dedup();
    peaks
}

/// **Var musiken börjar** i ett fönster, som sekunder i källan (Fas 8.14 steg 2).
///
/// Frågan är *"var börjar musiken?"* — inte *"var är alla slagen?"* (det är
/// [`detect_onsets`]) och inte *"var ligger anslaget?"*. Svaret är **första stunden
/// ljud**: 20 ms RMS över en golv-nivå som ligger −40 dB under fönstrets topp, så att
/// en tyst stämma och en stark stämma döms med samma mått.
///
/// **Varför inte detektorns svar rakt av** — mätt på Alex' egna stämmor 2026-09-13:
///
/// | Stämma | Hörbart | Detektorns första slag | Skillnad |
/// | :--- | ---: | ---: | ---: |
/// | Trummor | 0,600 s | 0,502 s | **−98 ms** (backningen före attacken) |
/// | Sång | 7,18 s | 7,28 s | +100 ms (den mjuka attacken sågs inte) |
/// | Gitarr | 0,00 s | 0,178 s | +178 ms (**inget** anslag i början) |
/// | Bas | 1,68 s | 1,689 s | +9 ms |
///
/// Backningen är rätt för en *slice* (den ska börja före sin attack), men fel för ett
/// *rutnät*: 98 ms är 5,7 % av ett slag i 140 BPM och hörs som flam — det var Alex'
/// *"hoppade till markören och klippte bort början"*. Och en mjuk stråke eller ett
/// legato-anslag har inget anslag alls för HFC-höljet att hitta. Därför är ljudet
/// ankaret, och detektorn får **finputsa** när den ser samma sak: ligger ett slag inom
/// 20 ms av den hörbara starten används dess sampelnoggrannhet.
///
/// `None` betyder att fönstret är tyst: ingen musik att sätta på ett rutnät, och då
/// hittar vi inte på någon (8.5-regeln).
pub fn music_start_source_secs(
    samples: &[f32],
    sample_rate: f32,
    from_secs: f32,
    window_secs: f32,
    params: &OnsetParams,
) -> Option<f32> {
    if sample_rate <= 0.0 || samples.is_empty() {
        return None;
    }
    let from = (from_secs.max(0.0) * sample_rate) as usize;
    if from >= samples.len() {
        return None;
    }
    let to = ((from_secs.max(0.0) + window_secs.max(0.01)) * sample_rate) as usize;
    let to = to.min(samples.len());
    if to <= from {
        return None;
    }
    let slice = &samples[from..to];
    let secs_at = |i: usize| (from + i) as f32 / sample_rate;

    // 1. Var hörs ljudet? 20 ms RMS mot fönstrets egen topp, −40 dB under den.
    let win = ((0.020 * sample_rate) as usize).max(1);
    let mut peak = 0.0f32;
    let mut env: Vec<f32> = Vec::with_capacity(slice.len() / win + 1);
    for c in slice.chunks(win) {
        let e = (c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32).sqrt();
        peak = peak.max(e);
        env.push(e);
    }
    if peak <= 0.0 {
        return None;
    }
    let floor = (peak * 10f32.powf(-40.0 / 20.0)).max(1e-5);
    let first_bucket = env.iter().position(|&e| e > floor)?;
    let audible = secs_at(first_bucket * win);

    // 2. Finputs: detektorns slag om det ligger inom 20 ms av det hörbara.
    let refined = detect_onsets(slice, sample_rate, params)
        .into_iter()
        .map(secs_at)
        .filter(|t| (t - audible).abs() <= 0.020)
        .min_by(|a, b| {
            (a - audible)
                .abs()
                .partial_cmp(&(b - audible).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    Some(refined.unwrap_or(audible))
}


/// Räknar fram en slicekarta ur slagpunkterna.
///
/// Slicarna täcker **hela** filen utan glapp och utan överlapp: varje slice
/// slutar där nästa börjar, och den sista går till filens slut. Två gränser
/// närmare varandra än `min_slice_ms` slås ihop (en slice som är kortare än
/// ~30 ms går inte att spela menligt ändå, och blir bara ett klick).
pub fn slices_from_onsets(
    onsets: &[usize],
    samples: usize,
    sample_rate: f32,
    min_slice_ms: f32,
) -> Vec<Slice> {
    if samples == 0 {
        return Vec::new();
    }
    let min_len = ((sample_rate.max(1.0) * min_slice_ms / 1000.0).round() as usize).max(1);
    let mut bounds: Vec<usize> = Vec::with_capacity(onsets.len() + 2);
    bounds.push(0);
    for &o in onsets {
        let o = o.min(samples);
        // Närmare än minsta längd från förra gränsen: hoppa över.
        if o >= bounds[bounds.len() - 1] + min_len {
            bounds.push(o);
        }
    }
    if bounds[bounds.len() - 1] != samples {
        bounds.push(samples);
    }
    let mut slices: Vec<Slice> = Vec::with_capacity(bounds.len().saturating_sub(1));
    for w in bounds.windows(2) {
        if w[1] > w[0] {
            slices.push(Slice { start: w[0], end: w[1] });
        }
    }
    slices
}

/// **Flytta en slicegräns — nudge** (Fas 8.7 steg 2).
///
/// En gräns inne i kartan ligger *mellan* två slicar: slutet på den ena är början på den nästa.
/// Att flytta bara den ena sidan hade lämnat ett glapp eller ett överlapp, och invarianten som
/// redan har ett prov — "kartan täcker hela filen utan glapp eller överlapp" — hade brutits.
/// Därför flyttas **båda**, i en och samma operation, och kartan förblir en sammanhängande
/// indelning av filen.
///
/// `boundary` är gränsens index: `0` är filens början och `slices.len()` är filens slut. De två
/// yttre är **filens kanter och rörs inte** — en karta som inte börjar på noll eller slutar på
/// slutet spelar inte hela filen längre, och det är en annan sak än att nudga.
///
/// `min_len` är kortaste tillåtna slice i **samma enhet som kartan** (choppern räknar i andel av
/// filen, 0…1). En gräns som skulle korsa sin granne **kläms** mot grannen i stället för att
/// slicen tas bort: att tyst slå ihop två slicar vore att ändra kartan, inte att flytta en gräns.
///
/// Ett oändligt eller `NaN`-igt `delta` lämnar kartan orörd — hellre oförändrat än förstört.
pub fn nudge_slice_boundary(
    slices: &[(f32, f32)],
    boundary: usize,
    delta: f32,
    min_len: f32,
) -> Vec<(f32, f32)> {
    if slices.is_empty() || !delta.is_finite() || boundary == 0 || boundary >= slices.len() {
        return slices.to_vec();
    }
    // Gränsen är slutet på slice `boundary - 1` och början på slice `boundary`.
    let (lower, upper) = (slices[boundary - 1].0, slices[boundary].1);
    // Grannen på var sida sätter taket: ingen av dem får bli kortare än `min_len`.
    let lo = (lower + min_len).min(upper);
    let hi = (upper - min_len).max(lower);
    let moved = (slices[boundary].0 + delta).clamp(lo.min(hi), hi.max(lo));
    let mut out = slices.to_vec();
    out[boundary - 1].1 = moved;
    out[boundary].0 = moved;
    out
}

/// **Hur många slicar ett rutnät rymmer** (Fas 8.7 steg 2).
///
/// Stegen är **tid** och raderna är **tonhöjd** — två axlar som betyder olika saker — och en
/// dump måste rymmas i **båda**. Det är det *mindre* antalet som sätter taket.
///
/// Regeln har ett eget prov av ett skäl: när piano roll-varianten skrevs togs radantalet (24)
/// och tiden lappades med `% 16`, vilket hade lagt slice 17 på samma steg som slice 1. Ett prov
/// här gör att det misstaget inte kan komma tillbaka.
pub fn grid_capacity(rows: usize, steps: usize) -> usize {
    rows.min(steps)
}

/// **Dumpa slicarna till stegraden** (Fas 8.7 steg 2).
///
/// Slice `i` hamnar på steg `i` och får noten `bas + i` — samma kromatiska adressering som
/// [`window_for_note`] redan spelar efter både live, från stegraden och i exporten. Därför
/// behövs **ingen motorändring**: noten *är* adressen. Det är samma princip som FL:s
/// "Convert to score and dump to piano roll" och Reapers "Create chromatic MIDI item from
/// slices" — att låta en analys bli spelbart material utan att kopiera ljud.
///
/// Fler slicar än steg **kapas**, och kapat talas om av anroparen. Att tyst slå ihop eller
/// tappa slicar vore ett annat ljud än kartan visar; den som har fler slicar än steg får välja
/// en längre takt först, och det är ett beslut — inte något en dump ska fatta i smyg.
///
/// Noten kläms till MIDI-omfånget, så en hög basnot inte kan linda runt (not 128 blir 0).
pub fn slices_to_steps(slices: &[(f32, f32)], base_note: u8, steps: usize) -> Vec<(usize, u8)> {
    slices
        .iter()
        .take(steps)
        .enumerate()
        .map(|(i, _)| (i, base_note.saturating_add(i as u8).min(127)))
        .collect()
}

/// Fönstret en not ska spela på en kanal med en **slicekarta** (Fas 8.7 steg 2).
///
/// **Kromatiskt från basnoten:** noten `bas + i` spelar slice `i`. Det är samma
/// princip som Reapers "Create chromatic MIDI item from slices" och Abeltons
/// Slice-to-MIDI, och den gör att slicekartan kan spelas från stegraden, från
/// piano rollen och i exporten utan ytterligare data — noten **är** adressen.
///
/// Noter utanför kartan (under basnoten, eller ovanför sista slicen) spelar
/// kanalens eget trimfönster, precis som innan kartan fanns. Att i stället
/// upprepa sista slicen hade varit ett påhittat ljud — samma regel som 8.5 vilar
/// på — och att låta dem vara tysta hade gjort ett tangentbord över kartan stumt
/// utan förklaring.
pub fn window_for_note(
    slices: &[(f32, f32)],
    base_note: u8,
    note: u8,
    fallback: (f32, f32),
) -> (f32, f32) {
    if slices.is_empty() {
        return fallback;
    }
    let idx = note as i32 - base_note as i32;
    if idx < 0 {
        return fallback;
    }
    match slices.get(idx as usize).copied() {
        Some((start, end)) if end > start => (start, end),
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44_100.0;

    /// En kanal med klick: ett kort anslag (två sampel med hög amplitud följt av
    /// tystnad) vid varje given tidpunkt.
    fn clicks(times_sec: &[f32], total_sec: f32) -> Vec<f32> {
        let mut buf = vec![0.0f32; (SR * total_sec) as usize];
        for &t in times_sec {
            let i = (SR * t) as usize;
            if i + 2 < buf.len() {
                buf[i] = 0.9;
                buf[i + 1] = -0.7;
            }
        }
        buf
    }

    #[test]
    fn four_clicks_become_four_onsets_where_the_clicks_are() {
        let times = [0.20, 0.70, 1.20, 1.70];
        let buf = clicks(&times, 2.0);
        let found = detect_onsets(&buf, SR, &OnsetParams::default());
        assert_eq!(found.len(), times.len(), "ett slag per klick: {found:?}");
        for (i, &t) in times.iter().enumerate() {
            let expected = (SR * t) as usize;
            let got = found[i];
            // Backtrack flyttar slaget strax *före* anslaget — aldrig efter.
            assert!(
                got <= expected && expected - got < (SR * 0.02) as usize,
                "slag {} hamnade på {got}, klicket på {expected}",
                i + 1
            );
        }
    }

    #[test]
    fn a_steady_tone_is_not_a_click_track() {
        // En jämn ton har ett litet men jämnt hölje: ingen tröskel ska passeras
        // efter att tonen har börjat. Ett enskilt anslag i början är rimligt —
        // det *är* en onset — men inte ett slag per period.
        let buf: Vec<f32> = (0..(SR * 1.0) as usize)
            .map(|i| (std::f32::consts::TAU * 220.0 * i as f32 / SR).sin() * 0.8)
            .collect();
        let found = detect_onsets(&buf, SR, &OnsetParams::default());
        assert!(
            found.is_empty(),
            "en jämn ton ska inte ge några slag alls (mätt: 0 efter den centrerade tröskeln): {found:?}"
        );
    }

    #[test]
    fn two_hits_inside_the_minimum_gap_become_one_slice() {
        // 12 ms mellan två träffar är ett dubbelslag, inte två slag.
        let buf = clicks(&[0.5, 0.512], 1.0);
        let found = detect_onsets(&buf, SR, &OnsetParams::default());
        assert_eq!(found.len(), 1, "dubbelslaget ska bli ett slag: {found:?}");

        // Med ett kortare minsta avstånd blir de två.
        let tight = OnsetParams { min_gap_ms: 5.0, ..OnsetParams::default() };
        assert_eq!(detect_onsets(&buf, SR, &tight).len(), 2);
    }

    #[test]
    fn the_slice_map_covers_the_whole_file_without_gaps_or_overlaps() {
        let buf = clicks(&[0.20, 0.70, 1.20], 2.0);
        let onsets = detect_onsets(&buf, SR, &OnsetParams::default());
        let slices = slices_from_onsets(&onsets, buf.len(), SR, 30.0);

        assert_eq!(
            slices.len(),
            onsets.len() + 1,
            "en slice per slag, plus stycket före det första"
        );
        assert_eq!(slices[0].start, 0, "kartan börjar på noll");
        assert_eq!(
            slices[slices.len() - 1].end,
            buf.len(),
            "sista slicen går till filens slut"
        );
        for w in slices.windows(2) {
            assert_eq!(w[0].end, w[1].start, "inga glapp och inget överlapp");
        }
        assert!(slices.iter().all(|s| s.end > s.start), "inga tomma slicar");
    }

    /// Taket är det mindre av tid och tonhöjd — och det är inte samma tal.
    #[test]
    fn the_grids_capacity_is_the_smaller_of_time_and_pitch() {
        assert_eq!(grid_capacity(24, 16), 16, "16 steg sätter taket, inte 24 rader");
        assert_eq!(grid_capacity(8, 16), 8, "få rader sätter taket");
        assert_eq!(grid_capacity(0, 16), 0);
        assert_eq!(grid_capacity(24, 0), 0);
    }

    /// **Kontraktet mellan dumpen och uppspelningen.** Dumpar man slicarna till stegraden måste
    /// varje stegs not spela **just den slicen** — annars är dumpen bara en rad ettor som låter
    /// fel. Provet prövar dem mot varandra, inte var för sig, så en ändrad adressering i den ena
    /// fångas av den andra.
    #[test]
    fn every_dumped_step_plays_the_slice_it_came_from() {
        let slices = vec![(0.0, 0.1), (0.1, 0.3), (0.3, 0.6), (0.6, 1.0)];
        let base = 48;
        let dumped = slices_to_steps(&slices, base, 16);
        assert_eq!(dumped.len(), 4);
        let fallback = (0.0, 1.0);
        for (i, &(step, note)) in dumped.iter().enumerate() {
            assert_eq!(step, i, "stegen ska följa slicarnas ordning");
            let (start, end) = window_for_note(&slices, base, note, fallback);
            assert_eq!(
                (start, end),
                slices[step],
                "not {note} (steg {step}) spelar fel slice — dumpen och uppspelningen är oense"
            );
        }
    }

    /// Fler slicar än steg **kapas** — inte slås ihop, inte tystas.
    #[test]
    fn more_slices_than_steps_are_capped() {
        let slices: Vec<(f32, f32)> = (0..40).map(|i| (i as f32 / 40.0, (i + 1) as f32 / 40.0)).collect();
        let dumped = slices_to_steps(&slices, 36, 16);
        assert_eq!(dumped.len(), 16, "fler steg än raden har");
        assert_eq!(dumped[15].0, 15);
        assert!(slices_to_steps(&[], 36, 16).is_empty(), "tom karta ger inga steg");
    }

    /// En hög basnot får inte linda runt: not 128 är inte not 0 i MIDI.
    #[test]
    fn a_high_base_note_clamps_instead_of_wrapping() {
        let slices = vec![(0.0, 0.5), (0.5, 1.0)];
        let dumped = slices_to_steps(&slices, 127, 16);
        assert_eq!(dumped[0].1, 127);
        assert_eq!(dumped[1].1, 127, "not 128 blev {}", dumped[1].1);
    }

    /// **En gräns är delad, och en flytt rör båda sidorna.** Det är hela poängen: flyttas bara
    /// den ena uppstår ett glapp (tyst i slicen) eller ett överlapp (samma ljud två gånger).
    #[test]
    fn nudging_a_boundary_moves_it_for_both_neighbours() {
        let slices = vec![(0.0, 0.25), (0.25, 0.5), (0.5, 1.0)];
        let moved = nudge_slice_boundary(&slices, 1, 0.05, 0.01);
        assert!((moved[0].1 - 0.30).abs() < 1e-6, "{:?}", moved);
        assert!((moved[1].0 - 0.30).abs() < 1e-6, "{:?}", moved);
        // Grannarna i övrigt orörda, och kartan fortfarande utan glapp eller överlapp.
        assert_eq!(moved[0].0, 0.0);
        assert_eq!(moved[2].1, 1.0);
        for w in moved.windows(2) {
            assert_eq!(w[0].1, w[1].0, "glapp eller överlapp efter nudge");
        }
        // Indelningen är fortfarande lika lång — en nudge tar inte bort någon slice.
        assert_eq!(moved.len(), slices.len());
    }

    /// En gräns som skulle korsa sin granne **kläms** — slicar slås inte ihop av en nudge.
    #[test]
    fn a_boundary_that_would_cross_its_neighbour_is_clamped_not_merged() {
        let slices = vec![(0.0, 0.25), (0.25, 0.5), (0.5, 1.0)];
        let far_left = nudge_slice_boundary(&slices, 1, -10.0, 0.05);
        assert!((far_left[1].0 - 0.05).abs() < 1e-6, "{:?}", far_left);
        assert_eq!(far_left.len(), 3, "en slice försvann i stället för att klämmas");
        let far_right = nudge_slice_boundary(&slices, 2, 10.0, 0.05);
        assert!((far_right[2].0 - 0.95).abs() < 1e-6, "{:?}", far_right);
        for w in far_left.windows(2).chain(far_right.windows(2)) {
            assert!(w[0].1 <= w[1].0 + 1e-6, "överlapp: {:?}", w);
        }
    }

    /// Filens kanter är inte gränser man nudgar, och skräp i `delta` ska lämna kartan orörd.
    #[test]
    fn the_files_edges_are_not_nudged_and_bad_input_changes_nothing() {
        let slices = vec![(0.0, 0.5), (0.5, 1.0)];
        assert_eq!(nudge_slice_boundary(&slices, 0, 0.3, 0.01), slices, "filens början");
        assert_eq!(nudge_slice_boundary(&slices, 2, -0.3, 0.01), slices, "filens slut");
        assert_eq!(nudge_slice_boundary(&slices, 9, 0.3, 0.01), slices, "index utanför");
        assert_eq!(nudge_slice_boundary(&slices, 1, f32::NAN, 0.01), slices, "NaN");
        assert_eq!(nudge_slice_boundary(&slices, 1, f32::INFINITY, 0.01), slices, "oändligt");
        assert!(nudge_slice_boundary(&[], 1, 0.1, 0.01).is_empty());
    }

    #[test]
    fn boundaries_closer_than_the_minimum_slice_length_are_merged() {
        // Slag 10 ms efter varandra: den andra gränsen faller inom minsta längd.
        let onsets = [0usize, (SR * 0.010) as usize, (SR * 0.100) as usize];
        let slices = slices_from_onsets(&onsets, (SR * 0.2) as usize, SR, 30.0);
        assert_eq!(slices.len(), 2, "den för täta gränsen hoppas över: {slices:?}");
        assert_eq!(slices[0].start, 0);
        assert_eq!(slices[1].start, (SR * 0.100) as usize);
    }

    #[test]
    fn an_empty_or_tiny_buffer_gives_no_slices_and_no_panic() {
        assert!(detect_onsets(&[], SR, &OnsetParams::default()).is_empty());
        assert!(detect_onsets(&[0.1, 0.2, 0.3], SR, &OnsetParams::default()).is_empty());
        assert!(slices_from_onsets(&[], 0, SR, 30.0).is_empty());
        // Utan sampel blir det ingen karta — inte en karta med en tom slice.
        assert!(slices_from_onsets(&[10, 20], 0, SR, 30.0).is_empty());
    }

    #[test]
    fn a_note_plays_its_slice_and_notes_outside_the_map_keep_the_trim_window() {
        let slices = [(0.0f32, 0.25f32), (0.25, 0.5), (0.5, 1.0)];
        let fallback = (0.1f32, 0.9f32);

        // Kromatiskt från basnoten: bas + i spelar slice i.
        assert_eq!(window_for_note(&slices, 36, 36, fallback), (0.0, 0.25));
        assert_eq!(window_for_note(&slices, 36, 37, fallback), (0.25, 0.5));
        assert_eq!(window_for_note(&slices, 36, 38, fallback), (0.5, 1.0));
        // Under basnoten och ovanför sista slicen: kanalens eget trimfönster.
        assert_eq!(window_for_note(&slices, 36, 35, fallback), fallback);
        assert_eq!(window_for_note(&slices, 36, 39, fallback), fallback);
        // Utan karta: exakt som förut.
        assert_eq!(window_for_note(&[], 36, 36, fallback), fallback);
        // En trasig slice (slut före start) får inte bli ett tyst klipp med
        // trovärdig ram — den faller tillbaka på trimfönstret.
        assert_eq!(window_for_note(&[(0.5, 0.5)], 36, 36, fallback), fallback);
        assert_eq!(window_for_note(&[(0.8, 0.2)], 36, 36, fallback), fallback);
    }

    #[test]
    fn silence_has_no_onsets() {
        let buf = vec![0.0f32; (SR * 1.0) as usize];
        assert!(detect_onsets(&buf, SR, &OnsetParams::default()).is_empty());
    }

    #[test]
    fn the_envelope_rises_on_an_attack_and_not_on_a_decay() {
        // Ett anslag: tyst → stark. En utklingning: stark → tyst. Bara det
        // första ska ge ett hölje (halvvågslikriktningen).
        let mut attack = vec![0.0f32; 100];
        attack.extend(std::iter::repeat(0.8).take(100));
        let env_a = onset_envelope(&attack, SR);
        let mut decay = vec![0.8f32; 100];
        decay.extend(std::iter::repeat(0.0).take(100));
        let env_d = onset_envelope(&decay, SR);
        assert!(env_a[100] > 0.0, "anslaget ska synas i höljet");
        assert_eq!(env_d.iter().fold(0.0f32, |m, v| m.max(*v)), 0.0, "utklingningen ska inte ge något");
    }

    /// **Var musiken börjar** (Fas 8.14 steg 2): första stunden ljud — inte detektorns
    /// backning och inte ett senare anslag.
    #[test]
    fn the_music_starts_where_the_sound_starts() {
        let buf = clicks(&[0.20, 0.70, 1.20], 2.0);
        let p = OnsetParams::default();
        let start = music_start_source_secs(&buf, SR, 0.0, 2.0, &p).expect("klick finns");
        assert!(
            (start - 0.20).abs() < 0.005,
            "musiken börjar vid första klicket: {start}"
        );
    }

    /// **Trumfallet, mätt** (Alex' stämma 2026-09-13): tystnad till 0,600 s, sedan en
    /// träff. Detektorn backar till 0,502 s — rätt för en slice, fel för ett rutnät.
    /// Svaret ska vara träffen: 98 ms är 5,7 % av ett slag i 140 BPM, och skillnaden
    /// mellan "slaget på rutnätet" och "flam".
    #[test]
    fn the_music_start_is_the_hit_not_the_backtrack() {
        let mut buf = vec![0.0f32; (SR * 1.5) as usize];
        let hit = (SR * 0.600) as usize;
        buf[hit] = 0.9;
        buf[hit + 1] = -0.7;
        let p = OnsetParams::default();
        assert!(
            detect_onsets(&buf, SR, &p)[0] < hit,
            "detektorn ska backa före träffen — annars prövar provet inget"
        );
        let start = music_start_source_secs(&buf, SR, 0.0, 1.5, &p).expect("ljud finns");
        assert!(
            (start - 0.600).abs() < 0.005,
            "starten ska vara träffen på 0,600 s, inte backningen: {start}"
        );
    }

    /// **Ett mjukt anslag har inget slag för HFC-höljet att hitta** — sångens och
    /// stråkens fall. Där är ljudet ankaret: en ton som börjar 0,50 s in ska ge 0,50 s,
    /// fastän detektorn inte hittar något alls i den.
    #[test]
    fn a_soft_attack_is_found_by_the_sound_and_not_by_the_detector() {
        let sr = 48_000.0f32;
        let mut buf = vec![0.0f32; (sr * 2.0) as usize];
        let start_at = (sr * 0.5) as usize;
        for i in start_at..buf.len() {
            let t = (i - start_at) as f32 / sr;
            let env = (t / 0.2).min(1.0);
            buf[i] = 0.6 * env * (std::f32::consts::TAU * 220.0 * t).sin();
        }
        let p = OnsetParams::default();
        assert_eq!(
            detect_onsets(&buf, sr, &p).len(),
            0,
            "en mjuk ton ska inte ge några slag — därför måste ljudet vara ankaret"
        );
        let start = music_start_source_secs(&buf, sr, 0.0, 2.0, &p).expect("ljud finns");
        assert!(
            (start - 0.50).abs() < 0.03,
            "starten ska vara där tonen börjar (0,50 s): {start}"
        );
    }

    /// Ett fönster utan ljud är `None`: det finns ingen musik att sätta på ett rutnät,
    /// och då flyttas ingenting (8.5). Sökningen börjar dessutom där klippet börjar —
    /// ljud **före** klippets första sampel är inte klippets början.
    #[test]
    fn a_silent_window_has_no_music_start() {
        let p = OnsetParams::default();
        let quiet = vec![0.0f32; (SR * 1.0) as usize];
        assert_eq!(music_start_source_secs(&quiet, SR, 0.0, 1.0, &p), None);
        assert_eq!(music_start_source_secs(&[], SR, 0.0, 1.0, &p), None);
        let buf = clicks(&[0.20, 0.70], 1.0);
        let start = music_start_source_secs(&buf, SR, 0.50, 0.5, &p).expect("klick på 0,70");
        assert!(
            (start - 0.70).abs() < 0.01,
            "från 0,50 s är musiken klicket på 0,70 s, inte 0,20: {start}"
        );
    }

    /// **Mätning på Alex' riktiga stämma** (Fas 8.14 steg 2): var börjar ljudet, och vad
    /// svarar funktionen? Körs manuellt mot en riktig fil:
    ///
    /// ```text
    /// SONIX_ONSET_FILE="…/Rock and Hard Place (Drums).wav" \
    ///   cargo test --release --locked --bin sonix the_music_start_against_a_real_stem \
    ///   -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore]
    fn the_music_start_against_a_real_stem() {
        let path = std::env::var("SONIX_ONSET_FILE").expect("SONIX_ONSET_FILE");
        let (left, _right, sr) = crate::audio::load_wav_pcm(&path).expect("filen ska gå att läsa");
        let p = OnsetParams::default();
        let raw = detect_onsets(&left, sr as f32, &p)
            .first()
            .map(|&i| i as f32 / sr as f32);
        let found = music_start_source_secs(&left, sr as f32, 0.0, 20.0, &p);
        eprintln!("fil: {path}");
        eprintln!("  detektorns råa första slag {raw:?} s");
        eprintln!("  funktionens svar (musikens start) {found:?} s");
    }
}
