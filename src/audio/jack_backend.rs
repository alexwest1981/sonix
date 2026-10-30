//! JACK-backend — byggs bara med `--features jack`.
//!
//! JACK äger samplingsfrekvensen och ger en ny blocklängd varje cykel. Det enda som
//! skiljer från cpal-vägen är vem som anropar `engine::Renderer` och var de två talen
//! hamnar: två portar i stället för interleaved kanaler. DSP:n är densamma, med flit —
//! två kopior av mixern hade glidit isär.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use rtrb::{Consumer, Producer};

use super::command::AudioCommand;
use super::engine::{Renderer, report_callback_panic};

/// En cykels utljud, vänster och höger, exakt så lång som JACK säger.
pub type Render = Box<dyn FnMut(&mut [f32], &mut [f32]) + Send>;

/// Den levande klienten. Att släppa den avaktiverar klienten och stoppar
/// process-callbacken — samma sak som att släppa cpal-strömmen.
pub struct JackHandle {
    _client: jack::AsyncClient<(), Process>,
    pub sample_rate: u32,
    /// Cykelstorleken servern startade med. JACK får ändra den; längden per cykel
    /// kommer alltid från den buffert callbacken får, inte härifrån.
    pub buffer_frames: u32,
}

struct Process {
    out_l: jack::Port<jack::AudioOut>,
    out_r: jack::Port<jack::AudioOut>,
    render: Render,
    cycles: Arc<AtomicU32>,
    error_count: Arc<AtomicU32>,
}

impl jack::ProcessHandler for Process {
    fn process(&mut self, _client: &jack::Client, ps: &jack::ProcessScope) -> jack::Control {
        super::engine::flush_denormals();
        self.cycles.fetch_add(1, Ordering::Relaxed);
        // En panik får inte gå genom C-stacken: den avbryter processen i stället för
        // att tystna i en callback, och det är värre än en tyst buffert.
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let out_l = self.out_l.as_mut_slice(ps);
            let out_r = self.out_r.as_mut_slice(ps);
            (self.render)(out_l, out_r);
        }));
        if let Err(panic) = res {
            report_callback_panic(&self.error_count, &panic);
        }
        jack::Control::Continue
    }
}

