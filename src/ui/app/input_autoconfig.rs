//! **Ingången ställer in sig själv** — Sprint 1, punkt 1.
//!
//! Alex: *"…så det är plugg and enjoy för användaren."* Kopplar man in en gitarrkabel ska den
//! användas, sättas upp med instrumentvärden och höras — utan att någon öppnar en inställningsruta.
//!
//! **En regel, två dörrar:** automatiken när appen startar och knappen i ljudmodalen gör exakt
//! samma sak, via samma funktion (`apply_instrument_plan`). Skillnaden är bara vem som bestämmer
//! *när* — och att knappen får göras om medan automatiken bara får ske en gång.
//!
//! Ordningen i `apply_instrument_plan` är inte godtycklig: frekvensen byts **före** monitorn slås
//! på, eftersom en kabel låst till 48 kHz i en motor på 44,1 kHz skulle monitoreras i fel tempo.
//! Går bytet inte igenom stängs monitorn av och beskedet säger varför — samma läxa som 8.5: hellre
//! tyst och tydligt än fel och tyst.
//!
//! Status: byggs (Sprint 1, punkt 1) — automatiken och knappen är samma funktion.
//! Kvar: att valet minns mellan starter, och att kvittera på riktig hårdvara (Alex' kabel).
//! Rör inte: ordningen frekvens-före-monitor. Byter man den hörs fel tempo utan att något
//! säger till.

use crate::audio::input_profile::{self, ResolvedInput};
use crate::i18n::t;
use crate::ui::app::SonixApp;

impl SonixApp {
    /// Vad som finns att koppla in just nu, med profil.
    pub(crate) fn resolved_inputs(&self) -> Vec<ResolvedInput> {
        let names = crate::audio::recorder::LiveMicrophoneCapture::list_devices();
        let cards = input_profile::system_cards();
        input_profile::resolve_inputs(&names, &cards)
    }

    /// Sätter upp en instrumentingång och returnerar beskedet som ska stå i statusraden.
    pub(crate) fn apply_instrument_plan(&mut self, input: &ResolvedInput) -> String {
        let plan = input_profile::plan_for(input, self.engine.sample_rate);
        let index = input.device_index.unwrap_or(0);
        let switched = self.vocal_studio.select_microphone(index);
        self.vocal_studio.set_input_gain(plan.defaults.gain);
        self.vocal_studio.set_noise_gate(plan.defaults.gate);

        // 1. Frekvensen först (se modulens huvud).
        let mut rate_note = String::new();
        let mut rate_ok = true;
        if let Some(rate) = plan.set_rate {
            let applied = self.engine.reconfigure(Some(rate), None).is_ok()
                && self.engine.sample_rate == rate;
            rate_ok = applied;
            if applied {
                self.resync_engine_after_reconfigure();
                let settings = crate::audio::engine::AudioSettings {
                    sample_rate: Some(rate),
                    buffer_frames: self.engine.buffer_frames,
                };
                let _ = settings.save();
                rate_note = crate::tstatus!(" · {} Hz (kabelns frekvens)", rate);
            } else {
                rate_note = crate::tstatus!(" · kunde inte byta till {} Hz", rate);
            }
        }

        // 2. Monitorn: bara om ingång och motor går i samma tempo.
        let monitor_on = plan.defaults.monitor && (plan.monitor_allowed || rate_ok);
        if monitor_on {
            self.vocal_studio.monitoring_on = true;
            self.vocal_studio.mic_settings.direct_monitoring = true;
        }
        let monitor_note = if monitor_on {
            t(" · monitor på (sänk MONITOR-ratten om det tjuter)")
        } else if plan.defaults.monitor {
            t(" · monitor av — ingång och motor går i olika tempo")
        } else {
            t(" · monitor av")
        };

        let state = if switched {
            t("vald")
        } else {
            t("kunde inte öppnas")
        };
        crate::tstatus!(
            "🎸 {} — {}{}{}",
            input.label,
            state,
            rate_note,
            monitor_note
        )
    }

    /// **Automatiken.** Körs en gång per appstart: finns en känd instrumentingång används den,
    /// och statusraden får veta exakt vad som hände. Har användaren redan valt en enhet lämnas
    /// den i fred — den som pekat på något själv ska inte få det överskrivet.
    pub(crate) fn autodetect_instrument_input(&mut self) {
        let inputs = self.resolved_inputs();
        let Some(instrument) = input_profile::first_instrument(&inputs) else {
            return;
        };
        let already_connected = self
            .vocal_studio
            .mic_capture
            .as_ref()
            .map(|m| m.device_name == instrument.device_name)
            .unwrap_or(false);
        if already_connected {
            return;
        }
        let message = self.apply_instrument_plan(instrument);
        self.status_message = message;
    }
}
