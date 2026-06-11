use super::*;

#[test]
fn test_parse_synthesis_output_supports_structured_json() {
    let parsed = parse_synthesis_output(
        r#"{"answer_text":"Rust is a systems language.","cited_chunk_ids":["chunk-1","chunk-2"]}"#,
    );

    assert_eq!(parsed.answer_text, "Rust is a systems language.");
    assert_eq!(
        parsed.cited_chunk_ids,
        vec!["chunk-1".to_string(), "chunk-2".to_string()]
    );
}

#[test]
fn test_parse_synthesis_output_supports_block_schema() {
    let parsed = parse_synthesis_output(
        r#"{
            "answer_blocks": [
                {"type":"text","text":"Rust is a systems language.","citations":["chunk-1"]},
                {"type":"image","chunk_id":"chunk-img"},
                {"type":"text","text":"It emphasizes safety.","citations":["chunk-2","chunk-3"]}
            ],
            "cited_chunk_ids": ["chunk-1","chunk-2","chunk-3","chunk-img"]
        }"#,
    );

    assert_eq!(
        parsed.answer_text,
        "Rust is a systems language. [[1]]\n\n[[image:chunk-img]]\n\nIt emphasizes safety. [[3]] [[4]]"
    );
    assert_eq!(parsed.answer_blocks.len(), 3);
    assert_eq!(
        parsed.cited_chunk_ids,
        vec![
            "chunk-1".to_string(),
            "chunk-img".to_string(),
            "chunk-2".to_string(),
            "chunk-3".to_string()
        ]
    );
}

#[test]
fn test_parse_synthesis_output_falls_back_to_plain_text() {
    let parsed = parse_synthesis_output("plain answer");

    assert_eq!(parsed.answer_text, "plain answer");
    assert!(parsed.cited_chunk_ids.is_empty());
}
