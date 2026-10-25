//! **Newtime** — tidsredigering: flytta var i tiden ett ljud landar utan att tonhöjden följer.
//!
//! Modellen är en rad **ankare** `(källa, uttid)`. Ändpunkterna är låsta till ljudets början
//! och (som standard) dess slut; markörerna mellan dem får flyttas. Varje segment mellan två
//! ankare sträcks med WSOLA (`vocal_harmonizer::time_stretch`), som bevarar tonhöjden.
//!
//! Precis som Edison och Newtone arbetar Newtime på en **kopia**: renderingen blir ett nytt
//! spår, och källklippet rörs aldrig.

use crate::audio::vocal_harmonizer::time_stretch;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WarpMarker {
    /// Var i källjudet markören sitter, i sekunder.
    pub source_sec: f32,
    /// Var samma punkt hamnar i den redigerade tiden, i sekunder.
    pub out_sec: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Warp {
    pub markers: Vec<WarpMarker>,
    /// Utgångens längd. `0.0` = samma som källan (ingen ändring av slutet).
    pub end_out_sec: f32,
    pub source_sec: f32,
}

impl Warp {
    pub fn new(source_sec: f32) -> Self {
        Self { markers: Vec::new(), end_out_sec: 0.0, source_sec }
    }

    pub fn out_len(&self) -> f32 {
        if self.end_out_sec > 0.0 { self.end_out_sec } else { self.source_sec }
    }

    /// Ankarlistan med ändpunkterna inräknade, sorterad och klämd så att både källa och
    /// uttid är strikt växande. En markör som dragits förbi sin granne ska inte kunna vända
    /// på tiden — då hade renderingen läst baklänges.
    pub fn anchors(&self) -> Vec<(f32, f32)> {
        let mut markers = self.markers.clone();
        markers.sort_by(|a, b| {
            a.source_sec.partial_cmp(&b.source_sec).unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut out = Vec::with_capacity(markers.len() + 2);
        out.push((0.0, 0.0));
        let mut prev = (0.0_f32, 0.0_f32);
        for m in markers {
            let src = m.source_sec.clamp(prev.0 + 0.001, self.source_sec - 0.001);
            let o = m.out_sec.clamp(prev.1 + 0.001, self.out_len() - 0.001);
            out.push((src, o));
            prev = (src, o);
        }
        // Slutet är exakt, inte klämt: annars hade en oförändrad fil blivit 1 ms kortare.
        out.push((self.source_sec, self.out_len().max(prev.1 + 0.001)));
        out
    }

    /// Vilken källsekund som ligger på en viss uttid — används när ett klick i vågformen ska
    /// bli en ny markör: markören ska sitta på det ljud man pekade på, inte på samma klocka.
    pub fn source_at_out(&self, out_sec: f32) -> f32 {
        let a = self.anchors();
        for pair in a.windows(2) {
            let (s0, o0) = pair[0];
            let (s1, o1) = pair[1];
            if out_sec >= o0 && out_sec <= o1 && o1 > o0 {
                let t = (out_sec - o0) / (o1 - o0);
                return s0 + (s1 - s0) * t;
            }
        }
        self.source_sec
    }
}

/// Renderar den tidsredigerade kopian. Segment med oförändrat tempo kopieras rakt av —
/// WSOLA på ett 1:1-segment hade bara lagt fasbrus utan att ändra något.
pub fn render(left: &[f32], right: &[f32], sample_rate: f32, warp: &Warp) -> (Vec<f32>, Vec<f32>) {
    let sr = sample_rate.max(1.0);
    let src_len = left.len().min(right.len());
    if src_len < 2 {
        return (left.to_vec(), right.to_vec());
    }
    let mut out_l: Vec<f32> = Vec::new();
    let mut out_r: Vec<f32> = Vec::new();
    for pair in warp.anchors().windows(2) {
        let (s0, o0) = pair[0];
        let (s1, o1) = pair[1];
        let a = ((s0 * sr) as usize).min(src_len);
        let b = ((s1 * sr) as usize).min(src_len);
        if b <= a {
            continue;
        }
        let src_frames = b - a;
        let out_frames = ((o1 - o0) * sr).round().max(1.0) as usize;
        if out_frames == src_frames {
            out_l.extend_from_slice(&left[a..b]);
            out_r.extend_from_slice(&right[a..b]);
        } else {
            let ratio = out_frames as f32 / src_frames as f32;
            out_l.extend(time_stretch(&left[a..b], sr, ratio));
            out_r.extend(time_stretch(&right[a..b], sr, ratio));
        }
    }
    (out_l, out_r)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Starkaste frekvensen mellan `lo` och `hi`. **WSOLA på en ihållande ton ger
    /// sidband** kring grundtonen (kornen fasas om var 512:e sampel), så provet mäter att
    /// registret står kvar — inte att vågformen är bitidentisk. En naiv resampling hade
    /// flyttat 220 Hz till 293 Hz vid faktor 0,75, långt utanför fönstret.
    fn dominant_hz(samples: &[f32], sr: f32, lo: u32, hi: u32) -> f32 {
        let mut best = (0.0_f32, 0.0_f32);
        for hz in lo..=hi {
            let f = hz as f32;
            let (mut re, mut im) = (0.0_f32, 0.0_f32);
            for (i, v) in samples.iter().enumerate() {
                let ph = std::f32::consts::TAU * f * i as f32 / sr;
                re += v * ph.cos();
                im -= v * ph.sin();
            }
            let mag = (re * re + im * im).sqrt();
            if mag > best.1 {
                best = (f, mag);
            }
        }
        best.0
    }

    fn sine(secs: f32, freq: f32, sr: f32) -> Vec<f32> {
        let n = (secs * sr) as usize;
        (0..n)
            .map(|i| (std::f32::consts::TAU * freq * i as f32 / sr).sin() * 0.5)
            .collect()
    }

    #[test]
    fn without_markers_the_render_is_the_source() {
        let sr = 48_000.0;
        let l = sine(1.0, 220.0, sr);
        let r = l.clone();
        let warp = Warp::new(1.0);
        let (out_l, out_r) = render(&l, &r, sr, &warp);
        assert_eq!(out_l.len(), l.len());
        assert_eq!(out_l, l);
        assert_eq!(out_r, r);
    }

    #[test]
    fn moving_the_end_changes_the_length_and_keeps_the_pitch() {
        let sr = 48_000.0;
        let l = sine(2.0, 220.0, sr);
        let mut warp = Warp::new(2.0);
        warp.end_out_sec = 1.5;
        let (out_l, _) = render(&l, &l, sr, &warp);
        assert!(
            (out_l.len() as f32 / sr - 1.5).abs() < 0.02,
            "längden blev {:.3} s",
            out_l.len() as f32 / sr
        );
        let f = dominant_hz(&out_l[20_000..28_192], sr, 190, 260);
        assert!((f - 220.0).abs() < 25.0, "registret ska stå kvar, fick {f:.0} Hz");
    }

    #[test]
    fn a_middle_marker_stretches_only_its_own_segment() {
        let sr = 48_000.0;
        let l = sine(2.0, 220.0, sr);
        let mut warp = Warp::new(2.0);
        // Första sekunden tar 1,5 s; andra sekunden tar 0,5 s — summan är oförändrad.
        warp.markers.push(WarpMarker { source_sec: 1.0, out_sec: 1.5 });
        warp.end_out_sec = 2.0;
        let (out_l, _) = render(&l, &l, sr, &warp);
        assert!((out_l.len() as f32 / sr - 2.0).abs() < 0.03, "totalen ska vara 2 s");
        // Källans första sekund är nu längre: 220 Hz ligger kvar hela vägen.
        let f = dominant_hz(&out_l[30_000..38_192], sr, 190, 260);
        assert!((f - 220.0).abs() < 25.0, "registret ska bevaras, fick {f:.0} Hz");
    }

    #[test]
    fn crossed_markers_are_clamped_instead_of_reversing_time() {
        let sr = 48_000.0;
        let _ = sr;
        let mut warp = Warp::new(4.0);
        warp.markers = vec![
            WarpMarker { source_sec: 2.0, out_sec: 3.0 },
            WarpMarker { source_sec: 1.0, out_sec: 0.5 },
        ];
        let a = warp.anchors();
        for pair in a.windows(2) {
            assert!(pair[1].0 > pair[0].0, "källan ska vara växande: {pair:?}");
            assert!(pair[1].1 > pair[0].1, "uttiden ska vara växande: {pair:?}");
        }
    }

    #[test]
    fn source_at_out_follows_the_stretch() {
        let mut warp = Warp::new(2.0);
        warp.markers.push(WarpMarker { source_sec: 1.0, out_sec: 1.5 });
        // Halvvägs in i det utsträckta första segmentet ligger källan på 0,5 s.
        assert!((warp.source_at_out(0.75) - 0.5).abs() < 0.01);
        assert!((warp.source_at_out(1.5) - 1.0).abs() < 0.01);
    }


    /// **Ände-till-ände:** rendera en tidsredigerad kopia, skriv den som wav och läs tillbaka
    /// den. Det är vägen *Skapa spår* tar, och den hade aldrig körts i ett prov.
    #[test]
    fn the_rendered_copy_survives_a_round_trip_through_a_wav_file() {
        let sr = 48_000.0_f32;
        let l = sine(2.0, 220.0, sr);
        let mut warp = Warp::new(2.0);
        warp.end_out_sec = 1.0;
        let (out_l, out_r) = render(&l, &l, sr, &warp);
        let path = std::env::temp_dir().join("sonix_newtime_round_trip.wav");
        let path_str = path.to_string_lossy().to_string();
        crate::audio::write_stem_wav(&path_str, &out_l, &out_r, sr as u32)
            .expect("wav-filen ska gå att skriva");
        let (rl, rr, rsr) = crate::audio::wav_reader::load_wav_pcm(&path_str)
            .expect("den skrivna filen ska gå att läsa");
        let _ = std::fs::remove_file(&path);

        assert_eq!(rsr, sr as u32);
        let secs = rl.len() as f32 / rsr as f32;
        assert!((secs - 1.0).abs() < 0.02, "längden blev {secs:.3} s");
        assert_eq!(rl.len(), rr.len(), "kanalerna ska vara lika långa");
    }

}
