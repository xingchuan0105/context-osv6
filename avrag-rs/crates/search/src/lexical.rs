use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::{Context, Result, anyhow};
use jieba_rs::Jieba;
use tantivy::{
    Index, IndexReader, IndexWriter, TantivyDocument, Term,
    collector::TopDocs,
    doc,
    query::{BooleanQuery, Occur, Query, TermQuery},
    schema::{Field, IndexRecordOption, STORED, STRING, Schema, TEXT, Value},
};
use uuid::Uuid;

const ORG_ID_FIELD: &str = "org_id";
const DOC_ID_FIELD: &str = "doc_id";
const CHUNK_ID_FIELD: &str = "chunk_id";
const PAGE_FIELD: &str = "page";
const CONTENT_FIELD: &str = "content";
const WRITER_HEAP_BYTES: usize = 50_000_000;

#[derive(Debug, Clone)]
pub struct LexicalChunkDocument {
    pub org_id: Uuid,
    pub doc_id: Uuid,
    pub chunk_id: Uuid,
    pub page: Option<i64>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexicalSearchHit {
    pub chunk_id: Uuid,
    pub doc_id: Uuid,
    pub score: f32,
    pub page: Option<i64>,
}

pub struct TantivyLexicalIndex {
    path: PathBuf,
    reader: IndexReader,
    writer: Option<Mutex<IndexWriter>>,
    fields: LexicalFields,
}

#[derive(Debug, Clone, Copy)]
struct LexicalFields {
    org_id: Field,
    doc_id: Field,
    chunk_id: Field,
    page: Field,
    content: Field,
}

impl TantivyLexicalIndex {
    pub fn open_reader(path: impl AsRef<Path>) -> Result<Self> {
        Self::open(path.as_ref(), false, false)
    }

    pub fn open_writer(path: impl AsRef<Path>) -> Result<Self> {
        Self::open(path.as_ref(), true, true)
    }

    fn open(path: &Path, with_writer: bool, create_if_missing: bool) -> Result<Self> {
        let schema = build_schema();
        let index = match Index::open_in_dir(path) {
            Ok(index) => index,
            Err(_) if create_if_missing => {
                std::fs::create_dir_all(path)
                    .with_context(|| format!("create Tantivy index dir {}", path.display()))?;
                Index::create_in_dir(path, schema)
                    .with_context(|| format!("create Tantivy index {}", path.display()))?
            }
            Err(error) => return Err(error.into()),
        };
        let schema = index.schema();
        let fields = LexicalFields::from_schema(&schema)?;
        let reader = index
            .reader()
            .with_context(|| format!("open Tantivy index reader {}", path.display()))?;
        let writer = if with_writer {
            Some(Mutex::new(index.writer(WRITER_HEAP_BYTES)?))
        } else {
            None
        };

        Ok(Self {
            path: path.to_path_buf(),
            reader,
            writer,
            fields,
        })
    }

    pub fn replace_document_chunks(
        &self,
        org_id: Uuid,
        doc_id: Uuid,
        chunks: &[LexicalChunkDocument],
    ) -> Result<()> {
        let writer = self.writer.as_ref().ok_or_else(|| {
            anyhow!(
                "Tantivy index opened without writer: {}",
                self.path.display()
            )
        })?;
        let mut writer = writer
            .lock()
            .map_err(|_| anyhow!("Tantivy index writer lock poisoned"))?;

        writer.delete_term(Term::from_field_text(
            self.fields.doc_id,
            &doc_id.to_string(),
        ));
        for chunk in chunks {
            if chunk.org_id != org_id || chunk.doc_id != doc_id {
                return Err(anyhow!(
                    "Tantivy chunk belongs to a different org or document"
                ));
            }
            let page = chunk.page.unwrap_or(-1);
            writer.add_document(doc!(
                self.fields.org_id => org_id.to_string(),
                self.fields.doc_id => chunk.doc_id.to_string(),
                self.fields.chunk_id => chunk.chunk_id.to_string(),
                self.fields.page => page,
                self.fields.content => tokenize_for_index(&chunk.content),
            ))?;
        }
        writer.commit()?;
        self.reader.reload()?;
        Ok(())
    }

    pub fn search(
        &self,
        org_id: Uuid,
        query: &str,
        doc_ids: Option<&[Uuid]>,
        limit: usize,
    ) -> Result<Vec<LexicalSearchHit>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let query_tokens = tokenize_for_query(query);
        if query_tokens.is_empty() {
            return Ok(Vec::new());
        }

