//! BCF2000, FaderPort (classic) and BabyHUI as **pure** byte-stream codecs.
//!
//! All three speak ordinary MIDI: CCs and notes for the controls, sysex for the
//! BCF2000's LCD. [`SurfaceParser::feed`] is handed whatever bytes the port produced and
//! returns actions, and [`encode_feedback`] turns mixer state into the bytes the device
//! expects — no engine, no channel, no ALSA — so a whole session can be replayed from a
//! byte stream in a test, which is the only way to check it here (no hardware).
//!
//! MCU is *not* parsed here. It is read from ALSA sequencer events in `hardware_control`
//! (`mcu_cc_to_event` / `mcu_note_to_event`) and keeps that path unchanged; a second copy
//! of the same table in byte form would only let the two drift apart. Its feedback is
//! written here in Mackie's sysex/pitch-bend form, and the parser deliberately decodes
//! none of it.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use super::hardware_control::ControlEvent;

/// Which surface is connected. `Mcu` means "the existing ALSA event path, untouched".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SurfaceKind {
    #[default]
    Mcu,
    Bcf2000,
    FaderPort,
    BabyHui,
}

impl SurfaceKind {
    pub fn label(self) -> &'static str {
        match self {
            SurfaceKind::Mcu => "Mackie MCU",
            SurfaceKind::Bcf2000 => "Behringer BCF2000",
            SurfaceKind::FaderPort => "PreSonus FaderPort",
            SurfaceKind::BabyHui => "Mackie BabyHUI",
        }
    }

    /// Substring that identifies the device in the ALSA port name (`aconnect -l`).
    fn port_needle(self) -> &'static str {
        match self {
            SurfaceKind::Mcu => "mcu",
            SurfaceKind::Bcf2000 => "bcf",
            SurfaceKind::FaderPort => "faderport",
            SurfaceKind::BabyHui => "hui",
        }
    }
}

/// The MIDI channel every surface here listens on (channel 1 = status `0xB0`/`0x90`).
const CH: u8 = 0;
const CC: u8 = 0xB0 | CH;

/// One FaderPort (classic) transport button. The order is the CC order, 16..=23.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportCommand {
    Stop,
    Play,
    Record,
    Rewind,
    FastForward,
    Loop,
    Punch,
    User,
}

const TRANSPORT: [TransportCommand; 8] = [
    TransportCommand::Stop,
    TransportCommand::Play,
    TransportCommand::Record,
    TransportCommand::Rewind,
    TransportCommand::FastForward,
    TransportCommand::Loop,
    TransportCommand::Punch,
    TransportCommand::User,
];

impl TransportCommand {
    /// The CC number the button sends (and, mirrored, its LED).
    pub fn cc(self) -> u8 {
        16 + self as u8
    }
}

/// What a surface did, in surface terms. Faders carry the raw 7-bit value so the
/// parser stays a pure decoder; scaling to a track level happens in `to_control_event`.
#[derive(Debug, Clone, PartialEq)]
pub enum SurfaceAction {
    FaderMoved { index: usize, value: u8 },
    ButtonPressed { id: u8 },
    TransportCommand(TransportCommand),
    /// BCF2000 two-line LC display. Newlines survive, so both lines arrive together.
    DisplayText { text: String },
}

impl SurfaceAction {
    /// Hands the action to the app on the same `ControlEvent` path MCU uses.
    /// Actions the app has no control for (display, fader-bank buttons, ...) return `None`.
    pub fn to_control_event(&self) -> Option<ControlEvent> {
        match self {
            SurfaceAction::FaderMoved { index, value } => Some(ControlEvent::TrackVolume {
                track: *index,
                value: *value as f32 / 127.0 * 1.25,
            }),
            SurfaceAction::ButtonPressed { id } => {
                // MCU mutes are notes 16..=23; the BCF2000 sends the same numbers.
                (16..=23).contains(id).then(|| ControlEvent::TrackMute {
                    track: (*id - 16) as usize,
                    mute: true,
                })
            }
            SurfaceAction::TransportCommand(cmd) => match cmd {
                TransportCommand::Play => Some(ControlEvent::Play(true)),
                TransportCommand::Stop => Some(ControlEvent::Stop),
                TransportCommand::Record => Some(ControlEvent::Record(true)),
                // Rewind/FastForward/Loop/Punch/User have no ControlEvent yet.
                _ => None,
            },
            SurfaceAction::DisplayText { .. } => None,
        }
    }
}

/// Garbage guard: a sysex that never terminates must not grow without bound.
const MAX_SYSEX: usize = 4096;

/// Running byte-stream decoder for one surface.
#[derive(Debug, Default)]
pub struct SurfaceParser {
    kind: SurfaceKind,
    in_sysex: bool,
    sysex: Vec<u8>,
    msg: Vec<u8>,
}