/// Öppnar en `sonix`-klient med `out_l`/`out_r` och returnerar den tillsammans med
/// ringändarna motorn behåller. `Err` betyder "ingen JACK här" — anroparen faller
/// tillbaka på cpal och skriver varför.
#[allow(clippy::too_many_arguments)]
pub fn open(
    ring_buffer_size: usize,
    peak_level: Arc<AtomicU32>,
    peak_l: Arc<AtomicU32>,
    peak_r: Arc<AtomicU32>,
    song_pos_secs: Arc<AtomicU32>,
    stretched_tracks: Arc<AtomicU32>,
    audition_active: Arc<AtomicBool>,
    output_silent: Arc<AtomicBool>,
    master_gr_db: Arc<AtomicU32>,
    heartbeat: Arc<AtomicU32>,
    error_count: Arc<AtomicU32>,
) -> Result<(JackHandle, Producer<AudioCommand>, Consumer<f32>), String> {
    // NO_START_SERVER: appen får aldrig starta en JACK-server åt användaren. Utan
    // den flaggan startar libjack en demon som ingen bad om.
    let (client, _status) = jack::Client::new("sonix", jack::ClientOptions::NO_START_SERVER)
        .map_err(|e| format!("no JACK server answering: {e:?}"))?;
    let sample_rate = client.sample_rate() as u32;
    let buffer_frames = client.buffer_size();

    let (renderer, command_tx, scope_rx) = Renderer::new(
        sample_rate as f32,
        ring_buffer_size,
        peak_level,
        peak_l,
        peak_r,
        song_pos_secs,
        stretched_tracks,
        audition_active,
        output_silent,
        master_gr_db,
        heartbeat,
    );

    let out_l = client
        .register_port("out_l", jack::AudioOut)
        .map_err(|e| format!("could not register out_l: {e}"))?;
    let out_r = client
        .register_port("out_r", jack::AudioOut)
        .map_err(|e| format!("could not register out_r: {e}"))?;

    let mut renderer = renderer;
    let process = Process {
        out_l,
        out_r,
        render: Box::new(move |l: &mut [f32], r: &mut [f32]| {
            let frames = l.len().min(r.len());
            renderer.render(frames, |i, left, right| {
                l[i] = left;
                r[i] = right;
            });
        }),
        cycles: Arc::new(AtomicU32::new(0)),
        error_count,
    };

    let client = client
        .activate_async((), process)
        .map_err(|e| format!("could not activate the client: {e:?}"))?;
    Ok((
        JackHandle {
            _client: client,
            sample_rate,
            buffer_frames,
        },
        command_tx,
        scope_rx,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// Servernamnen är processglobala (libjack läser `JACK_DEFAULT_SERVER`), så de
    /// två proven får inte mutera miljön samtidigt.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Frågar servern en gång: svarar den finns en server, annars `false`.
    fn server_answers() -> bool {
        jack::Client::new("sonix-test-probe", jack::ClientOptions::NO_START_SERVER).is_ok()
    }

    /// En dummy-server för provet, dödad när provet är klart.
    struct DummyServer(Option<std::process::Child>);

    impl Drop for DummyServer {
        fn drop(&mut self) {
            if let Some(mut child) = self.0.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    /// En server att prata med: en som redan kör, annars `jackd -d dummy`. `None`
    /// betyder att provet inte kan köras på den här maskinen — och då står skälet.
    fn ensure_server() -> Option<DummyServer> {
        if server_answers() {
            println!("a JACK server is already running; using it");
            return Some(DummyServer(None));
        }
        let child = std::process::Command::new("jackd")
            .args(["-d", "dummy", "-r", "48000", "-p", "256"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        let child = match child {
            Ok(child) => child,
            Err(_) => {
                println!("jackd is not installed - the JACK backend cannot be tested here");
                return None;
            }
        };
        let server = DummyServer(Some(child));
        // Servern lyssnar först en bit efter start: fråga tills den svarar.
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if server_answers() {
                return Some(server);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        println!("jackd was started but never answered on its socket");
        None
    }

    /// **Backendens hela väg: klienten, portnamnen, JACK:s klocka och en
    /// cykelbuffert som inte är känd i förväg.** Provet öppnar först den riktiga
    /// backend-vägen (`open`), ser att `sonix:out_l`/`sonix:out_r` finns på servern,
    /// och skriver sedan 1 s 440 Hz i portarnas egna buffertar — samma minne servern
    /// läser — och mäter att det kom ut ljud med rätt antal frames, oavsett
    /// cykelstorlek.
    #[test]
    fn a_jack_client_renders_one_second_of_a_sine_into_non_silent_ports() {
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some(_server) = ensure_server() else {
            return;
        };
        let zero = || Arc::new(AtomicU32::new(0));
        let (handle, _command_tx, _scope_rx) = open(
            1024,
            zero(),
            zero(),
            zero(),
            zero(),
            zero(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            zero(),
            zero(),
            zero(),
        )
        .expect("the backend must open with a server running");
        println!(
            "backend open: {} Hz, {}-frame cycles",
            handle.sample_rate, handle.buffer_frames
        );
        let (probe, _status) =
            jack::Client::new("sonix-port-probe", jack::ClientOptions::NO_START_SERVER)
                .expect("probe client");
        let found = probe.ports(Some("sonix:.*"), None, jack::PortFlags::IS_OUTPUT);
        assert!(found.iter().any(|p| p.ends_with("out_l")), "{found:?}");
        assert!(found.iter().any(|p| p.ends_with("out_r")), "{found:?}");
        drop(probe);
        drop(handle);

        let (client, _status) =
            jack::Client::new("sonix-sine", jack::ClientOptions::NO_START_SERVER).expect("client");
        let sample_rate = client.sample_rate() as f32;
        let out_l = client
            .register_port("out_l", jack::AudioOut)
            .expect("out_l");
        let out_r = client
            .register_port("out_r", jack::AudioOut)
            .expect("out_r");

        // (frames, sum of squares): en Mutex är inte RT-säker, men det här är ett prov.
        let measured = Arc::new(Mutex::new((0u64, 0f64)));
        let cycles = Arc::new(AtomicU32::new(0));
        let sink = Arc::clone(&measured);
        let mut phase = 0f32;
        let step = 440.0 * std::f32::consts::TAU / sample_rate;
        let process = Process {
            out_l,
            out_r,
            render: Box::new(move |l: &mut [f32], r: &mut [f32]| {
                let mut acc = sink.lock().unwrap();
                for i in 0..l.len().min(r.len()) {
                    let s = 0.5 * phase.sin();
                    phase = (phase + step) % std::f32::consts::TAU;
                    l[i] = s;
                    r[i] = s;
                    acc.1 += f64::from(s) * f64::from(s);
                }
                acc.0 += l.len() as u64;
            }),
            cycles: Arc::clone(&cycles),
            error_count: Arc::new(AtomicU32::new(0)),
        };
        let client = client.activate_async((), process).expect("activate");

        // En sekund, mätt i frames: servern går i realtid men provet ska inte hänga
        // om den inte gör det.
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && measured.lock().unwrap().0 < sample_rate as u64 {
            std::thread::sleep(Duration::from_millis(20));
        }
        client.deactivate().expect("deactivate");

        let (frames, sum_squares) = *measured.lock().unwrap();
        let cycles = cycles.load(Ordering::Relaxed);
        let rms = (sum_squares / frames.max(1) as f64).sqrt();
        println!("jack: {frames} frames in {cycles} cycles at {sample_rate} Hz, RMS {rms:.3}");

        assert!(
            frames >= sample_rate as u64,
            "expected at least one second of audio, got {frames} frames"
        );
        assert!(cycles > 0, "the process callback never ran");
        // Sinus med amplitud 0,5 har RMS 0,354.
        assert!(rms > 0.2, "the ports carried {rms:.3} RMS, not a sine");
    }

    /// **Motorn väljer JACK när en server svarar** — samma `new_with` som appen
    /// använder, inte bara `open` underifrån. Utan server går provet inte att köra,
    /// och då står det.
    #[test]
    fn the_engine_picks_the_jack_backend_when_a_server_answers() {
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if !server_answers() {
            println!("no JACK server here - the engine cannot be checked against one");
            return;
        }
        let previous = std::env::var("SONIX_AUDIO").ok();
        unsafe { std::env::set_var("SONIX_AUDIO", "jack") };
        let opened = std::thread::Builder::new()
            .stack_size(32 << 20)
            .spawn(|| {
                super::super::AudioEngine::new_with(1024, None, None)
                    .map(|engine| {
                        (
                            engine.host_name.clone(),
                            engine.channels,
                            engine.sample_rate,
                        )
                    })
                    .map_err(|e| e.to_string())
            })
            .expect("thread")
            .join()
            .expect("no panic in the audio open");
        unsafe {
            match previous {
                Some(v) => std::env::set_var("SONIX_AUDIO", v),
                None => std::env::remove_var("SONIX_AUDIO"),
            }
        }
        let (host, channels, sample_rate) = opened.expect("the JACK backend must open");
        assert_eq!(
            host, "JACK",
            "SONIX_AUDIO=jack must pick JACK when a server answers"
        );
        assert_eq!(channels, 2, "the backend registers one stereo pair");
        println!("engine on JACK: {sample_rate} Hz, {channels} channels");
    }

    /// **`SONIX_AUDIO=jack` utan server ska falla tillbaka på cpal — och säga varför
    /// i en rad.** Provet är tidsbundet: en fallback som hänger är en app som inte
    /// startar.
    ///
    /// En server som redan kör måste pekas bort: `JACK_DEFAULT_SERVER` gäller för
    /// jackd, `PIPEWIRE_REMOTE` för PipeWires JACK-skin. Svarar servern ändå går
    /// fallbacken inte att pröva här, och då står det — provet tiger inte.
    #[test]
    fn asking_for_jack_without_a_server_falls_back_to_cpal_quickly() {
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var("SONIX_AUDIO").ok();
        let previous_server = std::env::var("JACK_DEFAULT_SERVER").ok();
        let previous_remote = std::env::var("PIPEWIRE_REMOTE").ok();
        unsafe {
            std::env::set_var("SONIX_AUDIO", "jack");
            std::env::set_var("JACK_DEFAULT_SERVER", "sonix-no-such-server");
            std::env::set_var("PIPEWIRE_REMOTE", "sonix-no-such-remote");
        }
        let restore = || unsafe {
            match previous.clone() {
                Some(v) => std::env::set_var("SONIX_AUDIO", v),
                None => std::env::remove_var("SONIX_AUDIO"),
            }
            match previous_server.clone() {
                Some(v) => std::env::set_var("JACK_DEFAULT_SERVER", v),
                None => std::env::remove_var("JACK_DEFAULT_SERVER"),
            }
            match previous_remote.clone() {
                Some(v) => std::env::set_var("PIPEWIRE_REMOTE", v),
                None => std::env::remove_var("PIPEWIRE_REMOTE"),
            }
        };

        if server_answers() {
            restore();
            println!(
                "the JACK server ignored JACK_DEFAULT_SERVER/PIPEWIRE_REMOTE - the fallback cannot be exercised here"
            );
            return;
        }

        let started = Instant::now();
        // Appen öppnar ljudet på huvudtråden; libtest-tråden har mindre stack än
        // ljudöppningen kräver, så provet ger den samma utrymme. Bara namnen lämnar
        // tråden — en cpal-ström är inte `Send` och ska stängas där den öppnades.
        let opened = std::thread::Builder::new()
            .stack_size(32 << 20)
            .spawn(|| {
                super::super::AudioEngine::new_with(1024, None, None)
                    .map(|engine| (engine.host_name.clone(), engine.device_name.clone()))
                    .map_err(|e| e.to_string())
            })
            .expect("thread")
            .join()
            .expect("no panic in the audio open");
        let elapsed = started.elapsed();
        restore();

        assert!(
            elapsed < Duration::from_secs(3),
            "opening without a JACK server took {elapsed:?}"
        );
        match opened {
            Ok((host, device)) => {
                assert_ne!(host, "JACK", "JACK cannot open without a server");
                println!("JACK unavailable; cpal opened {device} after {elapsed:?}");
            }
            // Att cpal-försöket nådde ALSA är beviset för fallbacken: JACK:s eget fel
            // ser annorlunda ut. Här svarar `PIPEWIRE_REMOTE` också för ALSA-enheten
            // (PipeWires ALSA-plugin), så enheten kan inte öppnas i samma andetag.
            Err(e) => {
                assert!(
                    e.contains("ALSA"),
                    "the fallback never reached cpal, it stopped at JACK: {e}"
                );
                println!(
                    "JACK unavailable and the poisoned PipeWire remote closed the ALSA default too; cpal was tried and said: {e} (after {elapsed:?})"
                );
            }
        }
    }
}
