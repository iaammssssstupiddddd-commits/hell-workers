//! `--spawn-*` と `HW_SPAWN_*` 環境変数からスポーン数を取得

use std::env;

/// コマンドライン引数 `--flag value` を優先し、なければ環境変数 `env_key` を参照。
/// どちらも無い場合 `default` を返す。
pub fn parse_spawn_count_from_args_or_env(
    flag: &str,
    env_key: &str,
    default: u32,
) -> u32 {
    parse_spawn_from_args::<u32>(flag)
        .or_else(|| env::var(env_key).ok().and_then(|v| v.parse().ok()))
        .unwrap_or(default)
}

fn parse_spawn_from_args<T>(flag: &str) -> Option<T>
where
    T: std::str::FromStr,
{
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == flag {
            let value = args.next()?;
            return value.parse().ok();
        }
    }
    None
}
