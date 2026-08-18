[lead_workers_handoff]
通道检索已结束。宿主共收集 {n_packs} 个 EvidencePack。

本轮进入合成：用户可见终答为自然语言 prose，关键事实应对齐 pack 中的 evidence；网页引用 `[[web:n]]`，知识库侧 `（#n）` / SELECTED 族。有命中的主张直接作答；未覆盖子问用普通人话说明缺口，必要时向用户澄清，勿在已有部分命中时整题拒答。多证据冲突时并陈并标明材料位置（正文/图/表/网页层级）。不用预训练知识补关键数字或实体。若上下文中已有 calculator / weather_query / user_context 的 ok 结果，直接采用该 observation。
