//! **Formel som modulationskälla** (FL Studio: Fruity Formula Controller). I stället för
//! ännu en rattkurva skriver användaren ett uttryck som räknas om per sampel:
//!
//! ```text
//! sin(t*2)*0.5
//! t % 1
//! lfo(2)+0.3
//! ```
//!
//! Språket är litet med flit: tal, `+ - * / %`, parenteser, unärt minus, variablerna
//! `t` (sekunder) och `n` (notnummer, 0 utan not) samt `sin cos tan abs min max floor
//! sqrt` och `lfo(hz)` (= `sin(2π·hz·t)`). Uttrycket tolkas **en gång** med
//! [`Formula::compile`]; sedan räknar [`Formula::eval`] bara på trädet, så ljudtråden
//! aldrig rör text. Resultatet kläms till −1…1. Ingen `eval`, inga beroenden.

// Utvärderaren kopplas in i modulationspanelen i en egen uppgift; tills dess är
// modulen oanvänd i binären och dead_code skulle varna för varje symbol.
#![allow(dead_code)]

use std::iter::Peekable;
use std::str::Chars;

/// Operatorerna. Bara de fem som faktiskt behövs — ingen potens, den går att skriva
/// som upprepad multiplikation.
#[derive(Debug, Clone, Copy)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

/// Funktionerna. `Min`/`Max` är de enda tvåställiga.
#[derive(Debug, Clone, Copy)]
enum Func {
    Sin,
    Cos,
    Tan,
    Abs,
    Floor,
    Sqrt,
    Lfo,
    Min,
    Max,
}

/// Uttrycksträdet. Ett andra argument räcker: `min`/`max` är ensamma om två.
#[derive(Debug, Clone)]
enum Ast {
    Num(f32),
    Time,
    Note,
    Neg(Box<Ast>),
    Bin(Op, Box<Ast>, Box<Ast>),
    Call(Func, Box<Ast>, Option<Box<Ast>>),
}

/// Ett färdigtolkat uttryck. `src` sparas för att panelen ska kunna visa formeln som
/// den skrevs, även efter att ett projekt laddats.
#[derive(Debug, Clone)]
pub struct Formula {
    src: String,
    ast: Ast,
}

impl Formula {
    /// Tolkar uttrycket en gång. Felet är en läsbar mening, aldrig en panik.
    pub fn compile(src: &str) -> Result<Formula, String> {
        let mut parser = Parser::new(src);
        let ast = parser.expr()?;
        if let Some(c) = parser.peek() {
            return Err(format!("oväntat tecken '{c}'"));
        }
        Ok(Formula { src: src.to_string(), ast })
    }

    /// Värdet vid tidpunkten `t` (sekunder) och noten `n` (0 utan not), klämt till
    /// −1…1. Division med noll mitt i uttrycket ger 0 — tyst, men inte panik.
    pub fn eval(&self, t: f32, n: f32) -> f32 {
        clamp(eval_ast(&self.ast, t, n).unwrap_or(0.0))
    }

    /// Formeln som den skrevs.
    pub fn src(&self) -> &str {
        &self.src
    }
}

/// Bekvämlighet för enstaka utvärderingar (förhandsvisning, tester): tolkar och räknar.
/// Per-sampel-vägen ska gå via [`Formula`] så att texten bara tolkas en gång.
pub fn eval(expr: &str, t: f32, n: f32) -> Option<f32> {
    let formula = Formula::compile(expr).ok()?;
    eval_ast(&formula.ast, t, n).map(clamp)
}

fn clamp(x: f32) -> f32 {
    // `sqrt(-1)` blir NaN; en NaN-kurva skulle tysta spåret, så den blir 0.
    if x.is_finite() { x.clamp(-1.0, 1.0) } else { 0.0 }
}

