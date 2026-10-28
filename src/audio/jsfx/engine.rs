//! **JSFX-motorn.** Kör ett [`Program`] ur `parser.rs`: sektionerna `@init` → `@slider` →
//! `@block` → `@sample`, med EEL2:s minnesmodell (alla variabler är platser i ett `f64`-minne),
//! egna funktioner, och de inbyggda funktionerna DSP-koden faktiskt använder.
//!
//! **Vad som avsiktligt inte är med.** `@gfx` (UI:t ritas av REAPER, inte av oss) och
//! `@serialize` (tillstånd sparas i projektet, inte i effekten) hoppas över vid tolkningen. En
//! effekt som bara ritar ett mätinstrument låter alltså som ingenting — det är rätt, den gör
//! ingenting med ljudet.
//!
//! Minnesmodellen är EEL2:s: variabler och arrayer delar ett enda `f64`-minne. `slider1..64` bor
//! på plats 0–63, som i REAPER, så en fil som skriver `slider1 = 5` beter sig som originalet.

use std::collections::HashMap;

use super::parser::{AssignOp, BinOp, Expr, Program, Stmt, UnOp};

/// Hur många steg **ett påstående** får köra innan vi ger upp. Skyddar mot `while(1)` i en fil vi
/// inte har provat — en hängning i ljudtråden är värre än ett felmeddelande.
///
/// Per påstående, inte per prov: `@init` bygger wavetabeller och delaybuffertar med miljontals
/// varv, och det är fullt normalt. Ett oändligt `while` fastnar ändå, eftersom det ligger i ett
/// enda påstående.
const MAX_STEPS: usize = 500_000_000;
/// Samma gräns per ljudprov: en effekt som behöver mer än så per prov är trasig, inte långsam.
const MAX_STEPS_PER_SAMPLE: usize = 2_000_000;

/// Storlek på det delade minnet vid start (8 M `f64` = 64 MB), och hur mycket det får växa.
const MEM_START: usize = 1 << 20;
const MEM_MAX: usize = 8 << 20;

/// Ett fel under körning. Sektionen och raden räcker för att hitta filen det gäller.
#[derive(Debug, Clone)]
pub struct RunError {
    pub section: &'static str,
    pub message: String,
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}: {}", self.section, self.message)
    }
}

/// En egen funktion: parametrar, lokala variabler, `instance`-namn och kropp.
#[derive(Clone)]
struct FnDef {
    params: Vec<String>,
    locals: Vec<String>,
    instances: Vec<String>,
    body: Vec<Stmt>,
}

/// En MIDI-händelse som JSFX-koden ser den: `(offset, status, data1, data2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiEvent {
    pub offset: usize,
    pub status: u8,
    pub data1: u8,
    pub data2: u8,
}

/// En kompilerad och körbar JSFX-instans.
pub struct Jsfx {
    program: Program,
    mem: Vec<f64>,
    slots: HashMap<String, usize>,
    strings: Vec<String>,
    str_slots: HashMap<String, usize>,
    funcs: HashMap<String, FnDef>,
    /// Namnrymden vi kör i: `rmsa.rms.init(...)` gör `this.x` till `rmsa.x`.
    namespace: Option<String>,
    /// Öppnade filer: prover, kanaler, frekvens och läsposition.
    files: HashMap<i32, OpenFile>,
    /// Mappen filen kom från — `file_open("ljud.wav")` letar där, som i REAPER.
    source_dir: Option<std::path::PathBuf>,
    /// `instance(...)`-namnen för funktionen som kör just nu.
    current_instances: Vec<String>,
    /// Senast slagna upp variabelnamn → plats.
    last_slot: Option<(String, usize)>,
    sample_rate: f64,
    spl: [f64; 2],
    /// Antal kanaler JSFX-koden tror att den ser.
    num_ch: usize,
    samplesblock: usize,
    midi_in: Vec<MidiEvent>,
    midi_out: Vec<MidiEvent>,
    /// Hur många in-händelser som redan lämnats ut av `midirecv`.
    midi_cursor: usize,
    steps: usize,
    step_limit: usize,
    pub errors: Vec<RunError>,
    /// Kända inbyggda som svarade "inget" under körningen (fil-API, MDCT, …).
    pub stubs: std::collections::BTreeSet<String>,
    /// Sätts när ett prov kastat fel. Effekten tystnar och värden kastas inte bort på CPU.
    failed: bool,
}

impl Jsfx {
    /// Tolkar och förbereder en fil. `@init` körs direkt, som i REAPER.
    pub fn compile(source: &str, sample_rate: f64) -> Result<Self, String> {
        Self::build(source, None, sample_rate)
    }

    /// Som [`Self::compile`], men filens egen mapp är känd redan när `@init` körs — det är då
    /// samplers öppnar sina ljudfiler.
    pub fn compile_in(
        source: &str,
        source_dir: std::path::PathBuf,
        sample_rate: f64,
    ) -> Result<Self, String> {
        Self::build(source, Some(source_dir), sample_rate)
    }

    fn build(
        source: &str,
        source_dir: Option<std::path::PathBuf>,
        sample_rate: f64,
    ) -> Result<Self, String> {
        let program = super::parser::parse(source)?;
        let mut vm = Self {
            program,
            mem: vec![0.0; MEM_START],
            slots: HashMap::new(),
            strings: Vec::new(),
            str_slots: HashMap::new(),
            funcs: HashMap::new(),
            namespace: None,
            files: HashMap::new(),
            source_dir,
            current_instances: Vec::new(),
            last_slot: None,
            sample_rate,
            spl: [0.0; 2],
            num_ch: 2,
            samplesblock: 0,
            midi_in: Vec::new(),
            midi_out: Vec::new(),
            midi_cursor: 0,
            steps: 0,
            step_limit: MAX_STEPS,
            errors: Vec::new(),
            stubs: std::collections::BTreeSet::new(),
            failed: false,
        };
        // Sliderna bor på plats 0..63, som i REAPER.
        let defs = vm.program.sliders.clone();
        for def in &defs {
            let slot = def.index.saturating_sub(1);
            if slot < 64 {
                vm.ensure_mem(slot);
                vm.mem[slot] = def.default;
            }
        }
        // Egna funktioner registreras före @init, så en funktion kan anropas var som helst.
        for stmt in std::mem::take(&mut vm.program.functions) {
            if let Stmt::Function {
                name,
                params,
                locals,
                instances,
                body,
            } = stmt
            {
                vm.funcs.insert(
                    name,
                    FnDef {
                        params,
                        locals,
                        instances,
                        body,
                    },
                );
            }
        }
        let init = vm.program.init.clone();
        vm.run_section("init", &init).map_err(|e| e.to_string())?;
        // REAPER kör `@slider` en gång när effekten laddas, så att koefficienter byggs av
        // sliderns startvärde. Utan det är en effekt tyst fram till första vridningen.
        let slider = vm.program.slider.clone();
        vm.run_section("slider", &slider)
            .map_err(|e| e.to_string())?;
        Ok(vm)
    }

