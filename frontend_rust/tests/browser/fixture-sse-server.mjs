// 受控 SSE 测试服务器（Node 标准库，无依赖）。
// - POST /api/v1/chat            → 按 stream-long-3000 夹具分块写出真实 SSE
//   ?split=chunks（默认，字符对齐 chunk）| ?split=bytes&n=113（字节切片，制造 UTF-8 跨包）
// - POST /case/401/api/v1/chat   → 401 JSON 错误
// - POST /case/bad-json/api/v1/chat → 200 但第一帧即坏 JSON
// - POST /case/slow/api/v1/chat  → start 后慢速 token 流，永不主动结束（验证 stop/abort）
// - POST /case/markdown/api/v1/chat → 含标题/列表/恶意 HTML 的短答案（W2 Markdown）
// - POST /case/citations/api/v1/chat → 含 [[1]] marker 与 citations 事件（W2 引用）
// - POST /case/progress/api/v1/chat → 一条 activity + reasoning，短答案（W2 终态折叠）
// - /case/files/*                → Session files 签名上传（W2.4）
// - /case/files-busy/*           → 列表里有一条 processing，发送闸
// - GET  /admin/state            → { aborted, requests, bytesWritten, lastChatBody }
// - POST /admin/reset            → 重置上述状态
// CORS 全放行（页面与夹具不同源，Authorization 头触发预检）。
import http from 'node:http';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const fixturePath =
  process.env.FIXTURE_CHUNKS_JSON ||
  join(here, '..', 'fixtures', 'stream-long-3000.chunks.json');
const chunks = JSON.parse(readFileSync(fixturePath, 'utf8')).chunks;
const wire = chunks.join('');
const wireBytes = Buffer.from(wire, 'utf8');

const PORT = Number(process.env.FIXTURE_PORT || 3201);
const CHUNK_DELAY_MS = Number(process.env.CHUNK_DELAY_MS || 15);

const state = {
  aborted: false,
  requests: 0,
  bytesWritten: 0,
  lastAuthorization: null,
  lastChatBody: null,
};
const filesState = { file: null, events: [] };
let secretsState = [];
let workspacesState = [
  {
    id: 'ws-materials',
    owner_user_id: 'fixture-user',
    owner_id: 'fixture-user',
    name: '材料研发知识库',
    title: '材料研发知识库',
    description: '特种合金与复合材料',
    created_at: '2026-09-01T00:00:00Z',
    updated_at: '2026-09-04T00:00:00Z',
    document_count: 1,
    status_summary: {},
    shared: false,
  },
];
let workspaceDocsState = [
  {
    id: 'doc-titanium',
    owner_user_id: 'fixture-user',
    owner_id: 'fixture-user',
    workspace_id: 'ws-materials',
    file_name: 'titanium-spec.pdf',
    mime_type: 'application/pdf',
    file_size: 1024,
    status: 'completed',
    chunk_count: 12,
    created_at: '2026-09-02T00:00:00Z',
    updated_at: '2026-09-02T00:00:00Z',
  },
];
let workspaceNotesState = [
  {
    id: 'note-weekly',
    workspace_id: 'ws-materials',
    title: '周会讨论要点',
    content: '# 周会讨论\n- 重点跟进抗拉强度',
    preview: '重点跟进抗拉强度',
    created_at: '2026-09-03T00:00:00Z',
    updated_at: '2026-09-03T00:00:00Z',
  },
];

// E3.5 管理后台夹具：accounts / users / flags / change-requests / audit-logs / broadcasts。
function seedAdminState() {
  const accounts = [];
  for (let i = 1; i <= 12; i += 1) {
    const id = `ow-${i}`;
    accounts.push({
      id,
      name: i === 1 ? 'Acme 研发中心' : `测试账户 ${String(i).padStart(2, '0')}`,
      created_at: 1725148800,
      blocked: i === 2,
      user_count: i,
      document_count: i * 3,
      query_count: i * 11,
    });
  }
  return {
    accounts,
    users: {
      'ow-1': [
        { id: 'u-1', email: 'alice@acme.dev', role: 'super_admin', created_at: 1725148800 },
        { id: 'u-2', email: 'bob@acme.dev', role: 'member', created_at: 1725235200 },
      ],
      'ow-3': [
        { id: 'u-3', email: 'carol@nano.dev', role: 'member', created_at: 1725321600 },
      ],
    },
    flags: [
      {
        key: 'rag.offline',
        category: 'rag',
        description: '离线检索降级开关',
        enabled: true,
        effective_enabled: true,
        config_ready: true,
        requires_config: false,
        source: 'default',
        updated_at: null,
        has_pending_request: false,
      },
      {
        key: 'agent.heavytail',
        category: 'agent',
        description: '重尾任务专用 Agent',
        enabled: false,
        effective_enabled: false,
        config_ready: true,
        requires_config: true,
        source: 'db',
        updated_at: 1725148800,
        has_pending_request: true,
      },
    ],
    requests: [
      {
        id: 'req-1',
        flag_key: 'agent.heavytail',
        current_enabled: false,
        requested_enabled: true,
        reason: '灰度上线',
        status: 'pending',
        requested_by: 'ow-1',
        reviewed_by: null,
        review_note: null,
        created_at: 1725148800,
        reviewed_at: null,
        executed_at: null,
      },
    ],
    auditLogs: [
      {
        id: 1,
        actor_id: 'adm-1',
        action: 'account.block',
        resource_type: 'account',
        resource_id: 'ow-2',
        owner_user_id: 'ow-2',
        created_at: 1725485000,
      },
      {
        id: 2,
        actor_id: 'adm-1',
        action: 'user.delete',
        resource_type: 'user',
        resource_id: 'u-9',
        owner_user_id: 'ow-1',
        created_at: 1725484000,
      },
      {
        id: 3,
        actor_id: 'adm-2',
        action: 'flag.review',
        resource_type: 'feature_flag',
        resource_id: 'agent.heavytail',
        owner_user_id: null,
        created_at: 1720000000,
      },
    ],
    broadcasts: [],
  };
}
let adminState = seedAdminState();

