use std::collections::BTreeMap;
use std::io::{Cursor, Write};

use lopdf::{dictionary, Dictionary, Document, Object, Stream};
use zip::write::SimpleFileOptions;

use super::config::{MineruApiMode, MineruConfig, DEFAULT_MINERU_BASE_URL};
use super::fallback::{is_low_value_ocr_document, skipped_ocr_page_unit};
use super::layout::markdown_blocks;
use super::table::{
    build_file_upload_batch_payload_v4, build_file_upload_batch_payload_v4_files,
    extract_markdown_and_images_from_zip, format_page_ranges,
};
use super::upload::{
    prepare_v4_file_upload_payload, prepare_v4_ocr_page_upload, should_use_remote_extract_v4,
};
use super::{NormalizedDocument, ParsedUnit};

#[test]
    fn mineru_config_from_env() {
        unsafe {
            std::env::set_var("MINERU_BASE_URL", "https://mineru.net/api/v4");
            std::env::set_var("MINERU_API_KEY", "test-key");
            std::env::set_var("MINERU_API_MODE", "extract_v4");
        }

        let config = MineruConfig::from_env().unwrap();
        assert_eq!(config.base_url, "https://mineru.net/api/v4");
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.timeout_ms, 30000);
        assert_eq!(config.api_mode, MineruApiMode::ExtractV4);

        unsafe {
            std::env::remove_var("MINERU_BASE_URL");
            std::env::remove_var("MINERU_API_KEY");
            std::env::remove_var("MINERU_API_MODE");
        }
    }

    #[test]
    fn mineru_config_defaults_to_v4_base_url() {
        unsafe {
            std::env::remove_var("MINERU_BASE_URL");
            std::env::set_var("MINERU_API_KEY", "test-key");
            std::env::remove_var("MINERU_API_MODE");
        }

        let config = MineruConfig::from_env().unwrap();
        assert_eq!(config.base_url, DEFAULT_MINERU_BASE_URL);
        assert_eq!(config.api_mode, MineruApiMode::ExtractV4);

        unsafe {
            std::env::remove_var("MINERU_BASE_URL");
            std::env::remove_var("MINERU_API_KEY");
            std::env::remove_var("MINERU_API_MODE");
        }
    }

    #[test]
    fn mineru_config_missing_env() {
        unsafe {
            std::env::remove_var("MINERU_BASE_URL");
            std::env::remove_var("MINERU_API_KEY");
            std::env::remove_var("MINERU_API_MODE");
        }

        let config = MineruConfig::from_env();
        assert!(config.is_none());
    }

    #[test]
    fn markdown_blocks_split_paragraphs() {
        let blocks = markdown_blocks("# Title\n\nParagraph one\n\nParagraph two");
        assert_eq!(blocks.len(), 3);
    }

    #[test]
    fn mineru_v4_zip_accepts_empty_markdown_output() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("full.md", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"").unwrap();
        let bytes = writer.finish().unwrap().into_inner();

        let (markdown, images) = extract_markdown_and_images_from_zip(&bytes, "empty-md").unwrap();

        assert_eq!(markdown, "");
        assert!(images.is_empty());
    }

    #[test]
    fn page_ranges_compact_contiguous_pages() {
        assert_eq!(format_page_ranges(&[1, 2, 3, 5, 8, 9]), "1-3,5,8-9");
    }

    #[test]
    fn v4_source_selection_treats_file_url_as_local_upload() {
        assert!(should_use_remote_extract_v4(Some(
            "https://example.com/demo.pdf"
        )));
        assert!(should_use_remote_extract_v4(Some(
            "http://example.com/demo.pdf"
        )));
        assert!(!should_use_remote_extract_v4(Some(
            "file://tenant/notebook/doc/demo.pdf"
        )));
        assert!(!should_use_remote_extract_v4(None));
    }

    #[test]
    fn v4_file_upload_batch_payload_includes_page_filter_and_ocr_flag() {
        let payload = build_file_upload_batch_payload_v4("demo.pdf", Some(&[3, 1, 2]), true);

        assert_eq!(payload["model_version"], "vlm");
        assert_eq!(payload["files"][0]["name"], "demo.pdf");
        assert_eq!(payload["files"][0]["page_ranges"], "1-3");
        assert_eq!(payload["files"][0]["is_ocr"], true);
    }

    #[test]
    fn v4_file_upload_batch_payload_keeps_ocr_flag_without_page_filter() {
        let payload = build_file_upload_batch_payload_v4("single-page.pdf", None, true);

        assert_eq!(payload["files"][0]["name"], "single-page.pdf");
        assert_eq!(payload["files"][0]["is_ocr"], true);
        assert!(payload["files"][0].get("page_ranges").is_none());
    }

    #[test]
    fn v4_pdf_page_filter_upload_splits_pdf_and_omits_remote_page_ranges() {
        let pdf = two_page_pdf_fixture();

        let upload = prepare_v4_file_upload_payload(&pdf, "demo.pdf", Some(&[2])).unwrap();

        assert!(upload.page_numbers.is_none());
        let split = Document::load_mem(&upload.bytes).unwrap();
        assert_eq!(split.get_pages().len(), 1);
    }

    #[test]
    fn v4_file_upload_batch_payload_supports_multiple_ocr_files() {
        let filenames = vec![
            "demo-page-0001.pdf".to_string(),
            "demo-page-0002.pdf".to_string(),
        ];

        let payload = build_file_upload_batch_payload_v4_files(&filenames, true);

        assert_eq!(payload["model_version"], "vlm");
        assert_eq!(payload["files"].as_array().unwrap().len(), 2);
        assert_eq!(payload["files"][0]["name"], "demo-page-0001.pdf");
        assert_eq!(payload["files"][1]["name"], "demo-page-0002.pdf");
        assert_eq!(payload["files"][0]["is_ocr"], true);
        assert!(payload["files"][0].get("page_ranges").is_none());
    }

    #[test]
    fn v4_ocr_page_upload_skips_blank_pdf_page() {
        let pdf = one_page_pdf_fixture(b"BT ET");

        let upload = prepare_v4_ocr_page_upload(&pdf, "demo.pdf", 1).unwrap();

        assert!(upload.is_none());
    }

    #[test]
    fn v4_ocr_page_upload_keeps_renderable_pdf_page() {
        let pdf = one_page_pdf_fixture(b"q /Im0 Do Q");

        let upload = prepare_v4_ocr_page_upload(&pdf, "demo.pdf", 1)
            .unwrap()
            .unwrap();

        assert_eq!(upload.filename, "demo-page-0001.pdf");
        assert_eq!(upload.page_number, 1);
        assert!(!upload.bytes.is_empty());
    }

    #[test]
    fn skipped_ocr_page_unit_preserves_requested_page() {
        let unit = skipped_ocr_page_unit(42);

        assert_eq!(unit.page, 42);
        assert_eq!(unit.parser_backend, "mineru_pdf_ocr");
        assert_eq!(
            unit.metadata.get("ocr_skipped").map(String::as_str),
            Some("low_value")
        );
    }

    #[test]
    fn v4_low_value_ocr_detection_skips_empty_and_page_number_only_text() {
        let empty = NormalizedDocument {
            title: "empty".to_string(),
            units: Vec::new(),
            metadata: BTreeMap::new(),
        };
        assert!(is_low_value_ocr_document(&empty));

        let page_number_only = NormalizedDocument {
            title: "page".to_string(),
            units: vec![ParsedUnit::new_text(
                1,
                "342".to_string(),
                "mineru_pdf_ocr".to_string(),
            )],
            metadata: BTreeMap::new(),
        };
        assert!(is_low_value_ocr_document(&page_number_only));

        let useful = NormalizedDocument {
            title: "useful".to_string(),
            units: vec![ParsedUnit::new_text(
                1,
                "meaningful OCR text".to_string(),
                "mineru_pdf_ocr".to_string(),
            )],
            metadata: BTreeMap::new(),
        };
        assert!(!is_low_value_ocr_document(&useful));
    }

    fn one_page_pdf_fixture(content: &[u8]) -> Vec<u8> {
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let page_id = document.new_object_id();
        let content_id = document.add_object(Stream::new(Dictionary::new(), content.to_vec()));
        let catalog_id = document.new_object_id();

        document.objects.insert(
            page_id,
            dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
                "Contents" => content_id,
            }
            .into(),
        );
        document.objects.insert(
            pages_id,
            dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => 1,
            }
            .into(),
        );
        document.objects.insert(
            catalog_id,
            dictionary! {
                "Type" => "Catalog",
                "Pages" => pages_id,
            }
            .into(),
        );
        document.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }

    fn two_page_pdf_fixture() -> Vec<u8> {
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let page1_id = document.new_object_id();
        let page2_id = document.new_object_id();
        let content1_id = document.add_object(Stream::new(Dictionary::new(), b"BT ET".to_vec()));
        let content2_id = document.add_object(Stream::new(Dictionary::new(), b"BT ET".to_vec()));
        let catalog_id = document.new_object_id();

        document.objects.insert(
            page1_id,
            dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
                "Contents" => content1_id,
            }
            .into(),
        );
        document.objects.insert(
            page2_id,
            dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
                "Contents" => content2_id,
            }
            .into(),
        );
        document.objects.insert(
            pages_id,
            dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page1_id), Object::Reference(page2_id)],
                "Count" => 2,
            }
            .into(),
        );
        document.objects.insert(
            catalog_id,
            dictionary! {
                "Type" => "Catalog",
                "Pages" => pages_id,
            }
            .into(),
        );
        document.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }
