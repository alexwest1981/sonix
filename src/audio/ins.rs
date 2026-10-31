//! **`.ins` instrument definitions** — names for the patches of an external synth, so a
//! program number can be shown as `Bank A, 12 Soft Pad` instead of `12`.
//!
//! Read here: the instrument header (`[Roland JV-1080]`), the `.Patch Name` marker, bank
//! headers (`[Bank A]`) and `number=Name` rows. Everything else is skipped on purpose —
//! `;` comments, the format's other `.`-directives and `Patch[*]=` controller settings are
//! editor settings, not patch names, and a file that carries them is not a broken file.

/// One instrument definition: its name and its banks, in file order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InsInstrument {
    pub name: String,
    pub banks: Vec<InsBank>,
}

/// One bank: the header name and `(program number, patch name)` rows, in file order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InsBank {
    pub name: String,
    pub patches: Vec<(u8, String)>,
}

/// Reads an `.ins` file. The `Err` carries the line that could not be placed, because a
/// file picked by mistake (a `.mid`, a listing with no banks) must say why, not load empty.
pub fn load(path: &str) -> Result<InsInstrument, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("could not read {path}: {e}"))?;
    parse(&text).map_err(|e| format!("{path}: {e}"))
}

fn parse(text: &str) -> Result<InsInstrument, String> {
    let mut ins = InsInstrument::default();
    for (number, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let header = header.trim();
            // The first header is the instrument; every later one opens a bank.
            if ins.name.is_empty() {
                ins.name = header.to_string();
            } else {
                ins.banks.push(InsBank { name: header.to_string(), patches: Vec::new() });
            }
            continue;
        }
        // `.Patch Name`, `.Controller Name`, `.Ins`, … — section markers, not data.
        if line.starts_with('.') {
            continue;
        }
        let Some((key, name)) = line.split_once('=') else {
            continue;
        };
        // `Patch[*]=`, `Key[36]=` and other settings are not patch rows.
        let Ok(program) = key.trim().parse::<u8>() else {
            continue;
        };
        let Some(bank) = ins.banks.last_mut() else {
            return Err(format!(
                "line {}: patch '{}' has no [Bank] section above it",
                number + 1,
                line
            ));
        };
        bank.patches.push((program, name.trim().to_string()));
    }
    if ins.name.is_empty() {
        return Err("no [instrument] section".to_string());
    }
    Ok(ins)
}

/// The patch name for `program` in bank number `bank` (0 = the first bank in the file).
pub fn program_name(ins: &InsInstrument, bank: u8, program: u8) -> Option<&str> {
    ins.banks
        .get(bank as usize)?
        .patches
        .iter()
        .find(|(number, _)| *number == program)
        .map(|(_, name)| name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(tag: &str, body: &str) -> String {
        let path =
            std::env::temp_dir().join(format!("sonix-ins-{tag}-{}.ins", std::process::id()));
        std::fs::write(&path, body).expect("fixture written");
        path.to_string_lossy().into_owned()
    }

    /// Two banks with three and two patches, plus the markers a real file carries.
    const TWO_BANKS: &str = "\
; written by the test, not by a synth
[Test Synth X]
.Patch Name
.Controller Name
[Bank A]
0=Piano
7=Strings
12=Soft Pad
[Bank B]
0=Bass
3=Organ
";

    #[test]
    fn two_banks_parse_into_two_banks_and_five_patches() {
        let ins = load(&fixture("banks", TWO_BANKS)).expect("the fixture loads");
        println!("instrument: {}", ins.name);
        for bank in &ins.banks {
            for (program, name) in &bank.patches {
                println!("  {} [{program}] = {name}", bank.name);
            }
        }
        let total: usize = ins.banks.iter().map(|b| b.patches.len()).sum();
        println!("{} banks, {total} patches", ins.banks.len());
        assert_eq!(ins.name, "Test Synth X");
        assert_eq!(ins.banks.len(), 2);
        assert_eq!(ins.banks[0].patches.len(), 3);
        assert_eq!(ins.banks[1].patches.len(), 2);
        assert_eq!(total, 5);
    }

    #[test]
    fn program_name_finds_bank_zero_program_twelve() {
        let ins = load(&fixture("names", TWO_BANKS)).expect("the fixture loads");
        let hit = program_name(&ins, 0, 12);
        let miss = program_name(&ins, 0, 99);
        println!("bank 0 program 12 -> {hit:?}");
        println!("bank 0 program 99 -> {miss:?}");
        assert_eq!(hit, Some("Soft Pad"));
        assert_eq!(miss, None);
    }

    #[test]
    fn comments_and_unknown_directives_are_skipped_without_an_error() {
        let body = "\
; header comment
.Ins
[Test Synth X]
.Patch Name
[Bank A]
; 99=Commented Out
Patch[*]=NRPN 0 0
0=Piano
";
        let ins = load(&fixture("skips", body)).expect("unknown lines are not an error");
        println!("{ins:#?}");
        assert_eq!(ins.name, "Test Synth X");
        assert_eq!(ins.banks.len(), 1);
        assert_eq!(ins.banks[0].patches, vec![(0, "Piano".to_string())]);
    }

    #[test]
    fn a_patch_row_before_any_bank_is_a_clear_error() {
        let body = "[Test Synth X]\n.Patch Name\n12=Soft Pad\n";
        let err = load(&fixture("malformed", body)).expect_err("no bank, so no home for the row");
        println!("error: {err}");
        assert!(err.contains("line 3"), "{err}");
        assert!(err.contains("12=Soft Pad"), "{err}");
        assert!(err.contains("[Bank]"), "{err}");
    }
}