function adminOk(res, data) {
  jsonOk(res, { data, ok: true });
}

function adminErr(res, status, code, message) {
  res.writeHead(status, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({ data: null, error: { code, message }, ok: false }));
}

function fileEvent(kind) {
  filesState.events.push(kind);
}

function jsonOk(res, body) {
  cors(res);
  res.writeHead(200, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify(body));
}

function cors(res) {
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Headers', '*');
  res.setHeader('Access-Control-Allow-Methods', 'POST, GET, PUT, DELETE, OPTIONS');
}

function sessionJson(id, title) {
  return {
    id,
    owner_user_id: 'fixture-user',
    scope_kind: 'personal',
    title: title || null,
    agent_type: 'chat',
    model_role: 'quick_chat',
    created_at: '2026-09-04T00:00:00Z',
    updated_at: '2026-09-04T00:00:00Z',
  };
}

function busyFileRow() {
  return {
    binding_id: 'bind-busy-1',
    document_id: 'doc-busy-1',
    file_name: 'busy.pdf',
    mime_type: 'application/pdf',
    file_size: 12,
    status: 'processing',
    created_at: '2026-09-04T00:00:00Z',
  };
}

function readBody(req) {
  return new Promise((resolve) => {
    const chunks = [];
    req.on('data', (chunk) => chunks.push(chunk));
    req.on('end', () => resolve(Buffer.concat(chunks)));
  });
}

function sseHead(res) {
  cors(res);
  res.writeHead(200, {
    'Content-Type': 'text/event-stream',
    'Cache-Control': 'no-cache',
    Connection: 'keep-alive',
  });
}

function writePiece(res, piece) {
  const buf = Buffer.isBuffer(piece) ? piece : Buffer.from(piece, 'utf8');
  state.bytesWritten += buf.length;
  res.write(buf);
}

function watchAbort(req, res, isDone) {
  // 请求体读完后 IncomingMessage 可能已经 close；客户端停流要看响应/socket。
  const mark = () => {
    if (!isDone()) state.aborted = true;
  };
  res.on('close', mark);
  res.on('error', mark);
  req.on('aborted', mark);
}

function streamFixture(req, res, url) {
  const mode = url.searchParams.get('split') || 'chunks';
  const n = Math.max(1, Number(url.searchParams.get('n') || 113));
  const pieces = mode === 'bytes'
    ? (() => {
        const out = [];
        for (let i = 0; i < wireBytes.length; i += n) out.push(wireBytes.subarray(i, i + n));
        return out;
      })()
    : chunks.map((c) => Buffer.from(c, 'utf8'));

  sseHead(res);
  let finished = false;
  watchAbort(req, res, () => finished);
  let i = 0;
  const timer = setInterval(() => {
    if (res.writableEnded || res.destroyed) {
      clearInterval(timer);
      return;
    }
    if (i >= pieces.length) {
      finished = true;
      clearInterval(timer);
      res.end();
      return;
    }
    writePiece(res, pieces[i]);
    i += 1;
    if (i >= pieces.length) {
      // chunks.json 末事件故意无结尾空行（覆盖 EOF flush）。Next 解析器
      // 不会把无换行的残留 buffer 当成 data 字段，done 不会落地、pending
      // 一直为 true。这里补一帧结束符，两边都看到完整 last event。
      writePiece(res, '\n\n');
    }
  }, CHUNK_DELAY_MS);
}

function streamMarkdown(req, res) {
  sseHead(res);
  const body = [
    '# 标题',
    '',
    '- 一项',
    '',
    '<script>alert(1)</script>',
    '<img src=x onerror="alert(1)">',
    '[xss](javascript:alert(1))',
    '',
    '安全段落。',
  ].join('\n');
  writePiece(
    res,
    `event: start\ndata: ${JSON.stringify({ request_id: 'req-md-1', session_id: 'sess-md-1' })}\n\n`,
  );
  writePiece(
    res,
    `event: answer_start\ndata: ${JSON.stringify({
      request_id: 'req-md-1',
      session_id: 'sess-md-1',
      message_id: 41,
      agent_type: 'chat',
    })}\n\n`,
  );
  writePiece(
    res,
    `event: token\ndata: ${JSON.stringify({
      request_id: 'req-md-1',
      message_id: 41,
      content: body,
    })}\n\n`,
  );
  // Done 的 answer 会覆盖流式文本；必须带同一份 Markdown，不能写成 "ok"。
  writePiece(
    res,
    `event: done\ndata: ${JSON.stringify({
      request_id: 'req-md-1',
      session_id: 'sess-md-1',
      message_id: 41,
      payload: {
        answer: body,
        answer_blocks: [],
        session_id: 'sess-md-1',
        agent_type: 'chat',
        sources: [],
        citations: [],
        trace: { mode: 'chat' },
        degrade_trace: [],
      },
    })}\n\n`,
  );
  res.end();
}

function streamShortOk(req, res, sessionId) {
  sseHead(res);
  writePiece(
    res,
    `event: start\ndata: ${JSON.stringify({ request_id: 'req-scope-1', session_id: sessionId })}\n\n`,
  );
  writePiece(
    res,
    `event: answer_start\ndata: ${JSON.stringify({
      request_id: 'req-scope-1',
      session_id: sessionId,
      message_id: 51,
      agent_type: 'chat',
    })}\n\n`,
  );
  writePiece(
    res,
    `event: token\ndata: ${JSON.stringify({
      request_id: 'req-scope-1',
      message_id: 51,
      content: '已收到。',
    })}\n\n`,
  );
  writePiece(
    res,
    `event: done\ndata: ${JSON.stringify({
      request_id: 'req-scope-1',
      session_id: sessionId,
      message_id: 51,
      payload: {
        answer: '已收到。',
        answer_blocks: [],
        session_id: sessionId,
        agent_type: 'chat',
        sources: [],
        citations: [],
        trace: { mode: 'chat' },
        degrade_trace: [],
      },
    })}\n\n`,
  );
  res.end();
}