    /// Startar om effekten: minnet nollställs (sliderna behålls) och `@init` körs igen, som när
    /// en plugin tas bort och läggs tillbaka.
    pub fn reset(&mut self) -> Result<(), RunError> {
        for cell in self.mem.iter_mut().skip(64) {
            *cell = 0.0;
        }
        self.last_slot = None;
        self.stubs.clear();
        self.errors.clear();
        self.failed = false;
        self.spl = [0.0; 2];
        let init = self.program.init.clone();
        self.run_section("init", &init)
    }

    /// `file_open`: relativt källfilens mapp (som i REAPER), annars användarens JSFX-mapp,
    /// annars sökvägen som den står. Bara WAV avkodas — FLAC hade krävt en egen avkodare.
    fn open_file(&mut self, name: &str) -> Option<i32> {
        let mut candidates = Vec::new();
        let given = std::path::Path::new(name);
        if given.is_absolute() {
            candidates.push(given.to_path_buf());
        } else {
            if let Some(dir) = &self.source_dir {
                candidates.push(dir.join(name));
            }
            candidates.push(super::user_dir().join(name));
        }
        let path = candidates.into_iter().find(|p| p.is_file())?;
        let (left, right, rate) = crate::audio::wav_reader::load_audio_pcm(&path.display().to_string()).ok()?;
        let channels = if right.iter().any(|r| *r != 0.0) { 2 } else { 1 };
        let samples = if channels == 2 {
            left.iter().zip(right.iter()).flat_map(|(l, r)| [*l, *r]).collect()
        } else {
            left
        };
        let handle = self.files.len() as i32 + 1;
        self.files.insert(
            handle,
            OpenFile {
                samples,
                channels,
                rate: rate as f64,
                cursor: 0,
            },
        );
        Some(handle)
    }

    /// Sant när effekten kastat fel. Värden ska då koppla bort den i stället för att köra den.
    pub fn has_failed(&self) -> bool {
        self.failed
    }

    pub fn desc(&self) -> &str {
        &self.program.desc
    }

    pub fn slider_defs(&self) -> &[super::parser::SliderDef] {
        &self.program.sliders
    }

    /// Sätter en slider och kör `@slider`, som REAPER gör när man vrider.
    pub fn set_slider(&mut self, index: usize, value: f64) -> Result<(), RunError> {
        let slot = index.saturating_sub(1);
        if let Some(def) = self.program.sliders.iter().find(|s| s.index == index) {
            let clamped = value.clamp(def.min, def.max);
            self.ensure_mem(slot);
            self.mem[slot] = clamped;
        } else {
            self.ensure_mem(slot);
            self.mem[slot] = value;
        }
        let slider = self.program.slider.clone();
        self.run_section("slider", &slider)
    }

    /// Kör ett block: `@block` en gång, sedan `@sample` per prov. MIDI-händelser skickas in i
    /// `midi` och det koden skickar ut hamnar i returvärdet.
    pub fn process(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        midi: &[MidiEvent],
    ) -> Result<Vec<MidiEvent>, RunError> {
        self.samplesblock = left.len();
        self.midi_in = midi.to_vec();
        self.midi_out.clear();
        self.midi_cursor = 0;
        let block = self.program.block.clone();
        self.run_section("block", &block)?;

        let sample = self.program.sample.clone();
        let frames = left.len().min(right.len());
        for i in 0..frames {
            self.spl = [left[i] as f64, right[i] as f64];
            // MIDI-händelser som ligger på just det här provet är synliga i @sample.
            let at = self.midi_in.iter().filter(|e| e.offset == i).copied().collect::<Vec<_>>();
            if !at.is_empty() {
                self.midi_in = at;
                self.midi_cursor = 0;
            } else {
                self.midi_in.clear();
            }
            if let Err(e) = self.run_section("sample", &sample) {
                self.errors.push(e);
                // **En effekt som kastar fel tystnar helt.** Annars eldas 2 miljoner steg per
                // prov på en loop som ändå inte kan bli rätt — och en trasig effekt ska kosta
                // tystnad, inte hela ljudtråden.
                self.failed = true;
                self.spl = [0.0, 0.0];
            }
            if self.failed {
                for frame in i..frames {
                    left[frame] = 0.0;
                    right[frame] = 0.0;
                }
                break;
            }
            // **Ingen konstgjord gräns här.** En JSFX får skriva vilken nivå som helst — det är
            // värden (och DAC:en) som avgör vad som klipper, inte tolken. Bara NaN/Inf stoppas,
            // för de förgiftar hela kedjan.
            left[i] = finite_or_zero(self.spl[0]);
            right[i] = finite_or_zero(self.spl[1]);
        }
        Ok(std::mem::take(&mut self.midi_out))
    }

    fn run_section(&mut self, name: &'static str, stmts: &[Stmt]) -> Result<(), RunError> {
        self.step_limit = if name == "sample" {
            MAX_STEPS_PER_SAMPLE
        } else {
            MAX_STEPS
        };
        for (index, stmt) in stmts.iter().enumerate() {
            // Räknaren gäller per påstående: en tabellbyggande loop i `@init` är inte en hängning.
            self.steps = 0;
            self.exec(stmt).map_err(|message| RunError {
                section: name,
                // Påståendenumret gör det möjligt att hitta raden i filen.
                message: format!("påstående {}: {message}", index + 1),
            })?;
        }
        Ok(())
    }

