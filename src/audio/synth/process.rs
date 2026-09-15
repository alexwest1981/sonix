//! Renderingsloopen: en bildruta ljud i taget — röster, spår, bussar, sidokedjor och master.
//! Status: stabil — motorns renderingsloop (en bildruta ljud i taget).
//! Rör inte: ordningen i `process_stereo` — en sändares utgång måste finnas när mottagarens kedja kör (sends mellan spår, 8.3); mät fasen mot en kontroll på samma nivå, inte mot "2×".

use super::*;

impl SynthEngine {
    pub fn process_stereo(&mut self) -> (f32, f32) {
        // 0. Fire any due scheduled chord/strum notes.
        if !self.scheduled_notes.is_empty() {
            let mut i = 0;
            while i < self.scheduled_notes.len() {
                if self.scheduled_notes[i].samples_until == 0 {
                    let ev = self.scheduled_notes.swap_remove(i);
                    let mut target = None;
                    for (vi, voice) in self.voices.iter().enumerate() {
                        if voice.is_active() && voice.note == ev.note {
                            target = Some(vi);
                            break;
                        }
                    }
                    if target.is_none() {
                        for (vi, voice) in self.voices.iter().enumerate() {
                            if !voice.is_active() {
                                target = Some(vi);
                                break;
                            }
                        }
                    }
                    let idx = target.unwrap_or(0);
                    self.voices[idx].trigger(ev.note, ev.freq, ev.velocity, self.sample_rate);
                } else {
                    self.scheduled_notes[i].samples_until -= 1;
                    i += 1;
                }
            }
        }

        let mut mixed = 0.0;
        let mut active_count = 0;

        // 1. Synthesizer voices (each with its own envelope + filter state)
        for voice in &mut self.voices {
            if voice.is_active() {
                mixed += voice.next_sample(
                    self.waveform,
                    &self.adsr,
                    &self.filter_params,
                    &self.filter_env,
                    self.filter_env_amount,
                );
                active_count += 1;
            }
        }

        if active_count > 1 {
            mixed *= 1.0 / (1.0 + 0.25 * (active_count as f32 - 1.0));
        }

        // 2. Drive / Saturation on Synth
        if self.drive > 1.05 {
            mixed = (mixed * self.drive).tanh() / (self.drive * 0.7 + 0.3);
        }

        // 3. Per-voice resonant lowpass is applied inside each voice; the synth
        //    bus itself is no longer filtered globally.
        let synth_out = mixed;

        // 4. Drum bus
        let mut drum_mix = 0.0;
        for drum in &mut self.drums {
            if drum.active {
                drum_mix += drum.next_sample();
            }
        }

        // 5. WAV one-shot sample voices (Channel Rack real samples)
        let mut sample_mix = 0.0;
        for sv in &mut self.sample_voices {
            if !sv.is_active() {
                continue;
            }
            let len = sv.left.len();
            if len == 0 {
                sv.active = false;
                continue;
            }
            // Not-av (Fas 8.4): uttryckligt (`ReleaseSampleVoices`) eller efter notens
            // längd. För en lopande röst betyder det "spela resten efter loopen"; för en
            // en-skottsröst betyder det ingenting, för den tar slut själv.
            if !sv.released && sv.hold_frames.is_some_and(|h| sv.frames_done >= h) {
                sv.released = true;
            }
            if sv.released && sv.env_on {
                sv.amp_env.gate_off();
            }
            let looping = match sv.loop_mode {
                LoopMode::Off => false,
                LoopMode::Forever => sv.loop_span.is_some(),
                LoopMode::UntilRelease => sv.loop_span.is_some() && !sv.released,
            };

            // Loopen vrids (eller vänder, med ping-pong) i sina ändar.
            if looping
                && let Some((lo, hi)) = sv.loop_span
            {
                if sv.dir >= 0.0 && sv.pos >= hi {
                    let over = sv.pos - hi;
                    sv.pos = if sv.ping_pong { hi - over } else { lo + over };
                    sv.pos = sv.pos.max(lo);
                    if sv.ping_pong {
                        sv.dir = -1.0;
                    }
                } else if sv.dir < 0.0 && sv.pos <= lo {
                    let over = lo - sv.pos;
                    sv.pos = if sv.ping_pong { hi + over } else { hi - over };
                    sv.pos = sv.pos.min(hi);
                    if sv.ping_pong {
                        sv.dir = 1.0;
                    }
                }
            }

            // Slutvillkoret: utan loop är det exakt som förut, för `dir` är då samma som
            // `reverse` och ändras aldrig.
            let past_end = if sv.dir >= 0.0 {
                sv.pos >= sv.end_frame
            } else {
                sv.pos <= sv.start_frame
            };
            if past_end {
                sv.active = false;
                continue;
            }
            let idx0 = sv.pos.floor() as usize;
            let frac = sv.pos - idx0 as f32;
            if idx0 >= len {
                sv.active = false;
                continue;
            }
            let idx1 = (idx0 + 1).min(len - 1);
            let l0 = sv.left[idx0];
            let l1 = sv.left[idx1];
            let r0 = if idx0 < sv.right.len() { sv.right[idx0] } else { l0 };
            let r1 = if idx1 < sv.right.len() { sv.right[idx1] } else { l1 };
            let mut s_l = l0 + (l1 - l0) * frac;
            let mut s_r = r0 + (r1 - r0) * frac;
            s_l *= sv.pan_l;
            s_r *= sv.pan_r;
            let mut g = sv.volume;
            // **Kantdämpning vid båda ändarna** (Fas 8.7 steg 2). In: hur långt in vi är.
            // Ut: hur många ramar som återstår till fönstrets kant — räknat i *utramar*, så
            // det stämmer även när rösten spelas med annan tonhöjd eller baklänges. Utan den
            // här sidan klickar en slice som tar slut mitt i en ton.
            g *= slice_edge_gain(sv.frames_done as f32, sv.fade_frames as f32);
            let frames_to_edge = if sv.step.abs() > 1e-6 {
                if sv.dir >= 0.0 {
                    (sv.end_frame - sv.pos) / sv.step
                } else {
                    (sv.pos - sv.start_frame) / sv.step
                }
            } else {
                f32::MAX
            };
            g *= slice_edge_gain(frames_to_edge, sv.fade_frames as f32);
            // Amplitud-ADSR (Fas 8.4) — bara när den är vald. Identiteten rör ingenting,
            // alltså är en-skottsvägen oförändrad (se provet om byte-identitet).
            if sv.env_on {
                g *= sv.amp_env.next_sample(&sv.env_params);
            }
            sample_mix += (s_l + s_r) * 0.5 * g;
            if sv.env_on && !sv.amp_env.is_active() {
                sv.active = false;
                continue;
            }

            if sv.dir >= 0.0 {
                sv.pos += sv.step;
            } else {
                sv.pos -= sv.step;
            }
            sv.frames_done += 1;
        }

        let combined = synth_out + drum_mix + sample_mix;

        // 5. Reverb FX
        let rev_out = self.reverb.process(combined, &self.reverb_params);

        // 6. Stereo Ping-Pong Delay FX
        let (del_l, del_r) = self.delay.process(rev_out, rev_out, &self.delay_params);

        // 6b. Plug-in delay compensation: find the largest insert latency so
        //     every bus can be aligned to it (Fas 4.2). Only relevant while the
        //     song is playing, so live monitoring keeps its low latency.
        let max_plugin_latency = if self.song_playing {
            self.stem_tracks
                .iter()
                .map(|t| {
                    let plugin = t
                        .plugin
                        .as_ref()
                        .map(|p| p.latency_frames())
                        .unwrap_or(0);
                    let pitch = if t.pitch_active {
                        t.pitch_shifter.latency_frames()
                    } else {
                        0
                    };
                    // Det manuella offsetet hör hit också (Fas 8.6): ett spår med offset ska
                    // dra med sig de andra, annars vore det ingen kompensation.
                    crate::audio::plugin_host_live::compensated_latency(
                        plugin,
                        pitch,
                        t.manual_latency_frames,
                    )
                })
                .max()
                .unwrap_or(0)
        } else {
            0
        };
        self.pdc_bus.set_delay(max_plugin_latency);
        let (del_l, del_r) = self.pdc_bus.process(del_l, del_r);

        // 7. Multi-Track Stem Audio Streaming (with Region Slicing & Fades)
        let mut stem_mix_l = 0.0;
        let mut stem_mix_r = 0.0;

        if self.song_playing && !self.stem_tracks.is_empty() {
            // Copy the group state so the loop can borrow `stem_tracks` mutably
            // (Fas 5.2). A track is soloed if its own solo, its bus's solo or its
            // VCA's solo is on; a track is silenced if its own, bus or VCA mute
            // is on. Group gain is applied post-fader, just before the master.
            let bus_volume = self.bus_volume;
            let bus_muted = self.bus_muted;
            let bus_solo = self.bus_solo;
            let vca_volume = self.vca_volume;
            let vca_muted = self.vca_muted;
            let vca_solo = self.vca_solo;
            let has_solo = self.has_stem_solo
                || bus_solo.iter().any(|&s| s)
                || vca_solo.iter().any(|&s| s);
            let current_time_sec = self.song_time_samples as f32 / self.sample_rate;
            // Sidokedjor (Fas 8.3): nyckelsignalerna är varje spårs senaste
            // utgångssample. De kopieras hit av samma skäl som gruppläget ovan —
            // loopen lånar `stem_tracks` mutabelt och kan inte läsa ett annat spår.
            let key_taps: Vec<(f32, f32)> = self
                .stem_tracks
                .iter()
                .map(|t| (t.last_out_l, t.last_out_r))
                .collect();

            // Buffetarna för det **pågående samplet** tas ut ur self: loopen lånar
            // `stem_tracks` mutabelt och kan inte läsa en annan medlem samtidigt. De
            // seedas med förra samplets utgångar, så att en send som (mot förmodan, i en
            // handredigerad fil) pekar bakåt ger förra samplet i stället för tystnad.
            let mut out_l = std::mem::take(&mut self.track_out_l);
            let mut out_r = std::mem::take(&mut self.track_out_r);
            let incoming = std::mem::take(&mut self.stem_incoming);
            // Pluginens egna utbussar (Fas 8.6) samlas per spår för **det här** samplet.
            // Nollningen sker här, en gång per sample: en buss vars mål är tystat läses
            // aldrig, och utan nollningen hade den blivit stående och läckt in senare.
            let mut plugin_bus_in = std::mem::take(&mut self.plugin_bus_in);
            for slot in plugin_bus_in.iter_mut() {
                *slot = [0.0, 0.0];
            }
            for (i, last) in self.stem_tracks.iter().enumerate() {
                if i < out_l.len() {
                    out_l[i] = last.last_out_l;
                    out_r[i] = last.last_out_r;
                }
            }

            let track_count = self.stem_tracks.len();
            for order_pos in 0..track_count {
                let track_idx = self
                    .stem_order
                    .get(order_pos)
                    .copied()
                    .unwrap_or(order_pos)
                    .min(track_count.saturating_sub(1));
                let here: &[(usize, f32)] = incoming.get(track_idx).map(|v| v.as_slice()).unwrap_or(&[]);
                let track = &mut self.stem_tracks[track_idx];
                let bus = track.bus.min(NUM_BUSES - 1);
                let vca = track.vca.filter(|&v| v < NUM_VCAS);
                let group_soloed = bus_solo[bus] || vca.map(|v| vca_solo[v]).unwrap_or(false);
                let group_muted = bus_muted[bus] || vca.map(|v| vca_muted[v]).unwrap_or(false);
                let audible = if has_solo {
                    track.solo || group_soloed
                } else {
                    !track.muted && !group_muted
                };
                if !audible || track.left.is_empty() {
                    // Keep an engaged shifter primed with silence so that it is
                    // continuous (and latency-aligned) the moment the track is
                    // heard again.
                    if track.pitch_active && !track.left.is_empty() {
                        track.pitch_shifter.process(0.0, 0.0);
                    }
                    continue;
                }
                let group_gain =
                    bus_volume[bus] * vca.map(|v| vca_volume[v]).unwrap_or(1.0);

                let pan_l = track.pan_l;
                let pan_r = track.pan_r;
                let mut track_l = 0.0_f32;
                let mut track_r = 0.0_f32;

                // **Spår-sends in i kedjan** (Fas 8.3): en del av sändarens utgång läggs
                // till här — *före* pitch, EQ och kompressor — så att mottagarens kedja
                // bearbetar den precis som sitt eget ljud. Sändaren är alltid räknad först
                // (`stem_order`), alltså är den exakt i fas och inte en sample sen.
                //
                // Målets eget läge gäller: en tystad mottagare kommer aldrig hit (loopen
                // hoppar över den ovan), och när något är soloat hörs bara den soloades
                // väg — samma regel som för buss-sends.
                for &(sender, level) in here {
                    if sender < out_l.len() {
                        track_l += out_l[sender] * level;
                        track_r += out_r[sender] * level;
                    }
                }
                // **Pluginens egna utbussar in i kedjan** (Fas 8.6): samma plats och samma
                // regel som spår-senden ovan — mottagarens pitch, EQ och kompressor
                // bearbetar bussen precis som sitt eget ljud. Källspåret är alltid räknat
                // först (`stem_order` får sin kant ur `extra_out_targets`), alltså är
                // bussen i fas och inte en sample sen.
                if let Some(bus) = plugin_bus_in.get(track_idx) {
                    track_l += bus[0];
                    track_r += bus[1];
                }

                if !track.regions.is_empty() {
                    // Play defined audio regions/slices
                    for region in &track.regions {
                        if region.muted {
                            continue;
                        }
                        let r_end = region.start_time_secs + region.length_secs;
                        if current_time_sec >= region.start_time_secs && current_time_sec < r_end {
                            let rel_time = current_time_sec - region.start_time_secs;
                            let mut env = 1.0_f32;
                            if region.fade_in_sec > 0.001 && rel_time < region.fade_in_sec {
                                env *= (rel_time / region.fade_in_sec).clamp(0.0, 1.0);
                            }
                            let time_left = r_end - current_time_sec;
                            if region.fade_out_sec > 0.001 && time_left < region.fade_out_sec {
                                env *= (time_left / region.fade_out_sec).clamp(0.0, 1.0);
                            }

                            // Var i källjudet regionen är (Fas 8.10): ren funktion,
                            // så att tidsmappningen kan prövas utan ljudmotor.
                            let sample_pos_sec = region_source_secs(region, rel_time);
                            // Klippets eget källjud (Fas 8.10 steg 2): en färdigsträckt
                            // fil läses i stället för spårets buffert, och då är faktorn
                            // 1,0. Allt annat i den här vägen är oförändrat — ingen ny
                            // felkälla i uppspelningen.
                            let (src_left, src_right, src_rate): (&[f32], &[f32], f32) =
                                match region.source_audio.as_ref() {
                                    Some((l, r, sr)) => (l.as_slice(), r.as_slice(), *sr),
                                    None => (track.left.as_slice(), track.right.as_slice(), track.sample_rate),
                                };
                            let sample_pos = (sample_pos_sec * src_rate).max(0.0);
                            let idx0 = sample_pos.floor() as usize;
                            let frac = sample_pos - idx0 as f32;

                            if idx0 + 1 < src_left.len() {
                                let raw_l = src_left[idx0] + (src_left[idx0 + 1] - src_left[idx0]) * frac;
                                let raw_r = if idx0 + 1 < src_right.len() {
                                    src_right[idx0] + (src_right[idx0 + 1] - src_right[idx0]) * frac
                                } else {
                                    raw_l
                                };
                                let g = track.volume * region.gain * env;
                                track_l += raw_l * g * pan_l;
                                track_r += raw_r * g * pan_r;
                            } else if idx0 < src_left.len() {
                                let raw_l = src_left[idx0];
                                let raw_r = if idx0 < src_right.len() { src_right[idx0] } else { raw_l };
                                let g = track.volume * region.gain * env;
                                track_l += raw_l * g * pan_l;
                                track_r += raw_r * g * pan_r;
                            }
                        }
                    }
                } else {
                    // Fallback to full track streaming
                    let track_rel_time = current_time_sec - track.start_time_secs;
                    if track_rel_time >= 0.0 {
                        let sample_pos = (track_rel_time * track.sample_rate).max(0.0);
                        let idx0 = sample_pos.floor() as usize;
                        let frac = sample_pos - idx0 as f32;

                        if idx0 + 1 < track.left.len() {
                            let raw_l = track.left[idx0] + (track.left[idx0 + 1] - track.left[idx0]) * frac;
                            let raw_r = if idx0 + 1 < track.right.len() {
                                track.right[idx0] + (track.right[idx0 + 1] - track.right[idx0]) * frac
                            } else {
                                raw_l
                            };
                            track_l += raw_l * track.volume * pan_l;
                            track_r += raw_r * track.volume * pan_r;
                        } else if idx0 < track.left.len() {
                            let raw_l = track.left[idx0];
                            let raw_r = if idx0 < track.right.len() { track.right[idx0] } else { raw_l };
                            track_l += raw_l * track.volume * pan_l;
                            track_r += raw_r * track.volume * pan_r;
                        }
                    }
                }

                // Real-time formant-preserving pitch shift (always run when
                // engaged, feeding silence through on empty frames so the
                // grain pipeline never stalls).
                if track.pitch_active {
                    let (pl, pr) = track.pitch_shifter.process(track_l, track_r);
                    track_l = pl;
                    track_r = pr;
                }

                // Per-track 3-band parametric EQ
                let (eq_l, eq_r) = track.eq_proc.process(track_l, track_r);
                let (mut tl, mut tr) = (eq_l, eq_r);

                // Per-track compressor (channel strip dynamics)
                if track.comp_ratio > 1.0 {
                    let params = CompressorParams {
                        threshold_db: track.comp_threshold_db,
                        ratio: track.comp_ratio,
                        attack_ms: 12.0,
                        release_ms: 140.0,
                        makeup_db: 0.0,
                    };
                    let (cl, cr) = track.comp.process(tl, tr, &params);
                    tl = cl;
                    tr = cr;
                }

                // Per-track aux sends: 100% wet reverb/delay scaled by send amount.
                if track.reverb_send > 0.0001 {
                    let wet = track.reverb.process((tl + tr) * 0.5, &ReverbParams {
                        room_size: 0.65,
                        damping: 0.4,
                        mix: 1.0,
                    });
                    tl += wet * track.reverb_send;
                    tr += wet * track.reverb_send;
                }
                if track.delay_send > 0.0001 {
                    let (dl, dr) = track.delay.process(tl, tr, &DelayParams {
                        time_ms: 350.0,
                        feedback: 0.35,
                        mix: 1.0,
                    });
                    tl += dl * track.delay_send;
                    tr += dr * track.delay_send;
                }

                // Optional CLAP insert, then PDC-align this track to the
                // project's maximum plugin latency.
                //
                // **Spårtsexakt samma regel som i maxvärdet ovan** (Fas 8.6): pluginens
                // rapport, tonhöjds-skiftaren och användarens manuella offset. Räknades det
                // ena stället med offsetet och inte det andra skulle ett offset flytta spåret
                // utan att de andra följde med — alltså raka motsatsen till kompensation.
                let plugin_latency = track
                    .plugin
                    .as_ref()
                    .map(|p| p.latency_frames())
                    .unwrap_or(0);
                // **Nyckeln till både pluginens sidokedja och vår duckare** (Fas 8.6): en
                // källa, två mottagare. Har spåret en sidokedja satt och pluginen en
                // sidokedje-ingång får pluginen nyckelsignalen — samma urval av nyckelspår
                // som duckaren använder, och samma bildruta som huvudingången, eftersom
                // `feed_sidechain` skriver på samma plats i sitt block.
                let side_key = track
                    .sidechain_from
                    .filter(|&k| k != track_idx && k < key_taps.len())
                    .map(|k| key_taps[k]);
                if let Some(plugin) = &mut track.plugin {
                    if plugin.has_sidechain() {
                        // Utan nyckelspår matas **nollor**, inte gammalt ljud: en borttagen
                        // nyckelkälla ska tystna, inte frysa sitt sista sampel.
                        let (kl, kr) = side_key.unwrap_or((0.0, 0.0));
                        plugin.feed_sidechain(kl, kr);
                    }
                    let (pl, pr) = plugin.process_sample(tl, tr);
                    tl = pl;
                    tr = pr;
                }
                let track_latency = crate::audio::plugin_host_live::compensated_latency(
                    plugin_latency,
                    if track.pitch_active {
                        track.pitch_shifter.latency_frames()
                    } else {
                        0
                    },
                    track.manual_latency_frames,
                );
                track.pdc.set_delay(max_plugin_latency.saturating_sub(track_latency));
                let (mut tl, mut tr) = track.pdc.process(tl, tr);

                // **Pluginens egna utbussar ut ur kedjan** (Fas 8.6): en buss går *förbi*
                // spårets egen kedja — den är pluginens egen utgång — och läggs i målspårets
                // ingång. Läsningen sker **efter** PDC-steget ovan och det är inte en detalj:
                // bussen är samma plugins utgång som huvudutgången, så den ska bära samma
                // kompensation. Låg den före PDC kom bussen fram *tidigare* än pluginens eget
                // ljud — mätt 2026-09-15, och det var provet som sa det.
                //
                // Före spårets **duckare** (som ligger nedanför): den är världens effekt på
                // spårets egen mix, medan bussen är pluginens utgång — den ska till ett annat
                // spår, inte duckas av källspårets sidokedja.
                //
                // Varje kopplad port läses **varje** sample, också när målet är tystat: kön
                // ligger i samma takt som ljudet, så en utebliven läsning vore att bussen
                // sackade efter en sample för varje gång den inte behövdes.
                if !track.extra_out_targets.is_empty() {
                    if let Some(plugin) = &mut track.plugin {
                        for (port, &target) in track.extra_out_targets.iter().enumerate() {
                            let (bl, br) = plugin.extra_output_sample(port);
                            if let Some(to) = target {
                                if let Some(slot) = plugin_bus_in.get_mut(to) {
                                    slot[0] += bl;
                                    slot[1] += br;
                                }
                            }
                        }
                    }
                }


                // Sidokedjan duckar spårets **eget** ljud, efter dess kedja: det är
                // där en kompressor med extern nyckel hade suttit, och det är det
                // som hörs. Nyckeln är key-spårets senaste utgång (se `Ducker` för
                // varför den är som mest ett sample gammal).
                if let Some(key) = track
                    .sidechain_from
                    .filter(|&k| k != track_idx && k < key_taps.len())
                {
                    let (kl, kr) = key_taps[key];
                    let gain = track.sidechain_ducker.process(
                        kl,
                        kr,
                        track.sidechain_threshold_db,
                        track.sidechain_amount_db,
                    );
                    tl *= gain;
                    tr *= gain;
                }
                track.last_out_l = tl;
                track.last_out_r = tr;
                // Spårets utgång för det här samplet: det är den en spår-send läser.
                if track_idx < out_l.len() {
                    out_l[track_idx] = tl;
                    out_r[track_idx] = tr;
                }

                stem_mix_l += tl * group_gain;
                stem_mix_r += tr * group_gain;

                // Sends (Fas 8.13): en del av spårets signal går till en ANNAN buss
                // också. Post-fader — signalen är densamma som går till spårets egen
                // buss, alltså efter volym, EQ, kompressor och sidokedja.
                //
                // Målets bussnivå läggs på här i stället för i en egen summering:
                // bussarna har ingen egen bearbetning (bara nivå, mute och solo), så
                // summan är linjär och det är samma sak — men det syns i koden att
                // det är ett antagande, och det är därför det står här.
                for send in &track.sends {
                    // Bara bussmål hör hit. Ett spårmål går in i mottagarens kedja i
                    // stället (spår-senden, Fas 8.3) — det är hela skillnaden.
                    let SendTarget::Bus { target_bus } = send.target else {
                        continue;
                    };
                    let target = target_bus.min(NUM_BUSES - 1);
                    // Målets eget gruppläge gäller målet: en tystad buss tar inte
                    // emot, och när något är soloat hörs bara det soloades väg. Att
                    // spåret självt är hörbart räcker alltså inte — samma regel som
                    // för spårets egen buss, räknad för MÅLET. (Mätt: utan den här
                    // raden lade en send till en tystad buss till signal.)
                    let target_audible = if has_solo {
                        bus_solo[target]
                    } else {
                        !bus_muted[target]
                    };
                    if !target_audible {
                        continue;
                    }
                    let g = send.level * bus_volume[target];
                    stem_mix_l += tl * g;
                    stem_mix_r += tr * g;
                }
            }

            self.track_out_l = out_l;
            self.track_out_r = out_r;
            self.stem_incoming = incoming;
            self.plugin_bus_in = plugin_bus_in;
        }

        // Ljudklockan går så länge transporten rullar (Fas 8.13b) — också i ett projekt
        // utan stämmor. UI:ts spelhuvud läser den här siffran i stället för att räkna
        // egna steg, och den får inte stanna bara för att inga filer är importerade.
        //
        // **Mätt 2026-09-13:** spelhuvudet låg 0,66 s efter ljudet 12,64 s in i Rock
        // and Hard Place, för att UI:t hade en egen klocka. Det här är den klockan örat
        // hör: en bildruta i taget, i den takt motorn spelar.
        if self.song_playing {
            self.song_time_samples += 1;
        }

        // 8. Isolated Audition Audio Playback (Vocal Studio & Sample Previews)
        if let Some(ref mut aud) = self.audition {
            if aud.is_playing && !aud.left.is_empty() {
                let len = aud.left.len();
                let sr_ratio = aud.sample_rate / self.sample_rate;
                let pr = aud.pitch_ratio.max(0.05);
                let tsr = aud.time_stretch_ratio.max(0.05);
                let use_wsola = !aud.is_reverse && (tsr - 1.0).abs() > 1e-3;

                if use_wsola {
                    // Pitch-preserving time-stretch: WSOLA at ratio tsr*pr, then
                    // resample by sr_ratio*pr to convert rate and apply pitch.
                    aud.wsola.set_ratio(tsr * sr_ratio * pr);
                    let step = sr_ratio * pr;
                    let (l, r) = aud
                        .wsola
                        .next_resampled(&aud.left, &aud.right, aud.loop_playback, step);
                    stem_mix_l += l * aud.volume;
                    stem_mix_r += r * aud.volume;
                    if aud.wsola.is_finished() && aud.wsola.available() <= 0.0 {
                        aud.is_playing = false;
                    }
                } else {
                    let idx_f = aud.play_pos_samples;
                    let idx0 = idx_f.floor() as usize;
                    let idx1 = (idx0 + 1).min(len.saturating_sub(1));
                    let frac = idx_f - idx0 as f32;

                    if idx0 < len {
                        let l0 = aud.left[idx0];
                        let l1 = aud.left[idx1];
                        let r0 = if idx0 < aud.right.len() { aud.right[idx0] } else { l0 };
                        let r1 = if idx1 < aud.right.len() { aud.right[idx1] } else { l1 };

                        let raw_l = l0 + (l1 - l0) * frac;
                        let raw_r = r0 + (r1 - r0) * frac;

                        stem_mix_l += raw_l * aud.volume;
                        stem_mix_r += raw_r * aud.volume;

                        let speed = sr_ratio * pr * tsr;
                        if aud.is_reverse {
                            aud.play_pos_samples -= speed;
                            if aud.play_pos_samples < 0.0 {
                                if aud.loop_playback {
                                    aud.play_pos_samples = (len as f32 - 1.0).max(0.0);
                                } else {
                                    aud.is_playing = false;
                                }
                            }
                        } else {
                            aud.play_pos_samples += speed;
                            if aud.play_pos_samples >= len as f32 {
                                if aud.loop_playback {
                                    aud.play_pos_samples = 0.0;
                                } else {
                                    aud.is_playing = false;
                                }
                            }
                        }
                    } else {
                        aud.is_playing = false;
                    }
                }
            }
        }

        // 8b. Modular Patcher output (real DSP graph)
        if self.patcher_enabled
            && let Some(patch) = &mut self.patcher
        {
            let (pl, pr) = patch.process();
            stem_mix_l += pl;
            stem_mix_r += pr;
        }

        // 8c. Zero-latency microphone direct monitoring (post auto-tune).
        //     Drains the ring once per buffer and plays the mono signal on both
        //     channels, routed through the master bus so the limiter catches
        //     peaks. Nivån kommer från MONITOR-ratten (`SetMonitorLevel`) — utan
        //     egen nivå vore mastervolymen enda sättet att bryta en rundgång.
        if let Some(ring) = self.monitor_ring.as_ref() {
            if self.monitor_idx >= self.monitor_buf.len() {
                self.monitor_buf.clear();
                self.monitor_idx = 0;
                if let Ok(mut r) = ring.lock() {
                    if !r.is_empty() {
                        self.monitor_buf.append(&mut r);
                    }
                }
            }
            if self.monitor_idx < self.monitor_buf.len() {
                let m = self.monitor_buf[self.monitor_idx] * self.monitor_level;
                self.monitor_idx += 1;
                stem_mix_l += m;
                stem_mix_r += m;
            }
        }

        // 9. Master bus FX chain (EQ, compressor, de-esser, doubler, gate, filter, limiter)
        let (fx_l, fx_r) = self.master_fx.process(del_l + stem_mix_l, del_r + stem_mix_r);

        let out_l = (fx_l * self.master_volume).tanh();
        let out_r = (fx_r * self.master_volume).tanh();

        // DJ performance FX on the final master bus.
        let (out_l, out_r) = if self.remix_fx.is_active() {
            self.remix_fx.process(out_l, out_r)
        } else {
            (out_l, out_r)
        };
        let (out_l, out_r) = self.tape_stop.process(out_l, out_r);

        self.dbg_frames += 1;
        if self.dbg_frames % 48000 == 0 {
            let active_regions = self
                .stem_tracks
                .iter()
                .map(|t| t.regions.iter().filter(|r| !r.muted).count())
                .sum::<usize>();
            dbg_log(
                "HB",
                &format!(
                    "frames={} out=({:.4},{:.4}) song_playing={} t={:.2}s stems={} regions={} voices={}",
                    self.dbg_frames,
                    out_l,
                    out_r,
                    self.song_playing,
                    self.song_time_samples as f32 / self.sample_rate,
                    self.stem_tracks.len(),
                    active_regions,
                    self.voices.iter().filter(|v| v.is_active()).count(),
                ),
            );
        }

        (out_l, out_r)
    }
}
