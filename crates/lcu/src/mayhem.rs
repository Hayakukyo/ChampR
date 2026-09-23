use std::collections::HashSet;

use anyhow::{anyhow, Context};
use regex::Regex;
use reqwest::header::{ACCEPT_LANGUAGE, USER_AGENT};

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

pub async fn fetch_augments(champion_alias: &str) -> anyhow::Result<Vec<AugmentRecommendation>> {
    let slug = champion_slug(champion_alias);
    let url = format!(
        "https://op.gg/zh-cn/lol/modes/aram-mayhem/{slug}/augments"
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .context("build OP.GG Mayhem client")?;

    let raw = client
        .get(url)
        .header(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/128 Safari/537.36",
        )
        .header(ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.7")
        .send()
        .await
        .context("request OP.GG Mayhem augments")?
        .error_for_status()
        .context("OP.GG Mayhem response")?
        .text()
        .await
        .context("read OP.GG Mayhem page")?;

    parse_augments(&raw)
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

fn parse_augments(raw: &str) -> anyhow::Result<Vec<AugmentRecommendation>> {
    // Next.js serializes the payload with escaped quotes. Normalizing them
    // makes the embedded augment objects straightforward to inspect.
    let normalized = raw.replace("\\\"", "\"");

    let pattern = Regex::new(
        r#"\{"id":(?P<id>\d+),"tier":(?P<tier>\d+),"performance":(?P<performance>[^,]+),"popular":(?P<popular>[^,]+),"name":"(?P<name>(?:\\.|[^"])*)","key":"(?:\\.|[^"])*","largeIcon":"(?:\\.|[^"])*","smallIcon":"(?:\\.|[^"])*","rarity":(?P<rarity>\d+),"desc":"(?P<desc>(?:\\.|[^"])*)","tooltip""#,
    )
    .context("compile OP.GG augment parser")?;

    let strip_tags = Regex::new(r"<[^>]+>").context("compile description cleaner")?;
    let mut seen = HashSet::new();
    let mut items = Vec::new();

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
        if name.is_empty() {
            continue;
        }

        let description = captures
            .name("desc")
            .map(|m| decode_json_fragment(m.as_str()))
            .map(|value| strip_tags.replace_all(&value, "").into_owned())
            .unwrap_or_default();

        items.push(AugmentRecommendation {
            id,
            name,
            tier: captures
                .name("tier")
                .and_then(|m| m.as_str().parse::<i32>().ok())
                .unwrap_or_default(),
            rarity: rarity_label(rarity_code).to_string(),
            popularity: captures
                .name("popular")
                .and_then(|m| parse_optional_number(m.as_str())),
            performance: captures
                .name("performance")
                .and_then(|m| parse_optional_number(m.as_str())),
            description,
        });

        if items.len() >= 12 {
            break;
        }
    }

    if items.is_empty() {
        return Err(anyhow!(
            "OP.GG Mayhem page did not contain recognizable augment data"
        ));
    }

    Ok(items)
}

fn decode_json_fragment(value: &str) -> String {
    let wrapped = format!("\"{value}\"");
    serde_json::from_str::<String>(&wrapped).unwrap_or_else(|_| value.to_string())
}

fn parse_optional_number(value: &str) -> Option<f64> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("null") {
        None
    } else {
        value.parse::<f64>().ok()
    }
}

fn rarity_label(code: i32) -> &'static str {
    match code {
        4 => "Gold",
        8 => "Prismatic",
        _ => "Silver",
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