function streamProgress(req, res) {
  sseHead(res);
  const body = '短答案。';
  writePiece(
    res,
    `event: start\ndata: ${JSON.stringify({ request_id: 'req-prog-1', session_id: 'sess-prog-1' })}\n\n`,
  );
  writePiece(
    res,
    `event: answer_start\ndata: ${JSON.stringify({
      request_id: 'req-prog-1',
      session_id: 'sess-prog-1',
      message_id: 61,
      agent_type: 'chat',
    })}\n\n`,
  );
  writePiece(
    res,
    `event: activity\ndata: ${JSON.stringify({
      request_id: 'req-prog-1',
      phase: 'compose',
      title: '组织短答',
      detail: '先列要点',
    })}\n\n`,
  );
  writePiece(
    res,
    `event: reasoning_summary_delta\ndata: ${JSON.stringify({
      request_id: 'req-prog-1',
      message_id: 61,
      content: '因果链已对齐。',
    })}\n\n`,
  );
  writePiece(
    res,
    `event: token\ndata: ${JSON.stringify({
      request_id: 'req-prog-1',
      message_id: 61,
      content: body,
    })}\n\n`,
  );
  writePiece(
    res,
    `event: done\ndata: ${JSON.stringify({
      request_id: 'req-prog-1',
      session_id: 'sess-prog-1',
      message_id: 61,
      payload: {
        answer: body,
        answer_blocks: [],
        session_id: 'sess-prog-1',
        agent_type: 'chat',
        sources: [],
        citations: [],
        trace: { mode: 'chat' },
        degrade_trace: [],
      },
    })}\n\n`,
  );
  res.end();
}

function streamCitations(req, res) {
  sseHead(res);
  const body = '结论见 [[1]]。\n\n<script>alert(1)</script>\n\n安全段落。';
  const citations = [
    {
      citation_id: 1,
      doc_id: 'doc-handbook',
      chunk_id: 'chunk-a',
      doc_name: '手册',
      preview: '背压与窗口',
      score: 0.6,
    },
  ];
  writePiece(
    res,
    `event: start\ndata: ${JSON.stringify({ request_id: 'req-cite-1', session_id: 'sess-cite-1' })}\n\n`,
  );
  writePiece(
    res,
    `event: answer_start\ndata: ${JSON.stringify({
      request_id: 'req-cite-1',
      session_id: 'sess-cite-1',
      message_id: 51,
      agent_type: 'chat',
    })}\n\n`,
  );
  writePiece(
    res,
    `event: citations\ndata: ${JSON.stringify({
      request_id: 'req-cite-1',
      message_id: 51,
      citations,
    })}\n\n`,
  );
  writePiece(
    res,
    `event: token\ndata: ${JSON.stringify({
      request_id: 'req-cite-1',
      message_id: 51,
      content: body,
    })}\n\n`,
  );
  writePiece(
    res,
    `event: done\ndata: ${JSON.stringify({
      request_id: 'req-cite-1',
      session_id: 'sess-cite-1',
      message_id: 51,
      payload: {
        answer: body,
        answer_blocks: [],
        session_id: 'sess-cite-1',
        agent_type: 'chat',
        sources: [],
        citations,
        trace: { mode: 'chat' },
        degrade_trace: [],
      },
    })}\n\n`,
  );
  res.end();
}

function streamSlow(req, res) {
  sseHead(res);
  let finished = false;
  watchAbort(req, res, () => finished);
  writePiece(
    res,
    'event: start\ndata: {"request_id":"req-slow-1","session_id":"sess-slow-1"}\n\n',
  );
  writePiece(
    res,
    'event: answer_start\ndata: {"request_id":"req-slow-1","session_id":"sess-slow-1","message_id":31,"agent_type":"chat"}\n\n',
  );
  let count = 0;
  const timer = setInterval(() => {
    if (res.writableEnded || res.destroyed) {
      clearInterval(timer);
      return;
    }
    count += 1;
    writePiece(
      res,
      `event: token\ndata: {"request_id":"req-slow-1","message_id":31,"content":"慢速片段 ${count}。"}\n\n`,
    );
  }, 180);
}

