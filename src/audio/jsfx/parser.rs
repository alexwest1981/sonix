//! **JSFX: lexer och parser.** EEL2 är REAPERs lilla språk; JSFX-filerna är källkoden till
//! effekterna och instrumenten som skeppas med REAPER (265 filer, ~80 000 rader i 7.83).
//!
//! Vi porterar dem inte en och en — vi tolkar dem. Samma val som gjorde att en enda
//! `PatternClip` räckte i stället för en klippa per stämma: **en motor i stället för 265 portar**,
//! och den som skriver egen JSFX får den att köra direkt.
//!
//! Här ligger bara syntaxen. Semantiken (variabler, minne, inbyggda funktioner, sektioner) ligger i
//! `engine.rs`.

/// Ett tecken som JSFX-koden använder, med radnummer för felmeddelanden.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: Kind,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Num(f64),
    /// Strängliteral. Index i strängtabellen, satt av parsern.
    Str(String),
    /// `identifierare` (gemener — EEL2 skiljer inte på skiftläge).
    Ident(String),
    /// `#textvariabel` (gemener, utan `#`).
    StrIdent(String),
    Op(&'static str),
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Semi,
    Eof,
}

/// Ett uttryck. Boxat där det behövs, annars kopieras det billigt.
#[derive(Debug, Clone)]
pub enum Expr {
    Num(f64),
    Str(String),
    Var(String),
    StrVar(String),
    Index(Box<Expr>, Box<Expr>),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Assign(Box<Expr>, AssignOp, Box<Expr>),
    /// `x++`, `--x`: mål, delta, prefix?
    IncDec(Box<Expr>, f64, bool),
    Call(String, Vec<Expr>),
    /// Ett parentesblock där ett uttryck väntas: `x = ( a = 1; a * 2; )`. Värdet är det sista
    /// påståendets värde — samma regel som för en funktionskropp.
    Seq(Vec<Stmt>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssignOp {
    Set,
    BitXorAssign,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BitOrAssign,
    BitAndAssign,
    ShlAssign,
    ShrAssign,
    Pow,
}

/// Ett påstående.
#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(Expr),
    Block(Vec<Stmt>),
    If {
        cond: Expr,
        then: Box<Stmt>,
        otherwise: Option<Box<Stmt>>,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    Loop {
        count: Expr,
        body: Box<Stmt>,
    },
    /// `function namn(a, b) local(x) instance(y) ( kropp )`
    Function {
        name: String,
        params: Vec<String>,
        locals: Vec<String>,
        /// `instance(...)`-namn hör till anropets namnrymd (`this.`), inte till anropet.
        instances: Vec<String>,
        body: Vec<Stmt>,
    },
    /// `local(...)`, `instance(...)`, `globals(...)`, `stack(...)` — deklarationer som bara
    /// påverkar namnrymden. Vi behandlar dem som no-ops och låter namnen bli globala.
    Declare,
    Empty,
}

/// Slider-metadata ur `sliderN:default<min,max,steg{enum}>namn`.
#[derive(Debug, Clone, PartialEq)]
pub struct SliderDef {
    pub index: usize,
    pub default: f64,
    pub min: f64,
    pub max: f64,
    pub name: String,
    /// Enum-etiketter om sliden är en `{a,b,c}`-lista.
    pub choices: Vec<String>,
}

/// En färdigparsad JSFX-fil.
#[derive(Debug, Default, Clone)]
pub struct Program {
    pub desc: String,
    pub sliders: Vec<SliderDef>,
    pub in_pins: usize,
    pub out_pins: usize,
    pub init: Vec<Stmt>,
    pub slider: Vec<Stmt>,
    pub block: Vec<Stmt>,
    pub sample: Vec<Stmt>,
    /// `@serialize` finns i filen men körs inte (vi sparar inget tillstånd än).
    pub has_serialize: bool,
    /// `@gfx` finns i filen men ritas inte (UI:t är REAPERs, inte vårt).
    pub has_gfx: bool,
    pub functions: Vec<Stmt>,
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Kind {
        &self.tokens[self.pos.min(self.tokens.len() - 1)].kind
    }

    fn line(&self) -> usize {
        self.tokens[self.pos.min(self.tokens.len() - 1)].line
    }

    fn bump(&mut self) -> Kind {
        let k = self.peek().clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        k
    }

    fn eat(&mut self, kind: &Kind) -> bool {
        if self.peek() == kind {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: Kind, what: &str) -> Result<(), String> {
        if self.eat(&kind) {
            Ok(())
        } else {
            Err(format!(
                "rad {}: väntade {what}, fick {:?}",
                self.line(),
                self.peek()
            ))
        }
    }

    fn ident(&mut self) -> Result<String, String> {
        match self.bump() {
            Kind::Ident(name) => Ok(name),
            other => Err(format!("rad {}: väntade namn, fick {other:?}", self.line())),
        }
    }

    /// Läser `( ... )` som en grupp påståenden. Värdet är det sista uttryckets värde; en grupp
    /// med ett enda uttryck blir det uttrycket (så AST:n hålls ren för vanliga fall).
    fn paren_group(&mut self, what: &str) -> Result<Expr, String> {
        self.expect(Kind::LParen, &format!("'(' före {what}"))?;
        let mut body = Vec::new();
        while self.peek() != &Kind::RParen && self.peek() != &Kind::Eof {
            body.push(self.statement()?);
        }
        self.expect(Kind::RParen, &format!("')' efter {what}"))?;
        // Ett avslutande `;` ger ett tomt påstående — det ska inte bli värdet.
        body.retain(|stmt| !matches!(stmt, Stmt::Empty));
        match body.len() {
            0 => Ok(Expr::Num(0.0)),
            1 => match body.remove(0) {
                Stmt::Expr(expr) => Ok(expr),
                other => Ok(Expr::Seq(vec![other])),
            },
            _ => Ok(Expr::Seq(body)),
        }
    }

    /// `( x += 1; )` är ett block, `(a + b) * 2` är ett uttryck. EEL2 använder samma parenteser
    /// för båda, så vi skiljer dem på om det finns ett `;` på djup 0 innan den matchande `)`.
    fn paren_holds_statements(&self) -> bool {
        if self.peek() != &Kind::LParen {
            return false;
        }
        let mut depth = 0usize;
        for token in &self.tokens[self.pos..] {
            match token.kind {
                Kind::LParen => depth += 1,
                Kind::RParen => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return false;
                    }
                }
                Kind::Semi if depth == 1 => return true,
                Kind::Eof => return false,
                _ => {}
            }
        }
        false
    }

    /// Ett påstående. `{ ... }` är ett block, `if`/`while`/`loop` har egen form.
    pub fn statement(&mut self) -> Result<Stmt, String> {
        if self.eat(&Kind::Semi) {
            return Ok(Stmt::Empty);
        }
        if self.peek() == &Kind::LParen && self.paren_holds_statements() {
            self.bump();
            let mut body = Vec::new();
            while self.peek() != &Kind::RParen && self.peek() != &Kind::Eof {
                body.push(self.statement()?);
            }
            self.expect(Kind::RParen, "')' efter block")?;
            return Ok(Stmt::Block(body));
        }
        if self.peek() == &Kind::LBrace {
            self.bump();
            let mut body = Vec::new();
            while self.peek() != &Kind::RBrace && self.peek() != &Kind::Eof {
                body.push(self.statement()?);
            }
            self.expect(Kind::RBrace, "'}'")?;
            return Ok(Stmt::Block(body));
        }
        if let Kind::Ident(word) = self.peek().clone() {
            match word.as_str() {
                "if" => {
                    self.bump();
                    let cond = self.paren_group("if-villkor")?;
                    let then = Box::new(self.statement()?);
                    let otherwise = if matches!(self.peek(), Kind::Ident(w) if w == "else") {
                        self.bump();
                        Some(Box::new(self.statement()?))
                    } else {
                        None
                    };
                    return Ok(Stmt::If {
                        cond,
                        then,
                        otherwise,
                    });
                }
                "while" => {
                    self.bump();
                    // **Två former finns.** `while (villkor) kropp`, och REAPER:s egen:
                    // `while ( kropp...; villkor )` där sista uttrycket är testet och det inte
                    // finns någon kropp efter parentesen. Vi skiljer dem på om gruppen har flera
                    // påståenden.
                    let cond = self.paren_group("while-villkor")?;
                    let multi = matches!(&cond, Expr::Seq(list) if list.len() > 1);
                    let body = if multi || !matches!(self.peek(), Kind::LParen | Kind::LBrace | Kind::Ident(_)) {
                        Box::new(Stmt::Empty)
                    } else {
                        Box::new(self.statement()?)
                    };
                    return Ok(Stmt::While { cond, body });
                }
                "loop" => {
                    self.bump();
                    self.expect(Kind::LParen, "'(' efter loop")?;
                    // `loop(i = 0; 128, kropp)`: initiering(ar) före `;`, antal efter.
                    let mut init = Vec::new();
                    let count = loop {
                        let expr = self.expression()?;
                        if self.eat(&Kind::Semi) {
                            init.push(Stmt::Expr(expr));
                            continue;
                        }
                        break expr;
                    };
                    let count = if init.is_empty() {
                        count
                    } else {
                        // Initieringen körs en gång, före varven.
                        init.push(Stmt::Expr(count));
                        Expr::Seq(init)
                    };
                    // `loop(count, kropp)`: kroppen är ett påstående, ofta flera
                    // (`loop(n, a = 0; b = 1;)` utan klamrar).
                    let body = if self.eat(&Kind::Comma) {
                        let mut body = Vec::new();
                        while self.peek() != &Kind::RParen && self.peek() != &Kind::Eof {
                            body.push(self.statement()?);
                        }
                        Box::new(Stmt::Block(body))
                    } else {
                        Box::new(self.statement()?)
                    };
                    self.expect(Kind::RParen, "')' efter loop")?;
                    return Ok(Stmt::Loop { count, body });
                }
                "function" => {
                    self.bump();
                    let name = self.ident()?;
                    self.expect(Kind::LParen, "'(' efter funktionsnamn")?;
                    let mut params = Vec::new();
                    while self.peek() != &Kind::RParen {
                        params.push(self.ident()?);
                        // `o*` markerar en ut-parameter i några portar; stjärnan hör inte till namnet.
                        self.eat(&Kind::Op("*"));
                        if !self.eat(&Kind::Comma) {
                            break;
                        }
                    }
                    self.expect(Kind::RParen, "')' efter parametrar")?;
                    // `local(x, y)` / `instance(...)` / `global(...)` mellan header och kropp.
                    let mut locals = Vec::new();
                    let mut instances = Vec::new();
                    while let Kind::Ident(word) = self.peek().clone() {
                        if matches!(word.as_str(), "local" | "instance" | "globals" | "global") {
                            let is_instance = word == "instance";
                            self.bump();
                            self.expect(Kind::LParen, "'(' efter deklaration")?;
                            while matches!(self.peek(), Kind::Ident(_)) {
                                let name = self.ident()?;
                                if is_instance {
                                    instances.push(name);
                                } else {
                                    locals.push(name);
                                }
                                self.eat(&Kind::Op("*"));
                                self.eat(&Kind::Comma);
                            }
                            self.expect(Kind::RParen, "')' efter deklaration")?;
                        } else {
                            break;
                        }
                    }
                    let body = match self.statement()? {
                        Stmt::Block(body) => body,
                        other => vec![other],
                    };
                    // `function f() ( ... );` — semikolonet efter kroppen hör till definitionen.
                    self.eat(&Kind::Semi);
                    return Ok(Stmt::Function {
                        name,
                        params,
                        locals,
                        instances,
                        body,
                    });
                }
                "local" | "instance" | "globals" | "global" | "stack" => {
                    self.bump();
                    // Namnen i deklarationen blir vanliga variabler i vår tolk; de behöver bara
                    // läsas förbi.
                    if self.eat(&Kind::LParen) {
                        // **Namnen kan separeras med blanksteg**, inte bara kommatecken:
                        // `instance(buf suml sumr cnt ptr size maxsz)`.
                        while matches!(self.peek(), Kind::Ident(_)) {
                            self.ident()?;
                            self.eat(&Kind::Op("*"));
                            self.eat(&Kind::Comma);
                        }
                        self.expect(Kind::RParen, "')' efter deklaration")?;
                    }
                    return Ok(Stmt::Declare);
                }
                _ => {}
            }
        }
        let expr = self.expression()?;
        self.eat(&Kind::Semi);
        Ok(Stmt::Expr(expr))
    }

    pub fn expression(&mut self) -> Result<Expr, String> {
        self.assignment()
    }

    fn assignment(&mut self) -> Result<Expr, String> {
        let left = self.ternary()?;
        let op = match self.peek() {
            Kind::Op("=") => AssignOp::Set,
            Kind::Op("+=") => AssignOp::Add,
            Kind::Op("-=") => AssignOp::Sub,
            Kind::Op("*=") => AssignOp::Mul,
            Kind::Op("/=") => AssignOp::Div,
            Kind::Op("%=") => AssignOp::Mod,
            Kind::Op("|=") => AssignOp::BitOrAssign,
            Kind::Op("&=") => AssignOp::BitAndAssign,
            Kind::Op("<<=") => AssignOp::ShlAssign,
            Kind::Op(">>=") => AssignOp::ShrAssign,
            Kind::Op("^=") => AssignOp::Pow,
            Kind::Op("~=") => AssignOp::BitXorAssign,
            _ => return Ok(left),
        };
        self.bump();
        let right = self.assignment()?;
        Ok(Expr::Assign(Box::new(left), op, Box::new(right)))
    }

    fn ternary(&mut self) -> Result<Expr, String> {
        let cond = self.binary(0)?;
        if self.eat(&Kind::Op("?")) {
            let then = self.branch()?;
            // **`cond ? ( ... );` utan `:` är giltig EEL2** — annars är värdet 0. Stillwells
            // effekter skriver så hela tiden.
            let otherwise = if self.eat(&Kind::Op(":")) {
                self.branch()?
            } else {
                Expr::Num(0.0)
            };
            return Ok(Expr::Ternary(
                Box::new(cond),
                Box::new(then),
                Box::new(otherwise),
            ));
        }
        Ok(cond)
    }

    /// En gren i `?:` — ett uttryck, eller ett parentesblock med påståenden
    /// (`cond ? ( a; b; ) : ( c; )`, som Stillwells kompressorer skriver).
    fn branch(&mut self) -> Result<Expr, String> {
        if self.paren_holds_statements() {
            return self.paren_group("block");
        }
        // EEL2 låter en gren vara ett påstående: `cond ? while ( ... ) ( ... );` finns i REAPER:s
        // MIDI-effekter. Utan det här blir `while` ett funktionsanrop och koden faller.
        if matches!(self.peek(), Kind::Ident(w) if matches!(w.as_str(), "while" | "loop" | "if" | "local" | "instance" | "globals" | "global" | "stack")) {
            let stmt = self.statement()?;
            return Ok(Expr::Seq(vec![stmt]));
        }
        self.expression()
    }

    /// Precedens enligt EEL2: `||` lägst, sedan `&&`, `|`, `^`, `&`, jämförelser, skift, `+ -`, `* / %`.
    fn precedence(op: &str) -> Option<(BinOp, u8)> {
        Some(match op {
            "||" => (BinOp::Or, 1),
            "&&" => (BinOp::And, 2),
            "|" => (BinOp::BitOr, 3),
            "xor" => (BinOp::BitXor, 4),
            // `^` är potens (högsta precedens bland de binära), `xor` är bitvis.

            "&" => (BinOp::BitAnd, 5),
            "~" => (BinOp::BitXor, 5),
            "==" | "===" => (BinOp::Eq, 6),
            "!=" | "!==" => (BinOp::Ne, 6),
            "<" => (BinOp::Lt, 7),
            ">" => (BinOp::Gt, 7),
            "<=" => (BinOp::Le, 7),
            ">=" => (BinOp::Ge, 7),
            "<<" => (BinOp::Shl, 8),
            ">>" => (BinOp::Shr, 8),
            "+" => (BinOp::Add, 9),
            "-" => (BinOp::Sub, 9),
            "*" => (BinOp::Mul, 10),
            "/" => (BinOp::Div, 10),
            "%" => (BinOp::Mod, 10),
            "^" => (BinOp::Pow, 11),
            _ => return None,
        })
    }

    fn binary(&mut self, min_prec: u8) -> Result<Expr, String> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Kind::Op(op) => *op,
                Kind::Ident(word) if word == "xor" => "xor",
                _ => break,
            };
            let Some((binop, prec)) = Self::precedence(op) else {
                break;
            };
            if prec < min_prec {
                break;
            }
            self.bump();
            let right = self.binary(prec + 1)?;
            left = Expr::Binary(binop, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        match self.peek().clone() {
            Kind::Op("-") => {
                self.bump();
                Ok(Expr::Unary(UnOp::Neg, Box::new(self.unary()?)))
            }
            Kind::Op("+") => {
                self.bump();
                self.unary()
            }
            Kind::Op("!") => {
                self.bump();
                Ok(Expr::Unary(UnOp::Not, Box::new(self.unary()?)))
            }
            Kind::Op("~") => {
                self.bump();
                Ok(Expr::Unary(UnOp::BitNot, Box::new(self.unary()?)))
            }
            Kind::Op("++") | Kind::Op("--") => {
                let delta = if matches!(self.peek(), Kind::Op("++")) { 1.0 } else { -1.0 };
                self.bump();
                let target = self.postfix()?;
                Ok(Expr::IncDec(Box::new(target), delta, true))
            }
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.primary()?;
        loop {
            match self.peek() {
                Kind::LBracket => {
                    self.bump();
                    // `x[]` är EEL2-förkortning för `x[x]`.
                    let index = if self.peek() == &Kind::RBracket {
                        expr.clone()
                    } else {
                        self.expression()?
                    };
                    self.expect(Kind::RBracket, "']'")?;
                    expr = Expr::Index(Box::new(expr), Box::new(index));
                }
                Kind::Op("++") | Kind::Op("--") => {
                    let delta = if matches!(self.peek(), Kind::Op("++")) { 1.0 } else { -1.0 };
                    self.bump();
                    expr = Expr::IncDec(Box::new(expr), delta, false);
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        match self.bump() {
            Kind::Num(v) => Ok(Expr::Num(v)),
            Kind::Str(s) => Ok(Expr::Str(s)),
            Kind::StrIdent(name) => Ok(Expr::StrVar(name)),
            Kind::Ident(name) => {
                if self.eat(&Kind::LParen) {
                    let mut args = Vec::new();
                    while self.peek() != &Kind::RParen {
                        args.push(self.expression()?);
                        if !self.eat(&Kind::Comma) {
                            break;
                        }
                    }
                    self.expect(Kind::RParen, "')' efter argument")?;
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Var(name))
                }
            }
            Kind::LParen => {
                // `(` kan innehålla både ett uttryck och en lista påståenden
                // (`(a-b)<0 ? bufSize+1:0;`), så gruppläsaren används även här.
                self.pos -= 1;
                self.paren_group("uttryck")
            }
            other => Err(format!("rad {}: oväntat {other:?}", self.line())),
        }
    }
}

/// Läser en hel JSFX-fil till ett [`Program`].
///
/// Sektionerna läses som de står: `@init`, `@slider`, `@block`, `@sample`, `@gfx`, `@serialize`.
/// Filhuvudet (desc/slider/in_pin/out_pin) läses som metadata, inte som kod.
pub fn parse(source: &str) -> Result<Program, String> {
    let mut program = Program {
        in_pins: 2,
        out_pins: 2,
        ..Program::default()
    };
    let mut section = String::new();
    let mut buffer = String::new();
    // Raden i filen där sektionen börjar — felmeddelandet ska peka på filens rad, inte sektionens.
    let mut section_start = 1usize;
    let mut line_no = 0usize;

    let flush = |section: &str,
                 buffer: &mut String,
                 program: &mut Program,
                 section_start: usize|
     -> Result<(), String> {
        let text = buffer.trim();
        if text.is_empty() {
            buffer.clear();
            return Ok(());
        }
        let mut statements = parse_statements(text).map_err(|e| {
            // `rad N` i felet är sektionens rad; räkna om till filens.
            match e.strip_prefix("rad ") {
                Some(rest) => match rest.split_once(':') {
                    Some((n, msg)) => match n.trim().parse::<usize>() {
                        Ok(n) => format!("@{section} rad {}:{}", section_start + n - 1, msg),
                        Err(_) => format!("@{section}: {e}"),
                    },
                    None => format!("@{section}: {e}"),
                },
                None => format!("@{section}: {e}"),
            }
        })?;
        match section {
            "init" => program.init.append(&mut statements),
            "slider" => program.slider.append(&mut statements),
            "block" => program.block.append(&mut statements),
            "sample" => program.sample.append(&mut statements),
            "gfx" => program.has_gfx = true,
            "serialize" => program.has_serialize = true,
            _ => {}
        }
        buffer.clear();
        Ok(())
    };

    // `/* ... */` är inte EEL2, men REAPER-filerna använder det för att kommentera ut exempel.
    // Vi tar bort blocken men behåller radbrytningarna så radnumren stämmer.
    let source = strip_block_comments(source);
    // `<? ... ?>` är kodgenerator-block: de `printf`ar fram källkod och gör inget med ljudet.
    // Utan dem låter effekten likadant, och att tolka dem som kod vore att gissa.
    let source = strip_codegen_blocks(&source);
    for raw in source.lines() {
        line_no += 1;
        let line = raw.trim_end_matches('\r');
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix('@') {
            flush(&section, &mut buffer, &mut program, section_start)?;
            section_start = line_no;
            section = rest
                .split(|c: char| c.is_whitespace() || c == '/')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            continue;
        }
        // En funktion som står före första sektionen hör till `@init` — EEL2 hoistar den ändå.
        if section.is_empty() && trimmed.starts_with("function") {
            section = "init".to_string();
            section_start = line_no;
        }
        if section.is_empty() {
            // Filhuvud: metadata, inte kod.
            if let Some(rest) = trimmed.strip_prefix("desc:") {
                if !program.desc.is_empty() {
                    program.desc.push(' ');
                }
                program.desc.push_str(rest.trim());
            } else if let Some(def) = parse_slider_line(trimmed) {
                program.sliders.push(def);
            } else if trimmed.starts_with("in_pin:") {
                program.in_pins = program.in_pins.max(1);
            } else if trimmed.starts_with("out_pin:") {
                program.out_pins = program.out_pins.max(1);
            }
            continue;
        }
        if section == "gfx" || section == "serialize" {
            // UI- och tillståndskod ritas/sparas inte; den behöver inte ens tolkas.
            continue;
        }
        // **`//` inuti en sträng är inte en kommentar.** Klipp bara utanför citattecken.
        let mut clean = String::with_capacity(line.len());
        let mut in_string = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '"' {
                in_string = !in_string;
                clean.push(c);
                continue;
            }
            if !in_string && c == '/' && chars.peek() == Some(&'/') {
                break;
            }
            clean.push(c);
        }
        buffer.push_str(&clean);
        buffer.push('\n');
    }
    flush(&section, &mut buffer, &mut program, section_start)?;

    // Funktionsdefinitioner plockas ur sektionerna och hoistas: EEL2 tillåter anrop före
    // definitionen, och en funktion som ligger i `@init` ska gå att anropa från `@sample`.
    let mut hoisted = Vec::new();
    for list in [
        &mut program.init,
        &mut program.slider,
        &mut program.block,
        &mut program.sample,
    ] {
        let mut kept = Vec::with_capacity(list.len());
        for stmt in list.drain(..) {
            match stmt {
                Stmt::Function { .. } => hoisted.push(stmt),
                other => kept.push(other),
            }
        }
        *list = kept;
    }
    program.functions = hoisted;
    Ok(program)
}

pub fn parse_slider_header(line: &str) -> Option<SliderDef> {
    parse_slider_line(line)
}

/// Tar bort `/* ... */` men behåller radbrytningarna, så att radnummer i felmeddelanden stämmer.
fn strip_block_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_block = false;
    while let Some(c) = chars.next() {
        if in_block {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block = false;
            } else if c == '\n' {
                out.push('\n');
            }
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            // `//` går före: en rad som `//*******` är en radkommentar, inte `/*`.
            out.push(c);
            out.push(chars.next().unwrap_or('/'));
            while let Some(next) = chars.peek() {
                if *next == '\n' {
                    break;
                }
                out.push(chars.next().unwrap_or(' '));
            }
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block = true;
            continue;
        }
        out.push(c);
    }
    out
}