    fn ensure_mem(&mut self, index: usize) {
        if index < self.mem.len() {
            return;
        }
        if index >= MEM_MAX {
            return; // utanför minnet: skrivningen tappas, som ett index ut i tomma luften
        }
        let mut size = self.mem.len();
        while size <= index {
            size *= 2;
        }
        self.mem.resize(size.min(MEM_MAX), 0.0);
    }

    fn slot(&mut self, name: &str) -> usize {
        // Envariabelscache: DSP-loopar läser samma namn tusentals gånger i rad.
        if let Some((cached, slot)) = &self.last_slot {
            if cached == name {
                return *slot;
            }
        }
        if let Some(slot) = self.slots.get(name) {
            self.last_slot = Some((name.to_string(), *slot));
            return *slot;
        }
        let slot = self.slots.len() + 64; // 0..63 är sliderna
        self.slots.insert(name.to_string(), slot);
        self.last_slot = Some((name.to_string(), slot));
        self.ensure_mem(slot);
        slot
    }

    fn tick(&mut self) -> Result<(), String> {
        self.steps += 1;
        if self.steps > self.step_limit {
            return Err(format!(
                "fler än {} steg i ett påstående — oändlig loop?",
                self.step_limit
            ));
        }
        Ok(())
    }

    fn exec(&mut self, stmt: &Stmt) -> Result<(), String> {
        self.tick()?;
        match stmt {
            Stmt::Empty | Stmt::Declare => Ok(()),
            Stmt::Expr(expr) => {
                self.eval(expr)?;
                Ok(())
            }
            Stmt::Block(body) => {
                for stmt in body {
                    self.exec(stmt)?;
                }
                Ok(())
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                if self.eval(cond)? != 0.0 {
                    self.exec(then)
                } else if let Some(other) = otherwise {
                    self.exec(other)
                } else {
                    Ok(())
                }
            }
            Stmt::While { cond, body } => {
                while self.eval(cond)? != 0.0 {
                    self.exec(body)?;
                    self.tick()?;
                }
                Ok(())
            }
            Stmt::Loop { count, body } => {
                let n = self.eval(count)?.max(0.0) as usize;
                for _ in 0..n {
                    self.exec(body)?;
                    self.tick()?;
                }
                Ok(())
            }
            // Funktioner är hoistade; en definition som ligger kvar körs inte.
            Stmt::Function { .. } => Ok(()),
        }
    }

