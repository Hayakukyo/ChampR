use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Context};
use regex::Regex;
use reqwest::header::{ACCEPT_LANGUAGE, USER_AGENT};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct AugmentRecommendation {
    pub id: i64,
    pub name: String,
    pub tier: i32,
    pub rarity: String,
    pub popularity: Option<f64>,
    pub performance: Option<f64>,
    pub description: String,
}

#[derive(Debug, Deserialize)]
struct StatsResponse {
    data: Vec<StatsItem>,
}

#[derive(Debug, Deserialize)]
struct StatsItem {
    id: i64,
    tier: Option<i32>,
    performance: Option<f64>,
    popular: Option<f64>,
}

#[derive(Debug, Clone, Default)]
struct PageDetail {
    name: String,
    rarity: String,
    description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientAugment {
    id: i64,
    #[serde(default, rename = "nameTRA")]
    name_tra: String,
    #[serde(default, rename = "simpleNameTRA")]
    simple_name_tra: String,
    #[serde(default)]
    rarity: String,
}

/// Fetch champion-specific ARAM Mayhem augment rankings.
///
/// Ranking/score data comes from OP.GG's structured endpoint. Human-readable
/// text is then enriched from the Tencent League client's localized game-data
/// catalog while the client is still alive. The OP.GG page remains a fallback
/// for descriptions and names.
pub async fn fetch_augments(
    champion_id: i64,
    champion_alias: &str,
    auth_url: Option<&str>,
) -> anyhow::Result<Vec<AugmentRecommendation>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .context("build OP.GG Mayhem client")?;

    let stats_url = format!(
        "https://lol-api-champion.op.gg/api/contents/stats/champions/{champion_id}/aram-augments"
    );
    let stats = client
        .get(stats_url)
        .header(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/128 Safari/537.36",
        )
        .send()
        .await
        .context("request OP.GG Mayhem augment stats")?
        .error_for_status()
        .context("OP.GG Mayhem stats response")?
        .json::<StatsResponse>()
        .await
        .context("decode OP.GG Mayhem augment stats")?;

    let page_details = fetch_page_details(&client, champion_alias)
        .await
        .unwrap_or_default();
    let client_details = match auth_url {
        Some(auth) if !auth.is_empty() => fetch_client_catalog(auth).await.unwrap_or_default(),
        _ => HashMap::new(),
    };

    let mut items = Vec::new();
    for stat in stats.data.into_iter().take(6) {
        let page = page_details.get(&stat.id).cloned().unwrap_or_default();
        let local = client_details.get(&stat.id);

        let local_name = local
            .map(|entry| {
                if !entry.name_tra.trim().is_empty() {
                    entry.name_tra.trim().to_string()
                } else {
                    entry.simple_name_tra.trim().to_string()
                }
            })
            .unwrap_or_default();

        let name = if !local_name.is_empty() && !looks_like_string_key(&local_name) {
            local_name
        } else if !page.name.is_empty() {
            page.name.clone()
        } else {
            format!("Augment {}", stat.id)
        };

        let rarity = local
            .map(|entry| rarity_label_from_key(&entry.rarity).to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| {
                if page.rarity.is_empty() {
                    None
                } else {
                    Some(page.rarity.clone())
                }
            })
            .unwrap_or_else(|| "Silver".to_string());

        let description = compact_description(&page.description, &name);

        items.push(AugmentRecommendation {
            id: stat.id,
            name,
            tier: stat.tier.unwrap_or_default(),
            rarity,
            popularity: stat.popular,
            performance: stat.performance,
            description,
        });
    }

    if items.is_empty() {
        return Err(anyhow!("OP.GG returned no ARAM Mayhem augments"));
    }

    Ok(items)
}

async fn fetch_client_catalog(auth_url: &str) -> anyhow::Result<HashMap<i64, ClientAugment>> {
    let endpoint = format!(
        "https://{auth_url}/lol-game-data/assets/v1/cherry-augments.json"
    );
    let rows = crate::lcu_api::make_get_request::<Vec<ClientAugment>>(&endpoint)
        .await
        .map_err(|err| anyhow!("fetch localized augment catalog: {err:?}"))?;

    Ok(rows.into_iter().map(|entry| (entry.id, entry)).collect())
}