impl SurfaceParser {
    pub fn new(kind: SurfaceKind) -> Self {
        Self {
            kind,
            ..Self::default()
        }
    }

    /// Feeds raw port bytes. A message split across calls is held until its last byte;
    /// fragments never produce a half action and nothing panics on foreign bytes.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<SurfaceAction> {
        let mut out = Vec::new();
        for &b in bytes {
            // Real-time bytes may appear anywhere, even inside a sysex. F7 is not one
            // of them: it ends the sysex below, so it must not be skipped here.
            if b >= 0xF8 {
                continue;
            }
            if self.in_sysex {
                self.sysex.push(b);
                if b == 0xF7 {
                    self.in_sysex = false;
                    if let Some(a) = self.sysex_action() {
                        out.push(a);
                    }
                    self.sysex.clear();
                } else if self.sysex.len() > MAX_SYSEX {
                    self.in_sysex = false;
                    self.sysex.clear();
                }
                continue;
            }
            match b {
                0xF0 => {
                    self.in_sysex = true;
                    self.sysex.clear();
                    self.sysex.push(b);
                    self.msg.clear();
                    continue;
                }
                // Other system messages have varying length; dropping them is safer
                // than guessing how many data bytes follow.
                0xF1..=0xF7 => {
                    self.msg.clear();
                    continue;
                }
                _ => {}
            }
            if b & 0x80 != 0 {
                self.msg.clear();
                self.msg.push(b);
                continue;
            }
            if self.msg.is_empty() {
                continue; // a data byte with no status byte: not ours
            }
            self.msg.push(b);
            if self.msg.len() == 3 {
                if let Some(a) = self.channel_action(self.msg[0], self.msg[1], self.msg[2]) {
                    out.push(a);
                }
                self.msg.clear();
            }
        }
        out
    }

    fn channel_action(&self, status: u8, d1: u8, d2: u8) -> Option<SurfaceAction> {
        let (d1, d2) = (d1 & 0x7F, d2 & 0x7F);
        match self.kind {
            // MCU keeps its ALSA event path; see the module comment.
            SurfaceKind::Mcu => None,
            SurfaceKind::Bcf2000 => match status & 0xF0 {
                0xB0 if d1 < 8 => Some(SurfaceAction::FaderMoved {
                    index: d1 as usize,
                    value: d2,
                }),
                // Note-off (and note-on with velocity 0) is the release half of the press.
                0x90 if d2 > 0 => Some(SurfaceAction::ButtonPressed { id: d1 }),
                _ => None,
            },
            SurfaceKind::FaderPort => match status {
                0xB0 if d1 == 0 => Some(SurfaceAction::FaderMoved {
                    index: 0,
                    value: d2,
                }),
                0xB0 if (16..=23).contains(&d1) => Some(SurfaceAction::TransportCommand(
                    TRANSPORT[(d1 - 16) as usize],
                )),
                // shortcut: real units report the buttons as poly aftertouch (0xA0 with
                // the button number as the note), not as CCs; add that arm when one is
                // on the bench.
                _ => None,
            },
            // BabyHUI: faders on CC 0..7, the eight buttons on CC 32..39. A CC value
            // above zero is the press half; zero is the release, same as a note-off.
            SurfaceKind::BabyHui => match status & 0xF0 {
                0xB0 if d1 < 8 => Some(SurfaceAction::FaderMoved {
                    index: d1 as usize,
                    value: d2,
                }),
                0xB0 if (32..=39).contains(&d1) && d2 > 0 => {
                    Some(SurfaceAction::ButtonPressed { id: d1 })
                }
                _ => None,
            },
        }
    }

    /// `F0 00 20 32 <device> <text...> F7` — Behringer's LC display dump.
    fn sysex_action(&self) -> Option<SurfaceAction> {
        if self.kind != SurfaceKind::Bcf2000 || !self.sysex.starts_with(&[0xF0, 0x00, 0x20, 0x32])
        {
            return None;
        }
        if self.sysex.len() < 6 {
            return None;
        }
        // Byte 4 is the device id; the payload runs up to the trailing F7.
        let text: String = self.sysex[5..self.sysex.len() - 1]
            .iter()
            .map(|&b| match b {
                0x0A => '\n',
                0x20..=0x7E => b as char,
                _ => ' ',
            })
            .collect();
        (!text.trim().is_empty()).then_some(SurfaceAction::DisplayText { text })
    }
}

/// FaderPort (classic) LED feedback: the button's own CC echoed back, 0x7F = lit.
pub fn faderport_led(command: TransportCommand, on: bool) -> [u8; 3] {
    [CC, command.cc(), if on { 0x7F } else { 0x00 }]
}