    fn eval(&mut self, expr: &Expr) -> Result<f64, String> {
        self.tick()?;
        match expr {
            Expr::Num(v) => Ok(*v),
            Expr::Str(s) => Ok(self.intern(s)),
            Expr::StrVar(name) => Ok(*self.str_slots.get(name).unwrap_or(&0) as f64),
            Expr::Var(name) => self.read_var(name),
            Expr::Index(base, index) => {
                let base = self.eval(base)?;
                let index = self.eval(index)?;
                let addr = self.address(base, index);
                Ok(self.mem.get(addr).copied().unwrap_or(0.0))
            }
            Expr::Unary(op, inner) => {
                let v = self.eval(inner)?;
                Ok(match op {
                    UnOp::Neg => -v,
                    UnOp::Not => {
                        if v == 0.0 {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    UnOp::BitNot => !to_int(v) as f64,
                })
            }
            Expr::Binary(op, a, b) => self.binary(*op, a, b),
            Expr::Ternary(cond, then, other) => {
                if self.eval(cond)? != 0.0 {
                    self.eval(then)
                } else {
                    self.eval(other)
                }
            }
            Expr::Assign(target, op, value) => self.assign(target, *op, value),
            Expr::IncDec(target, delta, prefix) => {
                let before = self.eval(target)?;
                let after = before + delta;
                self.write(target, after)?;
                Ok(if *prefix { after } else { before })
            }
            Expr::Call(name, args) => self.call(name, args),
            Expr::Seq(body) => {
                // Blockets värde är det sista uttryckets värde.
                let mut last = 0.0;
                for stmt in body {
                    match stmt {
                        Stmt::Expr(expr) => last = self.eval(expr)?,
                        other => self.exec(other)?,
                    }
                }
                Ok(last)
            }
        }
    }

    /// EEL2:s `base[index]` är `mem[base + index]`.
    fn address(&self, base: f64, index: f64) -> usize {
        let addr = base as i64 + index as i64;
        addr.clamp(0, (self.mem.len() - 1) as i64) as usize
    }

    fn read_var(&mut self, name: &str) -> Result<f64, String> {
        Ok(match name {
            "spl0" | "sample" => self.spl[0],
            "spl1" => self.spl[1],
            "srate" => self.sample_rate,
            "num_ch" => self.num_ch as f64,
            "samplesblock" => self.samplesblock as f64,
            // Tempot är appens, inte filens: en effekt som läser tempo ska få vårt.
            "tempo" => 120.0,
            "ts_num" => 4.0,
            "ts_denom" => 4.0,
            "play_state" => 1.0,
            "play_position" => 0.0,
            "beat_position" => 0.0,
            "slider_automate" => 0.0,
            _ => {
                if let Some(index) = name.strip_prefix("slider") {
                    if let Ok(n) = index.parse::<usize>() {
                        if (1..=64).contains(&n) {
                            return Ok(self.mem[n - 1]);
                        }
                    }
                }
                let instances = std::mem::take(&mut self.current_instances);
                let scoped = self.scoped(name, &instances).into_owned();
                self.current_instances = instances;
                let slot = self.slot(&scoped);
                self.mem.get(slot).copied().unwrap_or(0.0)
            }
        })
    }

    fn write(&mut self, target: &Expr, value: f64) -> Result<(), String> {
        match target {
            Expr::Var(name) => {
                match name.as_str() {
                    "spl0" => self.spl[0] = value,
                    "spl1" => self.spl[1] = value,
                    _ => {
                        if let Some(index) = name.strip_prefix("slider") {
                            if let Ok(n) = index.parse::<usize>() {
                                if (1..=64).contains(&n) {
                                    self.mem[n - 1] = value;
                                    return Ok(());
                                }
                            }
                        }
                        let instances = std::mem::take(&mut self.current_instances);
                        let scoped = self.scoped(name, &instances).into_owned();
                        self.current_instances = instances;
                        let slot = self.slot(&scoped);
                        self.ensure_mem(slot);
                        self.mem[slot] = value;
                    }
                }
                Ok(())
            }
            Expr::StrVar(name) => {
                let index = value as usize;
                self.str_slots.insert(name.clone(), index);
                Ok(())
            }
            Expr::Index(base, index) => {
                let base = self.eval(base)?;
                let index = self.eval(index)?;
                let addr = self.address(base, index);
                self.ensure_mem(addr);
                if addr < self.mem.len() {
                    self.mem[addr] = value;
                }
                Ok(())
            }
            // `slider(6 + k) = x` — sliden nås med index, nollbaserat som `slider1` = plats 0.
            Expr::Call(name, args) if name == "slider" => {
                let index = match args.first() {
                    Some(arg) => self.eval(arg)?.max(0.0) as usize,
                    None => 0,
                };
                self.ensure_mem(index);
                if index < self.mem.len() {
                    self.mem[index] = value;
                }
                Ok(())
            }
            other => Err(format!("kan inte tilldela till {other:?}")),
        }
    }

    fn assign(&mut self, target: &Expr, op: AssignOp, value: &Expr) -> Result<f64, String> {
        let rhs = self.eval(value)?;
        if op == AssignOp::Set {
            self.write(target, rhs)?;
            return Ok(rhs);
        }
        let current = self.eval(target)?;
        let new = match op {
            AssignOp::Add => current + rhs,
            AssignOp::Sub => current - rhs,
            AssignOp::Mul => current * rhs,
            AssignOp::Div => {
                if rhs == 0.0 {
                    0.0
                } else {
                    current / rhs
                }
            }
            AssignOp::Mod => {
                if rhs == 0.0 {
                    0.0
                } else {
                    current % rhs
                }
            }
            AssignOp::BitOrAssign => (to_int(current) | to_int(rhs)) as f64,
            AssignOp::BitAndAssign => (to_int(current) & to_int(rhs)) as f64,
            AssignOp::ShlAssign => (to_int(current) << (to_int(rhs) & 31)) as f64,
            AssignOp::ShrAssign => (to_int(current) >> (to_int(rhs) & 31)) as f64,
            AssignOp::Pow => current.powf(rhs),
            AssignOp::BitXorAssign => (to_int(current) ^ to_int(rhs)) as f64,
            AssignOp::Set => rhs,
        };
        self.write(target, new)?;
        Ok(new)
    }

    fn binary(&mut self, op: BinOp, a: &Expr, b: &Expr) -> Result<f64, String> {
        // Kortslutning som i EEL2: `&&` och `||` utvärderar bara det de måste.
        if op == BinOp::And {
            if self.eval(a)? == 0.0 {
                return Ok(0.0);
            }
            return Ok(if self.eval(b)? != 0.0 { 1.0 } else { 0.0 });
        }
        if op == BinOp::Or {
            if self.eval(a)? != 0.0 {
                return Ok(1.0);
            }
            return Ok(if self.eval(b)? != 0.0 { 1.0 } else { 0.0 });
        }
        let x = self.eval(a)?;
        let y = self.eval(b)?;
        Ok(match op {
            BinOp::Add => x + y,
            BinOp::Sub => x - y,
            BinOp::Mul => x * y,
            BinOp::Div => {
                if y == 0.0 {
                    0.0
                } else {
                    x / y
                }
            }
            BinOp::Mod => {
                if y == 0.0 {
                    0.0
                } else {
                    x % y
                }
            }
            BinOp::Pow => x.powf(y),
            BinOp::Lt => bool_num(x < y),
            BinOp::Gt => bool_num(x > y),
            BinOp::Le => bool_num(x <= y),
            BinOp::Ge => bool_num(x >= y),
            BinOp::Eq => bool_num(x == y),
            BinOp::Ne => bool_num(x != y),
            BinOp::And | BinOp::Or => unreachable!(),
            BinOp::BitAnd => (to_int(x) & to_int(y)) as f64,
            BinOp::BitOr => (to_int(x) | to_int(y)) as f64,
            BinOp::BitXor => (to_int(x) ^ to_int(y)) as f64,
            BinOp::Shl => (to_int(x) << (to_int(y) & 31)) as f64,
            BinOp::Shr => (to_int(x) >> (to_int(y) & 31)) as f64,
        })
    }

    fn intern(&mut self, text: &str) -> f64 {
        if let Some(index) = self.strings.iter().position(|s| s == text) {
            return index as f64;
        }
        self.strings.push(text.to_string());
        (self.strings.len() - 1) as f64
    }

    fn string(&self, index: f64) -> &str {
        self.strings.get(index as usize).map(|s| s.as_str()).unwrap_or("")
    }

    /// `rmsa.rms.init(...)` → funktionen `rms.init` i namnrymden `rmsa`. EEL2 letar efter den
    /// längsta kända svansen av namnet och gör resten till namnrymd (`this`).
    fn resolve_user_fn(&self, name: &str) -> Option<(String, Option<String>)> {
        if self.funcs.contains_key(name) {
            return Some((name.to_string(), None));
        }
        let mut rest = name;
        while let Some((head, tail)) = rest.split_once('.') {
            if self.funcs.contains_key(tail) {
                let prefix = name[..name.len() - tail.len()].trim_end_matches('.');
                return Some((tail.to_string(), Some(prefix.to_string())));
            }
            rest = tail;
            let _ = head;
        }
        None
    }

    /// Byter namn enligt namnrymden: `this.x` → `rmsa.x`, och `instance`-namn i den körande
    /// funktionen → `rmsa.x`. Returnerar lån när inget behöver skrivas om — det är det vanliga
    /// fallet, och en allokering per variabelåtkomst märks i en DSP-loop.
    fn scoped<'a>(&self, name: &'a str, instances: &[String]) -> std::borrow::Cow<'a, str> {
        use std::borrow::Cow;
        if let Some(field) = name.strip_prefix("this.") {
            if let Some(ns) = &self.namespace {
                return Cow::Owned(format!("{ns}.{field}"));
            }
        }
        if !instances.is_empty() && instances.iter().any(|i| i == name) {
            if let Some(ns) = &self.namespace {
                return Cow::Owned(format!("{ns}.{name}"));
            }
        }
        Cow::Borrowed(name)
    }

