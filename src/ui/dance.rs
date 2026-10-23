//! **Fruity Dance**: en figur som dansar i takt.
//!
//! Bara posen räknas här — ritningen ligger i `modals.rs` där egui finns. En pose är
//! normaliserad: `(x, y)` i −1…1 där y pekar **upp**, så figuren kan ritas i vilken ruta som
//! helst utan att matematiken bryr sig om pixlar.

/// Figurens leder vid en punkt i takten. `phase` är 0…1 där 1 = nästa takt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub hip: (f32, f32),
    pub head: (f32, f32),
    pub arm_l: (f32, f32),
    pub arm_r: (f32, f32),
    pub leg_l: (f32, f32),
    pub leg_r: (f32, f32),
    pub bob: f32,
}

/// Posen vid `phase` (0…1). Två studs per takt, armarna i motfas och benen växelvis —
/// samma enkla gång som en streckfigur behöver för att se levande ut.
pub fn pose(phase: f32) -> Pose {
    let p = (phase.fract().rem_euclid(1.0)) * std::f32::consts::TAU;
    let bob = (p * 2.0).sin();
    let swing = p.sin();
    let hip = (0.0, 0.0);
    Pose {
        hip,
        head: (swing * 0.12, 0.62 + bob * 0.05),
        arm_l: (-0.55 - swing * 0.15, 0.28 + bob * 0.12),
        arm_r: (0.55 - swing * 0.15, 0.28 - bob * 0.12),
        leg_l: (-0.28 - swing * 0.12, -0.75),
        leg_r: (0.28 - swing * 0.12, -0.75),
        bob: bob * 0.05,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Posen är periodisk och speglad:** en takt senare ser figuren likadan ut, och ett
    /// halvt varv senare har armarna bytt sida.
    #[test]
    fn the_dancer_is_periodic_and_mirrors() {
        let a = pose(0.0);
        let b = pose(1.0);
        assert!((a.arm_l.0 - b.arm_l.0).abs() < 1e-5);
        assert!((a.head.1 - b.head.1).abs() < 1e-5);

        // Sinus är noll både vid 0 och 0,5 — svängen syns en kvart in och en kvart före slutet.
        let quarter = pose(0.25);
        let three_quarters = pose(0.75);
        assert!(
            (quarter.arm_l.0 - three_quarters.arm_l.0).abs() > 0.2,
            "armarna ska svänga åt olika håll: {} mot {}",
            quarter.arm_l.0,
            three_quarters.arm_l.0
        );
        assert!(a.head.1 > 0.5 && pose(0.25).head.1 > 0.5, "huvudet är ovanför höften");
    }

    #[test]
    fn the_pose_stays_inside_the_box() {
        for i in 0..64 {
            let p = pose(i as f32 / 64.0);
            for (x, y) in [p.hip, p.head, p.arm_l, p.arm_r, p.leg_l, p.leg_r] {
                assert!(x.abs() <= 1.0 && y.abs() <= 1.0, "utanför rutan: ({x}, {y})");
            }
        }
    }
}