/// FaderPort (classic) 7-segment position display: one digit per CC from `base`,
/// value = segment mask (a..g in bits 0..6) with bit 7 as the decimal point.
/// shortcut: `base` is assumed to be 0x30; check it against a unit before shipping.
pub fn faderport_segments(digits: &[u8], base: u8, decimal: bool) -> Vec<u8> {
    const SEGMENTS: [u8; 10] = [
        0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F,
    ];
    let mut out = Vec::with_capacity(digits.len() * 3);
    let last = digits.len().saturating_sub(1);
    for (i, &d) in digits.iter().enumerate() {
        let mut mask = SEGMENTS[(d as usize).min(9)];
        if decimal && i == last && last > 0 {
            mask |= 0x80;
        }
        out.extend_from_slice(&[CC, base.wrapping_add(i as u8), mask]);
    }
    out
}

/// Mixer state as the surfaces display it, in the raw 7-bit numbers their parsers hand
/// back — so encoding and decoding a value is the same byte.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SurfaceFeedback {
    /// Per-channel fader position, 0..=127 (channel 0 is the first strip).
    pub volume: [u8; 8],
    pub mute: [bool; 8],
    pub solo: [bool; 8],
    pub playing: bool,
    pub recording: bool,
    /// Two display lines, e.g. "PLAY REC 120.0 BPM" / "Kick  100".
    pub line1: String,
    pub line2: String,
    /// FaderPort position display afterwards: one digit per segment, empty = leave it.
    pub position: Vec<u8>,
    pub position_decimal: bool,
}

/// The BCF2000's LCD line separator: the same 0x0A `sysex_action` turns back into `\n`.
const LCD_LINE_SEP: u8 = 0x0A;
/// Characters that fit one LCD line; a longer line is cut rather than wrapped.
const LCD_LINE: usize = 20;
/// BCF2000 LED ring for fader `i`. shortcut: the MCU-style ring bank a BCF listens to in
/// its Mackie preset; move it to 16..23 if a unit on the bench disagrees.
const BCF_RING_BASE: u8 = 48;
/// BabyHUI's eight buttons, also their LEDs (a CC echo, like the FaderPort ones).
const BABY_HUI_BUTTON_BASE: u8 = 32;

/// The bytes a surface needs to show `feedback`: display, motor faders and LEDs.
///
/// A disconnected surface never sees these, so an empty `SurfaceFeedback` (all zeros,
/// empty lines) is the "nothing to show" form — still a well-formed stream, never empty
/// because the device needs at least the clearing message.
pub fn encode_feedback(kind: SurfaceKind, feedback: &SurfaceFeedback) -> Vec<u8> {
    let f = feedback;
    let mut out = Vec::new();
    match kind {
        // Mackie's own form: display sysex, then 14-bit pitch bend per motor fader.
        SurfaceKind::Mcu => {
            out.extend_from_slice(&[0xF0, 0x00, 0x00, 0x66, 0x14]);
            push_line(&mut out, &f.line1);
            out.push(LCD_LINE_SEP);
            push_line(&mut out, &f.line2);
            out.push(0xF7);
            for (i, &v) in f.volume.iter().enumerate() {
                // u32 first: 127 * 16383 does not fit in u16.
                let bend = (v as u32 * 16383 / 127) as u16;
                // MCU motor fader i answers on pitch bend channel i.
                out.extend_from_slice(&[0xE0 | i as u8, (bend & 0x7F) as u8, (bend >> 7) as u8]);
            }
        }
        SurfaceKind::Bcf2000 => {
            // Behringer sysex display, then faders 0..7 and their LED rings.
            out.extend_from_slice(&[0xF0, 0x00, 0x20, 0x32, 0x00]);
            push_line(&mut out, &f.line1);
            out.push(LCD_LINE_SEP);
            push_line(&mut out, &f.line2);
            out.push(0xF7);
            for (i, &v) in f.volume.iter().enumerate() {
                out.extend_from_slice(&[CC, i as u8, v]);
                let ring = if f.mute[i] { 0 } else { v };
                out.extend_from_slice(&[CC, BCF_RING_BASE + i as u8, ring]);
            }
        }
        SurfaceKind::FaderPort => {
            // One fader, three transport LEDs, then the 7-segment position.
            out.extend_from_slice(&[CC, 0, f.volume[0]]);
            out.extend_from_slice(&faderport_led(TransportCommand::Stop, !f.playing));
            out.extend_from_slice(&faderport_led(TransportCommand::Play, f.playing));
            out.extend_from_slice(&faderport_led(TransportCommand::Record, f.recording));
            out.extend_from_slice(&faderport_segments(&f.position, 0x30, f.position_decimal));
        }
        SurfaceKind::BabyHui => {
            // faders 0..7, then the V-Pot LEDs on the button CCs.
            for (i, &v) in f.volume.iter().enumerate() {
                out.extend_from_slice(&[CC, i as u8, v]);
            }
            for i in 0..8 {
                let lit = f.mute[i] || f.solo[i];
                out.extend_from_slice(&[CC, BABY_HUI_BUTTON_BASE + i as u8, if lit { 0x7F } else { 0x00 }]);
            }
        }
    }
    out
}