    fn call(&mut self, name: &str, args: &[Expr]) -> Result<f64, String> {
        // Egna funktioner först: de skuggar inga inbyggda, men EEL2 tillåter egna namn.
        if let Some((resolved, namespace)) = self.resolve_user_fn(name) {
            let Some(FnDef {
                params,
                locals,
                instances,
                body,
            }) = self.funcs.get(&resolved).cloned()
            else {
                return Err(format!("okänd funktion '{name}'"));
            };
            let previous_namespace = self.namespace.clone();
            let previous_instances = std::mem::take(&mut self.current_instances);
            if let Some(ns) = namespace {
                self.namespace = Some(ns);
            } else {
                // Ett anrop utan namnrymd ärver den som gäller.
            }
            self.current_instances = instances.clone();
            let result = self.call_user(&params, &locals, &instances, &body, args);
            self.namespace = previous_namespace;
            self.current_instances = previous_instances;
            return result;
        }
        self.builtin(name, args)
    }

    fn call_user(
        &mut self,
        params: &[String],
        locals: &[String],
        instances: &[String],
        body: &[Stmt],
        args: &[Expr],
    ) -> Result<f64, String> {
        {
            let values: Vec<f64> = args
                .iter()
                .map(|a| self.eval(a))
                .collect::<Result<Vec<_>, _>>()?;
            // Parametrar och lokala variabler sparas och återställs: EEL2:s funktioner har egen
            // namnrymd, och rekursion ska fungera.
            let mut saved: Vec<(String, f64)> = Vec::new();
            for (i, param) in params.iter().enumerate() {
                let name = self.scoped(param, instances).into_owned();
                let slot = self.slot(&name);
                saved.push((name, self.mem[slot]));
                self.mem[slot] = values.get(i).copied().unwrap_or(0.0);
            }
            for local in locals {
                let name = self.scoped(local, instances).into_owned();
                let slot = self.slot(&name);
                saved.push((name, self.mem[slot]));
                self.mem[slot] = 0.0;
            }
            let mut result = 0.0;
            for stmt in body {
                match stmt {
                    // Sista uttrycket i kroppen är returvärdet.
                    Stmt::Expr(expr) => result = self.eval(expr)?,
                    other => self.exec(other)?,
                }
            }
            for (name, value) in saved {
                let slot = self.slot(&name);
                self.mem[slot] = value;
            }
            Ok(result)
        }
    }

