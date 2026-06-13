#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SkillCatalogConfig {
    #[serde(default)]
    pub retrieve_clusters: Vec<String>,
    #[serde(default)]
    pub synthesis_clusters: Vec<String>,
    #[serde(default)]
    pub clusters: Vec<SkillCluster>,
    #[serde(default)]
    pub mandatory: MandatorySkills,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillCluster {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,
    pub skills: Vec<String>,
    #[serde(default)]
    pub atomic: bool,
    #[serde(default)]
    pub disclose_at: DiscloseAt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscloseAt {
    #[default]
    Retrieve,
    Synthesis,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MandatorySkills {
    /// Clusters forced on the first retrieve round (e.g. RAG `codegen`).
    #[serde(default)]
    pub retrieve: Vec<String>,
    #[serde(default, alias = "mandatory_synthesis")]
    pub synthesis: Vec<String>,
}

impl SkillCatalogConfig {
    pub fn clusters_at(&self, phase: DiscloseAt) -> Vec<&SkillCluster> {
        self.clusters
            .iter()
            .filter(|c| c.disclose_at == phase)
            .collect()
    }

    pub fn cluster_by_id(&self, id: &str) -> Option<&SkillCluster> {
        self.clusters.iter().find(|c| c.id == id)
    }

    pub fn flat_skill_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for cluster in &self.clusters {
            for skill in &cluster.skills {
                if !ids.contains(skill) {
                    ids.push(skill.clone());
                }
            }
        }
        for skill in &self.mandatory.synthesis {
            if !ids.contains(skill) {
                ids.push(skill.clone());
            }
        }
        ids
    }

    pub fn expand_cluster_skills(&self, cluster_id: &str) -> Vec<String> {
        if self.cluster_by_id(cluster_id).is_some() {
            return vec![cluster_id.to_string()];
        }
        Vec::new()
    }

    /// Build cluster entries from CDS v1.1 yaml lists + PromptRegistry metadata.
    pub fn hydrate_clusters(&mut self) {
        if !self.clusters.is_empty()
            && self.retrieve_clusters.is_empty()
            && self.synthesis_clusters.is_empty()
        {
            return;
        }
        if self.retrieve_clusters.is_empty() && self.synthesis_clusters.is_empty() {
            return;
        }

        let registry = crate::agents::progressive::PromptRegistry::standard_cached();
        let mut clusters = Vec::new();

        for id in &self.retrieve_clusters {
            if let Some(cluster) = cluster_from_registry(registry, id, DiscloseAt::Retrieve) {
                clusters.push(cluster);
            }
        }
        for id in &self.synthesis_clusters {
            if let Some(cluster) = cluster_from_registry(registry, id, DiscloseAt::Synthesis) {
                clusters.push(cluster);
            }
        }
        self.clusters = clusters;
    }
}

fn cluster_from_registry(
    registry: &crate::agents::progressive::PromptRegistry,
    id: &str,
    default_phase: DiscloseAt,
) -> Option<SkillCluster> {
    let skill = registry.skill(id)?;
    let md = skill.metadata();
    let disclose_at = md
        .get("disclose_at")
        .and_then(|v| match v.as_str() {
            "retrieve" => Some(DiscloseAt::Retrieve),
            "synthesis" => Some(DiscloseAt::Synthesis),
            _ => None,
        })
        .unwrap_or(default_phase);
    let atomic = md.get("atomic").map(|v| v == "true").unwrap_or(false);
    Some(SkillCluster {
        id: id.to_string(),
        description: Some(skill.description().to_string()),
        skills: vec![id.to_string()],
        atomic,
        disclose_at,
    })
}

pub fn deserialize_skill_catalog<'de, D>(deserializer: D) -> Result<SkillCatalogConfig, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, SeqAccess, Visitor};

    struct SkillCatalogVisitor;
    impl<'de> Visitor<'de> for SkillCatalogVisitor {
        type Value = SkillCatalogConfig;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a skill catalog list or structured object")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut legacy_ids = Vec::new();
            while let Some(id) = seq.next_element::<String>()? {
                legacy_ids.push(id);
            }
            Ok(SkillCatalogConfig {
                clusters: legacy_ids
                    .into_iter()
                    .map(|id| SkillCluster {
                        id: id.clone(),
                        description: None,
                        skills: vec![id],
                        atomic: false,
                        disclose_at: DiscloseAt::Retrieve,
                    })
                    .collect(),
                ..Default::default()
            })
        }

        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: de::MapAccess<'de>,
        {
            let mut retrieve_clusters = Vec::new();
            let mut synthesis_clusters = Vec::new();
            let mut clusters = Vec::new();
            let mut mandatory = MandatorySkills::default();
            while let Some(key) = map.next_key::<String>()? {
                match key.as_str() {
                    "retrieve" | "retrieve_clusters" => {
                        retrieve_clusters = map.next_value()?;
                    }
                    "synthesis" | "synthesis_clusters" => {
                        synthesis_clusters = map.next_value()?;
                    }
                    "clusters" => {
                        clusters = map.next_value()?;
                    }
                    "mandatory" => {
                        mandatory = map.next_value()?;
                    }
                    "mandatory_retrieve" => {
                        mandatory.retrieve = map.next_value()?;
                    }
                    "mandatory_synthesis" => {
                        mandatory.synthesis = map.next_value()?;
                    }
                    _ => {
                        let _: de::IgnoredAny = map.next_value()?;
                    }
                }
            }
            Ok(SkillCatalogConfig {
                retrieve_clusters,
                synthesis_clusters,
                clusters,
                mandatory,
            })
        }
    }

    deserializer.deserialize_any(SkillCatalogVisitor)
}
