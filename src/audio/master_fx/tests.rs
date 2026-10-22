    use super::*;

    #[test]
    fn remix_fx_repeats_captured_slice() {
        let mut fx = RemixFx::new(48_000.0);
        // 120 BPM -> one beat = 24000 samples; capture then loop.
        fx.set(1, 120.0);
        let mut captured = Vec::new();
        for i in 0..24_000 {
            let x = (i as f32 / 24_000.0).sin();
            let (l, _) = fx.process(x, x);
            captured.push(l);
        }
        // Now looping: the output must repeat the captured slice.
        let mut a = 0.0;
        for _ in 0..1000 {
            let (l, _) = fx.process(0.0, 0.0);
            a = l;
        }
        assert!(captured.iter().any(|&v| (v - a).abs() < 1e-6));
    }

    #[test]
    fn tape_stop_is_transparent_at_normal_rate() {
        let mut ts = TapeStop::new(48_000.0);
        // Inactive -> rate 1.0: output must equal the input with no added
        // latency (regression for a full ring-buffer read-ahead).
        for i in 0..2000 {
            let x = ((i as f32) * 0.01).sin();
            let (l, r) = ts.process(x, -x);
            assert!(
                (l - x).abs() < 1e-6 && (r + x).abs() < 1e-6,
                "tape-stop must be a passthrough when idle (frame {i}: {l} vs {x})"
            );
        }
    }

    #[test]
    fn tape_stop_fades_to_silence() {
        let mut ts = TapeStop::new(48_000.0);
        ts.set_active(true);
        let mut last = 1.0;
        for _ in 0..48_000 {
            let (l, _) = ts.process(1.0, 1.0);
            last = l;
        }
        assert!(last.abs() < 1e-3, "expected silence, got {last}");
    }

    #[test]
    fn compressor_reduces_loud_signal() {
        let mut c = Compressor::new(48_000.0);
        let p = CompressorParams {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            makeup_db: 0.0,
        };
        let mut out = 0.0;
        for _ in 0..48_000 {
            let (l, _) = c.process(0.5, 0.5, &p);
            out = l;
        }
        assert!(out < 0.5 * 0.6, "compressor should pull 0.5 down, got {out}");
        assert!(out > 0.0);
    }

    #[test]
    fn compressor_bypassed_below_threshold() {
        let mut c = Compressor::new(48_000.0);
        let p = CompressorParams {
            threshold_db: -6.0,
            ratio: 4.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            makeup_db: 0.0,
        };
        let mut out = 0.0;
        for _ in 0..48_000 {
            let (l, _) = c.process(0.01, 0.01, &p);
            out = l;
        }
        assert!((out - 0.01).abs() < 1e-6, "quiet signal should pass, got {out}");
    }

    #[test]
    fn limiter_respects_ceiling() {
        let mut lim = Limiter::new(48_000.0);
        let p = LimiterParams { ceiling: 0.5, release_ms: 80.0, boost: 2.0 };
        let mut peak = 0.0f32;
        for _ in 0..48_000 {
            let (l, r) = lim.process(1.0, -1.0, &p);
            peak = peak.max(l.abs()).max(r.abs());
        }
        assert!(peak <= 0.5 + 1e-4, "limiter exceeded ceiling: {peak}");
        assert!(peak > 0.4, "limiter should still pass level, got {peak}");
    }

    #[test]
    fn gate_closes_below_threshold_and_opens_above() {
        let mut gate = NoiseGate::new(48_000.0);
        let p = GateParams { threshold: 0.1, attack_ms: 5.0, release_ms: 120.0 };
        let mut quiet = 0.0;
        for _ in 0..96_000 {
            let (l, _) = gate.process(0.001, 0.001, &p);
            quiet = l;
        }
        assert!(quiet.abs() < 1e-4, "gate should mute sub-threshold signal, got {quiet}");

        let mut loud = 0.0;
        for _ in 0..48_000 {
            let (l, _) = gate.process(0.5, 0.5, &p);
            loud = l;
        }
        assert!((loud - 0.5).abs() < 0.02, "gate should open on loud signal, got {loud}");
    }

    #[test]
    fn master_chain_passes_signal_when_effects_off() {
        let mut chain = MasterFxChain::new(48_000.0);
        let params = MasterFxParams::default();
        chain.set_params(params);
        let mut out = 0.0;
        for _ in 0..1000 {
            let (l, _) = chain.process(0.3, 0.3);
            out = l;
        }
        assert!(out.is_finite() && out.abs() > 0.1, "dry chain should pass audio, got {out}");
        assert_eq!(chain.gain_reduction_db(), 0.0);
    }

    #[test]
    fn master_chain_reports_compressor_gain_reduction() {
        let mut chain = MasterFxChain::new(48_000.0);
        let mut params = MasterFxParams::default();
        params.comp_enabled = true;
        params.comp = CompressorParams {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            makeup_db: 0.0,
        };
        chain.set_params(params);
        for _ in 0..48_000 {
            chain.process(0.5, 0.5);
        }
        assert!(chain.gain_reduction_db() < -1.0, "expected gain reduction, got {}", chain.gain_reduction_db());
    }

    /// Amplituden hos en enskild frekvens, mätt med Goertzel-formen (en bin av en DFT).
    fn tone_mag(x: &[f32], freq: f32, sr: f32) -> f32 {
        let w = 2.0 * std::f32::consts::PI * freq / sr;
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for (i, s) in x.iter().enumerate() {
            let ph = w * i as f32;
            re += s * ph.cos();
            im += s * ph.sin();
        }
        2.0 * (re * re + im * im).sqrt() / x.len() as f32
    }

    #[test]
    fn transient_shaper_is_transparent_at_zero() {
        // 0 dB på båda reglagen måste ge exakt insignalen tillbaka — förstärkningen är
        // 10⁰ · 10⁰ = 1,0, alltså ingen avrundning alls.
        let mut ts = TransientShaper::new(48_000.0);
        let p = TransientParams::default();
        for i in 0..5_000 {
            let t = i as f32 / 48_000.0;
            let x = (t * 220.0 * 2.0 * std::f32::consts::PI).sin() * 0.7;
            let (l, r) = ts.process(x, -x, &p);
            assert_eq!(l, x, "frame {i}");
            assert_eq!(r, -x, "frame {i}");
        }
    }

    #[test]
    fn attack_boost_lifts_the_attack_of_a_hit() {
        // Ett anslag (1,0 följt av tystnad): med +12 dB ska första samplen upp, resten vara tyst.
        let mut ts = TransientShaper::new(48_000.0);
        let p = TransientParams { attack_db: 12.0, sustain_db: 0.0, sensitivity: 0.5 };
        let (l, _) = ts.process(1.0, 1.0, &p);
        assert!(l > 3.0, "anslaget skulle lyfts med ~4 gånger, fick {l}");
        let (l2, _) = ts.process(0.0, 0.0, &p);
        assert_eq!(l2, 0.0, "tystnad ska förbli tyst");
    }

    #[test]
    fn sustain_cut_lowers_a_steady_tone() {
        // En jämn ton har inget anslag: −12 dB kropp ska ge ~0,25 av insignalen, inte 1,0.
        let mut ts = TransientShaper::new(48_000.0);
        let p = TransientParams { attack_db: 0.0, sustain_db: -12.0, sensitivity: 0.5 };
        let mut out = Vec::new();
        for i in 0..48_000 {
            let t = i as f32 / 48_000.0;
            let x = (t * 220.0 * 2.0 * std::f32::consts::PI).sin() * 0.5;
            let (l, _) = ts.process(x, x, &p);
            out.push(l);
        }
        let tail = &out[24_000..];
        let rms = (tail.iter().map(|v| v * v).sum::<f32>() / tail.len() as f32).sqrt();
        let input_rms = 0.5 / 2.0_f32.sqrt();
        let ratio = rms / input_rms;
        assert!(
            (ratio - 0.251).abs() < 0.03,
            "jämn ton med −12 dB kropp blev {ratio:.3} av insignalen (väntat ~0,251)"
        );
    }

    #[test]
    fn saturator_is_transparent_at_zero_mix() {
        let mut sat = Saturator::new(48_000.0);
        let p = SaturatorParams { drive: 12.0, tone: 0.3, mix: 0.0 };
        for i in 0..1_000 {
            let x = (i as f32 * 0.01).sin();
            let (l, r) = sat.process(x, -x, &p);
            assert_eq!(l, x);
            assert_eq!(r, -x);
        }
    }

    #[test]
    fn saturation_harmonics_grow_with_drive() {
        let sr = 48_000.0;
        let measure = |drive: f32| {
            let mut sat = Saturator::new(sr);
            let p = SaturatorParams { drive, tone: 1.0, mix: 1.0 };
            let mut out = Vec::new();
            for i in 0..9_600 {
                let x = (i as f32 / sr * 1_000.0 * 2.0 * std::f32::consts::PI).sin() * 0.4;
                let (l, _) = sat.process(x, x, &p);
                out.push(l);
            }
            let f = tone_mag(&out[1_000..], 1_000.0, sr);
            let h3 = tone_mag(&out[1_000..], 3_000.0, sr);
            h3 / f
        };
        let (d2, d4, d8) = (measure(2.0), measure(4.0), measure(8.0));
        assert!(d2 < d4 && d4 < d8, "övertonerna ska växa med drive: {d2:.4} {d4:.4} {d8:.4}");
        assert!(d8 > 0.01, "drive 8 ska ge hörbar tredjeton, fick {d8:.4}");
    }

    #[test]
    fn saturator_never_exceeds_full_scale() {
        // Normaliseringen 1/tanh(drive) håller utgången inom ±1 även för insignal 10.
        let mut sat = Saturator::new(48_000.0);
        let p = SaturatorParams { drive: 20.0, tone: 1.0, mix: 1.0 };
        for i in 0..1_000 {
            let x = if i % 2 == 0 { 10.0 } else { -10.0 };
            let (l, r) = sat.process(x, -x, &p);
            assert!(l.abs() <= 1.0 + 1e-6, "utgång {l} över full skala");
            assert!(r.abs() <= 1.0 + 1e-6, "utgång {r} över full skala");
        }
    }

    #[test]
    fn multiband_crossover_is_flat() {
        // Delningsfiltret ska släppa igenom allt när inget band komprimeras: tre toner in,
        // samma tre toner ut. Det är summan low+mid+high som mäts, alltså filtret självt.
        let sr = 48_000.0;
        let mut mb = MultibandCompressor::new(sr);
        mb.set_params(MultibandParams {
            amounts: [0.0; 3],
            threshold_db: -50.0,
            ratio: 10.0,
            ..MultibandParams::default()
        });
        let bands = [(100.0f32, 0.3f32), (1_000.0, 0.2), (10_000.0, 0.1)];
        let mut out = Vec::new();
        for i in 0..48_000 {
            let t = i as f32 / sr;
            let x: f32 = bands
                .iter()
                .map(|(f, a)| (t * f * 2.0 * std::f32::consts::PI).sin() * a)
                .sum();
            let (l, _) = mb.process(x, x);
            out.push(l);
        }
        for (f, a) in bands {
            let got = tone_mag(&out[24_000..], f, sr);
            let ratio = got / a;
            assert!(
                (0.88..1.13).contains(&ratio),
                "{f} Hz blev {ratio:.3} av insignalen — delningen är inte platt"
            );
        }
    }

    #[test]
    fn a_loud_band_is_tamed_and_the_quiet_one_is_not() {
        // 100 Hz på 0,5 och 5 kHz på 0,02: bara basbandet ska sänkas.
        let sr = 48_000.0;
        let mut mb = MultibandCompressor::new(sr);
        mb.set_params(MultibandParams {
            amounts: [1.0, 0.0, 1.0],
            threshold_db: -24.0,
            ratio: 4.0,
            ..MultibandParams::default()
        });
        let mut out = Vec::new();
        for i in 0..48_000 {
            let t = i as f32 / sr;
            let x = (t * 100.0 * 2.0 * std::f32::consts::PI).sin() * 0.5
                + (t * 5_000.0 * 2.0 * std::f32::consts::PI).sin() * 0.02;
            let (l, _) = mb.process(x, x);
            out.push(l);
        }
        let low = tone_mag(&out[24_000..], 100.0, sr) / 0.5;
        let high = tone_mag(&out[24_000..], 5_000.0, sr) / 0.02;
        assert!(low < 0.72, "basbandet skulle sänkts minst 3 dB, blev {low:.3}");
        assert!(
            (0.9..1.1).contains(&high),
            "diskanten skulle vara orörd, blev {high:.3}"
        );
    }

    #[test]
    fn convolver_without_ir_is_a_passthrough() {
        let mut c = Convolver::new(48_000.0);
        let p = ConvolverParams { mix: 1.0, gain_db: 0.0 };
        for i in 0..1_000 {
            let x = (i as f32 * 0.01).sin();
            let (l, r) = c.process(x, -x, &p);
            assert_eq!(l, x);
            assert_eq!(r, -x);
        }
    }

    #[test]
    fn convolver_delays_by_exactly_one_block() {
        // IR = [1,0,0…] ska ge insignalen tillbaka, fördröjd med **ett block** (CONV_BLOCK).
        // Latensen mäts här i stället för att döljas: den är priset för att svaret inte kan
        // räknas förrän ett helt block inkommit.
        let mut c = Convolver::new(48_000.0);
        c.set_ir(Some(Arc::new(vec![1.0])));
        let p = ConvolverParams { mix: 1.0, gain_db: 0.0 };
        let mut out = Vec::new();
        for i in 0..1_024 {
            let x = if i == 0 { 1.0 } else { 0.0 };
            let (l, _) = c.process(x, x, &p);
            out.push(l);
        }
        assert!(
            out[..CONV_BLOCK].iter().all(|v| *v == 0.0),
            "innan första blocket är färdigt finns inget svar"
        );
        assert!((out[CONV_BLOCK] - 1.0).abs() < 1e-5, "impulsen ska komma på plats {}", CONV_BLOCK);
    }

    #[test]
    fn convolver_applies_the_ir_shape() {
        // En två-taps-respons [0,5, −0,5] ska ge två sampel med rätt tecken — alltså är det
        // filens respons som faltas in, inte en fördröjning.
        let mut c = Convolver::new(48_000.0);
        c.set_ir(Some(Arc::new(vec![0.5, -0.5])));
        let p = ConvolverParams { mix: 1.0, gain_db: 0.0 };
        let mut out = Vec::new();
        for i in 0..1_024 {
            let x = if i == 0 { 1.0 } else { 0.0 };
            let (l, _) = c.process(x, x, &p);
            out.push(l);
        }
        assert!((out[CONV_BLOCK] - 0.5).abs() < 1e-5, "första tappen: {}", out[CONV_BLOCK]);
        assert!((out[CONV_BLOCK + 1] + 0.5).abs() < 1e-5, "andra tappen: {}", out[CONV_BLOCK + 1]);
        assert!(out[CONV_BLOCK + 2].abs() < 1e-5, "responsen är två sampel lång");
    }

    #[test]
    fn an_all_ones_pattern_is_transparent() {
        let mut g = StepGate::new(48_000.0);
        let p = GatePatternParams { enabled: true, ..GatePatternParams::default() };
        for i in 0..5_000 {
            let x = (i as f32 * 0.01).sin();
            let (l, r) = g.process(x, -x, &p);
            assert_eq!(l, x);
            assert_eq!(r, -x);
        }
    }

    #[test]
    fn a_gate_pattern_mutes_the_off_steps() {
        // 120 BPM → ett 1/16-steg är 6 000 sampel. Steg 0 öppet, resten stängt.
        let mut g = StepGate::new(48_000.0);
        let mut gains = [0.0f32; 16];
        gains[0] = 1.0;
        let p = GatePatternParams { gains, bpm: 120.0, smooth: 0.0, enabled: true };
        let mut open = Vec::new();
        let mut closed = Vec::new();
        for i in 0..12_000 {
            let x = (i as f32 * 0.05).sin();
            let (l, _) = g.process(x, x, &p);
            if i < 6_000 { open.push(l) } else { closed.push(l) }
        }
        let rms = |v: &[f32]| (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt();
        assert!(rms(&open) > 0.5, "steg 0 ska släppa igenom, fick {}", rms(&open));
        assert_eq!(rms(&closed), 0.0, "steg 1 ska vara tyst");
    }

    #[test]
    fn mod_fx_is_transparent_at_zero_mix() {
        for kind in 0..3 {
            let mut chorus = ModDelay::new(48_000.0, 12.0, 8.0);
            let mut flanger = ModDelay::new(48_000.0, 2.5, 2.0);
            let mut phaser = Phaser::new(48_000.0);
            let p = ModFxParams { rate: 3.0, depth: 1.0, mix: 0.0, feedback: 0.7 };
            for i in 0..256 {
                let x = ((i as f32) * 0.01).sin();
                let out = match kind {
                    0 => chorus.process(x, -x, &p),
                    1 => flanger.process(x, -x, &p),
                    _ => phaser.process(x, -x, &p),
                };
                assert_eq!(out.0.to_bits(), x.to_bits(), "kind {kind} ändrade signalen vid mix 0");
                assert_eq!(out.1.to_bits(), (-x).to_bits(), "kind {kind} ändrade signalen vid mix 0");
            }
        }
    }

    #[test]
    fn chorus_sweeps_the_delay_and_flanger_sweeps_less() {
        let p = ModFxParams { rate: 2.0, depth: 1.0, mix: 0.5, feedback: 0.0 };
        let mut chorus = ModDelay::new(48_000.0, 12.0, 8.0);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for _ in 0..24_000 {
            chorus.process(0.0, 0.0, &p);
            lo = lo.min(chorus.delay_ms());
            hi = hi.max(chorus.delay_ms());
        }
        // 12 ms bas ± 8 ms svep: hela svepet ska rymmas, annars rör sig inte LFO:n.
        assert!(lo < 12.5, "nedre läget {lo} ms nådde inte basen");
        assert!(hi > 19.0, "övre läget {hi} ms nådde inte toppen");

        let mut flanger = ModDelay::new(48_000.0, 2.5, 2.0);
        let (mut flo, mut fhi) = (f32::MAX, f32::MIN);
        for _ in 0..24_000 {
            flanger.process(0.0, 0.0, &p);
            flo = flo.min(flanger.delay_ms());
            fhi = fhi.max(flanger.delay_ms());
        }
        assert!((fhi - flo) > 1.8, "flangern svepte bara {} ms", fhi - flo);
        assert!(fhi < 5.0, "flangern är för långsam: {} ms", fhi);
    }

    #[test]
    fn phaser_moves_a_notch_only_when_the_lfo_runs() {
        let sr = 48_000.0;
        let blocks = |depth: f32| -> f32 {
            let mut ph = Phaser::new(sr);
            let p = ModFxParams { rate: 2.0, depth, mix: 0.5, feedback: 0.0 };
            let mut levels = Vec::new();
            let block = 2_400; // 50 ms
            let mut acc = 0.0;
            let mut n = 0;
            for i in 0..(sr as usize) {
                let x = (2.0 * PI * 1_000.0 * i as f32 / sr).sin() * 0.5;
                let (out, _) = ph.process(x, x, &p);
                acc += out * out;
                n += 1;
                if n == block {
                    levels.push((acc / block as f32).sqrt());
                    acc = 0.0;
                    n = 0;
                }
            }
            let mean = levels.iter().sum::<f32>() / levels.len() as f32;
            let max = levels.iter().cloned().fold(f32::MIN, f32::max);
            let min = levels.iter().cloned().fold(f32::MAX, f32::min);
            (max - min) / mean.max(1e-6)
        };
        let still = blocks(0.0);
        let swept = blocks(1.0);
        // Stilla LFO = fast kam = jämn nivå. Svepande = kammen vandrar över tonen.
        assert!(still < 0.01, "nivån rörde sig {still} utan svep");
        assert!(swept > 10.0 * still, "svepet gav {swept}, stilla gav {still}");
        assert!(swept > 0.05, "svepet hörs knappt: {swept}");
    }


    #[test]
    fn bank_ir_is_bounded_finite_and_normalised() {
        for recipe in IR_BANK.iter() {
            let ir = synthesize_ir(recipe, 48_000.0);
            assert!(ir.len() > 1_000, "{}: {} sampel", recipe.name, ir.len());
            assert!(ir.iter().all(|v| v.is_finite()), "{}: icke-finita värden", recipe.name);
            let peak = ir.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!((peak - 1.0).abs() < 1e-3, "{}: topp {peak}", recipe.name);
        }
    }

    #[test]
    fn bank_ir_decays_and_the_hall_is_longer_than_the_room() {
        let energy = |ir: &[f32], from: f32, to: f32| -> f32 {
            let a = (ir.len() as f32 * from) as usize;
            let b = ((ir.len() as f32 * to) as usize).min(ir.len());
            ir[a..b].iter().map(|v| v * v).sum::<f32>() / (b - a).max(1) as f32
        };
        for recipe in IR_BANK.iter().filter(|r| !r.reverse) {
            let ir = synthesize_ir(recipe, 48_000.0);
            let head = energy(&ir, 0.10, 0.20);
            let tail = energy(&ir, 0.85, 0.95);
            assert!(head > 4.0 * tail, "{}: svansen dog inte ({head} mot {tail})", recipe.name);
        }
        let room = synthesize_ir(&IR_BANK[0], 48_000.0);
        let hall = synthesize_ir(&IR_BANK[3], 48_000.0);
        // Convolvern kapar vid CONV_MAX_LEN (2 s), så katedralens recept blir 96 000 sampel.
        assert!(hall.len() > 3 * room.len(), "katedralen är inte längre än rummet");
    }

    #[test]
    fn a_click_through_the_bank_leaves_a_tail() {
        let mut conv = Convolver::new(48_000.0);
        conv.set_ir(Some(std::sync::Arc::new(synthesize_ir(&IR_BANK[0], 48_000.0))));
        let p = ConvolverParams { mix: 1.0, gain_db: 0.0 };
        let mut out = Vec::new();
        for i in 0..(48_000 / 2) {
            let x = if i == 0 { 1.0 } else { 0.0 };
            out.push(conv.process(x, x, &p));
        }
        // Direktljudet ligger en blockfördröjning in; svansen ska leva långt efter det.
        let peak = out[CONV_BLOCK..CONV_BLOCK + 512]
            .iter()
            .fold(0.0f32, |m, (l, _)| m.max(l.abs()));
        let early: f32 = out[CONV_BLOCK..CONV_BLOCK + 256].iter().map(|(l, _)| l * l).sum();
        let late: f32 = out[6_000..6_256].iter().map(|(l, _)| l * l).sum();
        assert!(peak > 0.5, "inget direktljud alls (topp {peak})");
        assert!(late > 0.0, "svansen är tom 125 ms efter klicket");
        assert!(late < early, "svansen är starkare än direktljudet ({late} mot {early})");
    }

    /// **Granulizern spelar vidare efter att ljudet tystnat.** Ett 200 ms-burst matas in,
    /// sedan tystnad: med granulizern på hörs korn ur det som nyss spelades, med den av
    /// är utgången tyst. Det är skillnaden mellan en granulizer och en efterklang.
    #[test]
    fn the_granulizer_keeps_sounding_after_the_input_stops() {
        let tail_energy = |enabled: bool| {
            let mut chain = MasterFxChain::new(48_000.0);
            let mut params = MasterFxParams::default();
            params.granular_enabled = enabled;
            params.granular = GranularParams {
                size_ms: 80.0,
                density: 20.0,
                position_ms: 100.0,
                spray_ms: 30.0,
                speed: 1.0,
                mix: 1.0,
            };
            chain.set_params(params);
            for i in 0..9_600 {
                let x = (2.0 * std::f32::consts::PI * 1_000.0 * i as f32 / 48_000.0).sin() * 0.5;
                chain.process(x, x);
            }
            let mut sum = 0.0f64;
            for _ in 0..9_600 {
                let (l, _) = chain.process(0.0, 0.0);
                sum += (l as f64).abs();
            }
            sum / 9_600.0
        };
        let off = tail_energy(false);
        let on = tail_energy(true);
        println!("medelutgång efter tystnaden: av {off:.5}, på {on:.5}");
        assert!(off < 1e-6, "utan granulizer ska det vara tyst: {off:.6}");
        assert!(on > 0.01, "med granulizer ska kornen höras: {on:.5}");
    }

    /// **Kornens hastighet transponerar.** Ett enda korn spelas in med 500 Hz och spelas upp
    /// med hastighet 2,0 — då ska kornet innehålla 1 kHz. Kornet mäts för sig (låg täthet,
    /// långt korn), eftersom överlappande korn med olika fas summa inte ger en ren ton.
    #[test]
    fn the_granular_speed_transposes_the_grains() {
        let render = |speed: f32| {
            let mut chain = MasterFxChain::new(48_000.0);
            let mut params = MasterFxParams::default();
            params.granular_enabled = true;
            params.granular = GranularParams {
                size_ms: 200.0,
                density: 2.0,
                position_ms: 100.0,
                spray_ms: 0.0,
                speed,
                mix: 1.0,
            };
            chain.set_params(params);
            let mut out = Vec::new();
            for i in 0..96_000 {
                let x = (2.0 * std::f32::consts::PI * 500.0 * i as f32 / 48_000.0).sin() * 0.5;
                let (l, _) = chain.process(x, x);
                out.push(l);
            }
            out
        };
        // Grundton i ett fönster, med autokorrelation. Första toppen, inte högsta — en
        // periodisk signal toppar också på två och tre perioder.
        let f0 = |samples: &[f32], sr: f32| -> f32 {
            let (min_lag, max_lag) = ((sr / 3_000.0) as usize, (sr / 200.0) as usize);
            let mut scores = Vec::new();
            for lag in min_lag..max_lag {
                let mut dot = 0.0f32;
                let mut energy = 0.0f32;
                for i in 0..samples.len() - lag {
                    dot += samples[i] * samples[i + lag];
                    energy += samples[i] * samples[i];
                }
                scores.push((lag, dot / (energy + 1e-9)));
            }
            let best = scores.iter().map(|(_, v)| *v).fold(f32::NEG_INFINITY, f32::max);
            let lag = scores
                .iter()
                .find(|(_, v)| *v >= best * 0.9)
                .map(|(l, _)| *l)
                .unwrap_or(min_lag);
            sr / lag as f32
        };
        // Korn nummer två börjar vid 24000 sampel; mät mitten av det.
        let plain = render(1.0);
        let double = render(2.0);
        let a = f0(&plain[28_000..32_000], 48_000.0);
        let b = f0(&double[28_000..32_000], 48_000.0);
        println!("kornets tonhöjd: 1,0× {a:.0} Hz, 2,0× {b:.0} Hz");
        // Absolutvärdena ligger några procent högt (autokorrelationen har heltalsfördröjning),
        // men **kvoten** är det som mäts: två gånger hastigheten ska ge två gånger frekvensen.
        assert!(a > 400.0, "kornet ska höras: {a:.0} Hz");
        let ratio = b / a;
        assert!(
            (ratio - 2.0).abs() < 0.1,
            "dubbel hastighet ska ge dubbel frekvens: {ratio:.3} ({a:.0} Hz mot {b:.0} Hz)"
        );
    }

    /// **Monokontroll lägger ihop kanalerna, dämpningen sänker nivån.** Båda mäts efter
    /// limitern, där Control Room sitter. Kanalerna jämförs **sampel för sampel** — att
    /// jämföra medelbelopp hade gett samma tal för två olika sinustoner.
    #[test]
    fn control_room_folds_to_mono_and_dims_by_the_set_amount() {
        // (skillnaden mellan kanalerna, summan av vänsterns belopp)
        let run = |mono: bool, dim: bool, dim_db: f32| {
            let mut chain = MasterFxChain::new(48_000.0);
            let mut params = MasterFxParams::default();
            params.control = ControlRoomParams { mono, dim, dim_db, level_db: 0.0 };
            chain.set_params(params);
            let (mut diff, mut energy) = (0.0f64, 0.0f64);
            for i in 0..4_800 {
                let t = i as f32 / 48_000.0;
                let left = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
                let right = (2.0 * std::f32::consts::PI * 660.0 * t).sin() * 0.5;
                let (a, b) = chain.process(left, right);
                diff += (a - b).abs() as f64;
                energy += a.abs() as f64;
            }
            (diff, energy)
        };
        let (stereo_diff, stereo_energy) = run(false, false, -20.0);
        assert!(stereo_diff > 10.0, "stereo ska ha olika kanaler: {stereo_diff:.1}");

        let (mono_diff, _) = run(true, false, -20.0);
        assert!(mono_diff < 1e-3, "mono ska ge identiska kanaler: {mono_diff:.6}");

        let (_, dim_energy) = run(false, true, -20.0);
        let ratio_db = 20.0 * (dim_energy / stereo_energy).log10();
        assert!(
            (ratio_db + 20.0).abs() < 0.5,
            "dämpningen ska vara -20 dB: {ratio_db:.2} dB"
        );
    }

    /// Amplituden vid en frekvens, mätt med en komplex korrelation (det som Goertzel gör).
    fn mag_at(samples: &[f32], freq: f32, sr: f32) -> f32 {
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for (n, &x) in samples.iter().enumerate() {
            let ph = 2.0 * std::f32::consts::PI * freq * n as f32 / sr;
            re += x * ph.cos();
            im -= x * ph.sin();
        }
        (re * re + im * im).sqrt() / samples.len() as f32
    }

    /// **Skiftet flyttar spektrumet, det transponerar det inte.** En 1 kHz-ton skiftad +200 Hz
    /// ska ge 1 200 Hz — och **inte** 800 Hz, som en ringmodulering hade gett, och inte heller
    /// kvar på 1 000 Hz. Motsatt riktning prövas i samma prov.
    #[test]
    fn the_frequency_shifter_moves_one_side_of_the_spectrum() {
        let sr = 48_000.0;
        for (hz, want, unwanted) in [(200.0f32, 1200.0f32, 800.0f32), (-200.0, 800.0, 1200.0)] {
            let mut shift = FrequencyShifter::new(sr);
            let params = FreqShiftParams {
                hz,
                mix: 1.0,
                spread: false,
            };
            let out: Vec<f32> = (0..24_000)
                .map(|n| {
                    let x =
                        (2.0 * std::f32::consts::PI * 1000.0 * n as f32 / sr).sin() * 0.5;
                    shift.process(x, x, &params).0
                })
                .collect();
            // Filtret är 63 tapp långt: mät efter att det fyllts.
            let tail = &out[12_000..];
            let moved = mag_at(tail, want, sr);
            let mirror = mag_at(tail, unwanted, sr);
            let source = mag_at(tail, 1000.0, sr);
            println!(
                "{hz:+.0} Hz: {want:.0} Hz = {moved:.4}, {unwanted:.0} Hz = {mirror:.4}, 1 kHz = {source:.4}"
            );
            assert!(moved > 0.1, "den skiftade tonen ska finnas kvar: {moved:.4}");
            assert!(
                mirror < moved * 0.1,
                "spegelbilden ska vara borta: {mirror:.4} mot {moved:.4}"
            );
            assert!(
                source < moved * 0.1,
                "källtonen ska vara borta: {source:.4} mot {moved:.4}"
            );
        }
    }

    /// 0 Hz skift ska vara en ofärgad genomgång — annars hade varje projekt med pedalen på
    /// slaget läge fått en ljudförändring.
    #[test]
    fn a_zero_shift_passes_the_signal_through_uncoloured() {
        let sr = 48_000.0;
        let mut shift = FrequencyShifter::new(sr);
        let params = FreqShiftParams {
            hz: 0.0,
            mix: 1.0,
            spread: false,
        };
        let input: Vec<f32> = (0..8_000)
            .map(|n| (2.0 * std::f32::consts::PI * 440.0 * n as f32 / sr).sin() * 0.5)
            .collect();
        let out: Vec<f32> = input
            .iter()
            .map(|&x| shift.process(x, x, &params).0)
            .collect();
        let rms = |v: &[f32]| (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt();
        let before = rms(&input);
        let after = rms(&out[2_000..]);
        println!("0 Hz: RMS {before:.4} in, {after:.4} ut");
        assert!((after - before).abs() < before * 0.01, "nivån ska stå still");
    }

    /// **Stereoläget skiftar kanalerna åt varsitt håll**, alltså inte bara olika panorering:
    /// vänster får den skiftade tonen, höger får spegelbilden.
    #[test]
    fn the_spread_shift_moves_the_channels_in_opposite_directions() {
        let sr = 48_000.0;
        let mut shift = FrequencyShifter::new(sr);
        let params = FreqShiftParams {
            hz: 200.0,
            mix: 1.0,
            spread: true,
        };
        let (mut left, mut right) = (Vec::new(), Vec::new());
        for n in 0..6_000 {
            let x = (2.0 * std::f32::consts::PI * 1000.0 * n as f32 / sr).sin() * 0.5;
            let (l, r) = shift.process(x, x, &params);
            left.push(l);
            right.push(r);
        }
        let (l_tail, r_tail) = (&left[3_000..], &right[3_000..]);
        let l_up = mag_at(l_tail, 1200.0, sr);
        let l_down = mag_at(l_tail, 800.0, sr);
        let r_up = mag_at(r_tail, 1200.0, sr);
        let r_down = mag_at(r_tail, 800.0, sr);
        println!("vänster: 1200 Hz {l_up:.4}, 800 Hz {l_down:.4} · höger: 1200 Hz {r_up:.4}, 800 Hz {r_down:.4}");
        assert!(l_up > 0.1 && l_down < l_up * 0.1, "vänster ska gå upp");
        assert!(r_down > 0.1 && r_up < r_down * 0.1, "höger ska gå ned");
    }

    /// **Krossen kvantiserar amplituden.** 2 bitar ger fyra steg per polaritet, alltså sju
    /// möjliga värden (nollan inräknad). 16 bitar ger i praktiken ett kontinuum — att de två
    /// skiljer sig är beviset på att ratten biter.
    #[test]
    fn the_bit_crusher_quantizes_to_the_chosen_number_of_levels() {
        let count = |bits: f32| {
            let mut crush = BitCrusher::new(48_000.0);
            let params = BitCrushParams {
                bits,
                downsample: 1.0,
                tone: 1.0,
                mix: 1.0,
            };
            let mut levels: Vec<i32> = (0..4_800)
                .map(|n| {
                    let x =
                        (2.0 * std::f32::consts::PI * 220.0 * n as f32 / 48_000.0).sin() * 0.8;
                    (crush.process(x, x, &params).0 * 400.0).round() as i32
                })
                .collect();
            levels.sort_unstable();
            levels.dedup();
            levels.len()
        };
        let coarse = count(2.0);
        let fine = count(16.0);
        println!("2 bitar: {coarse} nivåer, 16 bitar: {fine} nivåer");
        assert_eq!(coarse, 7, "2 bitar ska ge sju värden (fyra steg per polaritet + nollan)");
        assert!(fine > coarse * 20, "16 bitar ska vara i praktiken kontinuerligt: {fine}");
    }

    /// **Nedblandningen håller sampelvärdena stilla.** Med 16 steg ska antalet ändringar
    /// mellan intilliggande sampel falla till omkring en sextondel.
    #[test]
    fn the_downsampler_holds_each_sample_for_the_chosen_number_of_steps() {
        let changes = |downsample: f32| {
            let mut crush = BitCrusher::new(48_000.0);
            let params = BitCrushParams {
                bits: 16.0,
                downsample,
                tone: 1.0,
                mix: 1.0,
            };
            let out: Vec<f32> = (0..9_600)
                .map(|n| {
                    let x = (2.0 * std::f32::consts::PI * 220.0 * n as f32 / 48_000.0).sin() * 0.5;
                    crush.process(x, x, &params).0
                })
                .collect();
            out.windows(2).filter(|w| (w[1] - w[0]).abs() > 1e-7).count()
        };
        let plain = changes(1.0);
        let held = changes(16.0);
        println!("ändringar: {plain} utan nedblandning, {held} med 16 steg");
        assert!(
            held * 8 < plain,
            "nedblandningen ska minska ändringarna kraftigt: {held} mot {plain}"
        );
    }

    /// **Varje band får sin egen tid.** En impuls in ger tre svar: ett vid varje bandtid och
    /// ingenting däremellan. Ordningen är beviset på att tiderna inte hamnat på samma rad.
    #[test]
    fn the_multiband_delay_answers_once_per_band_time() {
        let sr = 48_000.0;
        let mut delay = MultibandDelay::new(sr);
        let params = MultibandDelayParams {
            times_ms: [50.0, 100.0, 150.0],
            xover_low: 250.0,
            xover_high: 4000.0,
            mix: 1.0,
        };
        let out: Vec<f32> = (0..12_000)
            .map(|n| delay.process(if n == 0 { 1.0 } else { 0.0 }, 0.0, &params).0)
            .collect();
        let energy = |center_ms: f32| {
            let c = (center_ms * 0.001 * sr) as usize;
            let half = (0.003 * sr) as usize;
            out[c.saturating_sub(half)..(c + half).min(out.len())]
                .iter()
                .map(|v| v.abs())
                .sum::<f32>()
        };
        let peaks: Vec<f32> = [50.0, 100.0, 150.0].iter().map(|&t| energy(t)).collect();
        let between: Vec<f32> = [75.0, 125.0, 175.0].iter().map(|&t| energy(t)).collect();
        println!("toppar {peaks:?} · mellan {between:?}");
        for (i, p) in peaks.iter().enumerate() {
            assert!(*p > 1e-3, "band {} ska svara vid sin tid: {p}", i);
            assert!(
                *p > between[i] * 10.0,
                "band {} ska svara vid sin tid och inte mellan: {} mot {}",
                i,
                p,
                between[i]
            );
        }
    }
    /// **Banken svarar en gång per linje, på sin egen tid.** En impuls in ska ge tre ekon vid
    /// de tre tiderna, med de tre nivåerna, och ingenting däremellan.
    #[test]
    fn the_delay_bank_answers_once_per_line() {
        let sr = 48_000.0;
        let mut bank = DelayBank::new(sr);
        let params = DelayBankParams {
            times_ms: [50.0, 120.0, 200.0],
            levels: [1.0, 0.5, 0.25],
            feedback: 0.0,
            spread: 0.0,
            mix: 1.0,
        };
        let out: Vec<f32> = (0..24_000)
            .map(|n| bank.process(if n == 0 { 1.0 } else { 0.0 }, 0.0, &params).0)
            .collect();
        let energy = |ms: f32| {
            let c = (ms * 0.001 * sr) as usize;
            let half = (0.002 * sr) as usize;
            out[c.saturating_sub(half)..(c + half).min(out.len())]
                .iter()
                .map(|v| v.abs())
                .sum::<f32>()
        };
        let taps: Vec<f32> = [50.0, 120.0, 200.0].iter().map(|&t| energy(t)).collect();
        println!("linjer: {taps:?}");
        assert!(taps[0] > 0.4, "linje 1 (nivå 1,0): {:.3}", taps[0]);
        assert!(taps[1] > 0.2 && taps[1] < taps[0] * 0.8, "linje 2 ska vara svagare: {taps:?}");
        assert!(taps[2] > 0.1 && taps[2] < taps[1] * 0.8, "linje 3 ska vara svagast: {taps:?}");
    }

    /// **Återkopplingen ger ett fjärde eko på dubbla tiden** — och utan återkoppling finns
    /// det inte där. Det är skillnaden mellan en delay med och utan minne.
    #[test]
    fn the_delay_bank_feedback_returns_a_second_time_around() {
        let sr = 48_000.0;
        let at_100ms = |feedback: f32| {
            let mut bank = DelayBank::new(sr);
            let params = DelayBankParams {
                times_ms: [50.0, 500.0, 900.0],
                levels: [1.0, 0.0, 0.0],
                feedback,
                spread: 0.0,
                mix: 1.0,
            };
            let out: Vec<f32> = (0..24_000)
                .map(|n| bank.process(if n == 0 { 1.0 } else { 0.0 }, 0.0, &params).0)
                .collect();
            let c = (0.1 * sr) as usize;
            out[c - 100..c + 100].iter().map(|v| v.abs()).sum::<f32>()
        };
        let without = at_100ms(0.0);
        let with = at_100ms(0.6);
        println!("100 ms: {without:.4} utan återkoppling, {with:.4} med 0,6");
        assert!(without < 1e-4, "utan återkoppling ska det vara tyst där: {without:.6}");
        assert!(with > 0.1, "med återkoppling ska andra varvet höras: {with:.4}");
    }

    /// Övertoner i förhållande till grundtonen — det som gör distorsion mätbar.
    fn thd(samples: &[f32], f0: f32, sr: f32) -> f32 {
        let mag = |f: f32| {
            let (mut re, mut im) = (0.0f32, 0.0f32);
            for (n, &x) in samples.iter().enumerate() {
                let ph = 2.0 * std::f32::consts::PI * f * n as f32 / sr;
                re += x * ph.cos();
                im -= x * ph.sin();
            }
            (re * re + im * im).sqrt() / samples.len() as f32
        };
        let fund = mag(f0);
        let harmonics: f32 = [3.0f32, 5.0, 7.0].iter().map(|k| mag(f0 * k)).sum();
        if fund <= 1e-9 { f32::INFINITY } else { harmonics / fund }
    }

    /// **Distructor biter, och de två kopplingarna låter olika.** Mer drive ska ge mer
    /// övertoner — annars är det ingen distorsion — och serie/parallellt ska inte ge samma
    /// resultat, annars är ratten en attrapp.
    #[test]
    fn the_distructor_stacks_its_stages_in_series() {
        let sr = 48_000.0;
        let render = |drive: f32, parallel: f32| {
            let mut d = Distructor::new(sr);
            let params = DistructorParams {
                drive: [drive; 3],
                parallel,
                tone_hz: 20_000.0,
                mix: 1.0,
            };
            let out: Vec<f32> = (0..24_000)
                .map(|n| {
                    let x = (2.0 * std::f32::consts::PI * 220.0 * n as f32 / sr).sin() * 0.5;
                    d.process(x, x, &params).0
                })
                .collect();
            out[12_000..].to_vec()
        };
        let soft = thd(&render(0.1, 0.0), 220.0, sr);
        let hard = thd(&render(0.9, 0.0), 220.0, sr);
        let par = thd(&render(0.9, 1.0), 220.0, sr);
        println!("THD: drive 0,1 {soft:.3}, drive 0,9 {hard:.3}, drive 0,9 parallellt {par:.3}");
        assert!(hard > soft * 2.0, "mer drive ska ge mer övertoner: {soft:.3} → {hard:.3}");
        assert!(
            (par - hard).abs() > hard * 0.2,
            "serie och parallellt ska ge olika övertonston: {par:.3} mot {hard:.3}"
        );
    }

    /// **Vikningssteget låter annorlunda än mättnaden** — vid samma drive ska det ge en
    /// annan övertonston, inte bara mer av samma.
    #[test]
    fn the_foldback_stage_differs_from_the_soft_one() {
        let sr = 48_000.0;
        let render_one_stage = |drive: [f32; 3]| {
            let mut d = Distructor::new(sr);
            let params = DistructorParams {
                drive,
                parallel: 1.0,
                tone_hz: 20_000.0,
                mix: 1.0,
            };
            let out: Vec<f32> = (0..24_000)
                .map(|n| {
                    let x = (2.0 * std::f32::consts::PI * 220.0 * n as f32 / sr).sin() * 0.5;
                    d.process(x, x, &params).0
                })
                .collect();
            out[12_000..].to_vec()
        };
        let soft = thd(&render_one_stage([0.8, 0.0, 0.0]), 220.0, sr);
        let folded = thd(&render_one_stage([0.0, 0.0, 0.8]), 220.0, sr);
        println!("THD: mjukt steg {soft:.3}, vikning {folded:.3}");
        assert!(
            (folded - soft).abs() > soft * 0.25,
            "stegen ska ge olika övertonston: {soft:.3} mot {folded:.3}"
        );
    }