    fn builtin(&mut self, name: &str, args: &[Expr]) -> Result<f64, String> {
        // Argumenten utvärderas i ordning; de flesta inbyggda är rena funktioner.
        macro_rules! eval {
            ($expr:expr) => {
                self.eval($expr)?
            };
        }
        Ok(match name {
            "sin" => eval!(&args[0]).sin(),
            "cos" => eval!(&args[0]).cos(),
            "tan" => eval!(&args[0]).tan(),
            "asin" => eval!(&args[0]).asin(),
            "acos" => eval!(&args[0]).acos(),
            "atan" => eval!(&args[0]).atan(),
            "atan2" => eval!(&args[0]).atan2(eval!(&args[1])),
            "exp" => eval!(&args[0]).exp(),
            "log" => eval!(&args[0]).ln(),
            "log10" => eval!(&args[0]).log10(),
            "pow" => eval!(&args[0]).powf(eval!(&args[1])),
            "sqrt" => eval!(&args[0]).sqrt(),
            "invsqrt" => 1.0 / eval!(&args[0]).max(0.0).sqrt(),
            "abs" => eval!(&args[0]).abs(),
            "min" => eval!(&args[0]).min(eval!(&args[1])),
            "max" => eval!(&args[0]).max(eval!(&args[1])),
            "floor" => eval!(&args[0]).floor(),
            "ceil" => eval!(&args[0]).ceil(),
            "sign" => eval!(&args[0]).signum(),
            "sqr" => {
                let v = eval!(&args[0]);
                v * v
            }
            "rand" => {
                // Deterministisk nog för ett prov, slumpmässig nog för en effekt.
                let x = eval!(&args.get(0).cloned().unwrap_or(Expr::Num(1.0))).max(1.0);
                (self.psuedo_random() * x).floor()
            }
            "fmod" => {
                let a = eval!(&args[0]);
                let b = eval!(&args[1]);
                if b == 0.0 { 0.0 } else { a % b }
            }
            "memset" => {
                let base = eval!(&args[0]);
                let value = eval!(&args[1]);
                let count = eval!(&args[2]);
                let start = self.address(base, 0.0);
                let end = (start + count.max(0.0) as usize).min(self.mem.len());
                for cell in &mut self.mem[start..end] {
                    *cell = value;
                }
                0.0
            }
            "memcpy" => {
                let dest_base = eval!(&args[0]);
                let src_base = eval!(&args[1]);
                let dest = self.address(dest_base, 0.0);
                let src = self.address(src_base, 0.0);
                let count = eval!(&args[2]).max(0.0) as usize;
                let count = count.min(self.mem.len() - dest).min(self.mem.len() - src);
                let copy: Vec<f64> = self.mem[src..src + count].to_vec();
                self.mem[dest..dest + count].copy_from_slice(&copy);
                0.0
            }
            "mem_set_values" => {
                // `mem_set_values(dest, v1, v2, ...)` — tabeller skrivs så i REAPERs filer.
                let base = eval!(&args[0]);
                let start = self.address(base, 0.0);
                for (i, arg) in args[1..].iter().enumerate() {
                    let value = self.eval(arg)?;
                    let addr = start + i;
                    if addr < self.mem.len() {
                        self.mem[addr] = value;
                    }
                }
                0.0
            }
            "alloc" => {
                // Enkel bump-allokator: minnet är ett enda block och vi lämnar tillbaka en offset
                // långt över variablerna.
                let count = eval!(&args[0]).max(1.0) as usize;
                let start = self.heap_top();
                self.ensure_mem(start + count);
                self.set_heap_top(start + count);
                let end = (start + count).min(self.mem.len());
                memset_zero(&mut self.mem[start..end]);
                start as f64
            }
            "freembuf" => 0.0,
            // Slider- och MIDI-namn-API:t ändrar bara hur REAPER *visar* saker; det finns ingen
            // display att ändra, men koden ska inte falla på att den anropas.
            "sliderchange" | "slider_automate" | "slider_next_chg" | "slider_show" | "slider_hide"
            | "slider_name" | "slider_get" | "set_midi_note_name" | "set_midi_cc_name" => {
                for arg in args {
                    self.eval(arg)?;
                }
                0.0
            }
            "time_precise" => 0.0,
            "printf" => 0.0,
            "sprintf" => {
                let format = self.string_at(&args[0]);
                let mut values = Vec::new();
                for arg in &args[1..] {
                    values.push(self.eval(arg)?);
                }
                let text = format_string(&format, &values);
                self.intern(&text)
            }
            "strcpy" => {
                let text = self.string_at(&args[1]).to_string();
                self.intern(&text)
            }
            "strlen" => self.string_at(&args[0]).chars().count() as f64,
            "str_getchar" => {
                let text = self.string_at(&args[0]).to_string();
                let index = eval!(&args.get(1).cloned().unwrap_or(Expr::Num(0.0))) as usize;
                text.chars().nth(index).map(|c| c as u32 as f64).unwrap_or(0.0)
            }
            "fft" => {
                let base = eval!(&args[0]);
                let size = eval!(&args[1]).max(2.0) as usize;
                self.fft_in_place(base, size, false);
                0.0
            }
            "ifft" => {
                let base = eval!(&args[0]);
                let size = eval!(&args[1]).max(2.0) as usize;
                self.fft_in_place(base, size, true);
                0.0
            }
            "fft_real" => {
                let base = eval!(&args[0]);
                let size = eval!(&args[1]).max(2.0) as usize;
                let inverse = args.get(2).map(|e| self.eval(e)).transpose()?.unwrap_or(0.0) != 0.0;
                let start = self.address(base, 0.0);
                if inverse {
                    self.ifft_real(start, size);
                } else {
                    self.fft_real_forward(start, size);
                }
                0.0
            }
            "fft_permute" | "fft_ipermute" => 0.0,
            "midirecv" => {
                let offset = eval!(&args[0]);
                // Varje händelse lämnas ut en gång, annars snurrar `while (midirecv(...))` för evigt.
                let event = self.midi_in[self.midi_cursor..]
                    .iter()
                    .find(|e| e.offset as f64 >= offset)
                    .copied();
                match event {
                    Some(e) => {
                        self.midi_cursor += 1;
                        let packed = (e.data1 as f64) + (e.data2 as f64) * 256.0;
                        if let Some(slot) = args.get(1) {
                            let _ = self.write(slot, e.status as f64);
                        }
                        if let Some(slot) = args.get(2) {
                            // Tre argument: msg23 är packat. Fyra: separata databytar.
                            let value = if args.len() >= 4 { e.data1 as f64 } else { packed };
                            let _ = self.write(slot, value);
                        }
                        if let Some(slot) = args.get(3) {
                            let _ = self.write(slot, e.data2 as f64);
                        }
                        1.0
                    }
                    None => 0.0,
                }
            }
            "midisend" => {
                let offset = eval!(&args[0]) as usize;
                let status = eval!(&args[1]) as u8;
                let msg23 = eval!(&args[2]) as u32;
                self.midi_out.push(MidiEvent {
                    offset,
                    status,
                    data1: (msg23 & 0xff) as u8,
                    data2: ((msg23 >> 8) & 0xff) as u8,
                });
                0.0
            }
            "midisend_buf" | "midirecv_buf" => 0.0,
            "spl" => {
                let index = eval!(&args[0]) as usize;
                self.spl.get(index).copied().unwrap_or(0.0)
            }
            // **Kända inbyggda som vi inte har data för.** De svarar "inget" (0 / -1) och
            // bokförs, så att listan över stubbade funktioner går att läsa ut i stället för att
            // gissas. Fil-API:t är det viktigaste: en sampler som inte hittar sina samplingar
            // ska tystna, inte krascha.
            "file_open" | "fopen" => {
                let name = self.string_at(&args[0]);
                match self.open_file(&name) {
                    Some(handle) => handle as f64,
                    None => {
                        self.stubs.insert("file_open".to_string());
                        -1.0
                    }
                }
            }
            "file_close" | "fclose" => {
                let handle = eval!(&args[0]) as i32;
                self.files.remove(&handle);
                0.0
            }
            "file_riff" => {
                // `file_riff(handle, nch, srate)` — skriver kanaler och samplingsfrekvens.
                let handle = eval!(&args[0]) as i32;
                if let Some(file) = self.files.get(&handle) {
                    let nch = file.channels;
                    let rate = file.rate;
                    let _ = self.write(&args[1], nch as f64);
                    let _ = self.write(&args[2], rate);
                    1.0
                } else {
                    0.0
                }
            }
            "file_avail" => {
                let handle = eval!(&args[0]) as i32;
                self.files
                    .get(&handle)
                    .map(|f| f.samples.len() as f64)
                    .unwrap_or(0.0)
            }
            "file_mem" => {
                // `file_mem(handle, dest, len)` — kopierar prover till minnet, från markören.
                let handle = eval!(&args[0]) as i32;
                let dest = eval!(&args[1]);
                let count = eval!(&args[2]).max(0.0) as usize;
                let start = self.address(dest, 0.0);
                let Some(file) = self.files.get_mut(&handle) else {
                    return Ok(0.0);
                };
                let take = count.min(file.samples.len().saturating_sub(file.cursor));
                let copy = file.samples[file.cursor..file.cursor + take].to_vec();
                file.cursor += take;
                for (i, value) in copy.iter().enumerate() {
                    if start + i < self.mem.len() {
                        self.mem[start + i] = *value as f64;
                    }
                }
                take as f64
            }
            "file_var" | "file_text" | "fread" | "fwrite" | "fseek" | "ftell" => {
                for arg in args {
                    self.eval(arg)?;
                }
                self.stubs.insert(name.to_string());
                0.0
            }
            "midi_get_evt" | "midi_set_evt" => 0.0,
            "strcmp" | "strncmp" | "stricmp" | "strnicmp" => {
                let a = self.string_at(&args[0]).to_lowercase();
                let b = self.string_at(&args[1]).to_lowercase();
                let n = args.get(2).map(|e| self.eval(e)).transpose()?.map(|v| v as usize);
                let (a, b) = match n {
                    Some(n) => (a.chars().take(n).collect::<String>(), b.chars().take(n).collect::<String>()),
                    None => (a, b),
                };
                match a.cmp(&b) {
                    std::cmp::Ordering::Less => -1.0,
                    std::cmp::Ordering::Equal => 0.0,
                    std::cmp::Ordering::Greater => 1.0,
                }
            }
            "strcat" | "strcpy_from" | "str_setchar" | "str_insert" | "str_delsub" | "str_setlen"
            | "strcpy_substr" | "get_pinmapping" | "set_pinmapping" => {
                for arg in args {
                    self.eval(arg)?;
                }
                0.0
            }
            "__memtop" => {
                for arg in args {
                    self.eval(arg)?;
                }
                (MEM_START / 2) as f64
            }
            "ifft_real" | "imdct" | "convolve_c" | "sleep" | "get_host_numchan" => {
                for arg in args {
                    self.eval(arg)?;
                }
                self.stubs.insert(name.to_string());
                0.0
            }
            "tanh" => eval!(&args[0]).tanh(),
            "slider" => {
                let index = eval!(&args[0]).max(0.0) as usize;
                self.mem.get(index).copied().unwrap_or(0.0)
            }
            // Allt gfx: UI:t ritas inte, men koden ska kunna köra utan att stanna.
            other if other.starts_with("gfx_") => {
                for arg in args {
                    self.eval(arg)?;
                }
                0.0
            }
            other => return Err(format!("okänd funktion '{other}'")),
        })
    }

