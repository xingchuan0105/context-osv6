# Session Summary Examples

These examples show the expected JSON output format for the `session-summary` skill.
Remember: the runtime only stores the `summary` field. Do NOT emit the legacy fields.

---

## Example 1: Short technical conversation

### Input (recent messages)

```
user: How do I configure TLS for a Rust gRPC server?
assistant: Use tonic with `tonic::transport::Server` and pass a TLS config built from `ServerTlsConfig::new()`.
user: What about mTLS?
assistant: For mTLS, set `client_ca_root` to the PEM-encoded CA cert and use `ClientAuth::Require`.
user: I need to verify the cert is from a specific CA. Can I do that?
assistant: Yes, use `client_auth_roots` and configure `ClientAuth::Require` with the specific root bundle.
user: Thanks! Will this work with rustls or only with OpenSSL?
assistant: tonic supports both backends. For rustls, enable the `tls` feature and use `ServerTlsConfig::new().rustls_server_config(...)`.
```

### Output

```json
{
  "summary": "User is configuring mTLS for a Rust gRPC server using tonic. Active constraints: must use rustls backend (not OpenSSL), must verify client certs are from a specific CA. Next step: implement and test the mTLS handshake."
}
```

---

## Example 2: Multi-turn exploration with pivot

### Input (recent messages)

```
user: I'm thinking about switching from MongoDB to PostgreSQL for our analytics workload.
assistant: That's a significant change. What's driving it?
user: The cost of running Mongo at scale.
assistant: Have you looked at cost optimization for Mongo first?
user: Yeah, but the team's not familiar with it. Postgres seems more standard.
assistant: Team expertise is a real constraint. What's the timeline?
user: Q3. But honestly, on reflection, I think we should stay with Mongo for now. The team expertise matters more than cost savings.
assistant: Understood. Want me to draft a cost-optimization plan for Mongo instead?
user: Yes, focus on indexing and schema denormalization.
```

### Output

```json
{
  "summary": "User decided to stay with MongoDB for analytics (team expertise > cost). Pivoting to Mongo cost optimization for Q3. Active constraints: Q3 deadline, team Mongo expertise. Next step: indexing and schema denormalization plan."
}
```

---

## Example 3: Low-signal chitchat

### Input (recent messages)

```
user: Hey
assistant: Hi! How can I help?
user: Not much, just checking in
assistant: Nice! Anything on your mind?
user: Nope
assistant: Cool, I'm here when you need me.
user: thanks
```

### Output

```json
{
  "summary": "Casual check-in, no actionable content."
}
```

---

## Example 4: Multilingual conversation (Chinese + English terms)

### Input (recent messages)

```
user: 帮我看看这个 Rust 错误：tokio runtime 启动失败，报错 "there is no reactor running"
assistant: 这个错误通常是因为你在非 tokio 上下文中调用了 async 函数。请确认你的 main 函数上有 `#[tokio::main]`。
user: 有的。但我是在一个自定义线程里调用的 reqwest::get。
assistant: 那需要在那个线程里创建一个新的 tokio runtime，或者使用 `Handle::current().block_on(...)`。
user: 明白了，用 `runtime.block_on` 解决了。谢谢！
```

### Output

```json
{
  "summary": "用户在自定义线程中调用 reqwest::get 时遇到 tokio "there is no reactor running" 错误。已通过在新线程中创建 tokio runtime 并使用 `runtime.block_on` 解决。无待办事项。"
}
```
