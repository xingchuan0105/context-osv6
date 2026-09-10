use super::*;
use desktop_gpui::workspace::{Action, status_label};
use knowledge_view::{Confirm, Destination, Panel};

impl ChatApp {
    pub(super) fn render_overview(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut content = div()
            .id("workspace-overview")
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .p_4()
            .gap_4()
            .overflow_y_scroll()
            .child(div().text_2xl().child("工作区"))
            .child("资料、会话和笔记在工作区内持续保留。")
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .id("workspace-name")
                            .test_support()
                            .flex_1()
                            .min_w_0()
                            .child(Textarea::new(&self.knowledge.name)),
                    )
                    .child(
                        Button::new("create-workspace")
                            .primary()
                            .label("创建工作区")
                            .disabled(
                                self.token.is_none()
                                    || self.knowledge.busy(|a| matches!(a, Action::Create(_))),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.knowledge_action(
                                    Action::Create(
                                        this.knowledge.name.read(cx).value().to_string(),
                                    ),
                                    cx,
                                );
                            })),
                    ),
            )
            .child(
                Button::new("refresh-workspaces")
                    .ghost()
                    .label("刷新工作区")
                    .disabled(self.token.is_none())
                    .on_click(
                        cx.listener(|this, _, _, cx| this.knowledge_action(Action::List, cx)),
                    ),
            );
        if self.token.is_none() {
            content = content.child("请先在个人聊天或本机服务面板连接本机服务。");
        }
        if self.knowledge.busy(|a| matches!(a, Action::List)) {
            content = content.child("正在加载工作区…");
        } else if self.knowledge.workspaces.is_empty() {
            content = content.child("还没有工作区，创建一个开始整理资料。");
        }
        if let Some(error) = &self.knowledge.error {
            content = content.child(div().child(error.clone()));
        }
        for workspace in &self.knowledge.workspaces {
            let item = workspace.clone();
            content = content.child(
                Button::new(SharedString::from(format!("workspace-{}", item.id)))
                    .ghost()
                    .w_full()
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .text_ellipsis()
                            .child(format!("{} · {} 份资料", item.name, item.document_count)),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.navigate(Destination::Workspace(item.clone()), window, cx)
                    })),
            );
        }
        content
    }

    pub(super) fn render_confirm(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let leaving = matches!(self.knowledge.confirm, Some(Confirm::Leave(_)));
        div()
            .id("knowledge-confirm")
            .flex_1()
            .min_w_0()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(if leaving {
                "笔记有未保存的修改"
            } else {
                "确认删除这项内容？"
            })
            .child(if leaving {
                "继续编辑并保存，或放弃本次笔记修改后离开。"
            } else {
                "删除后无法从当前工作区恢复。"
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("cancel-knowledge-confirm")
                            .label(if leaving { "继续编辑" } else { "取消" })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.knowledge.confirm = None;
                                window.focus(&this.knowledge.note_content.focus_handle(cx), cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("accept-knowledge-confirm")
                            .label(if leaving {
                                "放弃修改并离开"
                            } else {
                                "确认删除"
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                match this.knowledge.confirm.take() {
                                    Some(Confirm::Leave(to)) => this.navigate_now(to, window, cx),
                                    Some(Confirm::Delete(action)) => {
                                        this.knowledge_action(action, cx)
                                    }
                                    None => {}
                                }
                                cx.notify();
                            })),
                    ),
            )
    }

    pub(super) fn render_knowledge_panel(
        &self,
        wide: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let panel = self.knowledge.panel.unwrap_or(Panel::Documents);
        let mut content = div()
            .id("knowledge-panel")
            .flex()
            .flex_col()
            .h_full()
            .min_h_0()
            .min_w_0()
            .p_4()
            .gap_3()
            .when(wide, |v| {
                v.w(px(336.))
                    .flex_shrink_0()
                    .border_l_1()
                    .border_color(cx.theme().border)
            })
            .when(!wide, |v| v.flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(match panel {
                        Panel::Documents => "工作区资料",
                        Panel::Notes => "工作区笔记",
                        Panel::Preview => "来源原文",
                    })
                    .child(
                        Button::new("close-knowledge-panel")
                            .ghost()
                            .label("收起")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.knowledge.panel = None;
                                cx.notify();
                            })),
                    ),
            );
        if let Some(error) = &self.knowledge.error {
            content = content.child(div().text_sm().child(error.clone()));
        }
        let mut body = div()
            .id("knowledge-panel-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .gap_3();
        match panel {
            Panel::Documents => {
                body =
                    body.child(
                        Button::new("refresh-materials")
                            .ghost()
                            .label("刷新资料")
                            .disabled(self.knowledge.busy(|a| matches!(a, Action::Load)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.knowledge_action(Action::Load, cx)
                            })),
                    )
                    .child(div().text_sm().child(
                        if self.knowledge.selected.is_empty() {
                            "问答范围：当前工作区全部可检索资料".to_string()
                        } else {
                            format!("问答范围：已选 {} 份资料", self.knowledge.selected.len())
                        },
                    ));
                for (id, upload) in &self.knowledge.uploads {
                    if Some(&upload.workspace) != self.knowledge.active.as_ref().map(|w| &w.id) {
                        continue;
                    }
                    if !upload.running && upload.error.is_none() {
                        continue;
                    }
                    let path = upload.path.clone();
                    let mut row = div()
                        .min_w_0()
                        .child(
                            div().text_ellipsis().child(
                                path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string(),
                            ),
                        )
                        .child(if upload.running {
                            if upload.document_id.is_some() {
                                "正在提交…"
                            } else {
                                "正在上传…"
                            }
                            .to_string()
                        } else {
                            upload.error.clone().unwrap_or_default()
                        });
                    if !upload.running {
                        let id = *id;
                        let retry = upload
                            .document_id
                            .as_ref()
                            .map(|id| Action::CompleteUpload(id.clone()))
                            .unwrap_or_else(|| Action::Upload(path.clone()));
                        let completing = upload.document_id.is_some();
                        row = row.child(
                            Button::new(SharedString::from(format!("retry-upload-{id}")))
                                .label(if completing {
                                    "重试提交"
                                } else {
                                    "重试上传"
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !completing {
                                        this.knowledge.uploads.remove(&id);
                                    }
                                    this.knowledge_action(retry.clone(), cx);
                                })),
                        );
                    }
                    body = body.child(row);
                }
                if self.knowledge.documents.is_empty() {
                    body = body.child("还没有资料，使用顶栏“添加资料”选择文件。");
                }
                for document in &self.knowledge.documents {
                    let id = document.id.clone();
                    let preview_id = id.clone();
                    let delete_id = id.clone();
                    let retry_id = id.clone();
                    let ready = document.status == "completed";
                    let mut row = div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .gap_2()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .pb_3()
                        .child(div().text_ellipsis().child(document.file_name.clone()))
                        .child(div().text_sm().child(status_label(&document.status)))
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_2()
                                .child(
                                    Button::new(SharedString::from(format!("select-doc-{id}")))
                                        .ghost()
                                        .label(if self.knowledge.selected.contains(&id) {
                                            "已选"
                                        } else {
                                            "限定范围"
                                        })
                                        .selected(self.knowledge.selected.contains(&id))
                                        .disabled(!ready)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if !this.knowledge.selected.remove(&id) {
                                                this.knowledge.selected.insert(id.clone());
                                            }
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "preview-doc-{preview_id}"
                                    )))
                                    .ghost()
                                    .label("查看原文")
                                    .disabled(!ready)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.preview_document(preview_id.clone(), cx)
                                    })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "delete-doc-{delete_id}"
                                    )))
                                    .ghost()
                                    .label("删除")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.knowledge.confirm = Some(Confirm::Delete(
                                            Action::DeleteDocument(delete_id.clone()),
                                        ));
                                        cx.notify();
                                    })),
                                ),
                        );
                    if matches!(document.status.as_str(), "failed" | "upload_invalid") {
                        row = row.child(
                            Button::new(SharedString::from(format!("reindex-doc-{retry_id}")))
                                .label("重新处理")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.knowledge_action(Action::Reindex(retry_id.clone()), cx)
                                })),
                        );
                    }
                    if document.status == "pending"
                        && !self
                            .knowledge
                            .uploads
                            .values()
                            .any(|upload| upload.document_id.as_deref() == Some(&document.id))
                    {
                        let id = document.id.clone();
                        row = row.child(Button::new(SharedString::from(format!("complete-doc-{id}")))
                            .label("继续提交")
                            .disabled(self.knowledge.busy(|a| matches!(a, Action::CompleteUpload(pending) if pending == &id)))
                            .on_click(cx.listener(move |this, _, _, cx| this.knowledge_action(Action::CompleteUpload(id.clone()), cx))));
                    }
                    body = body.child(row);
                }
            }
            Panel::Notes => {
                let saving = self
                    .knowledge
                    .busy(|a| matches!(a, Action::SaveNote { .. } | Action::DeleteNote(_)));
                body = body.child(Button::new("new-note").ghost().label("新建笔记").on_click(
                    cx.listener(|this, _, window, cx| {
                        this.navigate(Destination::Note(None), window, cx)
                    }),
                ));
                for note in &self.knowledge.notes {
                    let note = note.clone();
                    body =
                        body.child(
                            Button::new(SharedString::from(format!("note-{}", note.id)))
                                .ghost()
                                .w_full()
                                .child(div().w_full().text_ellipsis().child(
                                    if note.title.is_empty() {
                                        "未命名笔记".into()
                                    } else {
                                        note.title.clone()
                                    },
                                ))
                                .selected(self.knowledge.note_id.as_deref() == Some(&note.id))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.navigate(Destination::Note(Some(note.clone())), window, cx)
                                })),
                        );
                }
                body = body
                    .child(
                        div()
                            .id("note-title")
                            .test_support()
                            .child(Textarea::new(&self.knowledge.note_title).disabled(saving)),
                    )
                    .child(
                        div().id("note-content").test_support().child(
                            Textarea::new(&self.knowledge.note_content)
                                .disabled(saving)
                                .h(px(220.)),
                        ),
                    )
                    .child(
                        Button::new("save-note")
                            .primary()
                            .label("保存笔记")
                            .disabled(self.token.is_none() || saving)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.knowledge_action(
                                    Action::SaveNote {
                                        id: this.knowledge.note_id.clone(),
                                        title: this
                                            .knowledge
                                            .note_title
                                            .read(cx)
                                            .value()
                                            .to_string(),
                                        content: this
                                            .knowledge
                                            .note_content
                                            .read(cx)
                                            .value()
                                            .to_string(),
                                    },
                                    cx,
                                );
                            })),
                    );
                if let Some(id) = &self.knowledge.note_id {
                    let id = id.clone();
                    body = body.child(
                        Button::new("delete-note")
                            .ghost()
                            .label("删除笔记")
                            .disabled(saving)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.knowledge.confirm =
                                    Some(Confirm::Delete(Action::DeleteNote(id.clone())));
                                cx.notify();
                            })),
                    );
                }
            }
            Panel::Preview => {
                if let Some((id, text)) = &self.knowledge.preview {
                    if self.knowledge.busy(|a| matches!(a, Action::Preview(_))) {
                        body = body.child("正在读取原文…");
                    } else {
                        body = body.child(
                            TextView::markdown(
                                SharedString::from(format!("source-{id}")),
                                text.clone(),
                            )
                            .w_full()
                            .min_w_0(),
                        );
                        let id = id.clone();
                        body = body.child(
                            Button::new("reload-source")
                                .ghost()
                                .label("重新读取原文")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.preview_document(id.clone(), cx)
                                })),
                        );
                    }
                }
            }
        }
        content.child(body.test_support())
    }

    pub(super) fn citation_row(
        &self,
        key: &str,
        citations: &[web_sdk::CitationView],
        cx: &mut Context<Self>,
    ) -> Div {
        let mut row = div()
            .w_full()
            .max_w(px(760.))
            .min_w_0()
            .flex()
            .flex_wrap()
            .gap_2();
        for (index, citation) in citations.iter().enumerate() {
            let doc = citation.doc_id.clone();
            row = row.child(
                Button::new(SharedString::from(format!("citation-{key}-{index}")))
                    .ghost()
                    .label(format!("[{}] {}", index + 1, citation.doc_name))
                    .disabled(citation.tombstone || doc.is_empty())
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.preview_document(doc.clone(), cx)),
                    ),
            );
        }
        row
    }
}
