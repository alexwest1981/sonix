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
    let out_frames = ((source_frames as f64) * (file_ratio as f64)).round().max(1.0) as usize;
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
/// 3. **Klippet vill bandspelare** (`per_clip_tape`) → [`FollowMode::Tape`], tonhöjden följer.
/// 4. **Samma tempo** → inget att göra (faktor 1,0, bit-exakt väg).
/// 5. Annars → [`FollowMode::Stretch`] med projekt/källa, klämt till samma spann som
///    sångstudiens reglage.
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
    FollowDecision {
        mode: FollowMode::Stretch,
        ratio: raw,
    }
}

/// Sträcker ett stereopar med bevarad tonhöjd.
///
/// Kanalerna går genom **samma** WSOLA-instans, så grainsökningen (som avgör var varje korn
/// läggs) görs en gång för båda kanalerna. Att köra två separata instanser skulle kunna välja
/// olika skiften för vänster och höger och därmed sprida ut stereobilden — samma fälla som ett
/// separat monoklipp per kanal.
///
/// Vid faktor 1,0 returneras ingången oförändrad: ingen grain-process, ingen förlust.
pub fn stretch_stereo(
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

    let mut wsola = super::vocal_harmonizer::Wsola::new(sample_rate);
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
pub fn cache_key(source: &str, source_bpm: f32, project_bpm: f32) -> String {
    let stem = Path::new(source)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "klipp".to_string());
    format!(
        "{}-{:.2}-till-{:.2}",
        crate::autosave::slug(&stem),
        source_bpm,
        project_bpm
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
) -> Result<String, String> {
    let frames = left.len().min(right.len());
    let out_frames = ((frames as f64) * (ratio.clamp(MIN_RATIO, MAX_RATIO) as f64))
        .round()
        .max(1.0) as usize;
    let (out_l, out_r) = stretch_stereo(left, right, ratio, sample_rate);
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
        source: Some((
            std::sync::Arc::new(l),
            std::sync::Arc::new(r),
            sr as f32,
        )),
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
    match render_to_file(&job.dir, &job.key, &l, &r, plan.file_ratio, sr as f32) {
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
        Self { last: bpm, frames: 0 }
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
        assert!((up.playback_ratio - 1.25).abs() < 1e-6, "motorn: {}", up.playback_ratio);
        assert!((up.file_ratio - 0.8).abs() < 1e-6, "filen: {}", up.file_ratio);
        assert_eq!(up.out_frames, 35_280, "0,8 × 44 100 frames — kortare, inte längre");
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
        assert!(fast.out_frames >= 1, "en klämd plan får aldrig bli noll frames");
        let slow = plan(120.0, 1.0, 1_000).expect("en plan finns");
        assert!((slow.playback_ratio - MIN_RATIO).abs() < 1e-6);
        assert!(slow.out_frames >= 1);
    }

    /// **Acceptanskriterium 1:** vid faktor 1,0 händer ingenting alls — bit-exakt.
    #[test]
    fn a_ratio_of_one_returns_the_input_untouched() {
        let l = tone(220.0, 0.2, 0.7);
        let r: Vec<f32> = l.iter().map(|s| s * 0.5).collect();
        let (out_l, out_r) = stretch_stereo(&l, &r, 1.0, SR);
        assert_eq!(out_l, l, "vänster ska vara oförändrad");
        assert_eq!(out_r, r, "höger ska vara oförändrad");
    }

    /// **Acceptanskriterium 2:** tonhöjden står still när tempot ändras, och längden följer
    /// faktorn. Mätt med Goertzel vid grundtonen — och vid den frekvens en smurf hade hamnat på.
    #[test]
    fn the_pitch_stays_put_while_the_length_changes() {
        let l = tone(220.0, 0.5, 0.7);
        let r = l.clone();
        let (out_l, _out_r) = stretch_stereo(&l, &r, 1.5, SR);

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
        let (out_l, _) = stretch_stereo(&l, &l.clone(), 0.75, SR);
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
        let (out_l, out_r) = stretch_stereo(&l, &r, 1.25, SR);

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
            let (out, _) = stretch_stereo(&src, &src, 1.25, SR);
            let stretch_ms = t0.elapsed().as_millis();

            let dir = std::env::temp_dir().join(format!("sonix_stretch_cost_{minutes}"));
            let _ = std::fs::remove_dir_all(&dir);
            let t1 = std::time::Instant::now();
            let written = render_to_file(&dir, "kostnad", &src, &src, 1.25, SR)
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
            let written = render_to_file(&dir, name, &l, &r, file_ratio, sr as f32)
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
                let written = render_to_file(&dir, &format!("{label} {project}"), &l, &r, file_ratio, sr as f32)
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
        cache.request(
            &dir,
            key.clone(),
            src_str.clone(),
            120.0,
            180.0,
        );

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
        let path = render_to_file(&dir, &key, &l, &r, 1.25, SR).expect("skrivningen ska lyckas");

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
        let err = render_to_file(&dir, "tomt", &[], &[], 1.5, SR).expect_err("ska avvisas");
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

    /// En extrem tempodiff får inte ge en orimlig faktor, och inte heller en tyst väg runt
    /// klämningen: samma gränser som sångstudiens reglage gäller.
    #[test]
    fn the_policy_clamps_like_the_vocal_studio_does() {
        assert!((decide(120.0, 10_000.0, true, false).ratio - MAX_RATIO).abs() < 1e-6);
        assert!((decide(120.0, 1.0, true, false).ratio - MIN_RATIO).abs() < 1e-6);
    }
}
