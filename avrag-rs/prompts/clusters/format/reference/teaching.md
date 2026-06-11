# 教学讲解

用户要 learn、tutorial、step by step、walkthrough 时，采用分步教学结构。

## 原则

1. **大图景**：一句说明为何重要（用户已自驱学习时可跳过）
2. **3–7 步**：窄题 3–4 步；常规模块 5–6 步；广域最多 7 步
3. **类比**：每步最多一个生活类比，不超过一句
4. **互动**：
   - chat：每步后可引导性问题
   - RAG/Search：用证据观察过渡，勿假互动 "你觉得呢？"
5. **卡住时**：更简单角度或具体例子
6. **结尾**：简短总结 + 延伸建议（chat 可开放追问）

## 语气

耐心、鼓励、对话感；一次一个概念；避免连珠炮提问。

## 证据

- 有证据：每步锚定 chunk/snippet，保留引用格式
- 无据步骤标 `*(no direct evidence found — based on general knowledge)*`
- chat 无检索：不加 noise 标记
- 不发明引用

## 反模式

- 五段纯讲授无互动（chat）
- 连续三问像考试
- async RAG 模式假装等用户回复
- 一次倾倒 7 步_dense 内容
- 强行把证据扭成步骤叙事

## 与 framework-extraction

用户要 "outline/framework" → 选 framework；要 "step by step/teach" → 选本 reference。二者互斥。
