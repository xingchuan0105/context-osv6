use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Policy,
    Finance,
    Medical,
    ComputerScience,
    Legal,
    Technology,
    Science,
    Engineering,
    Business,
    Education,
    Arts,
    History,
    Literature,
    #[serde(other)]
    Unknown,
}

impl Domain {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::Finance => "finance",
            Self::Medical => "medical",
            Self::ComputerScience => "computer_science",
            Self::Legal => "legal",
            Self::Technology => "technology",
            Self::Science => "science",
            Self::Engineering => "engineering",
            Self::Business => "business",
            Self::Education => "education",
            Self::Arts => "arts",
            Self::History => "history",
            Self::Literature => "literature",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<&str> for Domain {
    fn from(value: &str) -> Self {
        match value
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '-'], "_")
            .as_str()
        {
            "policy" => Self::Policy,
            "finance" => Self::Finance,
            "medical" | "medicine" | "healthcare" => Self::Medical,
            "computer_science" | "cs" | "computerscience" | "comp_sci" => Self::ComputerScience,
            "legal" | "law" => Self::Legal,
            "technology" | "tech" => Self::Technology,
            "science" => Self::Science,
            "engineering" | "eng" => Self::Engineering,
            "business" => Self::Business,
            "education" | "edu" => Self::Education,
            "arts" | "art" => Self::Arts,
            "history" => Self::History,
            "literature" | "lit" => Self::Literature,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Genre {
    Regulation,
    Report,
    ResearchPaper,
    Slides,
    Manual,
    News,
    Article,
    Book,
    Thesis,
    Documentation,
    Tutorial,
    Review,
    Essay,
    Blog,
    #[serde(other)]
    Unknown,
}

impl Genre {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Regulation => "regulation",
            Self::Report => "report",
            Self::ResearchPaper => "research_paper",
            Self::Slides => "slides",
            Self::Manual => "manual",
            Self::News => "news",
            Self::Article => "article",
            Self::Book => "book",
            Self::Thesis => "thesis",
            Self::Documentation => "documentation",
            Self::Tutorial => "tutorial",
            Self::Review => "review",
            Self::Essay => "essay",
            Self::Blog => "blog",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for Genre {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<&str> for Genre {
    fn from(value: &str) -> Self {
        match value
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '-'], "_")
            .as_str()
        {
            "regulation" | "regulatory" | "rule" => Self::Regulation,
            "report" => Self::Report,
            "research_paper" | "researchpaper" | "paper" | "academic_paper" => Self::ResearchPaper,
            "slides" | "presentation" | "slide_deck" | "deck" => Self::Slides,
            "manual" | "guide" | "handbook" => Self::Manual,
            "news" | "newspaper" => Self::News,
            "article" => Self::Article,
            "book" | "publication" => Self::Book,
            "thesis" | "dissertation" => Self::Thesis,
            "documentation" | "docs" | "doc" => Self::Documentation,
            "tutorial" | "howto" | "how_to" => Self::Tutorial,
            "review" => Self::Review,
            "essay" => Self::Essay,
            "blog" | "weblog" => Self::Blog,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Era {
    Classical,
    Modern,
    Contemporary,
    Ancient,
    Medieval,
    Renaissance,
    Enlightenment,
    Industrial,
    Postmodern,
    #[serde(other)]
    Unknown,
}

impl Era {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Classical => "classical",
            Self::Modern => "modern",
            Self::Contemporary => "contemporary",
            Self::Ancient => "ancient",
            Self::Medieval => "medieval",
            Self::Renaissance => "renaissance",
            Self::Enlightenment => "enlightenment",
            Self::Industrial => "industrial",
            Self::Postmodern => "postmodern",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for Era {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<&str> for Era {
    fn from(value: &str) -> Self {
        match value
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '-'], "_")
            .as_str()
        {
            "classical" => Self::Classical,
            "modern" => Self::Modern,
            "contemporary" => Self::Contemporary,
            "ancient" => Self::Ancient,
            "medieval" | "middle_ages" => Self::Medieval,
            "renaissance" => Self::Renaissance,
            "enlightenment" => Self::Enlightenment,
            "industrial" | "industrial_age" | "industrial_era" => Self::Industrial,
            "postmodern" | "post_modern" => Self::Postmodern,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryMetadata {
    pub doc_id: String,
    pub filename: String,
    pub docname: String,
    pub language: String,
    pub domain: Domain,
    pub genre: Genre,
    pub era: Era,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub publication_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryOutput {
    pub summary_text: String,
    pub summary_metadata: SummaryMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocScopeProfile {
    pub languages: Vec<String>,
    pub domains: Vec<Domain>,
    pub genres: Vec<Genre>,
    pub eras: Vec<Era>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocScopeMetadata {
    pub documents: Vec<SummaryMetadata>,
    pub profile: DocScopeProfile,
}
