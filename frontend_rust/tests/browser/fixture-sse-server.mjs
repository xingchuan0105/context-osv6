// 受控 SSE 测试服务器（Node 标准库，无依赖）。
// - POST /api/v1/chat            → 按 stream-long-3000 夹具分块写出真实 SSE
//   ?split=chunks（默认，字符对齐 chunk）| ?split=bytes&n=113（字节切片，制造 UTF-8 跨包）
// - POST /case/401/api/v1/chat   → 401 JSON 错误
// - POST /case/bad-json/api/v1/chat → 200 但第一帧即坏 JSON
// - POST /case/slow/api/v1/chat  → start 后慢速 token 流，永不主动结束（验证 stop/abort）
// - GET  /admin/state            → { aborted, requests, bytesWritten }
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

const state = { aborted: false, requests: 0, bytesWritten: 0, lastAuthorization: null };

function jsonOk(res, body) {
  cors(res);
  res.writeHead(200, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify(body));
}

function cors(res) {
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Headers', 'authorization, content-type, accept');
  res.setHeader('Access-Control-Allow-Methods', 'POST, GET, OPTIONS');
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
  req.on('close', () => {
    if (!isDone()) state.aborted = true;
  });
  res.on('error', () => {
    state.aborted = true;
  });
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
  }, CHUNK_DELAY_MS);
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
    res.end(JSON.stringify(state));
    return;
  }
  if (req.method === 'GET') {
    const pathname = url.pathname;
    if (pathname.endsWith('/api/v1/chat/sessions')) {
      jsonOk(res, { sessions: [] });
      return;
    }
    const messagesMatch = pathname.match(/\/api\/v1\/chat\/sessions\/([^/]+)\/messages$/);
    if (messagesMatch) {
      jsonOk(res, { messages: [] });
      return;
    }
    const sessionMatch = pathname.match(/\/api\/v1\/chat\/sessions\/([^/]+)$/);
    if (sessionMatch) {
      const id = decodeURIComponent(sessionMatch[1]);
      jsonOk(res, {
        id,
        owner_user_id: 'fixture-user',
        scope_kind: 'personal',
        agent_type: 'chat',
        model_role: 'quick_chat',
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
    res.writeHead(204);
    res.end();
    return;
  }

  if (req.method === 'POST') {
    // 读取请求体但不依赖其内容（保持确定性）
    req.resume();
    req.on('end', () => {
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
