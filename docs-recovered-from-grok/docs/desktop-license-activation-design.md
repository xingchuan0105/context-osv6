# 桌面端授权与激活设计

> 日期：2026-07-08　状态：Proposed
> 关联 ADR：`docs/adr/0004-desktop-hybrid-business-model.md`

---

## 1. 目标

桌面端通过软件许可（买断制）获取收入，用户在 SaaS 网站购买 license key，在桌面端激活。设计要求：

1. **跳转闭环**：桌面端 → 系统浏览器购买 → 深链回桌面端自动激活
2. **主机-服务器验证**：三层验证（本地签名 / 心跳联网 / 关键操作实时）
3. **离线可用**：断网后 30 天宽限期
4. **浮动设备数**：Standard 1 台 / Pro 3 台，可解绑再绑
5. **7 天试用**：首次启动全功能试用，`device_id` 防重复
6. **大版本权益**：买断覆盖当前大版本（v1.x 终身免费），v2 需重新购买

---

## 2. 防滥用方案：Keygen CE 自托管

### 2.1 为什么选 Keygen CE

| 方案 | 成本 | 结论 |
|------|------|------|
| Keygen CE 自托管 | 免费（复用现有 VPS + Postgres + Redis） | **采纳** |
| Keygen.sh 云服务 | 按 ALU 持续付费（$0.05-0.15/用户/月） | 早期用户少时可接受，但长期成本高 |
| 自建（machineid-rs + Ed25519） | 免费 | 需自己写 seats 管理/吊销/僵尸回收/试用，工程量多 3-4 天 |

