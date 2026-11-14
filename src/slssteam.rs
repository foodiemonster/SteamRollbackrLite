use chrono::{DateTime, Local, Utc};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::history::{
    fetch_releases, parse_atom_entries, read_cache, read_stale_cache, write_cache, ReleaseEntry,
};
use crate::steam::{ManifestChannel, SlsVariant};

#[derive(Debug, Clone)]
pub struct SlsMoonStatus {
    pub variant: SlsVariant,
    pub monitor_timestamp_utc: i64,
    pub monitor_display: String,
    pub moon_display: String,
    pub latest_moon_version: Option<String>,
    pub latest_steam_display: Option<String>,
    pub updated_for_latest_steam: Option<bool>,
    pub local_pattern_supported: Option<bool>,
    pub local_steamclient_sha: Option<String>,
}

fn format_dt(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&Local)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

fn fetch_latest_atom_date(cache_key: &str, url: &str) -> Result<DateTime<Utc>, String> {
    let ttl = Duration::from_secs(600);
    let xml = if let Some(cached) = read_cache(cache_key, ttl) {
        cached
    } else {
        match ureq::get(url)
            .set("User-Agent", "SteamRollbackr/0.3")
            .call()
            .and_then(|r| r.into_string().map_err(ureq::Error::from))
        {
            Ok(body) => {
                write_cache(cache_key, &body);
                body
            }
            Err(e) => {
                if let Some(stale) = read_stale_cache(cache_key) {
                    stale
                } else {
                    return Err(format!("Failed to fetch {url}: {e}"));
                }
            }
        }
    };

    let entries = parse_atom_entries(&xml);
    let first = entries
        .into_iter()
        .next()
        .ok_or_else(|| format!("No entries found in {url}"))?;

    DateTime::parse_from_rfc3339(&first.1)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| format!("Invalid date in {url}: {e}"))
}

fn compute_sha256(path: &Path) -> Option<String> {
    let out = Command::new("sha256sum").arg(path).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout.split_whitespace().next().map(|s| s.to_string())
}

fn check_pattern_exists(sha256: &str) -> bool {
    let url = format!(
        "https://raw.githubusercontent.com/swwayps/steam-monitor/main/linux32/steamclient/{sha256}.toml"
    );
    ureq::head(&url)
        .set("User-Agent", "SteamRollbackr/0.3")
        .call()
        .map(|r| r.status() == 200)
        .unwrap_or(false)
}

fn fetch_latest_moon_release_version(variant: SlsVariant) -> Option<String> {
    let (cache_key, url) = match variant {
        SlsVariant::Moon32 => (
            "slsteam_moon_releases_atom.xml",
            "https://github.com/swwayps/slsteam-moon/releases.atom",
        ),
        SlsVariant::Moon64 => (
            "slssteam_moon_64_releases_atom.xml",
            "https://github.com/foodiemonster/slssteam-moon-64/releases.atom",
        ),
    };
    let ttl = Duration::from_secs(600);
    let xml = if let Some(cached) = read_cache(cache_key, ttl) {
        cached
    } else {
        match ureq::get(url)
            .set("User-Agent", "SteamRollbackr/0.4")
            .call()
            .and_then(|r| r.into_string().map_err(ureq::Error::from))
        {
            Ok(body) => {
                write_cache(cache_key, &body);
                body
            }
            Err(_) => read_stale_cache(cache_key)?,
        }
    };

    parse_atom_entries(&xml)
        .into_iter()
        .next()
        .map(|(_, _, title)| title)
        .filter(|t| !t.is_empty())
}