    fn string_at(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Str(s) => s.clone(),
            other => {
                let index = self.eval(other).unwrap_or(0.0);
                self.string(index).to_string()
            }
        }
    }

    fn psuedo_random(&mut self) -> f64 {
        let slot = self.slot("__rand");
        let mut state = self.mem[slot].to_bits() ^ 0x2545_f491_4f6c_dd1d;
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        self.mem[slot] = f64::from_bits(state | 0x3ff0_0000_0000_0000) - 1.0;
        self.mem[slot].abs().fract()
    }

    fn heap_top(&mut self) -> usize {
        let slot = self.slot("__heap");
        self.mem[slot].max(4096.0) as usize
    }

    fn set_heap_top(&mut self, value: usize) {
        let slot = self.slot("__heap");
        self.mem[slot] = value as f64;
    }

    /// Real-FFT framåt: `size` reella prov in, `size/2` komplexa bin ut (packade re,im).
    /// Nyquist-bin:en skrivs inte — den ligger utanför den plats REAPER-filerna reserverar.
    fn fft_real_forward(&mut self, start: usize, size: usize) {
        use rustfft::num_complex::Complex;
        if start + size > self.mem.len() {
            return;
        }
        let mut buf: Vec<Complex<f32>> = (0..size)
            .map(|i| Complex::new(self.mem[start + i] as f32, 0.0))
            .collect();
        let mut planner = rustfft::FftPlanner::<f32>::new();
        planner.plan_fft_forward(size).process(&mut buf);
        let half = size / 2;
        for bin in 0..half {
            let value = buf[bin];
            self.mem[start + 2 * bin] = value.re as f64;
            self.mem[start + 2 * bin + 1] = value.im as f64;
        }
    }

    /// Real-FFT bakåt: `size/2` bin in, `size` reella prov ut.
    fn ifft_real(&mut self, start: usize, size: usize) {
        use rustfft::num_complex::Complex;
        if start + size > self.mem.len() {
            return;
        }
        let half = size / 2;
        let mut buf: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); size];
        for bin in 0..half {
            buf[bin] = Complex::new(self.mem[start + 2 * bin] as f32, self.mem[start + 2 * bin + 1] as f32);
        }
        // Hermiteskt spektrum: bin k och size-k är varandras konjugat.
        for bin in 1..half {
            buf[size - bin] = buf[bin].conj();
        }
        let mut planner = rustfft::FftPlanner::<f32>::new();
        planner.plan_fft_inverse(size).process(&mut buf);
        let norm = 1.0 / size as f32;
        for i in 0..size {
            self.mem[start + i] = (buf[i].re * norm) as f64;
        }
    }

    /// Komplex FFT över `size` par `(re, im)` som ligger på `base[0..2*size]` i minnet.
    fn fft_in_place(&mut self, base: f64, size: usize, inverse: bool) {
        use rustfft::num_complex::Complex;
        let start = self.address(base, 0.0);
        if start + 2 * size > self.mem.len() {
            return;
        }
        let mut buf: Vec<Complex<f32>> = (0..size)
            .map(|i| Complex::new(self.mem[start + 2 * i] as f32, self.mem[start + 2 * i + 1] as f32))
            .collect();
        let mut planner = rustfft::FftPlanner::<f32>::new();
        let fft = if inverse {
            planner.plan_fft_inverse(size)
        } else {
            planner.plan_fft_forward(size)
        };
        fft.process(&mut buf);
        let norm = if inverse { 1.0 / size as f32 } else { 1.0 };
        for (i, value) in buf.iter().enumerate() {
            self.mem[start + 2 * i] = (value.re * norm) as f64;
            self.mem[start + 2 * i + 1] = (value.im * norm) as f64;
        }
    }
}

/// En öppnad fil i JSFX-minnet. Bara det `file_riff`/`file_avail`/`file_mem` behöver.
struct OpenFile {
    samples: Vec<f32>,
    channels: usize,
    rate: f64,
    cursor: usize,
}

fn finite_or_zero(value: f64) -> f32 {
    if value.is_finite() {
        value as f32
    } else {
        0.0
    }
}

fn memset_zero(slice: &mut [f64]) {
    for cell in slice {
        *cell = 0.0;
    }
}

fn bool_num(value: bool) -> f64 {
    if value { 1.0 } else { 0.0 }
}

fn to_int(value: f64) -> i64 {
    value as i64
}

