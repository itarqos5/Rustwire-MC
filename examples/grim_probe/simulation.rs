//! Deliberately narrow vanilla walking model: ordinary stone, yaw 0, survival,
//! no effects, no jumping, no collision except the known horizontal floor.
//! Float arithmetic preserves LivingEntity's ground acceleration and drag; the
//! 0.003 pre-travel cutoff is applied before acceleration. This is not a world
//! collision engine or a claim to reproduce a complete graphical client.
use rustwire_mc::{packet::PositionSync, Error, Result};

#[derive(Debug, Default)]
pub struct FlatWalk {
    pub position: [f64; 3],
    pub rotation: [f32; 2],
    velocity_z: f64,
    velocity_y: f64,
    pub on_ground: bool,
}
impl FlatWalk {
    pub fn teleport(&mut self, packet: &PositionSync) -> Result<()> {
        if packet.relative_flags & !0x1ff != 0 {
            return Err(Error::Unsupported("unknown teleport relative flags"));
        }
        // Rotation-relative velocity would require a complete velocity transform.
        // This fixture permits only zero teleport velocity and stationary resets.
        if packet.velocity.is_some_and(|v| v.iter().any(|n| *n != 0.))
            || (packet.relative_flags & 0x1e0 != 0 && self.velocity_z != 0.)
        {
            return Err(Error::Unsupported(
                "teleport velocity outside flat-stone fixture",
            ));
        }
        for (i, value) in [packet.x, packet.y, packet.z].into_iter().enumerate() {
            self.position[i] = value
                + if packet.relative_flags & (1 << i) != 0 {
                    self.position[i]
                } else {
                    0.
                };
        }
        for (i, value) in [packet.yaw, packet.pitch].into_iter().enumerate() {
            self.rotation[i] = value
                + if packet.relative_flags & (1 << (i + 3)) != 0 {
                    self.rotation[i]
                } else {
                    0.
                };
        }
        self.velocity_z = 0.;
        self.velocity_y = 0.;
        self.on_ground = false;
        Ok(())
    }
    pub fn verify_fixture(&self) -> Result<()> {
        if self
            .position
            .iter()
            .zip([0.5, -60., 0.5])
            .any(|(a, b)| (*a - b).abs() > 1e-7)
            || self.rotation != [0., 0.]
        {
            return Err(Error::State(
                "fixture must teleport to 0.5 -60 0.5, yaw 0 pitch 0 before valid phase",
            ));
        }
        Ok(())
    }
    pub fn tick(&mut self, forward: bool) {
        if self.velocity_z.abs() < 0.003 {
            self.velocity_z = 0.;
        }
        if self.velocity_y.abs() < 0.003 {
            self.velocity_y = 0.;
        }
        // Friction and acceleration use the previous tick's grounded state.
        // Gravity is applied AFTER movement, as in LivingEntity.travelInAir.
        let friction = if self.on_ground {
            0.6_f32 * 0.91_f32
        } else {
            0.91_f32
        };
        if forward {
            let stone_friction = 0.6_f32;
            let acceleration = if self.on_ground {
                0.1_f32 * (0.216_000_02_f32 / (stone_friction * stone_friction * stone_friction))
            } else {
                0.02_f32
            };
            self.velocity_z += f64::from(0.98_f32) * f64::from(acceleration);
        }
        self.position[2] += self.velocity_z;
        let requested_y = self.position[1] + self.velocity_y;
        // The harness verifies one horizontal stone floor with its top at -60.
        // A zero-Y teleport tick cannot establish ground contact; the following
        // downward tick collides with the floor and sets on-ground naturally.
        self.on_ground = self.velocity_y < 0. && requested_y < -60.;
        if self.on_ground {
            self.position[1] = -60.;
            self.velocity_y = 0.;
        } else {
            self.position[1] = requested_y;
        }
        self.velocity_y = (self.velocity_y - 0.08_f64) * f64::from(0.98_f32);
        self.velocity_z *= f64::from(friction);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stone_walk_accelerates_and_decays_without_snapping() {
        let mut walk = FlatWalk {
            position: [0.5, -60., 0.5],
            ..FlatWalk::default()
        };
        walk.tick(false);
        assert!(!walk.on_ground);
        walk.tick(false);
        assert!(walk.on_ground);
        walk.tick(true);
        assert!((walk.position[2] - 0.598_000_003_367_662_5).abs() < 1e-14);
        let first = walk.position[2];
        for _ in 1..20 {
            walk.tick(true);
        }
        let release = walk.position[2];
        walk.tick(false);
        assert!(walk.position[2] > release);
        for _ in 1..20 {
            walk.tick(false);
        }
        let stop = walk.position[2];
        walk.tick(false);
        assert_eq!(walk.position[2], stop);
        assert!(stop - first > 4.);
        assert_eq!(walk.position[..2], [0.5, -60.]);
    }
    #[test]
    fn stationary_post_teleport_gravity_establishes_ground_before_walking() {
        let mut walk = FlatWalk {
            position: [0.5, -60., 0.5],
            ..FlatWalk::default()
        };
        assert!(!walk.on_ground);
        walk.tick(false);
        assert!(!walk.on_ground);
        assert_eq!(walk.position, [0.5, -60., 0.5]);
        assert!((walk.velocity_y + 0.078_400_001_525_878_9).abs() < 1e-14);
        walk.tick(false);
        assert!(walk.on_ground);
        assert_eq!(walk.position, [0.5, -60., 0.5]);
        walk.tick(true);
        assert!(walk.on_ground);
        assert!((walk.position[2] - 0.598_000_003_367_662_5).abs() < 1e-14);
        let mut airborne = FlatWalk {
            position: [0.5, -60., 0.5],
            ..FlatWalk::default()
        };
        airborne.tick(true);
        assert!((airborne.position[2] - 0.519_6).abs() < 1e-8);
    }
    #[test]
    fn relative_position_rotation_is_applied_before_ack_response() {
        let mut walk = FlatWalk {
            position: [1., 2., 3.],
            rotation: [10., 20.],
            ..FlatWalk::default()
        };
        walk.teleport(&PositionSync {
            teleport_id: 1,
            x: 2.,
            y: 5.,
            z: 4.,
            velocity: Some([0.; 3]),
            yaw: 3.,
            pitch: 4.,
            relative_flags: 1 | 4 | 8,
        })
        .unwrap();
        assert_eq!(walk.position, [3., 5., 7.]);
        assert_eq!(walk.rotation, [13., 4.]);
    }
    #[test]
    fn unsupported_knockback_is_rejected() {
        let mut walk = FlatWalk::default();
        assert!(walk
            .teleport(&PositionSync {
                teleport_id: 1,
                x: 0.,
                y: 0.,
                z: 0.,
                velocity: Some([0., 0.1, 0.]),
                yaw: 0.,
                pitch: 0.,
                relative_flags: 0
            })
            .is_err());
    }
}
