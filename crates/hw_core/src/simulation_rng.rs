//! 固定 step 監査でだけ使用する actor-local な乱数列。
//!
//! 通常プレイは従来どおり thread-local RNG を使う。固定監査では各 actor に
//! 割り当てた安定 key と決定回数から短命な `StdRng` を作るため、system の
//! 実行順や別 actor の乱数消費に影響されない。

use bevy::prelude::{Component, Resource};
use rand::rngs::{StdRng, ThreadRng};
use rand::{Error, RngCore, SeedableRng};

/// 固定 step 決定性監査でのみ app に挿入する seed。
#[derive(Resource, Debug, Clone, Copy)]
pub struct FixedAuditSeed(pub u64);

/// fixture spawn 順に付与する actor-local な乱数状態。
///
/// 保存対象ではない。`FixedAuditSeed` と共に存在する場合だけ決定的な
/// 乱数列を選び、通常プレイでは `SimulationRng::Thread` にフォールバックする。
#[derive(Component, Debug, Clone, Copy)]
pub struct SimulationRandomState {
    key: u64,
    cursor: u64,
}

impl SimulationRandomState {
    pub const fn new(key: u64) -> Self {
        Self { key, cursor: 0 }
    }

    /// 固定 step fixture 内で actor を対応付けるための安定キー。
    pub const fn stable_key(&self) -> u64 {
        self.key
    }

    #[cfg(test)]
    const fn cursor(&self) -> u64 {
        self.cursor
    }

    fn next_seed(&mut self, master_seed: u64, stream: u64) -> u64 {
        let cursor = self.cursor;
        self.cursor = self.cursor.wrapping_add(1);
        splitmix64(master_seed ^ stream ^ self.key.rotate_left(17) ^ cursor.rotate_left(41))
    }
}

/// 通常と固定監査の乱数源を同じ `Rng` API で扱う小さな adapter。
pub enum SimulationRng {
    Fixed(Box<StdRng>),
    Thread(ThreadRng),
}

impl SimulationRng {
    pub fn for_actor(
        audit_seed: Option<&FixedAuditSeed>,
        state: Option<&mut SimulationRandomState>,
        stream: u64,
    ) -> Self {
        match (audit_seed, state) {
            (Some(seed), Some(state)) => Self::Fixed(Box::new(StdRng::seed_from_u64(
                state.next_seed(seed.0, stream),
            ))),
            _ => Self::Thread(rand::thread_rng()),
        }
    }
}

impl RngCore for SimulationRng {
    fn next_u32(&mut self) -> u32 {
        match self {
            Self::Fixed(rng) => rng.next_u32(),
            Self::Thread(rng) => rng.next_u32(),
        }
    }

    fn next_u64(&mut self) -> u64 {
        match self {
            Self::Fixed(rng) => rng.next_u64(),
            Self::Thread(rng) => rng.next_u64(),
        }
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        match self {
            Self::Fixed(rng) => rng.fill_bytes(destination),
            Self::Thread(rng) => rng.fill_bytes(destination),
        }
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Error> {
        match self {
            Self::Fixed(rng) => rng.try_fill_bytes(destination),
            Self::Thread(rng) => rng.try_fill_bytes(destination),
        }
    }
}

const fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::{FixedAuditSeed, SimulationRandomState, SimulationRng};
    use rand::RngCore;

    #[test]
    fn fixed_actor_stream_is_repeatable_and_advances_independently() {
        let seed = FixedAuditSeed(20260712);
        let mut first = SimulationRandomState::new(7);
        let mut second = SimulationRandomState::new(7);

        let first_value = SimulationRng::for_actor(Some(&seed), Some(&mut first), 0x101).next_u64();
        let second_value =
            SimulationRng::for_actor(Some(&seed), Some(&mut second), 0x101).next_u64();

        assert_eq!(first_value, second_value);
        assert_eq!(first.cursor(), 1);
        assert_eq!(second.cursor(), 1);
        assert_ne!(
            SimulationRng::for_actor(Some(&seed), Some(&mut first), 0x101).next_u64(),
            first_value
        );
    }
}