/// One printable LCD line, cut to the display width.
fn push_line(out: &mut Vec<u8>, line: &str) {
    out.extend(line.bytes().filter(|b| (0x20..=0x7E).contains(b)).take(LCD_LINE));
}

/// Opens the matching MIDI port and sends what its parser makes of the bytes on `tx` —
/// the same channel and the same `ControlEvent`s the MCU thread sends on.
pub struct SurfaceInput {
    pub event_count: Arc<AtomicUsize>,
    /// Holds the connections; dropping them disconnects.
    #[allow(dead_code, reason = "bears the connections for their lifetime, never read")]
    connections: Vec<midir::MidiInputConnection<()>>,
}

impl SurfaceInput {
    pub fn connect(kind: SurfaceKind, tx: Sender<ControlEvent>) -> Result<Self, String> {
        if kind == SurfaceKind::Mcu {
            return Err("MCU is read from the ALSA sequencer, not as bytes".to_string());
        }
        let probe = midir::MidiInput::new("Sonix Surface")
            .map_err(|e| format!("Could not start surface input: {e}"))?;
        let ports = probe.ports();
        drop(probe);

        let event_count = Arc::new(AtomicUsize::new(0));
        let mut connections = Vec::new();
        for port in ports {
            let Ok(input) = midir::MidiInput::new("Sonix Surface") else {
                continue;
            };
            let name = input.port_name(&port).unwrap_or_default();
            let lower = name.to_ascii_lowercase();
            if !lower.contains(kind.port_needle()) || super::midi_input::is_loopback_port(&name) {
                continue;
            }
            let mut parser = SurfaceParser::new(kind);
            let count_t = event_count.clone();
            let tx_t = tx.clone();
            let conn = input.connect(
                &port,
                "sonix-surface",
                move |_when, bytes, _| {
                    for action in parser.feed(bytes) {
                        count_t.fetch_add(1, Ordering::Relaxed);
                        if let Some(ev) = action.to_control_event() {
                            let _ = tx_t.send(ev);
                        }
                    }
                },
                (),
            );
            if let Ok(conn) = conn {
                connections.push(conn);
            }
        }
        if connections.is_empty() {
            return Err(format!("No {} port found", kind.label()));
        }
        Ok(Self {
            event_count,
            connections,
        })
    }

