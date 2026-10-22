//! **Follow Actions** (Ableton): en slinga som spelar kan själv välja vad som händer sedan.
//!
//! Modellen här är enkel och avsiktligt liten: när en slinga har spelat `after_bars` takter
//! lösar åtgärden ut **en gång**, och `chance` avgör om den alls gör det. Slumpen matas in
//! som ett tal i stället för att dras här, så att beteendet går att mäta i ett prov i stället
//! för att höras ibland.

/// Vad som händer när åtgärden löser ut.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FollowKind {
    /// Nästa slinga, och efter den sista börjar det om.
    Next,
    /// Föregående slinga, och före den första går den till den sista.
    Previous,
    First,
    Last,
    /// Hoppa till ett bestämt index.
    Jump,
    /// Stanna — ingenting spelas efter.
    Stop,
    /// Spela samma slinga igen.
    Again,
}

impl FollowKind {
    pub fn label(self) -> &'static str {
        match self {
            FollowKind::Next => "Nästa",
            FollowKind::Previous => "Föregående",
            FollowKind::First => "Första",
            FollowKind::Last => "Sista",
            FollowKind::Jump => "Hoppa till",
            FollowKind::Stop => "Stanna",
            FollowKind::Again => "Samma igen",
        }
    }
}

/// En follow action: när den löser ut, vad den gör och hur ofta.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FollowAction {
    /// Hur många takter slingan spelar innan åtgärden prövas.
    pub after_bars: f32,
    pub kind: FollowKind,
    /// Index för [`FollowKind::Jump`]; ignoreras för de andra.
    pub jump_index: usize,
    /// 0–1. Under det här talet löser åtgärden ut; annars händer ingenting.
    pub chance: f32,
}

impl Default for FollowAction {
    fn default() -> Self {
        Self {
            after_bars: 4.0,
            kind: FollowKind::Next,
            jump_index: 1,
            chance: 1.0,
        }
    }
}

impl FollowAction {
    /// Har slingan spelat klart sina takter?
    pub fn due(&self, bars_in_clip: f32) -> bool {
        bars_in_clip >= self.after_bars.max(0.0)
    }

    /// **Nästa slinga, eller `None` när den ska stanna.**
    ///
    /// `roll` är slumptalet (0–1) som avgör chansen; det matas in så att provet kan välja
    /// utfallet i stället för att köra tusen gånger och hoppas.
    pub fn target(&self, current: usize, count: usize, roll: f32) -> Option<usize> {
        if count == 0 || roll >= self.chance.clamp(0.0, 1.0) {
            return None;
        }
        let last = count - 1;
        Some(match self.kind {
            FollowKind::Next => (current + 1) % count,
            FollowKind::Previous => (current + count - 1) % count,
            FollowKind::First => 0,
            FollowKind::Last => last,
            FollowKind::Jump => self.jump_index.min(last),
            FollowKind::Stop => return None,
            FollowKind::Again => current.min(last),
        })
    }
}

/// Vad som ska hända vid taktgränsen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowOutcome {
    /// Ingen åtgärd utlöst: spela vidare på samma slinga.
    Stay,
    /// Byt till den här slingan.
    Switch(usize),
    /// Sluta spela.
    Stop,
}