        self.reader.reload()?;
        let searcher = self.reader.searcher();
        let query = self.build_query(org_id, doc_ids, &query_tokens)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit).order_by_score())?;

        let mut hits = Vec::with_capacity(top_docs.len());
        for (score, doc_address) in top_docs {
            let doc = searcher.doc::<TantivyDocument>(doc_address)?;
            let chunk_id = stored_uuid(&doc, self.fields.chunk_id)?;
            let doc_id = stored_uuid(&doc, self.fields.doc_id)?;
            let page = stored_i64(&doc, self.fields.page).filter(|value| *value >= 0);
            hits.push(LexicalSearchHit {
                chunk_id,
                doc_id,
                score,
                page,
            });
        }
        Ok(hits)
    }

    fn build_query(
        &self,
        org_id: Uuid,
        doc_ids: Option<&[Uuid]>,
        query_tokens: &[String],
    ) -> Result<BooleanQuery> {
        let mut filters: Vec<(Occur, Box<dyn Query>)> = vec![(
            Occur::Must,
            Box::new(TermQuery::new(
                Term::from_field_text(self.fields.org_id, &org_id.to_string()),
                IndexRecordOption::Basic,
            )),
        )];

        if let Some(doc_ids) = doc_ids {
            if doc_ids.is_empty() {
                return Err(anyhow!("doc_ids filter must not be empty"));
            }
            let doc_queries = doc_ids
                .iter()
                .map(|doc_id| {
                    Box::new(TermQuery::new(
                        Term::from_field_text(self.fields.doc_id, &doc_id.to_string()),
                        IndexRecordOption::Basic,
                    )) as Box<dyn Query>
                })
                .collect();
            filters.push((Occur::Must, Box::new(BooleanQuery::union(doc_queries))));
        }

        let token_queries = query_tokens
            .iter()
            .map(|token| {
                Box::new(TermQuery::new(
                    Term::from_field_text(self.fields.content, token),
                    IndexRecordOption::WithFreqs,
                )) as Box<dyn Query>
            })
            .collect();
        filters.push((Occur::Must, Box::new(BooleanQuery::union(token_queries))));

        Ok(BooleanQuery::new(filters))
    }
}

impl LexicalFields {
    fn from_schema(schema: &Schema) -> Result<Self> {
        Ok(Self {
            org_id: schema.get_field(ORG_ID_FIELD)?,
            doc_id: schema.get_field(DOC_ID_FIELD)?,
            chunk_id: schema.get_field(CHUNK_ID_FIELD)?,
            page: schema.get_field(PAGE_FIELD)?,
            content: schema.get_field(CONTENT_FIELD)?,
        })
    }
}

fn build_schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_text_field(ORG_ID_FIELD, STRING | STORED);
    builder.add_text_field(DOC_ID_FIELD, STRING | STORED);
    builder.add_text_field(CHUNK_ID_FIELD, STRING | STORED);
    builder.add_i64_field(PAGE_FIELD, STORED);
    builder.add_text_field(CONTENT_FIELD, TEXT);
    builder.build()
}

fn stored_uuid(doc: &TantivyDocument, field: Field) -> Result<Uuid> {
    let value = doc
        .get_first(field)
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("missing stored uuid field"))?;
    Uuid::parse_str(value).map_err(|error| anyhow!(error))
}

fn stored_i64(doc: &TantivyDocument, field: Field) -> Option<i64> {
    doc.get_first(field).and_then(|value| value.as_i64())
}

fn tokenize_for_index(text: &str) -> String {
    lexical_tokens(text).join(" ")
}

fn tokenize_for_query(text: &str) -> Vec<String> {
    lexical_tokens(text)
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn lexical_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    add_word_tokens(text, &mut tokens);
    add_cjk_bigrams(text, &mut tokens);
    add_alnum_tokens(text, &mut tokens);
    tokens
}

fn add_word_tokens(text: &str, tokens: &mut Vec<String>) {
    let jieba = Jieba::new();
    for token in jieba.cut(text, false) {
        let normalized = normalize_token(token);
        if !normalized.is_empty() {
            tokens.push(normalized);
        }
    }
}

fn add_cjk_bigrams(text: &str, tokens: &mut Vec<String>) {
    let mut run = Vec::new();
    for ch in text.chars() {
        if is_cjk(ch) {
            run.push(ch);
            continue;
        }
        flush_cjk_run(&mut run, tokens);
    }
    flush_cjk_run(&mut run, tokens);
}

fn flush_cjk_run(run: &mut Vec<char>, tokens: &mut Vec<String>) {
    match run.len() {
        0 => {}
        1 => tokens.push(run[0].to_string()),
        _ => {
            for pair in run.windows(2) {
                tokens.push(pair.iter().collect());
            }
        }
    }
    run.clear();
}

fn add_alnum_tokens(text: &str, tokens: &mut Vec<String>) {
    let mut token = String::new();
    for ch in text.chars() {
        if !is_cjk(ch) && ch.is_alphanumeric() {
            token.extend(ch.to_lowercase());
            continue;
        }
        if !token.is_empty() {
            tokens.push(std::mem::take(&mut token));
        }
    }
    if !token.is_empty() {
        tokens.push(token);
    }
}

