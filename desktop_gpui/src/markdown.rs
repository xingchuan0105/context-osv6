//! Source-preserving data for the native TextView extension.
use comrak::{
    Arena, Node, Options, format_html,
    nodes::{ListType, NodeValue},
    parse_document,
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BlockKind {
    Paragraph,
    Heading,
    List,
    Table,
}

#[derive(Clone, Debug)]
pub enum Block {
    Html {
        html: String,
        text: String,
    },
    List {
        start: Option<usize>,
        tight: bool,
        items: Vec<ListItem>,
    },
    Quote(Vec<Block>),
}

#[derive(Clone, Debug)]
pub struct ListItem {
    pub checked: Option<bool>,
    pub blocks: Vec<Block>,
}

impl Block {
    pub fn text(&self) -> String {
        match self {
            Self::Html { text, .. } => text.clone(),
            Self::Quote(blocks) => blocks.iter().map(Self::text).collect::<Vec<_>>().join("\n"),
            Self::List { start, items, .. } => items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let marker = list_marker(*start, index, item.checked);
                    format!(
                        "{marker} {}",
                        item.blocks
                            .iter()
                            .map(Self::text)
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

pub fn list_marker(start: Option<usize>, index: usize, checked: Option<bool>) -> String {
    if let Some(checked) = checked {
        return if checked { "☑" } else { "☐" }.into();
    }
    start
        .map(|start| format!("{}.", start.saturating_add(index)))
        .unwrap_or_else(|| "•".into())
}

pub struct PreparedMarkdown {
    blocks: HashMap<(BlockKind, usize), Block>,
}

impl PreparedMarkdown {
    pub fn new(source: &str) -> Self {
        let arena = Arena::new();
        let mut options = Options::default();
        options.extension.cjk_friendly_emphasis = true;
        options.extension.strikethrough = true;
        options.extension.table = true;
        options.extension.tasklist = true;
        options.extension.autolink = true;
        let root = parse_document(&arena, source, &options);
        // Comrak columns count source bytes; mdast columns expand tabs. Match offsets instead.
        let mut line_starts = vec![0];
        line_starts.extend(source.bytes().enumerate().filter_map(|(offset, byte)| {
            (byte == b'\n' || (byte == b'\r' && source.as_bytes().get(offset + 1) != Some(&b'\n')))
                .then_some(offset + 1)
        }));
        let mut blocks = HashMap::new();
        for node in root.descendants() {
            let kind = match node.data.borrow().value {
                NodeValue::Paragraph => BlockKind::Paragraph,
                NodeValue::Heading(_) => BlockKind::Heading,
                NodeValue::List(_) => BlockKind::List,
                NodeValue::Table(_) => BlockKind::Table,
                _ => continue,
            };
            let start = node.data.borrow().sourcepos.start;
            let offset = line_starts[start.line - 1] + start.column - 1;
            blocks.insert((kind, offset), convert(node, &options));
        }
        Self { blocks }
    }

    pub fn block(&self, kind: BlockKind, offset: usize) -> Option<&Block> {
        self.blocks.get(&(kind, offset))
    }
}

fn convert<'a>(node: Node<'a>, options: &Options) -> Block {
    match &node.data.borrow().value {
        NodeValue::List(list) => Block::List {
            start: (list.list_type == ListType::Ordered).then_some(list.start),
            tight: list.tight,
            items: node
                .children()
                .map(|item| ListItem {
                    checked: match &item.data.borrow().value {
                        NodeValue::TaskItem(task) => Some(task.symbol.is_some()),
                        _ => None,
                    },
                    blocks: item
                        .children()
                        .map(|child| convert(child, options))
                        .collect(),
                })
                .collect(),
        },
        NodeValue::BlockQuote => Block::Quote(
            node.children()
                .map(|child| convert(child, options))
                .collect(),
        ),
        _ => {
            let mut html = String::new();
            format_html(node, options, &mut html).expect("format HTML into String");
            Block::Html {
                html,
                text: plain_text(node),
            }
        }
    }
}

fn plain_text<'a>(node: Node<'a>) -> String {
    match &node.data.borrow().value {
        NodeValue::Text(text) => text.to_string(),
        NodeValue::Code(code) => code.literal.clone(),
        NodeValue::CodeBlock(code) => code.literal.clone(),
        NodeValue::SoftBreak | NodeValue::LineBreak => "\n".into(),
        _ => node.children().map(plain_text).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn html_at(document: &PreparedMarkdown, kind: BlockKind, offset: usize) -> &str {
        let Block::Html { html, .. } = document.block(kind, offset).unwrap() else {
            panic!("HTML block expected")
        };
        html
    }

    #[test]
    fn cjk_emphasis_keeps_original_text_and_resolves_reference_links() {
        let source = "原因是**某种散射（Example）**现象，参考[资料][ref]。\n\n[ref]: https://example.com/docs";
        let original = source.to_owned();
        let document = PreparedMarkdown::new(source);
        let html = html_at(&document, BlockKind::Paragraph, 0);
        assert!(html.contains("<strong>某种散射（Example）</strong>"));
        assert!(html.contains("href=\"https://example.com/docs\""));
        assert_eq!(source, original);
    }

    #[test]
    fn code_and_escaped_markers_remain_literal() {
        let source =
            "`**代码（示例）**文字` 与 \\*\\*转义\\*\\*\n\n```text\n4. **代码（示例）**文字\n```";
        let document = PreparedMarkdown::new(source);
        let html = html_at(&document, BlockKind::Paragraph, 0);
        assert!(html.contains("<code>**代码（示例）**文字</code>"));
        assert!(html.contains("**转义**"));
        assert!(!html.contains("<strong>"));
        assert!(
            document
                .block(BlockKind::List, source.find("4.").unwrap())
                .is_none()
        );
    }

    #[test]
    fn separate_and_nested_lists_keep_their_declared_start() {
        let source = "1. 前项\n\n说明\n\n4. **结果**\n5. 下一项\n\n   8. 嵌套项\n   9. 嵌套后项";
        let document = PreparedMarkdown::new(source);
        let Block::List { start, items, .. } = document
            .block(BlockKind::List, source.find("4.").unwrap())
            .unwrap()
        else {
            panic!("list expected")
        };
        assert_eq!(*start, Some(4));
        assert_eq!(list_marker(*start, 1, None), "5.");
        let Block::List { start: nested, .. } = &items[1].blocks[1] else {
            panic!("nested list expected")
        };
        assert_eq!(*nested, Some(8));
        assert_eq!(list_marker(*nested, 1, None), "9.");
        assert_eq!(list_marker(Some(0), 0, None), "0.");
    }

    #[test]
    fn tables_headings_tasks_and_multiline_items_keep_rich_content() {
        let source = "# **中文（标题）**后文\n\n| 列 |\n| --- |\n| **中文（值）**后文 |\n\n- [x] 完成\n- [ ] 待办\n\n4. 多段\n\n   第二段含**中文（例）**后文";
        let document = PreparedMarkdown::new(source);
        assert!(
            html_at(&document, BlockKind::Heading, 0).contains("<strong>中文（标题）</strong>")
        );
        assert!(
            html_at(&document, BlockKind::Table, source.find("| 列 |").unwrap())
                .contains("<strong>中文（值）</strong>")
        );
        let Block::List { items, .. } = document
            .block(BlockKind::List, source.find("- [x]").unwrap())
            .unwrap()
        else {
            panic!("tasks expected")
        };
        assert_eq!(items[0].checked, Some(true));
        assert_eq!(items[1].checked, Some(false));
        let Block::List { items, .. } = document
            .block(BlockKind::List, source.find("4.").unwrap())
            .unwrap()
        else {
            panic!("list expected")
        };
        assert_eq!(items[0].blocks.len(), 2);
        assert!(
            matches!(&items[0].blocks[1], Block::Html { html, .. } if html.contains("<strong>中文（例）</strong>"))
        );
    }

    #[test]
    fn streaming_prefixes_do_not_leak_into_other_documents() {
        for source in ["因为**示例（E", "因为**示例（Example）**后文", "另一条回答"]
        {
            let document = PreparedMarkdown::new(source);
            let block = document.block(BlockKind::Paragraph, 0).unwrap();
            if source == "另一条回答" {
                assert_eq!(block.text(), source);
            }
        }
    }

    #[test]
    fn native_parser_positions_resolve_to_prepared_blocks() {
        fn check(node: &::markdown::mdast::Node, document: &PreparedMarkdown) {
            use ::markdown::mdast::Node;
            let kind = match node {
                Node::Paragraph(_) => Some(BlockKind::Paragraph),
                Node::Heading(_) => Some(BlockKind::Heading),
                Node::List(_) => Some(BlockKind::List),
                Node::Table(_) => Some(BlockKind::Table),
                _ => None,
            };
            if let Some(kind) = kind {
                let start = &node.position().unwrap().start;
                assert!(
                    document.block(kind, start.offset).is_some(),
                    "missing {kind:?} at {}; keys={:?}",
                    start.offset,
                    document.blocks.keys().collect::<Vec<_>>()
                );
            }
            for child in node.children().into_iter().flatten() {
                check(child, document);
            }
        }
        for source in [
            "因为**中文（E）**后文\r\n\r\n4. **结果**\r\n5. 后文",
            "> 4. **中文（E）**后文\n> 5. 后文\n>\n>    8. 嵌套",
            "4.\t**中文（E）**后文\n5. 下一项",
            "4. **中文（E）**后文\r5. 下一项",
            "# **标题（E）**后文\n\n| 列 |\n| --- |\n| **值（E）**后文 |",
        ] {
            let root = ::markdown::to_mdast(source, &::markdown::ParseOptions::gfm()).unwrap();
            check(&root, &PreparedMarkdown::new(source));
        }
    }
}