function handleAdminGet(pathname, url, res) {
  if (pathname.endsWith('/api/v1/admin/accounts')) {
    adminOk(res, adminState.accounts);
    return;
  }
  const accountMatch = pathname.match(/\/api\/v1\/admin\/accounts\/([^/]+)$/);
  if (accountMatch) {
    const id = decodeURIComponent(accountMatch[1]);
    const acc = adminState.accounts.find((a) => a.id === id);
    if (!acc) {
      adminErr(res, 404, 'admin_not_found', 'account not found');
      return;
    }
    adminOk(res, acc);
    return;
  }
  if (pathname.endsWith('/api/v1/admin/users')) {
    const owner = url.searchParams.get('owner_user_id') || '';
    adminOk(res, adminState.users[owner] || []);
    return;
  }
  if (pathname.endsWith('/api/v1/admin/usage')) {
    adminOk(res, {
      owner_user_id: url.searchParams.get('owner_user_id') || 'ow-1',
      period: url.searchParams.get('period') || '30d',
      query_count: 120,
      document_count: 34,
      chunk_count: 512,
      storage_bytes: 1048576,
    });
    return;
  }
  if (pathname.endsWith('/api/v1/admin/health')) {
    adminOk(res, { status: 'ok', version: '0.4.2', uptime_secs: 86400 });
    return;
  }
  if (pathname.endsWith('/api/v1/admin/billing')) {
    adminOk(res, {
      active_subscriptions: 5,
      past_due_subscriptions: 1,
      unpaid_subscriptions: 0,
      canceled_subscriptions: 2,
    });
    return;
  }
  if (pathname.endsWith('/api/v1/admin/rag-health')) {
    adminOk(res, {
      failed_documents: 1,
      queued_tasks: 2,
      processing_tasks: 3,
      dead_letter_tasks: 0,
      recent_guard_events: 4,
    });
    return;
  }
  if (pathname.endsWith('/api/v1/admin/system/workers')) {
    adminOk(res, {
      runtime_mode: 'standalone',
      queued_tasks: 2,
      processing_tasks: 1,
      dead_letter_tasks: 0,
      failed_documents: 3,
    });
    return;
  }
  if (pathname.endsWith('/api/v1/admin/system/degradation')) {
    adminOk(res, { failed_documents: 0, recent_guard_events: 1, share_access_events: 7 });
    return;
  }
  if (pathname.endsWith('/api/v1/admin/feature-flags/change-requests')) {
    const status = url.searchParams.get('status');
    const list = status
      ? adminState.requests.filter((r) => r.status === status)
      : adminState.requests;
    adminOk(res, list);
    return;
  }
  if (pathname.endsWith('/api/v1/admin/feature-flags')) {
    adminOk(res, adminState.flags);
    return;
  }
  if (pathname.endsWith('/api/v1/admin/audit-logs')) {
    const like = (value) => (value || '').toLowerCase();
    const q = like(url.searchParams.get('query'));
    const action = url.searchParams.get('action') || '';
    const resourceType = url.searchParams.get('resource_type') || '';
    const actor = url.searchParams.get('actor') || '';
    const window = url.searchParams.get('window') || '';
    const cutoffs = { '24h': 1725484800, '7d': 1724880000, '30d': 1722633600, '90d': 1717276800 };
    const cutoff = cutoffs[window] || 0;
    const filtered = adminState.auditLogs.filter((entry) => {
      if (entry.created_at < cutoff) return false;
      if (action && entry.action !== action) return false;
      if (resourceType && entry.resource_type !== resourceType) return false;
      if (actor && entry.actor_id !== actor) return false;
      if (q) {
        const haystack = like(
          `${entry.action} ${entry.resource_id} ${entry.actor_id || ''} ${entry.owner_user_id || ''}`,
        );
        if (!haystack.includes(q)) return false;
      }
      return true;
    });
    if (url.searchParams.get('format') === 'csv') {
      cors(res);
      res.writeHead(200, { 'Content-Type': 'text/csv' });
      const rows = filtered.map(
        (e) =>
          `"${e.id}","${e.action}","${e.resource_type}","${e.resource_id}","${e.actor_id || ''}","${e.owner_user_id || ''}","${e.created_at}"`,
      );
      res.end(['id,action,resource_type,resource_id,actor_id,owner_user_id,created_at', ...rows].join('\n'));
      return;
    }
    const page = Math.max(1, Number(url.searchParams.get('page') || 1));
    const perPage = Math.min(200, Math.max(1, Number(url.searchParams.get('per_page') || 50)));
    adminOk(res, {
      items: filtered.slice((page - 1) * perPage, page * perPage),
      total: filtered.length,
      page,
      per_page: perPage,
    });
    return;
  }
  adminErr(res, 404, 'admin_not_found', 'unknown admin endpoint');
}

