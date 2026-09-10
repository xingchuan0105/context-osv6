use desktop_gpui::markdown::{Block, BlockKind, PreparedMarkdown, list_marker};
use gpui_kit::{
    component::text::{
        MarkdownNode, MarkdownParseContext, MarkdownPlugin, TextView, TextViewStyle, markdown_ast,
    },
    component::*,
    *,
};
use std::sync::Mutex;

/// Extends the existing TextView; source selection continues to use the original Markdown.
pub struct ChatMarkdown {
    namespace: String,
    prepared: Mutex<Option<(String, PreparedMarkdown)>>,
}

impl ChatMarkdown {
    pub fn new(namespace: String) -> Self {
        Self {
            namespace,
            prepared: Mutex::new(None),
        }
    }
}

struct RenderedBlock {
    id: String,
    block: Block,
}

impl MarkdownPlugin for ChatMarkdown {
    fn is_block(&self) -> bool {
        true
    }
    fn name(&self) -> &str {
        "context-chat-markdown"
    }

    fn parse(
        &self,
        node: &markdown_ast::Node,
        cx: &MarkdownParseContext<'_>,
    ) -> Option<MarkdownNode> {
        let kind = match node {
            markdown_ast::Node::List(_) => BlockKind::List,
            markdown_ast::Node::Paragraph(_) => BlockKind::Paragraph,
            markdown_ast::Node::Heading(_) => BlockKind::Heading,
            markdown_ast::Node::Table(_) => BlockKind::Table,
            _ => return None,
        };
        let source = cx.node_source(node)?;
        if kind != BlockKind::List && !source.contains(['*', '_']) {
            return None;
        }
        let position = &node.position()?.start;
        let mut prepared = self.prepared.lock().unwrap();
        if prepared
            .as_ref()
            .is_none_or(|(source, _)| source != cx.source())
        {
            *prepared = Some((cx.source().to_owned(), PreparedMarkdown::new(cx.source())));
        }
        let block = prepared.as_ref()?.1.block(kind, position.offset)?.clone();
        let text = block.text();
        Some(
            MarkdownNode::new(
                self.name(),
                RenderedBlock {
                    id: format!("{}-{}", self.namespace, cx.offset() + position.offset),
                    block,
                },
            )
            .text(text)
            .markdown(source.to_owned()),
        )
    }

    fn render(&self, node: &MarkdownNode, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let data = node.data::<RenderedBlock>().expect("chat Markdown block");
        render_block(&data.block, data.id.clone(), cx)
    }
}

fn render_block(block: &Block, id: String, cx: &App) -> AnyElement {
    match block {
        Block::Html { html, .. } => {
            let mut table = StyleRefinement::default();
            table.overflow.x = Some(Overflow::Scroll);
            TextView::html(SharedString::from(id), html.clone())
                .w_full()
                .min_w_0()
                .style(
                    TextViewStyle::default()
                        .paragraph_gap(rems(0.))
                        .table(table),
                )
                .into_any_element()
        }
        Block::Quote(blocks) => div()
            .w_full()
            .min_w_0()
            .pl_4()
            .border_l_3()
            .border_color(cx.theme().border)
            .children(
                blocks
                    .iter()
                    .enumerate()
                    .map(|(index, block)| render_block(block, format!("{id}-{index}"), cx)),
            )
            .into_any_element(),
        Block::List {
            start,
            tight,
            items,
        } => {
            let mut list = div().flex().flex_col().w_full().min_w_0().gap(if *tight {
                rems(0.)
            } else {
                rems(0.75)
            });
            for (index, item) in items.iter().enumerate() {
                list = list.child(
                    div()
                        .flex()
                        .w_full()
                        .min_w_0()
                        .gap_2()
                        .child(div().flex_shrink_0().min_w(rems(1.5)).child(list_marker(
                            *start,
                            index,
                            item.checked,
                        )))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .gap(if *tight { rems(0.) } else { rems(0.75) })
                                .children(item.blocks.iter().enumerate().map(|(child, block)| {
                                    render_block(block, format!("{id}-{index}-{child}"), cx)
                                })),
                        ),
                );
            }
            list.into_any_element()
        }
    }
}