/// `sprintf` med de format REAPER-filerna använder: `%d`, `%f`, `%.Nf`, `%s`, `%x`, `%e`.
fn format_string(format: &str, values: &[f64]) -> String {
    let mut out = String::new();
    let mut chars = format.chars().peekable();
    let mut value_index = 0;
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        // Flaggor och bredd (%02d, %.2f)
        let mut spec = String::from("%");
        while let Some(&next) = chars.peek() {
            if next == '%' {
                chars.next();
                out.push('%');
                spec.clear();
                break;
            }
            spec.push(next);
            chars.next();
            if next.is_ascii_alphabetic() {
                let value = values.get(value_index).copied().unwrap_or(0.0);
                value_index += 1;
                match next {
                    'd' | 'i' => out.push_str(&(value as i64).to_string()),
                    'x' => out.push_str(&format!("{:x}", value as i64)),
                    'X' => out.push_str(&format!("{:X}", value as i64)),
                    'f' | 'F' => {
                        let precision = spec
                            .split('.')
                            .nth(1)
                            .and_then(|p| p.trim_end_matches('f').parse::<usize>().ok())
                            .unwrap_or(6);
                        out.push_str(&format!("{value:.precision$}"));
                    }
                    'e' | 'E' => out.push_str(&format!("{value:e}")),
                    'g' => out.push_str(&format!("{value}")),
                    'c' => out.push((value as u8) as char),
                    's' => out.push_str(&(value as i64).to_string()),
                    _ => out.push_str(&value.to_string()),
                }
                spec.clear();
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod engine_tests {
    use super::*;

    fn run(src: &str, frames: usize) -> Vec<f32> {
        let mut vm = Jsfx::compile(src, 44_100.0).expect("kompilerar");
        let mut left = vec![1.0f32; frames];
        let mut right = vec![1.0f32; frames];
        vm.process(&mut left, &mut right, &[]).expect("kör");
        left
    }

    #[test]
    fn a_gain_effect_scales_the_signal() {
        let out = run("@sample\nspl0 = spl0 * 0.25;\nspl1 = spl1 * 0.25;\n", 8);
        assert!(out.iter().all(|v| (v - 0.25).abs() < 1e-9), "{out:?}");
    }

    #[test]
    fn init_slider_and_block_run_in_order() {
        let src = "@init\nk = 2;\n@slider\ng = slider1 * k;\n@block\nn = samplesblock;\n@sample\nspl0 = spl0 * g * n;\n";
        let mut vm = Jsfx::compile(src, 48_000.0).unwrap();
        vm.set_slider(1, 0.5).unwrap();
        let mut left = vec![1.0f32; 4];
        let mut right = vec![1.0f32; 4];
        vm.process(&mut left, &mut right, &[]).unwrap();
        // g = 0.5*2 = 1, n = 4 → 4.0 per prov.
        assert!(left.iter().all(|v| (v - 4.0).abs() < 1e-9), "{left:?}");
    }

    #[test]
    fn loops_functions_and_memory_work() {
        // Summerar 1..=10 genom en array i minnet, via en egen funktion.
        let src = "@init\nbuf = 4096;\nfunction total(n) local(i) ( t = 0; loop(n, ( i += 1; t += i; )); t );\n@sample\n\
                   memset(buf, 0, 4);\ni = 1; loop(10, ( buf[0] += i; i += 1; ));\n\
                   spl0 = spl0 + total(3);\n";
        let mut vm = Jsfx::compile(src, 44_100.0).unwrap();
        let mut left = vec![0.0f32; 1];
        let mut right = vec![0.0f32; 1];
        vm.process(&mut left, &mut right, &[]).unwrap();
        // total(3) = 1+2+3 = 6
        assert!((left[0] - 6.0).abs() < 1e-9, "{:?}", left[0]);
    }

    #[test]
    fn an_endless_loop_is_stopped_instead_of_hanging() {
        let src = "@sample\nwhile (1) ( x += 1; );\n";
        let mut vm = Jsfx::compile(src, 44_100.0).unwrap();
        let mut left = vec![1.0f32; 1];
        let mut right = vec![1.0f32; 1];
        vm.process(&mut left, &mut right, &[]).unwrap();
        assert!(!vm.errors.is_empty(), "oändliga loopar ska ge ett fel");
        assert_eq!(left[0], 0.0, "effekten tystnar i stället för att hänga");
    }

    #[test]
    fn mem_set_values_writes_a_table() {
        let out = run("@sample\nmem_set_values(4096, 0.25, 0.5);\nspl0 = mem[4097];\n", 2);
        assert!(out.iter().all(|v| (v - 0.5).abs() < 1e-9), "{out:?}");
    }

    #[test]
    fn an_opened_wav_hands_out_its_samples() {
        // En riktig wav i en temporär mapp: `file_open` letar i källfilens mapp, som i REAPER.
        let dir = std::env::temp_dir().join(format!("sonix-jsfx-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("t.wav");
        crate::audio::wav_writer::write_pcm_f32_to_wav(
            &wav.display().to_string(),
            &[0.25, 0.25, 0.5, 0.5, -0.25, -0.25, 0.0, 0.0],
            44_100,
            2,
        )
        .unwrap();

        let src = "@init\nh = file_open(\"t.wav\");\nn = 0;\nh >= 0 ? ( nch = 0; sr = 0; file_riff(h, nch, sr); n = file_mem(h, 4096, 8); );\n@sample\nspl0 = n;\n";
        let mut vm = Jsfx::compile_in(src, dir.clone(), 44_100.0).unwrap();
        let mut left = vec![0.0f32; 2];
        let mut right = vec![0.0f32; 2];
        vm.process(&mut left, &mut right, &[]).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        // Åtta prover lästes ur filen (stereo, fyra ramar) — inte noll, som en stubbad file_open ger.
        assert!((left[0] - 8.0).abs() < 1e-6, "file_mem läste {}", left[0]);
    }

    #[test]
    fn midi_is_received_and_sent() {
        let src = "@block\nwhile (midirecv(off, msg1, msg23)) ( midisend(off, msg1, msg23); );\n@sample\n";
        let mut vm = Jsfx::compile(src, 44_100.0).unwrap();
        let mut left = vec![0.0f32; 4];
        let mut right = vec![0.0f32; 4];
        let out = vm
            .process(
                &mut left,
                &mut right,
                &[MidiEvent {
                    offset: 0,
                    status: 0x90,
                    data1: 60,
                    data2: 100,
                }],
            )
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data1, 60);
    }
}
