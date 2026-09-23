use std::collections::HashSet;

use anyhow::{anyhow, Context};
use opgg::{
    champion::{Info, Item as OpggItem, Rune as OpggRune},
    ChampionPosition, Client, GameMode,
};

use crate::builds::{Block, BuildSection, Item, ItemBuild, Rune};

const OP_GG_RANKED: &str = "op.gg";
const OP_GG_ARAM: &str = "op.gg-aram";

pub fn is_native_source(source: &str) -> bool {
    matches!(source, OP_GG_RANKED | OP_GG_ARAM)
}

pub async fn source_metadata(source: &str) -> anyhow::Result<(String, String)> {
    let client = Client::new();
    let mode = match source {
        OP_GG_RANKED => GameMode::Ranked,
        OP_GG_ARAM => GameMode::Aram,
        _ => return Err(anyhow!("unsupported native OP.GG source: {source}")),
    };

    let response = client
        .champion_list(mode)
        .await
        .context("fetch OP.GG source metadata")?;

    let meta = response.meta;
    let version = meta.version.clone();
    let updated_at = meta
        .analyzed_at
        .or(meta.cached_at)
        .unwrap_or_default();

    Ok((version, updated_at))
}

pub async fn fetch_builds(
    champion_id: i64,
    champion_alias: &str,
    source: &str,
) -> anyhow::Result<Vec<BuildSection>> {
    let champion_id_u32 = u32::try_from(champion_id)
        .map_err(|_| anyhow!("invalid champion id {champion_id}"))?;
    let client = Client::new();

    match source {
        OP_GG_ARAM => {
            let response = client
                .champion_info(GameMode::Aram, champion_id_u32, ChampionPosition::None)
                .await
                .context("fetch OP.GG ARAM champion info")?;
            Ok(vec![transform_info(
                champion_id,
                champion_alias,
                "ARAM",
                GameMode::Aram,
                &response.meta.version,
                response.data,
            )])
        }
        OP_GG_RANKED => {
            let list = client
                .champion_list(GameMode::Ranked)
                .await
                .context("fetch OP.GG ranked champion list")?;
            let summary = list
                .data
                .into_iter()
                .find(|item| item.id == champion_id as usize)
                .ok_or_else(|| anyhow!("champion {champion_id} not found in OP.GG ranked list"))?;

            let mut positions = summary
                .positions
                .unwrap_or_default()
                .into_iter()
                .map(|position| position.name)
                .filter(|position| *position != ChampionPosition::None)
                .collect::<Vec<_>>();

            positions.sort_by_key(|position| position.to_string());
            positions.dedup();

            if positions.is_empty() {
                return Err(anyhow!("OP.GG has no ranked positions for champion {champion_id}"));
            }

            let mut sections = Vec::new();
            for position in positions {
                match client
                    .champion_info(GameMode::Ranked, champion_id_u32, position)
                    .await
                {
                    Ok(response) => sections.push(transform_info(
                        champion_id,
                        champion_alias,
                        &position.to_string(),
                        GameMode::Ranked,
                        &response.meta.version,
                        response.data,
                    )),
                    Err(err) => {
                        kv_log_macro::warn!(
                            "OP.GG champion {} position {} failed: {:?}",
                            champion_id,
                            position,
                            err
                        );
                    }
                }
            }

            if sections.is_empty() {
                Err(anyhow!("OP.GG returned no ranked builds for champion {champion_id}"))
            } else {
                Ok(sections)
            }
        }
        _ => Err(anyhow!("unsupported native OP.GG source: {source}")),
    }
}

fn transform_info(
    champion_id: i64,
    champion_alias: &str,
    position: &str,
    mode: GameMode,
    version: &str,
    info: Info,
) -> BuildSection {
    let map_id = match mode {
        GameMode::Aram => 12,
        GameMode::Ranked => 11,
    };
    let mode_label = match mode {
        GameMode::Aram => "ARAM",
        GameMode::Ranked => "Ranked",
    };

    let runes = transform_runes(champion_alias, position, &info.runes);
    let item_builds = transform_item_builds(
        champion_id,
        champion_alias,
        position,
        mode_label,
        map_id,
        &info,
    );

    let (pick_count, win_rate) = runes
        .first()
        .map(|rune| (rune.pick_count as i64, rune.win_rate.clone()))
        .unwrap_or((0, "0.00%".to_string()));

    BuildSection {
        index: 0,
        id: format!(
            "opgg-{}-{}-{}",
            champion_alias.to_lowercase(),
            mode_label.to_lowercase(),
            position.to_lowercase()
        ),
        version: version.to_string(),
        official_version: version.to_string(),
        pick_count,
        win_rate,
        timestamp: unix_timestamp_millis(),
        alias: champion_alias.to_string(),
        name: champion_alias.to_string(),
        position: position.to_string(),
        skills: None,
        spells: None,
        champion_tier: None,
        item_builds,
        runes,
    }
}