/// Tar bort `<? ... ?>` (kodgenerator-block) men behåller radbrytningarna.
fn strip_codegen_blocks(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_block = false;
    while let Some(c) = chars.next() {
        if in_block {
            if c == '?' && chars.peek() == Some(&'>') {
                chars.next();
                in_block = false;
                // Blocket stod där ett värde förväntades (`x = <? gen(); ?>;`) — noll är rätt
                // sorts tomhet: ett tal som inte påverkar ljudet.
                out.push('0');
            } else if c == '\n' {
                out.push('\n');
            }
            continue;
        }
        if c == '<' && chars.peek() == Some(&'?') {
            chars.next();
            in_block = true;
            continue;
        }
        out.push(c);
    }
    out
}

fn parse_slider_line(line: &str) -> Option<SliderDef> {
    let rest = line.strip_prefix("slider")?;
    let (index, rest) = rest.split_once(':')?;
    let index: usize = index.trim().parse().ok()?;
    // default<min,max,steg{enum}>namn
    let (default, rest) = rest.split_once('<')?;
    let (range, name) = rest.split_once('>')?;
    let mut parts = range.splitn(3, ',');
    let min: f64 = parts.next()?.trim().parse().ok()?;
    let max: f64 = parts.next()?.trim().parse().ok()?;
    let step = parts.next().unwrap_or("0").trim();
    let choices = if let (Some(start), Some(end)) = (step.find('{'), step.rfind('}')) {
        step[start + 1..end]
            .split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    Some(SliderDef {
        index,
        default: default.trim().parse().unwrap_or(0.0),
        min,
        max,
        name: name.trim().to_string(),
        choices,
    })
}

fn parse_statements(text: &str) -> Result<Vec<Stmt>, String> {
    let tokens = lex(text)?;
    let mut parser = Parser::new(tokens);
    let mut out = Vec::new();
    while parser.peek() != &Kind::Eof {
        out.push(parser.statement()?);
    }
    Ok(out)
}

/// Läser text till tokens.
pub fn lex(source: &str) -> Result<Vec<Token>, String> {
    let bytes: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1;
    while i < bytes.len() {
        let c = bytes[i];
        if c == '\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == '/' {
            while i < bytes.len() && bytes[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit())
        {
            let start = i;
            // `$x`/`0x` = hexadecimalt.
            let hex = c == '$' || (c == '0' && i + 1 < bytes.len() && (bytes[i + 1] == 'x' || bytes[i + 1] == 'X'));
            if hex {
                i += if c == '$' { 1 } else { 2 };
                while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
                    i += 1;
                }
                let digits: String = bytes[start..i].iter().collect();
                let digits = digits.trim_start_matches(['$', '0', 'x', 'X']);
                let value = u64::from_str_radix(digits, 16).unwrap_or(0) as f64;
                out.push(Token {
                    kind: Kind::Num(value),
                    line,
                });
                continue;
            }
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == '.') {
                i += 1;
            }
            // Exponent: 1e-3
            if i < bytes.len() && (bytes[i] == 'e' || bytes[i] == 'E') {
                let save = i;
                i += 1;
                if i < bytes.len() && (bytes[i] == '+' || bytes[i] == '-') {
                    i += 1;
                }
                if i < bytes.len() && bytes[i].is_ascii_digit() {
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                } else {
                    i = save;
                }
            }
            let text: String = bytes[start..i].iter().collect();
            let value: f64 = text.parse().map_err(|_| format!("rad {line}: talet '{text}'"))?;
            out.push(Token {
                kind: Kind::Num(value),
                line,
            });
            continue;
        }
        if c == '$' {
            // EEL2:s `$`-literaler: `$pi`, `$e`, `$x90` (hex) och `$'F'` (teckenkod).
            let mut j = i + 1;
            let mut name = String::new();
            while j < bytes.len() && (bytes[j].is_alphanumeric() || bytes[j] == '_') {
                name.push(bytes[j]);
                j += 1;
            }
            if name.is_empty() && j < bytes.len() && bytes[j] == '\'' {
                j += 1;
                if j < bytes.len() {
                    let ch = bytes[j];
                    j += 1;
                    if j < bytes.len() && bytes[j] == '\'' {
                        j += 1;
                    }
                    out.push(Token {
                        kind: Kind::Num(ch as u32 as f64),
                        line,
                    });
                    i = j;
                    continue;
                }
            }
            let lower = name.to_ascii_lowercase();
            let value = match lower.as_str() {
                "pi" => std::f64::consts::PI,
                "e" => std::f64::consts::E,
                other => {
                    let digits = other.trim_start_matches('x');
                    u64::from_str_radix(digits, 16).unwrap_or(0) as f64
                }
            };
            out.push(Token {
                kind: Kind::Num(value),
                line,
            });
            i = j.max(i + 1);
            continue;
        }
        if c == '"' || c == '\'' {
            // Enkla citattecken används också, mest för enstaka tecken.
            let quote = c;
            i += 1;
            let mut s = String::new();
            while i < bytes.len() && bytes[i] != quote {
                s.push(bytes[i]);
                i += 1;
            }
            i += 1; // stängande citat
            out.push(Token {
                kind: Kind::Str(s),
                line,
            });
            continue;
        }
        if c == '#' {
            i += 1;
            let start = i;
            while i < bytes.len() && (bytes[i].is_alphanumeric() || bytes[i] == '_') {
                i += 1;
            }
            let name: String = bytes[start..i].iter().collect::<String>().to_lowercase();
            out.push(Token {
                kind: Kind::StrIdent(name),
                line,
            });
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_alphanumeric() || bytes[i] == '_' || bytes[i] == '.') {
                i += 1;
            }
            let name: String = bytes[start..i].iter().collect::<String>().to_lowercase();
            out.push(Token {
                kind: Kind::Ident(name),
                line,
            });
            continue;
        }
        // Operatorer, längsta först.
        const OPS: [&str; 25] = [
            "!==", "===", "<<=", ">>=", "==", "!=", "<=", ">=", "&&", "||", "<<", ">>", "+=", "-=",
            "*=", "/=", "%=", "|=", "&=", "^=", "~=", "++", "--", "=", "+",
        ];
        let mut matched = None;
        for op in OPS {
            let len = op.chars().count();
            if i + len <= bytes.len() && bytes[i..i + len].iter().collect::<String>() == op {
                matched = Some(op);
                break;
            }
        }
        if let Some(op) = matched {
            i += op.chars().count();
            out.push(Token {
                kind: Kind::Op(op),
                line,
            });
            continue;
        }
        let kind = match c {
            '(' => Kind::LParen,
            ')' => Kind::RParen,
            '[' => Kind::LBracket,
            ']' => Kind::RBracket,
            '{' => Kind::LBrace,
            '}' => Kind::RBrace,
            ',' => Kind::Comma,
            ';' => Kind::Semi,
            '-' => Kind::Op("-"),
            '*' => Kind::Op("*"),
            '/' => Kind::Op("/"),
            '%' => Kind::Op("%"),
            '<' => Kind::Op("<"),
            '>' => Kind::Op(">"),
            '&' => Kind::Op("&"),
            '|' => Kind::Op("|"),
            '^' => Kind::Op("^"),
            '?' => Kind::Op("?"),
            ':' => Kind::Op(":"),
            '!' => Kind::Op("!"),
            '~' => Kind::Op("~"),
            other => return Err(format!("rad {line}: okänt tecken '{other}'")),
        };
        i += 1;
        out.push(Token { kind, line });
    }
    out.push(Token {
        kind: Kind::Eof,
        line,
    });
    Ok(out)
}

