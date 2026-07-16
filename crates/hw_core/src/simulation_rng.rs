//! 固定 step 監査でだけ使用する actor-local な乱数列。
//!
//! 通常プレイは従来どおり thread-local RNG を使う。固定監査では各 actor に
//! 割り当てた安定 key と決定回数から短命な `StdRng` を作るため、system の
//! 実行順や別 actor の乱数消費に影響されない。

use bevy::prelude::{Component, Resource};
use rand::rngs::{StdRng, ThreadRng};
use rand::{Error, RngCore, SeedableRng};
use std::collections::BTreeMap;

/// 固定 step 決定性監査でのみ app に挿入する seed。
#[derive(Resource, Debug, Clone, Copy)]
pub struct FixedAuditSeed(pub u64);

/// fixture spawn 順に付与する actor-local な乱数状態。
///
/// 保存対象ではない。`FixedAuditSeed` と共に存在する場合だけ決定的な
/// 乱数列を選び、通常プレイでは `SimulationRng::Thread` にフォールバックする。
#[derive(Component, Debug, Clone)]
pub struct SimulationRandomState {
    key: u64,
    /// Streamごとの呼び出し回数。
    ///
    /// 異なる stream の条件分岐が後続の乱数列をずらさないよう、actor全体で
    /// 一つの cursor を共有しない。`BTreeMap` は監査recordのhashも安定させる。
    stream_cursors: BTreeMap<u64, u64>,
}

impl SimulationRandomState {
    pub const fn new(key: u64) -> Self {
        Self {
            key,
            stream_cursors: BTreeMap::new(),
        }
    }

    /// 固定 step fixture 内で actor を対応付けるための安定キー。
    pub const fn stable_key(&self) -> u64 {
        self.key
    }

    #[cfg(test)]
    fn cursor(&self) -> u64 {
        self.stream_cursors.values().copied().sum()
    }

    /// 固定 step auditで乱数消費の分岐を検出するための現在位置。
    ///
    /// 通常実行では不要な診断値なので、profiling featureでのみ公開する。
    #[cfg(feature = "profiling")]
    pub fn audit_cursor(&self) -> u64 {
        let mut checksum = 0xcbf2_9ce4_8422_2325u64;
        for (&stream, &cursor) in &self.stream_cursors {
            checksum = fnv1a(checksum, stream);
            checksum = fnv1a(checksum, cursor);
        }
        checksum
    }

    fn next_seed(&mut self, master_seed: u64, stream: u64) -> u64 {
        let cursor = self.stream_cursors.entry(stream).or_default();
        let current = *cursor;
        *cursor = cursor.wrapping_add(1);
        splitmix64(master_seed ^ stream ^ self.key.rotate_left(17) ^ current.rotate_left(41))
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

#[cfg(feature = "profiling")]
const fn fnv1a(checksum: u64, value: u64) -> u64 {
    (checksum ^ value).wrapping_mul(0x0000_0100_0000_01b3)
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

    #[test]
    fn one_stream_does_not_shift_another_stream() {
        let seed = FixedAuditSeed(20260712);
        let mut with_intermediate_stream = SimulationRandomState::new(7);
        let mut without_intermediate_stream = SimulationRandomState::new(7);

        let _ = SimulationRng::for_actor(Some(&seed), Some(&mut with_intermediate_stream), 0x101)
            .next_u64();
        let _ = SimulationRng::for_actor(Some(&seed), Some(&mut with_intermediate_stream), 0x202)
            .next_u64();

        let _ =
            SimulationRng::for_actor(Some(&seed), Some(&mut without_intermediate_stream), 0x101)
                .next_u64();

        assert_eq!(
            SimulationRng::for_actor(Some(&seed), Some(&mut with_intermediate_stream), 0x101)
                .next_u64(),
            SimulationRng::for_actor(Some(&seed), Some(&mut without_intermediate_stream), 0x101)
                .next_u64(),
        );
    }
}
