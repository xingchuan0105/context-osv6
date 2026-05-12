# Claude Desktop 导入自定义 Skills / Prompts / MCP 服务器调研报告

## 1. claude_desktop_config.json 的 mcpServers 配置方式

### 配置文件位置
- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
- **Linux (第三方打包)**: `~/.config/Claude/claude_desktop_config.json`

### mcpServers 配置格式
```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/home/user/documents"]
    },
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "ghp_xxxxxxxxxxxx"
      }
    },
    "sqlite": {
      "command": "uvx",
      "args": ["mcp-server-sqlite", "--db-path", "/home/user/data.db"]
    }
  }
}
```

### 关键配置要点
- 每个 MCP 服务器需要一个唯一的名称作为 key
- `command`: 启动服务器的命令（如 `npx`, `uvx`, `python`, `node` 等）
- `args`: 传递给命令的参数数组
- `env`: 可选的环境变量配置
- 配置后需要重启 Claude Desktop 生效
- 可用 `python3 -m json.tool ~/.config/Claude/claude_desktop_config.json` 验证 JSON 语法

---

## 2. Claude Desktop 是否有 prompts 或 skills 目录

### 官方 Skills 系统
Anthropic 官方维护了一个 Skills 仓库 (`anthropics/skills`)，但**这是面向 Claude API / Claude Code 的**，不是 Claude Desktop 原生支持的。

### Skills 目录结构（API/Claude Code 用）
```
skill-name/
├── SKILL.md (required)
│   ├── YAML frontmatter (name, description required)
│   └── Markdown instructions
└── Bundled Resources (optional)
    ├── scripts/    - Executable code for deterministic/repetitive tasks
    ├── references/ - Docs loaded into context as needed
    └── assets/     - Files used in output (templates, icons, fonts)
```

### Claude Desktop 的导入入口
Claude Desktop **没有原生的 prompts/skills 目录机制**。用户可用的导入入口包括：

1. **MCP 服务器** (mcpServers) - 通过 claude_desktop_config.json 配置
2. **Projects** - Claude Desktop 内置的 Projects 功能，可在项目中附加文件作为上下文
3. **Custom Instructions** - 设置中的 Custom Instructions（自定义指令）
4. **Claude Code / API** - 通过 Claude Code CLI 或 API 使用 Skills 系统

### Claude Desktop vs Claude Code 的 Skills 支持对比

| 功能 | Claude Desktop | Claude Code |
|------|---------------|-------------|
| MCP 服务器 | ✅ mcpServers 配置 | ✅ 支持 |
| Skills 目录 | ❌ 无原生支持 | ✅ `.claude/skills/` |
| Prompts 目录 | ❌ 无原生支持 | ✅ 可通过 skills 实现 |
| Custom Agents | ❌ 有限 | ✅ 完整支持 |
| Custom Instructions | ✅ 设置中支持 | ✅ 支持 |

---

## 3. 社区方案：将 Markdown Skills 转换为 MCP 工具或系统提示

### 社区项目发现

#### 1. claude-desktop-debian (Linux 第三方打包)
- GitHub: `aaddrick/claude-desktop-debian`
- 将 Windows 版 Claude Desktop 重新打包为 Linux .deb/AppImage
- 完全支持 MCP，配置文件位于 `~/.config/Claude/claude_desktop_config.json`
- 提供了 `claude-desktop --doctor` 诊断工具

#### 2. MCP Think Tool
- GitHub: `cgize/claude-mcp-think-tool`
- 一个 MCP 服务器，实现了 Anthropic 的 "think" 工具
- 增强 Claude 的复杂推理和策略遵循能力
- 展示了如何将特定功能包装为 MCP 工具

### 将 Markdown Skills 转换为 MCP 的社区方案

目前**没有发现成熟的社区工具**可以直接将 Markdown Skills 转换为 MCP 服务器。但存在以下可行路径：

#### 路径 A: 将 Skill 内容转为 MCP Prompts
MCP 协议本身支持 `prompts` 能力，一个 MCP 服务器可以暴露 prompts：
```typescript
// MCP 服务器可以暴露 prompts
const { messages } = await mcpClient.getPrompt({ name: "my-prompt" });
```
Anthropic TypeScript SDK 提供了辅助函数：
```typescript
import { mcpMessages, mcpTools } from "@anthropic-ai/sdk/helpers/beta/mcp";
```

#### 路径 B: 将 Skill 作为 Custom Instructions
- 将 SKILL.md 的内容复制到 Claude Desktop 的 Custom Instructions 设置中
- 适用于简单的行为规则/系统提示

#### 路径 C: 构建自定义 MCP 服务器
- 将 Markdown Skills 的内容作为 MCP 服务器的 prompts/resources 暴露
- 需要自行开发一个 MCP 服务器来读取 `.md` 文件并提供给 Claude
- 参考: `@modelcontextprotocol/server-filesystem` 可以读取文件，但无法直接作为 prompts

#### 路径 D: 使用 Claude Code 的 Skills 系统
- Claude Code CLI 支持 `.claude/skills/` 目录
- 如果用户需要完整的 Skills 功能，建议使用 Claude Code 而非 Claude Desktop

---

## 4. 关键结论

### Claude Desktop 支持的外部导入入口
1. **MCP 服务器** - 最强大、最灵活的扩展方式
2. **Projects 文件附件** - 将文档作为上下文附加到特定项目
3. **Custom Instructions** - 全局或项目级的自定义系统提示
4. **Settings/Hooks** - 有限的自定义行为配置

### 不支持的功能
- ❌ 没有原生的 `prompts/` 目录
- ❌ 没有原生的 `skills/` 目录
- ❌ 不能直接加载 `SKILL.md` 文件
- ❌ 没有内置的 Markdown Skills 解析器

### 推荐方案
如果用户想将外部 Skills 导入 Claude Desktop：

1. **最简单**: 将 Skill 内容复制到 Custom Instructions 中
2. **最灵活**: 开发一个自定义 MCP 服务器，将 Skills 内容作为 prompts/resources 暴露
3. **最完整**: 使用 Claude Code CLI，它原生支持 `.claude/skills/` 目录
4. **折中**: 使用 Projects 功能，将 Skill 文件附加到项目中作为参考上下文

---

## 参考链接
- Anthropic Skills Repo: https://github.com/anthropics/skills
- Claude Desktop for Linux: https://github.com/aaddrick/claude-desktop-debian
- MCP Specification: https://modelcontextprotocol.io/
- Claude API Docs (MCP): https://platform.claude.com/docs/en/agents-and-tools/mcp-connector
