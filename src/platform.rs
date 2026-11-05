//! **Plattformens egna sätt** (Fas 7.1) — det som skiljer sig utan att vara sökvägar.
//!
//! Sökvägar bor i `paths.rs` (och bara där). Det här är kommandon som *gör* något med dem:
//! att visa en mapp i filhanteraren. `xdg-open` finns inte på Windows, så knappen "visa
//! mappen" gjorde ingenting där — den här modulen ger rätt kommando per plattform, med
//! den rena delen prövad.
//!
//! **`explorer` avslutar med en felkod även när den lyckas.** Det är dokumenterat beteende:
//! `explorer.exe` returnerar 1 när den startar ett nytt fönster. Därför tittar anroparen på
//! om *starten* gick igenom, aldrig på slutkoden — annars hade en fungerande knapp
//! rapporterat fel.

use std::path::Path;
use std::process::Command;

/// Kommandot som visar en mapp i plattformens filhanterare. Ren funktion: den bygger bara
/// `(program, argument)`, den kör inget — därför går den att pröva för varje plattform.
pub fn file_manager_command(os: &str, dir: &Path) -> (String, Vec<String>) {
    let path = dir.display().to_string();
    match os {
        "windows" => ("explorer".to_string(), vec![path]),
        "macos" => ("open".to_string(), vec![path]),
        // Linux och BSD: XDG:s egen väg, som redan användes.
        _ => ("xdg-open".to_string(), vec![path]),
    }
}

/// Kommandot som öppnar en **adress** (inloggningssidan) i webbläsaren. Skilt från
/// filhanteraren eftersom `explorer` tar mappar men inte URL:er — Windows behöver `start`.
pub fn url_command(os: &str, url: &str) -> (String, Vec<String>) {
    match os {
        "windows" => ("cmd".to_string(), vec!["/c".to_string(), "start".to_string(), String::new(), url.to_string()]),
        "macos" => ("open".to_string(), vec![url.to_string()]),
        _ => ("xdg-open".to_string(), vec![url.to_string()]),
    }
}

/// Öppnar `url` i användarens webbläsare. `Err` betyder att **starten** misslyckades, och då
/// visar anroparen adressen i stället — en inloggning utan adress är en återvändsgränd.
pub fn open_url(url: &str) -> std::io::Result<()> {
    // **Provsviten får aldrig öppna en webbläsare.** Inloggningsproven startar en egen
    // OAuth-server på en slumpad port och kör hela flödet mot den; utan den här raden öppnade
    // varje `cargo test` en riktig flik med `.../authorize/?client_id=dcr-mock-42` i Alex
    // webbläsare (rapporterat 2026-11-05).
    if cfg!(test) {
        return Ok(());
    }
    let (program, args) = url_command(std::env::consts::OS, url);
    Command::new(program).args(args).spawn().map(|_| ())
}

/// Visar `dir` i filhanteraren. `Err` bär felet från **starten** — om programmet finns och
/// gick att köra. Vad det sedan gör med mappen är upp till skrivbordet.
pub fn open_dir(dir: &Path) -> std::io::Result<()> {
    let (program, args) = file_manager_command(std::env::consts::OS, dir);
    Command::new(program).args(args).spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Rätt program per plattform, och **mappen som argument** — inte skalsträngen, som hade
    /// tappat mellanslag i namn med mellanslag i.
    #[test]
    fn the_file_manager_command_follows_the_platform() {
        let dir = PathBuf::from("/home/alex/Music/Sonix");
        let (program, args) = file_manager_command("linux", &dir);
        assert_eq!(program, "xdg-open");

        let (program, args_win) = file_manager_command("windows", &dir);
        assert_eq!(program, "explorer");
        assert_eq!(args_win.len(), 1);

        let (program, args_mac) = file_manager_command("macos", &dir);
        assert_eq!(program, "open");
        assert_eq!(args_mac, vec!["/home/alex/Music/Sonix".to_string()]);

        // Ett namn med mellanslag ska vara ETT argument, inte två.
        let spaced = PathBuf::from("/home/alex/Mina Filer/Sonix");
        let (_, args_spaced) = file_manager_command("linux", &spaced);
        assert_eq!(args_spaced, vec!["/home/alex/Mina Filer/Sonix".to_string()]);
        assert_eq!(args, vec!["/home/alex/Music/Sonix".to_string()]);
    }

    /// URL:er går till webbläsaren, inte till filhanteraren — och Windows behöver `start`.
    #[test]
    fn the_url_command_uses_a_browser() {
        let (program, args) = url_command("linux", "https://example.test/x");
        println!("linux: {program} {args:?}");
        assert_eq!(program, "xdg-open");
        assert_eq!(args, vec!["https://example.test/x".to_string()]);

        let (program, args) = url_command("windows", "https://example.test/x");
        println!("windows: {program} {args:?}");
        assert_eq!(program, "cmd");
        assert!(args.iter().any(|a| a == "start"));

        let (program, _) = url_command("macos", "https://example.test/x");
        assert_eq!(program, "open");
    }
}

/// Öppnar en fil med systemets spelare — t.ex. en hämtad låt som ska lyssnas igenom.
pub fn open_path(path: &str) -> bool {
    // Samma regel som `open_url`: ett prov öppnar ingen extern spelare.
    if cfg!(test) {
        return true;
    }
    let (program, args) = url_command(std::env::consts::OS, path);
    Command::new(program).args(args).spawn().is_ok()
}