#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn a_slider_line_becomes_metadata() {
        let p = parse(
            "desc:Test\nslider2:5<0,9,1{Boom,Four,All}>Ratio\nslider1:0<-60,0,0.1>Threshold (dB)\n@init\nx=1;\n",
        )
        .unwrap();
        assert_eq!(p.desc, "Test");
        assert_eq!(p.sliders.len(), 2);
        let ratio = p.sliders.iter().find(|s| s.index == 2).unwrap();
        assert_eq!(ratio.default, 5.0);
        assert_eq!(ratio.min, 0.0);
        assert_eq!(ratio.max, 9.0);
        assert_eq!(ratio.choices, vec!["Boom", "Four", "All"]);
        assert_eq!(p.init.len(), 1);
    }

    #[test]
    fn precedence_and_tertiary_follow_eel2() {
        // 1 + 2 * 3 = 7, och ?: binder lösare än jämförelser.
        let tokens = lex("x = 1+2*3; y = x>6 ? 10 : 20;").unwrap();
        let mut parser = Parser::new(tokens);
        let _ = parser.statement().unwrap();
        assert!(matches!(parser.statement().unwrap(), Stmt::Expr(Expr::Assign(..))));
    }

    #[test]
    fn functions_are_hoisted_out_of_their_section() {
        let p = parse(
            "@init\nfunction gain(a, b) local(t) ( t = a*b; t );\nn = gain(2,3);\n@sample\nspl0 = spl0 * n;\n",
        )
        .unwrap();
        assert_eq!(p.functions.len(), 1, "funktionen ska hoistas");
        assert_eq!(p.init.len(), 1, "bara anropet kvar i @init");
        assert_eq!(p.sample.len(), 1);
    }

    #[test]
    fn comments_disappear_but_not_inside_strings() {
        let p = parse("@init\nx = 1; // kommentar\ndesc = \"a // b\";\n").unwrap();
        assert_eq!(p.init.len(), 2);
    }
}