pub fn check_slsteam_moon(
    variant: SlsVariant,
    steam_dir: Option<&Path>,
    local_client_ts: Option<i64>,
) -> Result<SlsMoonStatus, String> {
    let latest_moon_version = fetch_latest_moon_release_version(variant);
    let latest_steam_date = fetch_latest_atom_date(
        "releases_steamrt64_atom.xml",
        "https://github.com/SteamTracking/SteamTracking/commits/HEAD/ClientManifest/steam_client_steamrt64.atom",
    )
    .ok();

    match variant {
        SlsVariant::Moon32 => {
            let monitor_date = fetch_latest_atom_date(
                "steam_monitor_atom.xml",
                "https://github.com/swwayps/steam-monitor/commits/HEAD.atom",
            )?;
            let moon_date = fetch_latest_atom_date(
                "slsteam_moon_atom.xml",
                "https://github.com/swwayps/slsteam-moon/commits/HEAD.atom",
            )?;

            let newest_pattern_update = monitor_date.max(moon_date);
            let updated_for_latest_steam = latest_steam_date.map(|st| newest_pattern_update >= st);

            let (local_steamclient_sha, local_pattern_supported) = match steam_dir {
                Some(dir) => {
                    let so = dir.join("ubuntu12_32/steamclient.so");
                    if so.is_file() {
                        if let Some(sha) = compute_sha256(&so) {
                            let supported = check_pattern_exists(&sha);
                            (Some(sha), Some(supported))
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                }
                None => (None, None),
            };

            Ok(SlsMoonStatus {
                variant,
                monitor_timestamp_utc: monitor_date.timestamp(),
                monitor_display: format_dt(monitor_date),
                moon_display: format_dt(moon_date),
                latest_moon_version,
                latest_steam_display: latest_steam_date.map(format_dt),
                updated_for_latest_steam,
                local_pattern_supported,
                local_steamclient_sha,
            })
        }
        SlsVariant::Moon64 => {
            let moon_date = fetch_latest_atom_date(
                "slssteam_moon_64_atom.xml",
                "https://github.com/foodiemonster/slssteam-moon-64/commits/HEAD.atom",
            )?;
            let updated_for_latest_steam = latest_steam_date.map(|st| moon_date >= st);

            let (local_steamclient_sha, local_pattern_supported) = match steam_dir {
                Some(dir) => {
                    let so = dir.join("steamrt64/steamclient.so");
                    if so.is_file() {
                        let sha = compute_sha256(&so);
                        let supported = local_client_ts
                            .map(|ts| ts <= moon_date.timestamp() + 3600)
                            .or(updated_for_latest_steam);
                        (sha, supported)
                    } else {
                        (None, Some(false))
                    }
                }
                None => (None, None),
            };

            Ok(SlsMoonStatus {
                variant,
                monitor_timestamp_utc: moon_date.timestamp(),
                monitor_display: format_dt(moon_date),
                moon_display: format_dt(moon_date),
                latest_moon_version,
                latest_steam_display: latest_steam_date.map(format_dt),
                updated_for_latest_steam,
                local_pattern_supported,
                local_steamclient_sha,
            })
        }
    }
}

/// Finds the newest Steam release compatible with `cutoff_utc` for the given `SlsVariant`.
pub fn find_best_compatible_release(
    variant: SlsVariant,
    cutoff_utc: i64,
) -> Result<(ReleaseEntry, Vec<ReleaseEntry>), String> {
    // 1-hour grace window accounts for SteamTracking vs pattern/release publish skew
    let limit = cutoff_utc + 3600;

    match variant {
        SlsVariant::Moon64 => {
            let rt64 = fetch_releases(ManifestChannel::SteamRt64, 1)?;
            if let Some(best) = rt64
                .iter()
                .find(|e| e.timestamp_utc > 0 && e.timestamp_utc <= limit)
                .cloned()
            {
                Ok((best, rt64))
            } else {
                Err(
                    "No compatible steamrt64 Steam release found within the latest slssteam-moon-64 window."
                        .to_string(),
                )
            }
        }
        SlsVariant::Moon32 => {
            let rt64 = fetch_releases(ManifestChannel::SteamRt64, 1)?;
            let u12 = fetch_releases(ManifestChannel::Ubuntu12, 1)?;

            let best_rt64 = rt64
                .iter()
                .find(|e| e.timestamp_utc > 0 && e.timestamp_utc <= limit)
                .cloned();
            let best_u12 = u12
                .iter()
                .find(|e| e.timestamp_utc > 0 && e.timestamp_utc <= limit)
                .cloned();

            match (best_rt64, best_u12) {
                (Some(r), Some(u)) => {
                    if r.timestamp_utc >= u.timestamp_utc {
                        Ok((r, rt64))
                    } else {
                        Ok((u, u12))
                    }
                }
                (Some(r), None) => Ok((r, rt64)),
                (None, Some(u)) => Ok((u, u12)),
                (None, None) => Err(
                    "No compatible Steam release found within the latest slsteam-moon pattern window."
                        .to_string(),
                ),
            }
        }
    }
}