fn transform_runes(champion_alias: &str, position: &str, source: &[OpggRune]) -> Vec<Rune> {
    let mut sorted = source.to_vec();
    sorted.sort_by(|a, b| b.play.cmp(&a.play));
    sorted.truncate(4);

    sorted
        .into_iter()
        .enumerate()
        .map(|(index, rune)| {
            let mut selected = Vec::new();
            selected.extend(rune.primary_rune_ids.iter().map(|id| *id as i64));
            selected.extend(rune.secondary_rune_ids.iter().map(|id| *id as i64));
            selected.extend(rune.stat_mod_ids.iter().map(|id| *id as i64));

            Rune {
                uuid: crate::builds::gen_uuid(),
                alias: champion_alias.to_string(),
                name: format!(
                    "[OP.GG] {} {} #{}",
                    champion_alias,
                    position,
                    index + 1
                ),
                position: position.to_string(),
                pick_count: rune.play as u64,
                win_rate: format_rate(rune.win, rune.play),
                primary_style_id: rune.primary_page_id as i64,
                sub_style_id: rune.secondary_page_id as i64,
                selected_perk_ids: selected,
                score: None,
                type_field: String::new(),
            }
        })
        .collect()
}

fn transform_item_builds(
    champion_id: i64,
    champion_alias: &str,
    position: &str,
    mode_label: &str,
    map_id: i64,
    info: &Info,
) -> Vec<ItemBuild> {
    let popular = build_item_set(
        champion_id,
        champion_alias,
        position,
        mode_label,
        map_id,
        "Most Popular",
        false,
        info,
        0,
    );
    let high_wr = build_item_set(
        champion_id,
        champion_alias,
        position,
        mode_label,
        map_id,
        "Highest Win Rate",
        true,
        info,
        1,
    );

    [popular, high_wr].into_iter().flatten().collect()
}

#[allow(clippy::too_many_arguments)]
fn build_item_set(
    champion_id: i64,
    champion_alias: &str,
    position: &str,
    mode_label: &str,
    map_id: i64,
    label: &str,
    by_win_rate: bool,
    info: &Info,
    sortrank: i64,
) -> Option<ItemBuild> {
    let mut blocks = Vec::new();

    push_item_block(
        &mut blocks,
        "Starter Items",
        &info.starter_items,
        by_win_rate,
        1,
    );
    push_item_block(&mut blocks, "Boots", &info.boots, by_win_rate, 6);
    push_item_block(&mut blocks, "Core Build", &info.core_items, by_win_rate, 1);
    push_item_block(
        &mut blocks,
        "Situational / Last Items",
        &info.last_items,
        by_win_rate,
        8,
    );

    if blocks.is_empty() {
        return None;
    }

    let mode_tag = if mode_label == "Ranked" {
        String::new()
    } else {
        format!(" {}", mode_label)
    };
    let position_tag = if position.is_empty() || position.eq_ignore_ascii_case(mode_label) {
        "Build".to_string()
    } else {
        position.to_string()
    };
    let suffix = if label == "Highest Win Rate" {
        " (Highest WR)"
    } else {
        ""
    };

    Some(ItemBuild {
        title: format!(
            "[OP.GG] {}{} - {}{}",
            champion_alias, mode_tag, position_tag, suffix
        ),
        associated_maps: vec![map_id],
        associated_champions: vec![champion_id],
        blocks,
        map: "any".to_string(),
        mode: "any".to_string(),
        preferred_item_slots: Some(Vec::new()),
        sortrank,
        started_from: "op.gg".to_string(),
        type_field: Some("custom".to_string()),
    })
}

fn push_item_block(
    blocks: &mut Vec<Block>,
    title: &str,
    rows: &[OpggItem],
    by_win_rate: bool,
    max_rows: usize,
) {
    if rows.is_empty() {
        return;
    }

    let mut sorted = rows.to_vec();
    if by_win_rate {
        sorted.sort_by(|a, b| {
            item_win_rate(b)
                .partial_cmp(&item_win_rate(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    } else {
        sorted.sort_by(|a, b| b.play.cmp(&a.play));
    }

    let mut ids = Vec::new();
    let mut seen = HashSet::new();
    for row in sorted.into_iter().take(max_rows) {
        for id in row.ids {
            if seen.insert(id) {
                ids.push(Item {
                    id: id.to_string(),
                    count: 1,
                });
            }
        }
    }

    if !ids.is_empty() {
        blocks.push(Block {
            type_field: title.to_string(),
            items: Some(ids),
        });
    }
}

fn item_win_rate(item: &OpggItem) -> f64 {
    if item.play == 0 {
        0.0
    } else {
        item.win as f64 / item.play as f64
    }
}

fn format_rate(win: usize, play: usize) -> String {
    if play == 0 {
        "0.00%".to_string()
    } else {
        format!("{:.2}%", (win as f64 / play as f64) * 100.0)
    }
}

fn unix_timestamp_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}