impl FollowAction {
    /// Pröva åtgärden vid en taktgräns: det transporten behöver, i ett enda svar.
    ///
    /// Skillnaden mot [`FollowAction::target`] är att `Again` här blir `Stay` — samma slinga
    /// ska inte räknas som ett byte, för då nollställs takträknaren och åtgärden löser ut igen
    /// nästa takt i all evighet.
    pub fn apply(&self, current: usize, count: usize, roll: f32) -> FollowOutcome {
        match self.target(current, count, roll) {
            None if self.kind == FollowKind::Stop && roll < self.chance.clamp(0.0, 1.0) => {
                FollowOutcome::Stop
            }
            None => FollowOutcome::Stay,
            Some(next) if next == current => FollowOutcome::Stay,
            Some(next) => FollowOutcome::Switch(next),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn act(kind: FollowKind) -> FollowAction {
        FollowAction {
            after_bars: 4.0,
            kind,
            jump_index: 1,
            chance: 1.0,
        }
    }

    /// **Nästa slinga rullar vidare och börjar om efter den sista.** Utan omslaget hade en
    /// follow action på sista slingan tystnat i stället för att fortsätta.
    #[test]
    fn next_advances_and_wraps() {
        let next = act(FollowKind::Next);
        assert_eq!(next.target(3, 5, 0.0), Some(4));
        assert_eq!(next.target(4, 5, 0.0), Some(0), "efter sista börjar det om");
        assert_eq!(next.target(0, 5, 0.0), Some(1));
    }

    /// **Föregående går baklänges och omsluter nedåt** — från första till sista.
    #[test]
    fn previous_wraps_backwards() {
        let prev = act(FollowKind::Previous);
        assert_eq!(prev.target(0, 5, 0.0), Some(4));
        assert_eq!(prev.target(3, 5, 0.0), Some(2));
    }

    /// **Hoppa till kläms mot listans slut** i stället för att peka utanför: ett projekt som
    /// krymper ska inte krascha på ett gammalt index.
    #[test]
    fn jump_is_clamped_to_the_last_clip() {
        let mut jump = act(FollowKind::Jump);
        jump.jump_index = 9;
        assert_eq!(jump.target(0, 4, 0.0), Some(3));
        jump.jump_index = 2;
        assert_eq!(jump.target(0, 4, 0.0), Some(2));
    }

    /// **Stanna stannar, samma-igen står kvar.**
    #[test]
    fn stop_ends_and_again_stays() {
        assert_eq!(act(FollowKind::Stop).target(2, 5, 0.0), None);
        assert_eq!(act(FollowKind::Again).target(2, 5, 0.0), Some(2));
        // En tom lista har ingen nästa slinga, oavsett åtgärd.
        assert_eq!(act(FollowKind::Next).target(0, 0, 0.0), None);
    }

    /// **Chansen gate:ar åtgärden.** Under chansen löser den ut, över händer ingenting —
    /// alltså inte "går vidare ändå", som vore en chans utan verkan.
    #[test]
    fn chance_gates_the_action() {
        let mut half = act(FollowKind::Next);
        half.chance = 0.5;
        assert_eq!(half.target(0, 4, 0.4), Some(1), "under chansen: löser ut");
        assert_eq!(half.target(0, 4, 0.6), None, "över chansen: händer ingenting");
        assert_eq!(half.target(0, 4, 0.5), None, "på gränsen räknas den som inte utlöst");
        // Chans 0 = aldrig.
        let never = FollowAction {
            chance: 0.0,
            ..act(FollowKind::Next)
        };
        assert_eq!(never.target(0, 4, 0.0), None);
    }

    /// **Utfallet skiljer byte från stillastående.** `Again` är `Stay`, inte ett byte, och
    /// `Stop` stannar bara när chansen faktiskt slår till.
    #[test]
    fn the_outcome_separates_a_switch_from_staying_put() {
        assert_eq!(act(FollowKind::Next).apply(4, 5, 0.0), FollowOutcome::Switch(0));
        assert_eq!(act(FollowKind::Again).apply(2, 5, 0.0), FollowOutcome::Stay);
        assert_eq!(act(FollowKind::Stop).apply(2, 5, 0.0), FollowOutcome::Stop);
        let mut half = act(FollowKind::Stop);
        half.chance = 0.5;
        assert_eq!(half.apply(2, 5, 0.7), FollowOutcome::Stay, "chansen slog inte till");
        assert_eq!(half.apply(2, 5, 0.2), FollowOutcome::Stop);
        // En lista med en enda slinga: nästa är samma, alltså inget byte.
        assert_eq!(act(FollowKind::Next).apply(0, 1, 0.0), FollowOutcome::Stay);
    }

    /// **Tidsgrinden räknar takter, inte sekunder**, och löser ut exakt när takterna är slut.
    #[test]
    fn the_bar_count_decides_when_it_fires() {
        let action = act(FollowKind::Next);
        assert!(!action.due(3.9), "4 takter betyder 4");
        assert!(action.due(4.0));
        assert!(action.due(5.5));
        let zero = FollowAction {
            after_bars: 0.0,
            ..action
        };
        assert!(zero.due(0.0), "noll takter löser ut direkt");
    }
}
