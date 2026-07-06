//! Skeleton stage: article outline from topic + material cards (spec §8).

use anyhow::Result;

use crate::llm::WriterLlm;
use crate::workspace::RhythmMode;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MaterialKind {
    Fact,
    Quote,
    Figure,
    Term,
    Inspiration,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MaterialCard {
    pub id: String,
    pub kind: MaterialKind,
    pub content: String,
    /// MVP placeholder for `SourceRef`; integration maps real citations in Task 17.
    pub source: serde_json::Value,
    pub section_hint: Option<String>,
    pub rare_terms: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ParagraphPlan {
    pub rhythm: RhythmMode,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkeletonSection {
    pub heading: String,
    pub key_points: Vec<String>,
    pub card_refs: Vec<String>,
    pub target_chars: usize,
    pub paragraphs: Vec<ParagraphPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Skeleton {
    pub title: String,
    pub sections: Vec<SkeletonSection>,
}

const SKELETON_SYSTEM: &str = "\
你是中文长文写作的大纲编辑。根据主题、目标篇幅和素材卡片，输出文章骨架 JSON。
只返回 JSON，不要 markdown 代码块或解释。";

/// One LLM call producing a sectioned outline with rhythm plans per paragraph.
pub async fn plan_skeleton(
    llm: &WriterLlm,
    topic: &str,
    target_chars: usize,
    cards: &[MaterialCard],
    tokens_used: &mut usize,
) -> Result<Skeleton> {
    let user = build_skeleton_user_prompt(topic, target_chars, cards);
    let (raw, tokens): (LlmSkeleton, u32) = llm.json(SKELETON_SYSTEM, &user).await?;
    *tokens_used += tokens as usize;
    normalize_skeleton(raw, target_chars, cards)
}

fn build_skeleton_user_prompt(topic: &str, target_chars: usize, cards: &[MaterialCard]) -> String {
    let cards_json = serde_json::to_string_pretty(cards).unwrap_or_else(|_| "[]".into());
    format!(
        "主题：{topic}\n目标总字数：约 {target_chars} 字\n\n素材卡片（id 可用于 card_refs）：\n{cards_json}\n\n\
         请输出 JSON，结构如下：\n\
         {{\n\
           \"title\": \"文章标题\",\n\
           \"sections\": [\n\
             {{\n\
               \"heading\": \"小节标题\",\n\
               \"key_points\": [\"要点1\", \"要点2\"],\n\
               \"card_refs\": [\"card-id\"],\n\
               \"target_chars\": 800,\n\
               \"paragraphs\": [{{\"rhythm\": \"ShortBurst\"}}]\n\
             }}\n\
           ]\n\
         }}\n\n\
         要求：\n\
         - sections 3–6 个，各节 target_chars 之和约等于目标总字数\n\
         - 每节 key_points 2–5 条\n\
         - card_refs 只能引用上面卡片 id\n\
         - paragraphs 至少 1 个；rhythm 只能是 ShortBurst、LongFlow 或 Mixed\n\
         - 交替使用 ShortBurst / LongFlow / Mixed 以形成节奏变化"
    )
}

#[derive(Debug, serde::Deserialize)]
struct LlmSkeleton {
    title: String,
    sections: Vec<LlmSkeletonSection>,
}

#[derive(Debug, serde::Deserialize)]
struct LlmSkeletonSection {
    heading: String,
    key_points: Vec<String>,
    card_refs: Vec<String>,
    target_chars: usize,
    paragraphs: Vec<LlmParagraphPlan>,
}

#[derive(Debug, serde::Deserialize)]
struct LlmParagraphPlan {
    rhythm: String,
}

fn normalize_skeleton(
    raw: LlmSkeleton,
    target_chars: usize,
    cards: &[MaterialCard],
) -> Result<Skeleton> {
    let card_ids: std::collections::BTreeSet<&str> =
        cards.iter().map(|c| c.id.as_str()).collect();

    if raw.title.trim().is_empty() {
        anyhow::bail!("skeleton title is empty");
    }
    if raw.sections.is_empty() {
        anyhow::bail!("skeleton has no sections");
    }

    let mut sections = Vec::with_capacity(raw.sections.len());
    for (idx, sec) in raw.sections.into_iter().enumerate() {
        if sec.heading.trim().is_empty() {
            anyhow::bail!("section {idx} heading is empty");
        }
        if !(2..=5).contains(&sec.key_points.len()) {
            anyhow::bail!(
                "section {idx} key_points must have 2–5 items, got {}",
                sec.key_points.len()
            );
        }
        for id in &sec.card_refs {
            if !card_ids.contains(id.as_str()) {
                anyhow::bail!("section {idx} references unknown card id {id:?}");
            }
        }

        let paragraphs: Vec<ParagraphPlan> = if sec.paragraphs.is_empty() {
            vec![ParagraphPlan {
                rhythm: RhythmMode::Mixed,
            }]
        } else {
            sec.paragraphs
                .into_iter()
                .map(|p| ParagraphPlan {
                    rhythm: parse_rhythm(&p.rhythm),
                })
                .collect()
        };

        sections.push(SkeletonSection {
            heading: sec.heading.trim().to_string(),
            key_points: sec
                .key_points
                .into_iter()
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect(),
            card_refs: sec.card_refs,
            target_chars: sec.target_chars.max(100),
            paragraphs,
        });
    }

    let sum: usize = sections.iter().map(|s| s.target_chars).sum();
    if sum == 0 {
        anyhow::bail!("skeleton section budgets sum to zero");
    }
    if target_chars > 0 {
        let scale = target_chars as f64 / sum as f64;
        if (scale - 1.0).abs() > 0.35 {
            tracing::debug!(
                requested = target_chars,
                planned = sum,
                "skeleton char budget deviates from target; keeping LLM allocation"
            );
        }
    }

    Ok(Skeleton {
        title: raw.title.trim().to_string(),
        sections,
    })
}

fn parse_rhythm(raw: &str) -> RhythmMode {
    match raw.trim().to_ascii_lowercase().as_str() {
        "shortburst" | "short_burst" | "short-burst" | "short burst" => RhythmMode::ShortBurst,
        "longflow" | "long_flow" | "long-flow" | "long flow" => RhythmMode::LongFlow,
        "mixed" => RhythmMode::Mixed,
        _ => RhythmMode::Mixed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_cards() -> Vec<MaterialCard> {
        vec![MaterialCard {
            id: "c1".into(),
            kind: MaterialKind::Fact,
            content: "示例数据".into(),
            source: serde_json::json!({"kind": "test"}),
            section_hint: Some("背景".into()),
            rare_terms: vec!["术语".into()],
        }]
    }

    #[test]
    fn normalize_skeleton_accepts_llm_shape() {
        let raw = LlmSkeleton {
            title: "测试标题".into(),
            sections: vec![LlmSkeletonSection {
                heading: "第一节".into(),
                key_points: vec!["甲".into(), "乙".into()],
                card_refs: vec!["c1".into()],
                target_chars: 500,
                paragraphs: vec![
                    LlmParagraphPlan {
                        rhythm: "ShortBurst".into(),
                    },
                    LlmParagraphPlan {
                        rhythm: "long-flow".into(),
                    },
                ],
            }],
        };
        let sk = normalize_skeleton(raw, 500, &sample_cards()).expect("normalize");
        assert_eq!(sk.title, "测试标题");
        assert_eq!(sk.sections.len(), 1);
        assert_eq!(sk.sections[0].paragraphs[0].rhythm, RhythmMode::ShortBurst);
        assert_eq!(sk.sections[0].paragraphs[1].rhythm, RhythmMode::LongFlow);
    }

    #[test]
    fn normalize_rejects_unknown_card_ref() {
        let raw = LlmSkeleton {
            title: "T".into(),
            sections: vec![LlmSkeletonSection {
                heading: "H".into(),
                key_points: vec!["a".into(), "b".into()],
                card_refs: vec!["missing".into()],
                target_chars: 100,
                paragraphs: vec![LlmParagraphPlan {
                    rhythm: "Mixed".into(),
                }],
            }],
        };
        assert!(normalize_skeleton(raw, 100, &sample_cards()).is_err());
    }

    #[tokio::test]
    #[ignore = "requires live AGENT_LLM API; run with --ignored --nocapture"]
    async fn plan_skeleton_smoke() {
        let llm = WriterLlm::from_env().expect("from_env");
        let cards = sample_cards();
        let sk = plan_skeleton(
            &llm,
            "人工智能在医疗影像中的应用",
            2000,
            &cards,
            &mut 0,
        )
        .await
        .expect("plan_skeleton");
        assert!(!sk.title.is_empty());
        assert!(!sk.sections.is_empty());
    }
}