fn eval_ast(ast: &Ast, t: f32, n: f32) -> Option<f32> {
    Some(match ast {
        Ast::Num(v) => *v,
        Ast::Time => t,
        Ast::Note => n,
        Ast::Neg(a) => -eval_ast(a, t, n)?,
        Ast::Bin(op, a, b) => {
            let (a, b) = (eval_ast(a, t, n)?, eval_ast(b, t, n)?);
            match op {
                Op::Add => a + b,
                Op::Sub => a - b,
                Op::Mul => a * b,
                // `%` med noll ger NaN i Rust och `/` oändligt — båda räknas som fel.
                Op::Div | Op::Rem if b == 0.0 => return None,
                Op::Div => a / b,
                Op::Rem => a % b,
            }
        }
        Ast::Call(func, a, b) => {
            let x = eval_ast(a, t, n)?;
            match func {
                Func::Sin => x.sin(),
                Func::Cos => x.cos(),
                Func::Tan => x.tan(),
                Func::Abs => x.abs(),
                Func::Floor => x.floor(),
                Func::Sqrt => x.sqrt(),
                Func::Lfo => (std::f32::consts::TAU * x * t).sin(),
                Func::Min => x.min(eval_ast(b.as_deref()?, t, n)?),
                Func::Max => x.max(eval_ast(b.as_deref()?, t, n)?),
            }
        }
    })
}

