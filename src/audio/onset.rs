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
}
