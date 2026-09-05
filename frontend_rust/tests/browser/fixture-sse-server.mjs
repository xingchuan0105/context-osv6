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