Keygen CE 优势：
- Tauri 官方支持（[集成指南](https://keygen.sh/for-tauri-apps/)）
- Rust SDK 现成（`keygen-rs` crate）
- 离线许可证用 Ed25519 签名，桌面端内嵌公钥验签
- 浮动 seats 原生支持
- 试用 license 原生支持
- 大版本权益通过 license `metadata` 字段存

### 2.2 部署架构

```
┌─ SaaS (现有 VPS) ──────────────────────────────┐
│                                                 │
│  avrag-rs (Rust API)       Keygen CE (Docker)   │
│  :8080                     :3001                │
│  ├ Postgres (:5432) ←───── 共用实例              │
│  ├ Redis (:6379) ←──────── 共用实例              │
│  └ Creem/支付宝 checkout                        │
│                                                 │
│  Nginx                                          │
│  ├ /v1/*         → Keygen CE (:3001)            │
│  ├ /api/*        → avrag-rs (:8080)             │
│  └ /            → Next.js (:3000)               │
│                                                 │
└─────────────────────────────────────────────────┘
```

Keygen CE 用独立数据库（`keygen`），与 avrag 的 Postgres 实例共享但不冲突。Redis 用独立 db index（`/1`）。

### 2.3 部署步骤

```bash
# 1. 为 Keygen 创建独立数据库
psql -c "CREATE DATABASE keygen;"

# 2. Keygen CE Docker（web + worker）
# 环境变量由 avrag-rs/.env 统一管理（KEYGEN_* 前缀）
docker compose -f docker-compose.keygen.yml up -d

# 3. 初始化（通过 Keygen console 或 API）
#    - 创建 Product: "AVRag Desktop"
#    - 创建 Policy: "Desktop Pro - Perpetual"（买断, max_machines=3, floating=true）
#    - 创建 Policy: "Desktop Standard - Perpetual"（买断, max_machines=1, floating=true）
#    - 创建 Policy: "Desktop Trial - 7 days"（timed, duration=7d, max_machines=1）
```

Keygen CE 环境变量（写入 `avrag-rs/.env`）：

```env
KEYGEN_EDITION=CE
KEYGEN_MODE=singleplayer
KEYGEN_HOST=license.avrag.com
KEYGEN_ACCOUNT_ID=<uuidgen 生成>
KEYGEN_PRODUCT_TOKEN=<Keygen console 创建 product 时获取>
KEYGEN_LICENSE_TOKEN=<Keygen console 创建 license policy 时获取>
KEYGEN_TRIAL_POLICY_ID=<Keygen console 创建 trial policy 时获取>
KEYGEN_PUBLIC_KEY=<Account.sole.ed25519_public_key，桌面端验签用>
```

### 2.4 Keygen Policy 配置

| Policy | scheme | duration | max_machines | floating | require_fingerprint_scope | metadata |
|--------|--------|----------|-------------|----------|--------------------------|----------|
| Desktop Pro | perpetual | nil | 3 | true | true | `{ major_version_included: 1 }` |
| Desktop Standard | perpetual | nil | 1 | true | true | `{ major_version_included: 1 }` |
| Desktop Trial | timed | 7d | 1 | true | true | `{ major_version_included: 1 }` |

---

## 3. 三层验证机制

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 1: 本地签名校验（离线可用，零延迟）                      │
│  - Ed25519 公钥内嵌二进制，验签离线 license file              │
│  - 检查 expiry、device_id 匹配、major_version_included        │
│  - 每次 chat_stream 前调用                                    │
├─────────────────────────────────────────────────────────────┤
│  Layer 2: 心跳校验（在线时定期联网）                            │
│  - 每 24h POST 到 Keygen CE /v1/licenses/{id}/validate       │
│  - 服务器校验：license 仍 active？device 未被 deactivate？     │
│  - 返回新的离线 certificate（续签）                            │
│  - 后台自动执行，失败不阻塞                                    │
├─────────────────────────────────────────────────────────────┤
│  Layer 3: 关键操作实时校验（在线时即时联网）                     │
│  - 仅在"激活新设备"、"大版本升级检查"时调用                     │
│  - 确保被吊销的 license 无法激活新设备                          │
└─────────────────────────────────────────────────────────────┘
```

### 3.1 License 状态机

```
                    ┌──────────────┐
        首次启动 ──→ │ unactivated  │
                    └──────┬───────┘
                           │ 输入 license_key / 开始试用
                           ↓
                    ┌──────────────┐  exp 到期 + 联网心跳成功  ┌────────┐
                    │   active     │ ──────────────────────→ │ active │
                    │ (买断永不过期) │                          └────────┘
                    └──────┬───────┘  exp 到期 + 断网
                           │            ↓
                           │     ┌──────────────┐  联网续签成功
                           │     │   expired     │ ──────→ active
                           │     │ (只读模式)    │
                           │     └──────────────┘
                           │ server 返回 revoked
                           ↓
                    ┌──────────────┐
                    │   revoked    │ (锁定，不可用)
                    └──────────────┘

        首次启动(无license) ──→ trial(7天) ──到期──→ unactivated
                                       │ 输入 license
                                       └────→ active
```

### 3.2 降级策略

| 状态 | chat_stream 行为 | UI 表现 |
|------|-----------------|---------|
| Active | 正常 | 顶栏 ✓ 正常 |
| Trial | 正常 | 顶栏显示剩余天数 |
| OfflineGrace（断网但 cert 未过期） | 正常 | 黄色角标"离线宽限 X 天" |
| Expired（cert 过期且心跳失败） | **只读**：可查历史，不可新建 | 顶部黄条提示续费 |
| Revoked | **锁定**：不可使用 | 弹窗"授权已被吊销" |
| UpgradeRequired（app_major > included） | v1.x 正常，提示升级 | 提示但不阻塞 |
| Unactivated | 拒绝，引导激活 | 跳转激活页 |

---

## 4. 跳转闭环

### 4.1 三个跳转入口

桌面端激活页通过系统浏览器（非 Tauri WebView）打开 SaaS 页面：

| 入口 | URL | 用途 |
|------|-----|------|
| 购买授权 | `https://app.avrag.com/desktop/buy?device_id={device_id}` | 购买后自动回填 device_id |
| 管理授权 | `https://app.avrag.com/account/licenses` | 查看 license、解绑设备 |
| 帮助 | `https://app.avrag.com/help/desktop-activation` | 激活排错 |

Tauri 实现（`tauri-plugin-shell`）：

```rust
#[tauri::command]
async fn open_in_browser(url: String, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_shell::ShellExt;
    app.shell().open(url, None).map_err(|e| e.to_string())
}
```

### 4.2 深链回流（购买后自动激活）

用户在 SaaS 浏览器购买完成后，SaaS 页面显示 license key + 深链：

```
avrag-desktop://activate?key=AVRG-XXXX-XXXX-XXXX-XXXX
```

点击深链 → 系统拉起桌面端 → 自动填入 key → 触发激活。

Tauri 注册自定义 URL scheme：

```json
// tauri.conf.json
{
  "app": {
    "deepLinks": ["avrag-desktop"]
  }
}
```

```rust
// desktop/src-tauri/src/lib.rs
tauri::Builder::default()
    .plugin(tauri_plugin_deep_link::init())
    .on_deep_link(|event| {
        // 解析 avrag-desktop://activate?key=AVRG-XXXX
        // emit("deep-link-activate", { key }) 给前端
    })
```

---

## 5. Tauri Commands

新增 `desktop/src-tauri/src/commands/license.rs`：

```rust
/// 获取设备指纹（SHA256 of system UUID + CPU cores + drive serial）
#[tauri::command]
fn get_device_id() -> Result<String, String>

/// 开始 7 天试用
#[tauri::command]
async fn start_trial(app: AppHandle) -> Result<TrialResult, LicenseError>

/// 激活正式 license
#[tauri::command]
async fn activate_license(license_key: String, app: AppHandle) -> Result<ActivationResult, LicenseError>

/// 本地校验（Layer 1 - 每次操作前调用，零延迟）
#[tauri::command]
async fn get_license_status(app: AppHandle) -> Result<LicenseStatus, String>

/// 心跳（Layer 2 - 后台每 24h 自动调用）
#[tauri::command]
async fn heartbeat_license(app: AppHandle) -> Result<HeartbeatResult, String>

/// 解绑本机（释放一个 seat）
#[tauri::command]
async fn revoke_this_device(app: AppHandle) -> Result<(), String>

/// 在系统浏览器打开 SaaS 页面
#[tauri::command]
async fn open_in_browser(url: String, app: AppHandle) -> Result<(), String>
```

### 5.1 设备指纹

使用 `machineid-rs` crate（跨平台，无 root 权限）：

```rust
use machineid_rs::{IdBuilder, Encryption, HWIDComponent};

fn get_device_id() -> Result<String, String> {
    let mut builder = IdBuilder::new(Encryption::SHA256);
    builder
        .add_component(HWIDComponent::SystemID)      // 系统 UUID（最稳定）
        .add_component(HWIDComponent::CPUCores)      // 防虚拟机克隆
        .add_component(HWIDComponent::DriveSerial);  // 硬盘序列号
    builder.build("avrag-desktop-salt").map_err(|e| e.to_string())
}
```

组件选择理由：
- `SystemID`：操作系统级 UUID，重装系统不变（macOS IOPlatformUUID / Linux /etc/machine-id / Windows MachineGuid）
- `CPUCores`：物理核心数，防虚拟机批量克隆
- `DriveSerial`：系统盘序列号，换硬盘会变（此时用户需解绑再绑）

### 5.2 本地验签

桌面端内嵌 Keygen 账户的 Ed25519 公钥（`KEYGEN_PUBLIC_KEY`），离线验证 certificate：

```rust
use ed25519_dalek::{VerifyingKey, Signature};

fn verify_certificate(
    cert: &str,
    pubkey: &VerifyingKey,
    device_id: &str,
) -> Result<CertificateClaims, LicenseError> {
    let (payload_b64, sig_b64) = cert.rsplit_once('.').ok_or(LicenseError::Malformed)?;
    let payload = base64_decode(payload_b64)?;
    let sig_bytes = base64_decode(sig_b64)?;
    let sig = Signature::from_slice(&sig_bytes)?;
    pubkey.verify(&payload, &sig)?;   // 验签失败 = 篡改
    let claims: CertificateClaims = serde_json::from_slice(&payload)?;
    if claims.device_id != device_id {
        return Err(LicenseError::DeviceMismatch);
    }
    Ok(claims)
}
```

比 JWT 对称密钥的优势：Ed25519 公钥可安全内嵌二进制（不可推导私钥），即使被逆向提取也无法伪造 license。

### 5.3 License File 持久化

写入 `app_data_dir/license.json`：

```rust
#[derive(Serialize, Deserialize)]
struct LicenseFile {
    key: String,
    license_id: String,
    device_id: String,
    certificate: String,         // Ed25519 签名的离线证书
    kind: LicenseKind,           // Trial | Standard | Pro
    issued_at: i64,
    last_heartbeat: Option<i64>,
}
```

---

## 6. SaaS 侧接口

### 6.1 License 管理代理路由

新增 `transport-http/src/routes/license.rs`，代理到 Keygen CE API（用 admin token）：

| 路由 | 方法 | 说明 |
|------|------|------|
| `/api/v1/licenses/checkout` | POST | 购买（复用 Creem/支付宝） |
| `/api/v1/licenses/me` | GET | 当前用户的 license 列表 |
| `/api/v1/licenses/{id}/machines` | GET | 某 license 的已激活设备 |
| `/api/v1/licenses/{id}/machines/{mid}` | DELETE | 解绑设备 |
| `/api/v1/licenses/trial` | POST | 创建试用 license |

这些是代理接口——Rust 后端用 admin token 调 Keygen CE API，转发结果给前端。不暴露 Keygen token 给前端。

### 6.2 购买流程

```
用户 → (marketing)/desktop/buy → Creem/支付宝 checkout
  → webhook → 后端 → Keygen CE POST /v1/licenses（创建 license，metadata 含 major_version_included）
  → 邮件发 license key
  → SaaS 显示 license key + 深链 avrag-desktop://activate?key=XXX
```

---

## 7. 定价

| 档位 | USD | CNY | 设备数 | 定位 |
|------|-----|-----|--------|------|
| Standard | $39 | ¥299 | 1 | 个人单机 |
| Pro | $99 | ¥699 | 3 + 优先支持 | 多设备/小团队 |

买断制：`expires_at = NULL`，`major_version_included = 1`。v1.x 终身免费升级，v2 需重新购买。

---

## 8. 依赖

```toml
# desktop/src-tauri/Cargo.toml 新增
[dependencies]
keygen-rs = "0.6"                     # Keygen Rust SDK
machineid-rs = "1.2"                  # 跨平台机器指纹
ed25519-dalek = "2.0"                 # 离线签名验证
tauri-plugin-deep-link = "2"          # 深链 avrag-desktop://
base64 = "0.22"
```

---

## 9. 关联

- `docs/adr/0004-desktop-hybrid-business-model.md` — 混合商业模式决策
- `docs/desktop-execution-plan.md` — 总执行计划（WP1-WP3 覆盖本设计）
- Keygen CE 文档：[self-hosting](https://keygen.sh/docs/self-hosting/)、[machine activation](https://keygen.sh/docs/activating-machines/)