fn normalize_token(token: &str) -> String {
    token
        .trim()
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|ch| ch.is_alphanumeric() || is_cjk(*ch))
        .collect()
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(org_id: Uuid, doc_id: Uuid, chunk_id: Uuid, content: &str) -> LexicalChunkDocument {
        LexicalChunkDocument {
            org_id,
            doc_id,
            chunk_id,
            page: Some(1),
            content: content.to_string(),
        }
    }

    #[test]
    fn chinese_bigram_tokens_allow_sliding_matches() {
        let tokens = tokenize_for_query("能力体");
        assert!(tokens.contains(&"能力".to_string()));
        assert!(tokens.contains(&"力体".to_string()));
    }

    #[test]
    fn search_matches_chinese_substring_across_segmentation() {
        let dir = tempfile::tempdir().unwrap();
        let index = TantivyLexicalIndex::open_writer(dir.path()).unwrap();
        let org_id = Uuid::new_v4();
        let doc_id = Uuid::new_v4();
        let chunk_id = Uuid::new_v4();

        index
            .replace_document_chunks(
                org_id,
                doc_id,
                &[chunk(org_id, doc_id, chunk_id, "智能能力体系建设")],
            )
            .unwrap();

        let hits = index.search(org_id, "能力体", Some(&[doc_id]), 10).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk_id, chunk_id);
    }

    #[test]
    fn doc_scope_uses_or_semantics() {
        let dir = tempfile::tempdir().unwrap();
        let index = TantivyLexicalIndex::open_writer(dir.path()).unwrap();
        let org_id = Uuid::new_v4();
        let doc_a = Uuid::new_v4();
        let doc_b = Uuid::new_v4();
        let chunk_a = Uuid::new_v4();
        let chunk_b = Uuid::new_v4();

        index
            .replace_document_chunks(org_id, doc_a, &[chunk(org_id, doc_a, chunk_a, "合同风险")])
            .unwrap();
        index
            .replace_document_chunks(org_id, doc_b, &[chunk(org_id, doc_b, chunk_b, "合同条款")])
            .unwrap();

        let hits = index
            .search(org_id, "合同", Some(&[doc_a, doc_b]), 10)
            .unwrap();
        let hit_docs = hits
            .into_iter()
            .map(|hit| hit.doc_id)
            .collect::<BTreeSet<_>>();

        assert_eq!(hit_docs, BTreeSet::from([doc_a, doc_b]));
    }

    #[test]
    fn replace_document_chunks_removes_old_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let index = TantivyLexicalIndex::open_writer(dir.path()).unwrap();
        let org_id = Uuid::new_v4();
        let doc_id = Uuid::new_v4();
        let old_chunk = Uuid::new_v4();
        let new_chunk = Uuid::new_v4();

        index
            .replace_document_chunks(
                org_id,
                doc_id,
                &[chunk(org_id, doc_id, old_chunk, "旧版本")],
            )
            .unwrap();
        index
            .replace_document_chunks(
                org_id,
                doc_id,
                &[chunk(org_id, doc_id, new_chunk, "新版本")],
            )
            .unwrap();

        assert!(
            index
                .search(org_id, "旧版", Some(&[doc_id]), 10)
                .unwrap()
                .is_empty()
        );
        let hits = index.search(org_id, "新版本", Some(&[doc_id]), 10).unwrap();
        assert_eq!(hits[0].chunk_id, new_chunk);
    }

    #[test]
    fn full_content_is_not_stored() {
        let dir = tempfile::tempdir().unwrap();
        let index = TantivyLexicalIndex::open_writer(dir.path()).unwrap();
        let org_id = Uuid::new_v4();
        let doc_id = Uuid::new_v4();
        let chunk_id = Uuid::new_v4();

        index
            .replace_document_chunks(
                org_id,
                doc_id,
                &[chunk(org_id, doc_id, chunk_id, "不可直接从索引取回的正文")],
            )
            .unwrap();

        let searcher = index.reader.searcher();
        let hits = searcher
            .search(
                &index
                    .build_query(org_id, Some(&[doc_id]), &tokenize_for_query("正文"))
                    .unwrap(),
                &TopDocs::with_limit(1).order_by_score(),
            )
            .unwrap();
        let doc = searcher.doc::<TantivyDocument>(hits[0].1).unwrap();

        assert!(doc.get_first(index.fields.content).is_none());
    }

    #[test]
    fn reader_does_not_create_missing_index() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");

        match TantivyLexicalIndex::open_reader(&missing) {
            Ok(_) => panic!("reader should not create a missing index"),
            Err(_) => {}
        }

        assert!(!missing.exists());
    }
}