/// Rekursiv nedstigning: `expr` → `term` → `unary` → `atom`, alltså vanlig
/// prioritetsordning (`* / %` binder hårdare än `+ -`).
struct Parser<'a> {
    it: Peekable<Chars<'a>>,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Parser { it: src.chars().peekable() }
    }

    /// Nästa tecken, med blanksteg överhoppade.
    fn peek(&mut self) -> Option<char> {
        while matches!(self.it.peek(), Some(c) if c.is_whitespace()) {
            self.it.next();
        }
        self.it.peek().copied()
    }

    fn eat(&mut self, want: char) -> bool {
        if self.peek() == Some(want) {
            self.it.next();
            true
        } else {
            false
        }
    }

    fn expr(&mut self) -> Result<Ast, String> {
        let mut lhs = self.term()?;
        loop {
            let op = match self.peek() {
                Some('+') => Op::Add,
                Some('-') => Op::Sub,
                _ => return Ok(lhs),
            };
            self.it.next();
            lhs = Ast::Bin(op, Box::new(lhs), Box::new(self.term()?));
        }
    }

    fn term(&mut self) -> Result<Ast, String> {
        let mut lhs = self.unary()?;
        loop {
            let op = match self.peek() {
                Some('*') => Op::Mul,
                Some('/') => Op::Div,
                Some('%') => Op::Rem,
                _ => return Ok(lhs),
            };
            self.it.next();
            lhs = Ast::Bin(op, Box::new(lhs), Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> Result<Ast, String> {
        if self.eat('-') {
            return Ok(Ast::Neg(Box::new(self.unary()?)));
        }
        if self.eat('+') {
            return self.unary();
        }
        self.atom()
    }

    fn atom(&mut self) -> Result<Ast, String> {
        match self.peek() {
            Some(c) if c.is_ascii_digit() || c == '.' => self.number(),
            Some(c) if c.is_ascii_alphabetic() => self.name(),
            Some('(') => {
                self.it.next();
                let inner = self.expr()?;
                if !self.eat(')') {
                    return Err("saknar ')'".to_string());
                }
                Ok(inner)
            }
            Some(c) => Err(format!("oväntat tecken '{c}'")),
            None => Err("uttrycket tar slut för tidigt".to_string()),
        }
    }

    fn number(&mut self) -> Result<Ast, String> {
        let mut text = String::new();
        while let Some(c) = self.peek() {
            if !(c.is_ascii_digit() || c == '.') {
                break;
            }
            text.push(c);
            self.it.next();
        }
        text.parse::<f32>().map(Ast::Num).map_err(|_| format!("ogiltigt tal: {text}"))
    }

    fn name(&mut self) -> Result<Ast, String> {
        let mut text = String::new();
        while let Some(c) = self.peek() {
            if !(c.is_ascii_alphanumeric() || c == '_') {
                break;
            }
            text.push(c);
            self.it.next();
        }
        match text.as_str() {
            "t" => return Ok(Ast::Time),
            "n" => return Ok(Ast::Note),
            _ => {}
        }
        // Ett namn utan parentes är en okänd variabel, inte ett anrop.
        if !self.eat('(') {
            return Err(format!("okänd variabel: {text}"));
        }
        let first = self.expr()?;
        let second = if self.eat(',') { Some(Box::new(self.expr()?)) } else { None };
        if !self.eat(')') {
            return Err(format!("{text}: saknar ')'"));
        }
        let func = match text.as_str() {
            "sin" => Func::Sin,
            "cos" => Func::Cos,
            "tan" => Func::Tan,
            "abs" => Func::Abs,
            "floor" => Func::Floor,
            "sqrt" => Func::Sqrt,
            "lfo" => Func::Lfo,
            "min" => Func::Min,
            "max" => Func::Max,
            _ => return Err(format!("okänd funktion: {text}")),
        };
        match (matches!(func, Func::Min | Func::Max), second) {
            (true, Some(b)) => Ok(Ast::Call(func, Box::new(first), Some(b))),
            (false, None) => Ok(Ast::Call(func, Box::new(first), None)),
            (true, None) => Err(format!("{text} behöver två argument")),
            (false, Some(_)) => Err(format!("{text} tar ett argument")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Otäckt (oklämt) värde — `eval` klämmer, så 7 blir 1 och går inte att mäta där.
    fn raw(expr: &str, t: f32, n: f32) -> Option<f32> {
        let formula = Formula::compile(expr).expect(expr);
        eval_ast(&formula.ast, t, n)
    }

    #[test]
    fn precedence_and_parentheses() {
        assert_eq!(raw("1+2*3", 0.0, 0.0), Some(7.0));
        assert_eq!(raw("(1+2)*3", 0.0, 0.0), Some(9.0));
        assert_eq!(raw("2*-3", 0.0, 0.0), Some(-6.0));
        assert_eq!(raw("3--2", 0.0, 0.0), Some(5.0));
        assert_eq!(raw("-2*3", 0.0, 0.0), Some(-6.0));
        assert_eq!(raw("7 % 4", 0.0, 0.0), Some(3.0));
        assert_eq!(raw("1.5 + .5", 0.0, 0.0), Some(2.0));
    }

    #[test]
    fn variables_and_functions() {
        assert_eq!(raw("t*2", 0.5, 0.0), Some(1.0));
        assert_eq!(raw("n + 1", 0.0, 60.0), Some(61.0));
        assert!((raw("sin(0.25)", 0.0, 0.0).unwrap() - 0.247_404).abs() < 1e-6);
        assert!((raw("cos(0)", 0.0, 0.0).unwrap() - 1.0).abs() < 1e-6);
        assert_eq!(raw("abs(-0.4)", 0.0, 0.0), Some(0.4));
        assert_eq!(raw("floor(2.9)", 0.0, 0.0), Some(2.0));
        assert_eq!(raw("sqrt(9)", 0.0, 0.0), Some(3.0));
        assert_eq!(raw("min(1, 4)", 0.0, 0.0), Some(1.0));
        assert_eq!(raw("max(1, 4)", 0.0, 0.0), Some(4.0));
        // Fel antal argument är ett fel, inte tyst 0.
        assert!(Formula::compile("min(1)").is_err());
        assert!(Formula::compile("sin(1,2)").is_err());
    }

    #[test]
    fn lfo_is_a_sine_on_the_clock() {
        // lfo(1) vid t = 0,25 s = sin(π/2) = 1.
        assert!((eval("lfo(1)", 0.25, 0.0).unwrap() - 1.0).abs() < 1e-6);
        // lfo(hz) är samma sak som sin(2π·hz·t).
        assert!((eval("lfo(2)+0.3", 0.0, 0.0).unwrap() - 0.3).abs() < 1e-6);
        let a = eval("lfo(2)", 0.07, 0.0).unwrap();
        let b = eval("sin(2*3.14159265*2*0.07)", 0.07, 0.0).unwrap();
        assert!((a - b).abs() < 1e-5, "lfo {a} mot sin {b}");
    }

    #[test]
    fn result_is_clamped() {
        assert_eq!(eval("t*2", 0.5, 0.0), Some(1.0));
        assert_eq!(eval("t*100", 0.5, 0.0), Some(1.0)); // kläms
        assert_eq!(eval("-t*100", 0.5, 0.0), Some(-1.0));
        assert_eq!(eval("n*2", 0.0, 127.0), Some(1.0));
        // NaN (sqrt av negativt) blir 0, inte tyst tystnad i spåret.
        assert_eq!(eval("sqrt(0-1)", 0.0, 0.0), Some(0.0));
    }

    #[test]
    fn division_by_zero_is_an_error_not_infinity() {
        assert_eq!(eval("1/0", 0.0, 0.0), None);
        assert_eq!(eval("1 % 0", 0.0, 0.0), None);
        assert_eq!(eval("t/(n-1)", 1.0, 1.0), None);
    }

    #[test]
    fn broken_text_is_an_error_never_a_panic() {
        for bad in ["", "(((", "2^", "sin(", "1 +", "1 + * 2", "foo(1)", "x", "min(1)", "sin(1,2)", "1)", ")", "."] {
            assert!(Formula::compile(bad).is_err(), "borde vara fel: {bad:?}");
            assert_eq!(eval(bad, 0.0, 0.0), None, "borde vara fel: {bad:?}");
        }
        // Tomma parenteser och bara operatorer får inte panika heller.
        assert!(Formula::compile("()").is_err());
        assert!(Formula::compile("*/").is_err());
    }

    #[test]
    fn src_survives_compilation() {
        let f = Formula::compile(" sin(t*2) * 0.5 ").unwrap();
        assert_eq!(f.src(), " sin(t*2) * 0.5 ");
    }

    #[test]
    fn compiled_formula_does_not_reparse() {
        // Tolka en gång, räkna 10 000 gånger: detta är per-sampel-vägen.
        let compile_start = std::time::Instant::now();
        let f = Formula::compile("sin(t*2)*0.5 + n*0.001 + lfo(3)*0.2").unwrap();
        let compile_us = compile_start.elapsed().as_secs_f64() * 1e6;

        // **Jämför två vägar i samma process**, inte mot en väggklocka: en absolut gräns
        // faller när maskinen är lastad (den här föll en gång på 11 ms), medan förhållandet
        // mellan "tolka en gång" och "tolka varje gång" står stilla oavsett last.
        let n = 5_000;
        let start = std::time::Instant::now();
        let mut sum = 0.0f32;
        for i in 0..n {
            sum += f.eval(i as f32 / 48_000.0, 60.0);
        }
        let compiled = start.elapsed();

        let start = std::time::Instant::now();
        for i in 0..n {
            let again = Formula::compile("sin(t*2)*0.5 + n*0.001 + lfo(3)*0.2").unwrap();
            sum += again.eval(i as f32 / 48_000.0, 60.0);
        }
        let reparsed = start.elapsed();
        println!(
            "kompilering: {compile_us:.1} µs, {n} eval: {compiled:?} mot {reparsed:?} med omtolkning, summa {sum}"
        );
        assert!(
            compiled * 2 < reparsed,
            "färdigtolkad formel ska vara minst dubbelt så snabb: {compiled:?} mot {reparsed:?}"
        );
        assert!(sum.is_finite());
    }
}
