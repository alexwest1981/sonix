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
}