const server = http.createServer((req, res) => {
  cors(res);
  if (req.method === 'OPTIONS') {
    res.writeHead(204);
    res.end();
    return;
  }
  const url = new URL(req.url, `http://127.0.0.1:${PORT}`);

  if (req.method === 'GET' && url.pathname === '/admin/state') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ ...state, filesEvents: filesState.events, hasFile: Boolean(filesState.file) }));
    return;
  }
  if (req.method === 'GET') {
    const pathname = url.pathname;
    if (pathname.endsWith('/api/auth/me')) {
      const auth = req.headers.authorization || '';
      if (!auth.startsWith('Bearer ') || auth.length <= 7) {
        res.writeHead(401, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ success: false, data: null, error: 'unauthorized' }));
        return;
      }
      jsonOk(res, {
        success: true,
        data: {
          token: '',
          user: { id: 'fixture-user', email: 'poc@example.com', full_name: 'PoC' },
          reset_ticket: null,
        },
        error: null,
      });
      return;
    }
    if (pathname.includes('/api/v1/admin/')) {
      if (pathname.includes('/case/403-admin/')) {
        adminErr(res, 403, 'admin_access_denied', 'admin access denied');
        return;
      }
      handleAdminGet(pathname, url, res);
      return;
    }
    const filesMatch = pathname.match(/\/api\/v1\/chat\/sessions\/([^/]+)\/files$/);
    if (filesMatch) {
      if (pathname.includes('/case/files-busy/')) {
        jsonOk(res, { files: [busyFileRow()] });
        return;
      }
      if (pathname.includes('/case/files/') && filesState.file) {
        fileEvent('list');
        jsonOk(res, { files: [filesState.file] });
        return;
      }
      jsonOk(res, { files: [] });
      return;
    }
    const sharedKbMatch = pathname.match(/\/api\/shared\/kb\/([^/]+)$/);
    if (sharedKbMatch) {
      const tok = decodeURIComponent(sharedKbMatch[1]);
      if (tok === 'tok-expired') {
        res.writeHead(404, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: 'not_found', message: 'share expired' }));
        return;
      }
      jsonOk(res, {
        knowledge_base: {
          id: 'ws-materials',
          title: '公开材料知识库',
          description: '公开分享的特种金属资料',
        },
        share: {
          permission: 'read_only',
          expires_at: null,
          allow_download: true,
          scope: 'full',
        },
        sources: [
          {
            id: 's-shared-1',
            file_name: 'titanium-guide.pdf',
            status: 'completed',
          },
        ],
        owner: {
          display_name: '公开分享者',
          bio: '材料科学专家',
          profile_enabled: true,
        },
      });
      return;
    }
    const publicUserMatch = pathname.match(/\/api\/public\/users\/([^/]+)\/shares$/);
    if (publicUserMatch) {
      const uid = decodeURIComponent(publicUserMatch[1]);
      if (uid === 'u-disabled') {
        jsonOk(res, { profile_enabled: false });
        return;
      }
      jsonOk(res, {
        profile_enabled: true,
        display_name: '李工',
        bio: '高级材料工程师',
      });
      return;
    }
    const billingOrderMatch = pathname.match(/\/api\/v1\/billing\/orders\/([^/]+)$/);
    if (billingOrderMatch) {
      const oid = decodeURIComponent(billingOrderMatch[1]);
      jsonOk(res, {
        order_id: oid,
        status: 'paid',
        plan_id: 'pro',
      });
      return;
    }
    if (pathname.endsWith('/api/v1/billing/plans')) {
      jsonOk(res, {
        plans: [
          {
            plan_id: 'free',
            name: '免费体验版',
            description: '个人基础问答',
            price_label_cny: '¥0',
            interval: 'month',
            current: true,
          },
          {
            plan_id: 'pro',
            name: 'Pro 专业版',
            description: '深度知识库与高速 Agent',
            price_label_cny: '¥99/月',
            interval: 'month',
            current: false,
          },
        ],
        current_plan_id: 'free',
      });
      return;
    }
    if (pathname.endsWith('/api/v1/billing/wallet')) {
      jsonOk(res, {
        user_id: 'fixture-user',
        balance_fen: 2500,
        lifetime_paid_topup_fen: 5000,
      });
      return;
    }
    if (pathname.endsWith('/api/v1/billing/wallet/topup-packs')) {
      jsonOk(res, [
        { pack_id: 'topup_50', amount_fen: 5000, amount_yuan: 50, label_cny: '50元' },
        { pack_id: 'topup_100', amount_fen: 10000, amount_yuan: 100, label_cny: '100元' },
        { pack_id: 'topup_200', amount_fen: 20000, amount_yuan: 200, label_cny: '200元' },
      ]);
      return;
    }
    const shareSettingsMatch = pathname.match(/\/api\/v1\/workspaces\/([^/]+)\/share\/settings$/);
    if (shareSettingsMatch) {
      jsonOk(res, {
        share_token: 'tok-valid-123',
        access_level: 'read_only',
        expires_at: null,
        allow_download: true,
      });
      return;
    }
    const shareLogsMatch = pathname.match(/\/api\/v1\/workspaces\/([^/]+)\/share\/access-logs$/);
    if (shareLogsMatch) {
      jsonOk(res, {
        logs: [
          {
            id: 'l1',
            visitor_id: 'visitor-88',
            accessed_at: '2026-09-05T00:00:00Z',
            action: 'view',
          },
        ],
      });
      return;
    }
    const shareAnalyticsMatch = pathname.match(/\/api\/v1\/workspaces\/([^/]+)\/share\/analytics$/);
    if (shareAnalyticsMatch) {
      jsonOk(res, {
        total_views: 45,
        total_unique_visitors: 18,
        views_by_day: {},
      });
      return;
    }
    const wsDocsMatch = pathname.match(/\/api\/v1\/workspaces\/([^/]+)\/documents$/);
    if (wsDocsMatch) {
      jsonOk(res, { documents: workspaceDocsState });
      return;
    }
    const wsNotesMatch = pathname.match(/\/api\/v1\/workspaces\/([^/]+)\/notes$/);
    if (wsNotesMatch) {
      jsonOk(res, { notes: workspaceNotesState });
      return;
    }
    const wsSingleMatch = pathname.match(/\/api\/v1\/workspaces\/([^/]+)$/);
    if (wsSingleMatch) {
      const wid = decodeURIComponent(wsSingleMatch[1]);
      const found = workspacesState.find((w) => w.id === wid) || {
        id: wid,
        owner_user_id: 'fixture-user',
        owner_id: 'fixture-user',
        name: '材料研发知识库',
        title: '材料研发知识库',
        description: '工作区描述',
        created_at: '2026-09-01T00:00:00Z',
        updated_at: '2026-09-04T00:00:00Z',
        document_count: workspaceDocsState.length,
        status_summary: {},
        shared: false,
      };
      jsonOk(res, { workspace: found });
      return;
    }
    if (pathname.endsWith('/api/v1/workspaces')) {
      jsonOk(res, { workspaces: workspacesState });
      return;
    }
    if (pathname.endsWith('/api/v1/settings/provider-secrets')) {
      const hasByok = pathname.includes('/case/byok');
      const baseSecrets = hasByok
        ? [
            {
              id: 'sec-quick-1',
              purpose: 'quick_chat',
              provider: 'bailian',
              model_hint: 'qwen3.8-flash',
              is_active: true,
            },
          ]
        : [];
      jsonOk(res, {
        secrets: [...baseSecrets, ...secretsState],
      });
      return;
    }
    if (pathname.endsWith('/api/v1/chat/sessions')) {
      const authed = Boolean(req.headers.authorization);
      jsonOk(res, {
        sessions: authed
          ? [
              {
                id: 'sess-900',
                owner_user_id: 'fixture-user',
                scope_kind: 'personal',
                title: '夹具会话',
                agent_type: 'chat',
                model_role: 'quick_chat',
                created_at: '2026-09-04T00:00:00Z',
                updated_at: '2026-09-04T00:00:00Z',
              },
              {
                id: 'sess-ws-901',
                owner_user_id: 'fixture-user',
                workspace_id: 'ws-materials',
                workspace_name: '材料研发',
                scope_kind: 'workspace',
                title: '合金强度分析',
                agent_type: 'rag',
                model_role: 'agent',
                created_at: '2026-09-04T00:00:00Z',
                updated_at: '2026-09-04T00:00:00Z',
              },
            ]
          : [],
      });
      return;
    }
    const messagesMatch = pathname.match(/\/api\/v1\/chat\/sessions\/([^/]+)\/messages$/);
    if (messagesMatch) {
      const id = decodeURIComponent(messagesMatch[1]);
      const withHistory = pathname.includes('/case/feedback') || id === 'sess-history';
      jsonOk(res, {
        messages: withHistory
          ? [
              {
                id: 101,
                session_id: id,
                role: 'user',
                content: '用户历史提问',
                answer_blocks: [],
                citations: [],
                created_at: '2026-09-04T00:00:00Z',
              },
              {
                id: 102,
                session_id: id,
                role: 'assistant',
                content: '这是历史助手的完整回答内容。',
                answer_blocks: [],
                citations: [
                  {
                    citation_id: 1,
                    doc_id: 'doc-del',
                    doc_name: '已下线文档',
                    citation_status: 'source_deleted',
                    preview: '旧文档残片',
                    score: 0.8,
                  },
                ],
                created_at: '2026-09-04T00:00:01Z',
              },
            ]
          : [],
      });
      return;
    }
    const sessionMatch = pathname.match(/\/api\/v1\/chat\/sessions\/([^/]+)$/);
    if (sessionMatch) {
      const id = decodeURIComponent(sessionMatch[1]);
      const isWs = id === 'sess-ws-901' || pathname.includes('/case/workspace/');
      jsonOk(res, {
        id,
        owner_user_id: 'fixture-user',
        workspace_id: isWs ? 'ws-materials' : null,
        workspace_name: isWs ? '材料研发' : null,
        scope_kind: isWs ? 'workspace' : 'personal',
        agent_type: isWs ? 'rag' : 'chat',
        model_role: isWs ? 'agent' : 'quick_chat',
        created_at: '2026-09-04T00:00:00Z',
        updated_at: '2026-09-04T00:00:00Z',
      });
      return;
    }
  }
  if (req.method === 'POST' && url.pathname === '/admin/reset') {
    state.aborted = false;
    state.requests = 0;
    state.bytesWritten = 0;
    state.lastAuthorization = null;
    state.lastChatBody = null;
    filesState.file = null;
    filesState.events = [];
    secretsState = [];
    adminState = seedAdminState();
    workspacesState = [
      {
        id: 'ws-materials',
        owner_user_id: 'fixture-user',
        owner_id: 'fixture-user',
        name: '材料研发知识库',
        title: '材料研发知识库',
        description: '特种合金与复合材料',
        created_at: '2026-09-01T00:00:00Z',
        updated_at: '2026-09-04T00:00:00Z',
        document_count: 1,
        status_summary: {},
        shared: false,
      },
    ];
    workspaceDocsState = [
      {
        id: 'doc-titanium',
        owner_user_id: 'fixture-user',
        owner_id: 'fixture-user',
        workspace_id: 'ws-materials',
        file_name: 'titanium-spec.pdf',
        mime_type: 'application/pdf',
        file_size: 1024,
        status: 'completed',
        chunk_count: 12,
        created_at: '2026-09-02T00:00:00Z',
        updated_at: '2026-09-02T00:00:00Z',
      },
    ];
    workspaceNotesState = [
      {
        id: 'note-weekly',
        workspace_id: 'ws-materials',
        title: '周会讨论要点',
        content: '# 周会讨论\n- 重点跟进抗拉强度',
        preview: '重点跟进抗拉强度',
        created_at: '2026-09-03T00:00:00Z',
        updated_at: '2026-09-03T00:00:00Z',
      },
    ];
    res.writeHead(204);
    res.end();
    return;
  }

  if (req.method === 'PUT' && /\/upload\//.test(url.pathname)) {
    fileEvent('put');
    req.resume();
    req.on('end', () => {
      cors(res);
      res.writeHead(204);
      res.end();
    });
    return;
  }

  if (req.method === 'PUT' && url.pathname.endsWith('/api/v1/settings/provider-secrets')) {
    readBody(req).then((buf) => {
      const body = JSON.parse(buf.toString('utf8') || '{}');
      const newSec = {
        id: `sec-${Date.now()}`,
        provider: body.provider,
        purpose: body.purpose,
        model_hint: body.model_hint,
        is_active: true,
      };
      secretsState = secretsState.filter((s) => s.purpose !== body.purpose);
      secretsState.push(newSec);
      jsonOk(res, { success: true, data: newSec });
    });
    return;
  }

  if (req.method === 'DELETE') {
    const adminUserMatch = url.pathname.match(/\/api\/v1\/admin\/users\/([^/]+)$/);
    if (adminUserMatch) {
      const userId = decodeURIComponent(adminUserMatch[1]);
      for (const owner of Object.keys(adminState.users)) {
        adminState.users[owner] = adminState.users[owner].filter((u) => u.id !== userId);
      }
      adminOk(res, null);
      return;
    }
    const wsDocMatch = url.pathname.match(/\/api\/v1\/workspaces\/([^/]+)\/documents\/([^/]+)$/);
    if (wsDocMatch) {
      const docId = decodeURIComponent(wsDocMatch[2]);
      workspaceDocsState = workspaceDocsState.filter((d) => d.id !== docId);
      res.writeHead(204);
      res.end();
      return;
    }
    const wsNoteMatch = url.pathname.match(/\/api\/v1\/workspaces\/([^/]+)\/notes\/([^/]+)$/);
    if (wsNoteMatch) {
      const noteId = decodeURIComponent(wsNoteMatch[2]);
      workspaceNotesState = workspaceNotesState.filter((n) => n.id !== noteId);
      res.writeHead(204);
      res.end();
      return;
    }
    const secMatch = url.pathname.match(/\/api\/v1\/settings\/provider-secrets\/([^/]+)$/);
    if (secMatch) {
      const secId = decodeURIComponent(secMatch[1]);
      secretsState = secretsState.filter((s) => s.id !== secId);
      jsonOk(res, { success: true, data: {} });
      return;
    }
    const delMatch = url.pathname.match(/\/api\/v1\/chat\/sessions\/[^/]+\/files\/([^/]+)$/);
    if (delMatch) {
      const bindingId = decodeURIComponent(delMatch[1]);
      if (filesState.file && filesState.file.binding_id === bindingId) {
        filesState.file = null;
      }
      jsonOk(res, { status: 'deleted' });
      return;
    }
  }

  if (req.method === 'POST') {
    const pathname = url.pathname;
    if (pathname.endsWith('/api/v1/workspaces')) {
      readBody(req).then((buf) => {
        const body = JSON.parse(buf.toString('utf8') || '{}');
        const newWs = {
          id: `ws-${Date.now()}`,
          owner_user_id: 'fixture-user',
          owner_id: 'fixture-user',
          name: body.name || '新工作区',
          title: body.name || '新工作区',
          description: body.description || '',
          created_at: '2026-09-05T00:00:00Z',
          updated_at: '2026-09-05T00:00:00Z',
          document_count: 0,
          status_summary: {},
          shared: false,
        };
        workspacesState.push(newWs);
        jsonOk(res, { workspace: newWs });
      });
      return;
    }
    const createNoteMatch = pathname.match(/\/api\/v1\/workspaces\/([^/]+)\/notes$/);
    if (createNoteMatch) {
      const wid = decodeURIComponent(createNoteMatch[1]);
      readBody(req).then((buf) => {
        const body = JSON.parse(buf.toString('utf8') || '{}');
        const newNote = {
          id: `note-${Date.now()}`,
          workspace_id: wid,
          title: body.title || '无标题笔记',
          content: body.content || '',
          preview: (body.content || '').slice(0, 50),
          created_at: '2026-09-05T00:00:00Z',
          updated_at: '2026-09-05T00:00:00Z',
        };
        workspaceNotesState.push(newNote);
        jsonOk(res, { note: newNote });
      });
      return;
    }
    if (pathname.endsWith('/api/v1/billing/checkout-session')) {
      readBody(req).then((buf) => {
        const body = JSON.parse(buf.toString('utf8') || '{}');
        jsonOk(res, {
          url: `http://127.0.0.1:${PORT}/mock-pay?session=cs_test_999`,
          session_id: 'cs_test_999',
          order_id: 'ord_test_888',
        });
      });
      return;
    }
    if (pathname.endsWith('/api/v1/admin/billing/block')) {
      readBody(req).then((buf) => {
        const body = JSON.parse(buf.toString('utf8') || '{}');
        const acc = adminState.accounts.find((a) => a.id === body.owner_user_id);
        if (!acc) {
          adminErr(res, 404, 'admin_not_found', 'account not found');
          return;
        }
        acc.blocked = Boolean(body.blocked);
        adminOk(res, null);
      });
      return;
    }
    if (pathname.endsWith('/api/v1/admin/notifications/broadcast')) {
      readBody(req).then((buf) => {
        const body = JSON.parse(buf.toString('utf8') || '{}');
        adminState.broadcasts.push({
          event_type: body.event_type || 'admin.broadcast',
          title: body.title,
          body: body.body,
        });
        adminOk(res, { created: adminState.broadcasts.length });
      });
      return;
    }
    const flagReqReview = pathname.match(
      /\/api\/v1\/admin\/feature-flags\/change-requests\/([^/]+)\/review$/,
    );
    if (flagReqReview) {
      const reqId = decodeURIComponent(flagReqReview[1]);
      readBody(req).then((buf) => {
        const body = JSON.parse(buf.toString('utf8') || '{}');
        const request = adminState.requests.find((r) => r.id === reqId);
        if (!request) {
          adminErr(res, 404, 'admin_not_found', 'change request not found');
          return;
        }
        request.status = body.approved ? 'approved' : 'rejected';
        request.reviewed_by = 'fixture-user';
        request.review_note = body.review_note || null;
        request.reviewed_at = 1725485100;
        if (body.approved) {
          request.executed_at = 1725485100;
          const flag = adminState.flags.find((f) => f.key === request.flag_key);
          if (flag) {
            flag.enabled = request.requested_enabled;
            flag.effective_enabled = request.requested_enabled;
            flag.has_pending_request = false;
          }
        }
        adminOk(res, request);
      });
      return;
    }
    const flagReqCreate = pathname.match(/\/api\/v1\/admin\/feature-flags\/([^/]+)\/change-requests$/);
    if (flagReqCreate) {
      const flagKey = decodeURIComponent(flagReqCreate[1]);
      readBody(req).then((buf) => {
        const body = JSON.parse(buf.toString('utf8') || '{}');
        const flag = adminState.flags.find((f) => f.key === flagKey);
        if (!flag) {
          adminErr(res, 404, 'admin_not_found', 'flag not found');
          return;
        }
        const request = {
          id: `req-${adminState.requests.length + 1}-${Date.now()}`,
          flag_key: flagKey,
          current_enabled: flag.enabled,
          requested_enabled: Boolean(body.enabled),
          reason: body.reason || '',
          status: 'pending',
          requested_by: 'fixture-user',
          reviewed_by: null,
          review_note: null,
          created_at: 1725485000,
          reviewed_at: null,
          executed_at: null,
        };
        adminState.requests.push(request);
        flag.has_pending_request = true;
        adminOk(res, request);
      });
      return;
    }
    if (pathname.endsWith('/api/auth/login')) {
      readBody(req).then((buf) => {
        const body = JSON.parse(buf.toString('utf8') || '{}');
        if (body.password === 'wrong') {
          res.writeHead(401, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ success: false, data: null, error: '账号或密码错误' }));
          return;
        }
        jsonOk(res, {
          success: true,
          data: {
            token: 'test-login-token-999',
            user: {
              id: 'user-auth-1',
              email: body.email || 'user@example.com',
              full_name: '测试用户',
              public_profile_enabled: false,
            },
            reset_ticket: null,
          },
          error: null,
        });
      });
      return;
    }
    if (pathname.endsWith('/api/auth/register')) {
      readBody(req).then((buf) => {
        const body = JSON.parse(buf.toString('utf8') || '{}');
        jsonOk(res, {
          success: true,
          data: {
            token: 'test-reg-token-888',
            user: {
              id: 'user-reg-1',
              email: body.email,
              full_name: body.full_name || '新注册用户',
              public_profile_enabled: false,
            },
            reset_ticket: null,
          },
          error: null,
        });
      });
      return;
    }
    if (pathname.endsWith('/api/auth/reset/send-code')) {
      readBody(req).then(() => {
        jsonOk(res, { success: true, data: {}, error: null });
      });
      return;
    }
    if (pathname.endsWith('/api/auth/reset/verify-code')) {
      readBody(req).then((buf) => {
        const body = JSON.parse(buf.toString('utf8') || '{}');
        if (body.code === '000000') {
          res.writeHead(400, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ success: false, data: null, error: '验证码无效' }));
          return;
        }
        jsonOk(res, {
          success: true,
          data: {
            token: '',
            user: { id: '', email: body.email, full_name: '' },
            reset_ticket: 'valid-reset-ticket-123',
          },
          error: null,
        });
      });
      return;
    }
    if (pathname.endsWith('/api/auth/reset/confirm')) {
      readBody(req).then(() => {
        jsonOk(res, { success: true, data: {}, error: null });
      });
      return;
    }
    if (pathname.endsWith('/api/v1/chat/sessions')) {
      req.resume();
      req.on('end', () => {
        const id = pathname.includes('/case/files/') ? 'sess-file-1' : 'sess-900';
        jsonOk(res, sessionJson(id, '夹具会话'));
      });
      return;
    }
    const createFile = pathname.match(/\/api\/v1\/chat\/sessions\/([^/]+)\/files$/);
    if (createFile) {
      readBody(req).then((buf) => {
        let filename = 'notes.txt';
        let mimeType = 'text/plain';
        let fileSize = 5;
        try {
          const parsed = JSON.parse(buf.toString('utf8') || '{}');
          filename = parsed.filename || filename;
          mimeType = parsed.mime_type || mimeType;
          fileSize = Number(parsed.file_size || fileSize);
        } catch {
          // keep defaults
        }
        fileEvent('presign');
        filesState.file = {
          binding_id: 'bind-file-1',
          document_id: 'doc-file-1',
          file_name: filename,
          mime_type: mimeType,
          file_size: fileSize,
          status: 'pending',
          created_at: '2026-09-04T00:00:00Z',
        };
        jsonOk(res, {
          document_id: 'doc-file-1',
          upload_url: `http://127.0.0.1:${PORT}${pathname.includes('/case/files/') ? '/case/files' : ''}/upload/doc-file-1`,
          status: 'pending',
        });
      });
      return;
    }
    const completeMatch = pathname.match(/\/api\/v1\/documents\/([^/]+)\/complete-upload$/);
    if (completeMatch) {
      req.resume();
      req.on('end', () => {
        fileEvent('complete');
        if (filesState.file && filesState.file.document_id === decodeURIComponent(completeMatch[1])) {
          filesState.file = { ...filesState.file, status: 'completed' };
        }
        jsonOk(res, { status: 'completed' });
      });
      return;
    }
    const reindexMatch = pathname.match(/\/api\/v1\/documents\/([^/]+)\/reindex$/);
    if (reindexMatch) {
      req.resume();
      req.on('end', () => {
        if (filesState.file && filesState.file.document_id === decodeURIComponent(reindexMatch[1])) {
          filesState.file = { ...filesState.file, status: 'completed' };
        }
        jsonOk(res, { status: 'queued' });
      });
      return;
    }
    const feedbackMatch = pathname.match(/\/api\/v1\/chat\/sessions\/([^/]+)\/messages\/([^/]+)\/feedback$/);
    if (feedbackMatch) {
      readBody(req).then(() => {
        if (pathname.includes('/case/feedback-fail/')) {
          res.writeHead(500, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ error: 'internal_error', message: 'feedback failed' }));
          return;
        }
        jsonOk(res, {});
      });
      return;
    }

    // 读出 chat JSON，供 scope bar 断言 capabilities / agent_type
    readBody(req).then((buf) => {
      try {
        state.lastChatBody = JSON.parse(buf.toString('utf8') || '{}');
      } catch {
        state.lastChatBody = null;
      }
      state.requests += 1;
      state.lastAuthorization = req.headers.authorization || null;
      if (url.pathname === '/case/401/api/v1/chat') {
        res.writeHead(401, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: 'unauthorized', message: 'invalid or missing token' }));
        return;
      }
      if (url.pathname === '/case/bad-json/api/v1/chat') {
        sseHead(res);
        writePiece(res, 'event: token\ndata: {not-valid-json\n\n');
        res.end();
        return;
      }
      if (url.pathname === '/case/slow/api/v1/chat') {
        streamSlow(req, res);
        return;
      }
      if (url.pathname === '/case/markdown/api/v1/chat') {
        streamMarkdown(req, res);
        return;
      }
      if (url.pathname === '/case/citations/api/v1/chat') {
        streamCitations(req, res);
        return;
      }
      if (url.pathname === '/case/progress/api/v1/chat') {
        streamProgress(req, res);
        return;
      }
      if (url.pathname === '/case/files/api/v1/chat') {
        streamShortOk(req, res, 'sess-file-1');
        return;
      }
      if (url.pathname === '/case/feedback/api/v1/chat') {
        streamShortOk(req, res, 'sess-history');
        return;
      }
      if (url.pathname === '/api/v1/chat') {
        streamFixture(req, res, url);
        return;
      }
      // /bytes/:n/api/v1/chat — 字节切片模式（UTF-8 跨包），query 无法放在 base URL 里
      const bytesMatch = url.pathname.match(/^\/bytes\/(\d+)\/api\/v1\/chat$/);
      if (bytesMatch) {
        const byteUrl = new URL(`${url.origin}/api/v1/chat?split=bytes&n=${bytesMatch[1]}`);
        streamFixture(req, res, byteUrl);
        return;
      }
      res.writeHead(404, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'not_found' }));
    });
    return;
  }

  res.writeHead(404, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({ error: 'not_found' }));
});

server.listen(PORT, '127.0.0.1', () => {
  console.log(`fixture-sse-server listening on http://127.0.0.1:${PORT}`);
});