    pub fn received(&self) -> usize {
        self.event_count.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic bytes for the fuzz-ish case (no `rand` in the dependency list).
    struct XorShift(u64);
    impl XorShift {
        fn next_byte(&mut self) -> u8 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 24) as u8
        }
        fn next_range(&mut self, n: usize) -> usize {
            (self.next_byte() as usize) % n
        }
    }

    fn bcf() -> SurfaceParser {
        SurfaceParser::new(SurfaceKind::Bcf2000)
    }

    fn faderport() -> SurfaceParser {
        SurfaceParser::new(SurfaceKind::FaderPort)
    }

    /// `F0 00 20 32 <device> <text...> F7`
    fn bcf_display(device: u8, text: &[u8]) -> Vec<u8> {
        let mut v = vec![0xF0, 0x00, 0x20, 0x32, device];
        v.extend_from_slice(text);
        v.push(0xF7);
        v
    }

    #[test]
    fn a_bcf_fader_move_is_exactly_one_action() {
        // CC 3 = 100: the middle of the BCF2000's 8 faders.
        let bytes = [0xB0u8, 0x03, 100];
        let actions = bcf().feed(&bytes);
        println!("BCF2000 CC 3 = 100 -> {actions:?}");
        assert_eq!(
            actions,
            vec![SurfaceAction::FaderMoved { index: 3, value: 100 }]
        );

        // Every fader channel, so the index is not just a lucky 3.
        for fader in 0..8u8 {
            let actions = bcf().feed(&[0xB0, fader, 64]);
            println!("BCF2000 CC {fader} = 64 -> {actions:?}");
            assert_eq!(
                actions,
                vec![SurfaceAction::FaderMoved {
                    index: fader as usize,
                    value: 64
                }]
            );
        }
        // CC 8 is not a fader on this surface.
        assert!(bcf().feed(&[0xB0, 8, 64]).is_empty());
    }

    #[test]
    fn a_bcf_button_note_is_a_press() {
        let actions = bcf().feed(&[0x90, 5, 0x7F]);
        println!("BCF2000 note-on 5 -> {actions:?}");
        assert_eq!(actions, vec![SurfaceAction::ButtonPressed { id: 5 }]);

        // Release, and note-on with velocity 0, are not new presses.
        assert!(bcf().feed(&[0x80, 5, 0x00]).is_empty());
        assert!(bcf().feed(&[0x90, 5, 0x00]).is_empty());
    }

    #[test]
    fn a_fragmented_sysex_displays_once_when_it_completes() {
        let message = bcf_display(0x00, b"TRACK 3\nVOL 100");
        println!("BCF2000 display bytes: {:02X?}", message);

        let mut parser = bcf();
        // Split mid-payload: the second call must not see a partial message either.
        let split = message.len() / 2;
        let first = parser.feed(&message[..split]);
        println!("  after first {} bytes -> {first:?}", split);
        assert!(first.is_empty(), "a fragment must not yield an action");

        let second = parser.feed(&message[split..]);
        println!("  after last {} bytes -> {second:?}", message.len() - split);
        let expected = SurfaceAction::DisplayText {
            text: "TRACK 3\nVOL 100".to_string(),
        };
        assert_eq!(second, vec![expected.clone()]);

        // Byte-at-a-time is the same message.
        let mut parser = bcf();
        let mut got = Vec::new();
        for b in &message {
            got.extend(parser.feed(std::slice::from_ref(b)));
        }
        assert_eq!(got, vec![expected]);
    }

    #[test]
    fn a_garbage_sysex_yields_nothing_and_does_not_poison_the_stream() {
        let mut parser = bcf();
        // Wrong manufacturer, unterminated, then a real CC.
        assert!(parser.feed(&[0xF0, 0xFF, 0x01, 0x02, 0x03]).is_empty());
        assert!(parser.feed(&[0xF7]).is_empty());
        println!("garbage sysex -> no action, stream still alive");

        // An unterminated sysex longer than the guard is dropped, not buffered forever.
        let mut flood = vec![0xF0];
        flood.extend(std::iter::repeat_n(0x11u8, MAX_SYSEX + 10));
        assert!(parser.feed(&flood).is_empty());
        let actions = parser.feed(&[0xB0, 0x03, 100]);
        println!("CC after the flood -> {actions:?}");
        assert_eq!(
            actions,
            vec![SurfaceAction::FaderMoved { index: 3, value: 100 }]
        );

        // A Behringer header with nothing readable in it is not a display update.
        assert!(bcf().feed(&bcf_display(0, b"\x00\x01\x02")).is_empty());
        assert!(bcf().feed(&[0xF0, 0x00, 0x20, 0x32]).is_empty());
        // The same display bytes on the FaderPort parser mean nothing.
        assert!(faderport().feed(&bcf_display(0, b"HI")).is_empty());
    }

    #[test]
    fn a_faderport_fader_move_is_a_fader_move() {
        let actions = faderport().feed(&[0xB0, 0x00, 42]);
        println!("FaderPort CC 0 = 42 -> {actions:?}");
        assert_eq!(
            actions,
            vec![SurfaceAction::FaderMoved { index: 0, value: 42 }]
        );
    }

    #[test]
    fn all_eight_faderport_transport_ccs_map_to_their_command() {
        let expected = [
            (16u8, TransportCommand::Stop),
            (17, TransportCommand::Play),
            (18, TransportCommand::Record),
            (19, TransportCommand::Rewind),
            (20, TransportCommand::FastForward),
            (21, TransportCommand::Loop),
            (22, TransportCommand::Punch),
            (23, TransportCommand::User),
        ];
        for (cc, command) in expected {
            let actions = faderport().feed(&[0xB0, cc, 0x7F]);
            println!("FaderPort CC {cc} = 0x7F -> {actions:?}");
            assert_eq!(actions, vec![SurfaceAction::TransportCommand(command)]);
            // The LED feedback mirrors the button's own CC.
            println!("  LED feedback -> {:02X?}", faderport_led(command, true));
            assert_eq!(faderport_led(command, true), [0xB0, cc, 0x7F]);
            assert_eq!(faderport_led(command, false), [0xB0, cc, 0x00]);
        }
        // 15 and 24 are outside the transport block.
        assert!(faderport().feed(&[0xB0, 15, 0x7F]).is_empty());
        assert!(faderport().feed(&[0xB0, 24, 0x7F]).is_empty());
    }

    #[test]
    fn faderport_segments_drive_the_position_display() {
        let bytes = faderport_segments(&[1, 2, 5], 0x30, false);
        println!("FaderPort segments 1 2 5 -> {bytes:02X?}");
        assert_eq!(bytes, [0xB0, 0x30, 0x06, 0xB0, 0x31, 0x5B, 0xB0, 0x32, 0x6D]);
        let with_point = faderport_segments(&[4, 0], 0x30, true);
        println!("FaderPort segments 4.0 -> {with_point:02X?}");
        assert_eq!(with_point, [0xB0, 0x30, 0x66, 0xB0, 0x31, 0xBF]);
    }

    #[test]
    fn actions_reach_the_mcu_control_event_path() {
        // The same sink MCU writes to: faders and transport become ControlEvents.
        assert_eq!(
            SurfaceAction::FaderMoved { index: 3, value: 127 }.to_control_event(),
            Some(ControlEvent::TrackVolume { track: 3, value: 1.25 })
        );
        assert_eq!(
            SurfaceAction::TransportCommand(TransportCommand::Play).to_control_event(),
            Some(ControlEvent::Play(true))
        );
        assert_eq!(
            SurfaceAction::TransportCommand(TransportCommand::Stop).to_control_event(),
            Some(ControlEvent::Stop)
        );
        println!(
            "button 21 -> {:?}",
            SurfaceAction::ButtonPressed { id: 21 }.to_control_event()
        );
        assert_eq!(
            SurfaceAction::ButtonPressed { id: 21 }.to_control_event(),
            Some(ControlEvent::TrackMute { track: 5, mute: true })
        );
        // No ControlEvent exists for these yet; they are dropped, not mis-routed.
        assert_eq!(
            SurfaceAction::DisplayText { text: "x".into() }.to_control_event(),
            None
        );
        assert_eq!(
            SurfaceAction::TransportCommand(TransportCommand::Loop).to_control_event(),
            None
        );
        assert_eq!(SurfaceAction::ButtonPressed { id: 5 }.to_control_event(), None);
    }

    #[test]
    fn unknown_and_random_bytes_yield_nothing_and_do_not_panic() {
        // Unknown but well-formed messages.
        let mut parser = bcf();
        assert!(parser.feed(&[0xA0, 0x40, 0x40]).is_empty());
        assert!(parser.feed(&[0xC0, 0x05]).is_empty());
        assert!(parser.feed(&[0xE0, 0x00, 0x40]).is_empty());
        assert!(parser.feed(&[0xD0, 0x10]).is_empty());
        assert!(parser.feed(&[]).is_empty());
        println!("unknown messages -> no actions");

        // 1000 random bytes in random chunk sizes, for every byte-driven parser.
        for kind in [
            SurfaceKind::Bcf2000,
            SurfaceKind::FaderPort,
            SurfaceKind::BabyHui,
        ] {
            let mut parser = SurfaceParser::new(kind);
            let mut rng = XorShift(0x2545_F491_4F6C_DD1D);
            let mut fed = 0usize;
            let mut actions = 0usize;
            while fed < 1000 {
                let chunk = 1 + rng.next_range(23);
                let chunk = chunk.min(1000 - fed);
                let bytes: Vec<u8> = (0..chunk).map(|_| rng.next_byte()).collect();
                actions += parser.feed(&bytes).len();
                fed += chunk;
            }
            println!("{kind:?}: 1000 random bytes -> {actions} actions, no panic");
        }
    }

    const KINDS: [SurfaceKind; 4] = [
        SurfaceKind::Mcu,
        SurfaceKind::Bcf2000,
        SurfaceKind::FaderPort,
        SurfaceKind::BabyHui,
    ];

    fn hex(bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Mixer state with every field distinct, so a swapped index cannot pass unnoticed.
    fn sample_feedback() -> SurfaceFeedback {
        let mut f = SurfaceFeedback {
            playing: true,
            recording: true,
            line1: "PLAY REC 120.0 BPM".to_string(),
            line2: "TRACK 3 100".to_string(),
            position: vec![1, 2, 5],
            ..SurfaceFeedback::default()
        };
        for i in 0..8 {
            f.volume[i] = 10 + i as u8 * 8;
            f.mute[i] = i % 3 == 0;
            f.solo[i] = i == 5;
        }
        f
    }

    /// Requirement (a): what goes out comes back in through the surface's own parser.
    #[test]
    fn feedback_round_trips_through_its_own_parser() {
        let f = sample_feedback();
        for kind in KINDS {
            let bytes = encode_feedback(kind, &f);
            let actions = SurfaceParser::new(kind).feed(&bytes);
            println!("{kind:?} feedback ({} bytes): {}", bytes.len(), hex(&bytes));
            println!("  given   -> {:?}", f);
            println!("  decoded -> {actions:?}");

            match kind {
                // MCU is decoded as ALSA sequencer events, never as bytes; the byte
                // form above is what the mixer sends it, and nothing reads it back.
                SurfaceKind::Mcu => {
                    assert!(actions.is_empty(), "MCU bytes must not decode as surface actions");
                    // The motor faders are 14-bit: decode the pitch bend pair back to a
                    // volume and it must be the one that went out (this caught a u16
                    // overflow in the first version).
                    for (i, &v) in f.volume.iter().enumerate() {
                        let at = bytes
                            .windows(3)
                            .position(|w| w[0] == 0xE0 | i as u8)
                            .expect("every motor fader is sent");
                        let raw = (bytes[at + 1] as u32) | (bytes[at + 2] as u32) << 7;
                        assert_eq!(raw * 127 / 16383, v as u32, "fader {i}");
                    }
                    println!("  motor faders: 14-bit pitch bend on channels 0..7");
                }
                SurfaceKind::FaderPort => {
                    // One fader plus the three transport LEDs, which come back as the
                    // button CCs they mirror.
                    assert_eq!(
                        actions,
                        vec![
                            SurfaceAction::FaderMoved { index: 0, value: f.volume[0] },
                            SurfaceAction::TransportCommand(TransportCommand::Stop),
                            SurfaceAction::TransportCommand(TransportCommand::Play),
                            SurfaceAction::TransportCommand(TransportCommand::Record),
                        ]
                    );
                    // The LED value itself carries the transport state.
                    let led = |cc: u8| {
                        let i = bytes
                            .windows(3)
                            .position(|w| w[0] == CC && w[1] == cc)
                            .expect("every transport LED is sent");
                        bytes[i + 2]
                    };
                    assert_eq!(led(17), if f.playing { 0x7F } else { 0x00 });
                    assert_eq!(led(18), if f.recording { 0x7F } else { 0x00 });
                    println!("  play LED 0x{:02X}, record LED 0x{:02X}", led(17), led(18));
                }
                SurfaceKind::Bcf2000 | SurfaceKind::BabyHui => {
                    let faders: Vec<_> = actions
                        .iter()
                        .filter_map(|a| match a {
                            SurfaceAction::FaderMoved { index, value } => Some((*index, *value)),
                            _ => None,
                        })
                        .collect();
                    assert_eq!(faders, (0..8).map(|i| (i, f.volume[i])).collect::<Vec<_>>());
                }
            }
        }
    }

    /// Requirement (b): the BCF2000 LCD message, byte for byte.
    #[test]
    fn bcf_display_carries_both_lines_in_behringer_sysex() {
        let f = sample_feedback();
        let bytes = encode_feedback(SurfaceKind::Bcf2000, &f);
        let end = bytes.iter().position(|&b| b == 0xF7).expect("sysex ends");
        let sysex = &bytes[..=end];
        println!("BCF2000 sysex: {}", hex(sysex));

        assert_eq!(&sysex[..4], &[0xF0, 0x00, 0x20, 0x32]);
        assert_eq!(*sysex.last().unwrap(), 0xF7);
        // The payload is line1, the 0x0A separator, line2 — and nothing else.
        let mut expected = Vec::new();
        expected.extend_from_slice(&[0xF0, 0x00, 0x20, 0x32, 0x00]);
        expected.extend_from_slice(f.line1.as_bytes());
        expected.push(0x0A);
        expected.extend_from_slice(f.line2.as_bytes());
        expected.push(0xF7);
        assert_eq!(sysex, &expected[..]);
        assert_eq!(
            SurfaceParser::new(SurfaceKind::Bcf2000).feed(sysex),
            vec![SurfaceAction::DisplayText {
                text: format!("{}\n{}", f.line1, f.line2)
            }]
        );
    }

    /// Requirement (c): the position display digits, through the encoder this time.
    #[test]
    fn faderport_position_segments_are_the_documented_masks() {
        let mut f = sample_feedback();
        f.position = vec![1, 2, 5];
        let bytes = encode_feedback(SurfaceKind::FaderPort, &f);
        let tail = &bytes[bytes.len() - 9..];
        println!("FaderPort segments 1 2 5 -> {}", hex(tail));
        assert_eq!(tail, [0xB0, 0x30, 0x06, 0xB0, 0x31, 0x5B, 0xB0, 0x32, 0x6D]);

        f.position = vec![4, 0];
        f.position_decimal = true;
        let bytes = encode_feedback(SurfaceKind::FaderPort, &f);
        let tail = &bytes[bytes.len() - 6..];
        println!("FaderPort segments 4.0 -> {}", hex(tail));
        assert_eq!(tail, [0xB0, 0x30, 0x66, 0xB0, 0x31, 0xBF]);
    }

    /// Requirement (d): BabyHUI's two CC blocks, one message at a time.
    #[test]
    fn babyhui_maps_faders_on_cc_0_to_7_and_buttons_on_cc_32_to_39() {
        for cc in 0..8u8 {
            let actions = SurfaceParser::new(SurfaceKind::BabyHui).feed(&[0xB0, cc, 64]);
            println!("BabyHUI CC {cc} = 64 -> {actions:?}");
            assert_eq!(
                actions,
                vec![SurfaceAction::FaderMoved {
                    index: cc as usize,
                    value: 64
                }]
            );
        }
        for cc in 32..40u8 {
            let actions = SurfaceParser::new(SurfaceKind::BabyHui).feed(&[0xB0, cc, 0x7F]);
            println!("BabyHUI CC {cc} = 0x7F -> {actions:?}");
            assert_eq!(actions, vec![SurfaceAction::ButtonPressed { id: cc }]);
            // Release.
            assert!(SurfaceParser::new(SurfaceKind::BabyHui)
                .feed(&[0xB0, cc, 0x00])
                .is_empty());
        }
        // The V-Pot LEDs sit on the button CCs, so the encoder echoes them there.
        let f = sample_feedback();
        let bytes = encode_feedback(SurfaceKind::BabyHui, &f);
        let leds = &bytes[24..];
        println!("BabyHUI V-Pot LEDs -> {}", hex(leds));
        for i in 0..8 {
            let lit = f.mute[i] || f.solo[i];
            assert_eq!(
                leds[i * 3],
                CC,
                "LED {i} is a CC on the button's own number"
            );
            assert_eq!(leds[i * 3 + 1], 32 + i as u8);
            assert_eq!(leds[i * 3 + 2], if lit { 0x7F } else { 0x00 }, "LED {i}");
        }
        // CC 8 and CC 40 are not on this surface.
        assert!(SurfaceParser::new(SurfaceKind::BabyHui)
            .feed(&[0xB0, 8, 64])
            .is_empty());
        assert!(SurfaceParser::new(SurfaceKind::BabyHui)
            .feed(&[0xB0, 40, 64])
            .is_empty());
    }

    /// Requirement (e): no feedback panics, and a wrong parser never invents a press,
    /// a transport command or a display line. Only CC 0..7 overlap — the three CC
    /// surfaces genuinely share those numbers, and are told apart by their port.
    #[test]
    fn empty_feedback_is_well_formed_and_wrong_parsers_see_no_semantics() {
        let empty = SurfaceFeedback::default();
        for kind in KINDS {
            let bytes = encode_feedback(kind, &empty);
            println!("{kind:?} empty feedback ({} bytes): {}", bytes.len(), hex(&bytes));
            assert!(!bytes.is_empty(), "a surface still gets its clearing message");
            assert!(!SurfaceParser::new(kind).feed(&bytes)
                .iter()
                .any(|a| matches!(a, SurfaceAction::DisplayText { .. })));
        }
        // The BCF sysex is well formed even with no text at all.
        let bcf_empty = encode_feedback(SurfaceKind::Bcf2000, &empty);
        let end = bcf_empty.iter().position(|&b| b == 0xF7).expect("sysex ends");
        assert_eq!(&bcf_empty[..4], &[0xF0, 0x00, 0x20, 0x32]);
        assert_eq!(end, 6, "header, the separator and F7, then the CCs");

        let f = sample_feedback();
        for from in KINDS {
            let bytes = encode_feedback(from, &f);
            for to in KINDS.into_iter().filter(|k| *k != from) {
                let actions = SurfaceParser::new(to).feed(&bytes);
                println!("{from:?} feedback -> {to:?} parser: {actions:?}");
                assert!(
                    actions
                        .iter()
                        .all(|a| matches!(a, SurfaceAction::FaderMoved { .. })),
                    "{from:?} bytes gave {to:?} a button/transport/display"
                );
                // The shared fader CCs, and nothing beyond them.
                let faders = actions.len();
                let expected = match (from, to) {
                    (SurfaceKind::Bcf2000, SurfaceKind::FaderPort) => 1,
                    (SurfaceKind::FaderPort, SurfaceKind::Bcf2000) => 1,
                    (SurfaceKind::FaderPort, SurfaceKind::BabyHui) => 1,
                    (SurfaceKind::Bcf2000, SurfaceKind::BabyHui) => 8,
                    (SurfaceKind::BabyHui, SurfaceKind::Bcf2000) => 8,
                    (SurfaceKind::BabyHui, SurfaceKind::FaderPort) => 1,
                    _ => 0,
                };
                assert_eq!(faders, expected, "{from:?} -> {to:?}");
            }
        }
    }
}