async fn fetch_page_details(
    client: &reqwest::Client,
    champion_alias: &str,
) -> anyhow::Result<HashMap<i64, PageDetail>> {
    let slug = champion_slug(champion_alias);
    let url = format!("https://op.gg/zh-cn/lol/modes/aram-mayhem/{slug}/augments");

    let raw = client
        .get(url)
        .header(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/128 Safari/537.36",
        )
        .header(ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.7")
        .send()
        .await
        .context("request OP.GG Mayhem page")?
        .error_for_status()
        .context("OP.GG Mayhem page response")?
        .text()
        .await
        .context("read OP.GG Mayhem page")?;

    parse_page_details(&raw)
}

fn champion_slug(alias: &str) -> String {
    match alias {
        "MonkeyKing" => "wukong".to_string(),
        "KSante" => "ksante".to_string(),
        "Kaisa" => "kaisa".to_string(),
        "Khazix" => "khazix".to_string(),
        "Velkoz" => "velkoz".to_string(),
        "RekSai" => "reksai".to_string(),
        "Belveth" => "belveth".to_string(),
        "Chogath" => "chogath".to_string(),
        _ => alias.to_ascii_lowercase(),
    }
}

fn parse_page_details(raw: &str) -> anyhow::Result<HashMap<i64, PageDetail>> {
    let normalized = raw.replace("\\"", """);

    let pattern = Regex::new(
        r#"\{"id":(?P<id>\d+),"tier":(?P<tier>\d+),"performance":(?P<performance>[^,]+),"popular":(?P<popular>[^,]+),"name":"(?P<name>(?:\\.|[^"])*)","key":"(?:\\.|[^"])*","largeIcon":"(?:\\.|[^"])*","smallIcon":"(?:\\.|[^"])*","rarity":(?P<rarity>\d+),"desc":"(?P<desc>(?:\\.|[^"])*)","tooltip""#,
    )
    .context("compile OP.GG augment parser")?;

    let strip_tags = Regex::new(r"<[^>]+>").context("compile description cleaner")?;
    let whitespace = Regex::new(r"\s+").context("compile whitespace cleaner")?;
    let mut seen = HashSet::new();
    let mut items = HashMap::new();

    for captures in pattern.captures_iter(&normalized) {
        let id = captures
            .name("id")
            .and_then(|m| m.as_str().parse::<i64>().ok())
            .unwrap_or_default();
        if id == 0 || !seen.insert(id) {
            continue;
        }

        let rarity_code = captures
            .name("rarity")
            .and_then(|m| m.as_str().parse::<i32>().ok())
            .unwrap_or_default();

        let name = captures
            .name("name")
            .map(|m| decode_json_fragment(m.as_str()))
            .unwrap_or_default();
        let description = captures
            .name("desc")
            .map(|m| decode_json_fragment(m.as_str()))
            .map(|value| strip_tags.replace_all(&value, " ").into_owned())
            .map(|value| whitespace.replace_all(&value, " ").trim().to_string())
            .unwrap_or_default();

        items.insert(
            id,
            PageDetail {
                name,
                rarity: rarity_label_from_code(rarity_code).to_string(),
                description,
            },
        );
    }

    Ok(items)
}

fn compact_description(value: &str, name: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return summary_from_name(name).to_string();
    }

    let mut output = value.chars().take(58).collect::<String>();
    if value.chars().count() > 58 {
        output.push('…');
    }
    output
}

fn summary_from_name(name: &str) -> &'static str {
    if name.contains("暴击") || name.contains("会心") || name.contains("无尽") {
        "偏暴击 / 暴击收益"
    } else if name.contains("攻速")
        || name.contains("快射")
        || name.contains("双刀")
        || name.contains("狂热")
    {
        "偏攻速 / 普攻强化"
    } else if name.contains("吸血")
        || name.contains("治疗")
        || name.contains("再生")
        || name.contains("渴血")
        || name.contains("虹吸")
    {
        "回复 / 吸血 / 生存"
    } else if name.contains("速度")
        || name.contains("移速")
        || name.contains("闪现")
        || name.contains("踢踏")
    {
        "移速 / 机动性"
    } else if name.contains("护盾")
        || name.contains("防御")
        || name.contains("巨人")
        || name.contains("生命")
    {
        "生命 / 防御 / 坦度"
    } else if name.contains("循环") || name.contains("终极") || name.contains("技能") {
        "技能循环 / 技能急速"
    } else if name.contains("法术") || name.contains("魔法") || name.contains("耀光") {
        "法术 / 技能伤害"
    } else {
        "英雄适配型强化"
    }
}

fn decode_json_fragment(value: &str) -> String {
    let wrapped = format!("\"{value}\"");
    serde_json::from_str::<String>(&wrapped).unwrap_or_else(|_| value.to_string())
}

fn looks_like_string_key(value: &str) -> bool {
    value.contains("Augment_")
        || value.contains("augment_")
        || value.contains("ARAM_")
        || value.contains("StringTable")
}

fn rarity_label_from_code(code: i32) -> &'static str {
    match code {
        4 => "Gold",
        8 => "Prismatic",
        _ => "Silver",
    }
}

fn rarity_label_from_key(key: &str) -> &'static str {
    match key {
        "kGold" | "Gold" => "Gold",
        "kPrismatic" | "Prismatic" => "Prismatic",
        "kSilver" | "kBronze" | "Silver" => "Silver",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_map_to_opgg_slugs() {
        assert_eq!(champion_slug("MonkeyKing"), "wukong");
        assert_eq!(champion_slug("Fizz"), "fizz");
        assert_eq!(champion_slug("Kaisa"), "kaisa");
    }
}
