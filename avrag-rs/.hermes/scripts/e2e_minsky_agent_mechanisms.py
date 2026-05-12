#!/usr/bin/env python3
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path('/home/chuan/context-osv6/avrag-rs')
PDF = Path('/mnt/e/Download/minsky86.pdf')
BASE = os.environ.get('AVRAG_BASE_URL', 'http://127.0.0.1:8080').rstrip('/')
RUN_TS = int(time.time())
RUN_NAME = f'e2e-minsky-agent-mech-{RUN_TS}'
OUT_DIR = ROOT / '.hermes' / 'runs'
OUT_DIR.mkdir(parents=True, exist_ok=True)
OUT_JSON = OUT_DIR / f'{RUN_NAME}.json'

QUERY = (
    '请基于 minsky86.pdf 中 Minsky 的 Society of Mind 论文，用中文解释：'
    'Society of Mind、agents、K-lines、frames 之间是什么关系？'
    '这种由许多简单 agent 协作形成智能的机制，与现代软件 agent 的规划和工具/检索机制有什么可类比之处？'
)
PASSWORD = os.environ.get('E2E_PASSWORD') or f'e2e-{RUN_TS}-password'

result = {
    'run_name': RUN_NAME,
    'base_url': BASE,
    'pdf': str(PDF),
    'pdf_size': None,
    'query': QUERY,
    'timeline': [],
    'health': None,
    'ready': None,
    'email': None,
    'token_present': False,
    'notebook_id': None,
    'document_id': None,
    'session_id': None,
    'final_status': None,
    'status_history': [],
    'chat_request_id': f'{RUN_NAME}-chat',
    'chat': None,
    'product_event_metadata': None,
    'execute_plan_request': None,
    'channel_probes': {},
    'db_counts': None,
    'error': None,
}

def log(msg):
    stamp = time.strftime('%H:%M:%S')
    line = f'[{stamp}] {msg}'
    result['timeline'].append(line)
    print(line, flush=True)

def req(method, url, data=None, headers=None, timeout=30):
    headers = dict(headers or {})
    body = None
    if data is not None:
        if isinstance(data, (bytes, bytearray)):
            body = bytes(data)
        else:
            body = json.dumps(data).encode('utf-8')
            headers.setdefault('Content-Type', 'application/json')
    request = urllib.request.Request(url, data=body, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request, timeout=timeout) as resp:
            payload = resp.read()
            text = payload.decode('utf-8', errors='replace')
            return resp.status, text, dict(resp.headers)
    except urllib.error.HTTPError as e:
        text = e.read().decode('utf-8', errors='replace')
        return e.code, text, dict(e.headers)

def json_req(method, path, data=None, token=None, timeout=30):
    headers = {}
    if token:
        headers['Authorization'] = f'Bearer {token}'
    status, text, hdrs = req(method, BASE + path, data=data, headers=headers, timeout=timeout)
    try:
        parsed = json.loads(text) if text else None
    except Exception:
        parsed = {'_raw': text[:2000]}
    return status, parsed, text, hdrs

def abbreviate(obj, n=600):
    text = obj if isinstance(obj, str) else json.dumps(obj, ensure_ascii=False)
    return text if len(text) <= n else text[:n] + '...'

def parse_sse(text):
    events = []
    event_name = 'message'
    data_parts = []
    for raw in text.splitlines():
        if raw.startswith('event:'):
            event_name = raw.split(':', 1)[1].strip()
        elif raw.startswith('data:'):
            data_parts.append(raw.split(':', 1)[1].lstrip())
        elif raw == '':
            if data_parts:
                data = '\n'.join(data_parts)
                try:
                    parsed = json.loads(data)
                except Exception:
                    parsed = {'_raw': data}
                events.append({'event': event_name, 'data': parsed})
            event_name = 'message'
            data_parts = []
    if data_parts:
        data = '\n'.join(data_parts)
        try:
            parsed = json.loads(data)
        except Exception:
            parsed = {'_raw': data}
        events.append({'event': event_name, 'data': parsed})
    return events

def read_env_value(name):
    env_path = ROOT / '.env'
    if not env_path.exists():
        return None
    for line in env_path.read_text(errors='ignore').splitlines():
        line = line.strip()
        if not line or line.startswith('#') or '=' not in line:
            continue
        key, value = line.split('=', 1)
        if key == name:
            return value.strip().strip('"').strip("'")
    return None

def psql_json(sql):
    dburl = read_env_value('DATABASE_URL') or os.environ.get('DATABASE_URL')
    if not dburl:
        return None
    proc = subprocess.run(
        ['psql', dburl, '-X', '-v', 'ON_ERROR_STOP=1', '-At', '-c', sql],
        cwd=str(ROOT), text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60,
    )
    if proc.returncode != 0:
        raise RuntimeError(f'psql failed: {proc.stderr.strip()}')
    text = proc.stdout.strip()
    if not text:
        return None
    return json.loads(text)

def get_product_event_metadata(request_id):
    sql = """
    select coalesce(jsonb_agg(jsonb_build_object(
      'event_name', event_name,
      'event_time', event_time,
      'session_id', session_id,
      'notebook_id', notebook_id,
      'result', result,
      'metadata', metadata
    ) order by event_time), '[]'::jsonb)::text
    from product_events
    where request_id = '%s'
    """ % request_id.replace("'", "''")
    return psql_json(sql)

def get_db_counts(document_id):
    sql = """
    select jsonb_build_object(
      'documents', (select jsonb_agg(row_to_json(x)) from (select id, notebook_id, status, chunk_count, file_name, created_at from documents where id = '%(doc)s') x),
      'chunks_by_type', (select coalesce(jsonb_object_agg(chunk_type, ct), '{}'::jsonb) from (select chunk_type, count(*) ct from chunks where document_id = '%(doc)s' group by chunk_type) s),
      'multimodal_chunks', (select count(*) from document_multimodal_chunks where document_id = '%(doc)s'),
      'parse_runs', (select jsonb_agg(row_to_json(r)) from (select id, status, duration_ms, created_at, updated_at from document_parse_runs where document_id = '%(doc)s' order by created_at desc limit 3) r)
    )::text
    """ % {'doc': document_id.replace("'", "''")}
    return psql_json(sql)

def summarize_chunks(resp, max_chunks=8):
    bundle = (resp or {}).get('bundle') or {}
    chunks = []
    for kind, items in [('regular', bundle.get('chunks') or []), ('graph_supported', bundle.get('graph_supported_chunks') or [])]:
        for ch in items[:max_chunks]:
            chunks.append({
                'kind': kind,
                'chunk_id': ch.get('chunk_id'),
                'doc_id': ch.get('doc_id'),
                'chunk_type': ch.get('chunk_type'),
                'page': ch.get('page'),
                'score': ch.get('score'),
                'retrieval_channel': ch.get('retrieval_channel'),
                'parser_backend': ch.get('parser_backend'),
                'caption': ch.get('caption'),
                'text_preview': re.sub(r'\s+', ' ', (ch.get('text') or '').strip())[:500],
            })
    return chunks

def execute_plan(token, plan, label, channel_budget, final_chunk_budget=8):
    probe = json.loads(json.dumps(plan))
    probe['trace'] = {'request_id': result['chat_request_id'], 'origin': f'e2e_probe:{label}'}
    probe['budget'] = {'total_candidate_budget': 40, 'final_chunk_budget': final_chunk_budget}
    probe['channel_budget'] = channel_budget
    status, body, text, _ = json_req('POST', '/api/v1/rag/execute-plan', probe, token=token, timeout=180)
    out = {
        'http_status': status,
        'request': probe,
        'coverage': (body or {}).get('coverage') if isinstance(body, dict) else None,
        'backend_trace': (body or {}).get('backend_trace') if isinstance(body, dict) else None,
        'degrade_trace': (body or {}).get('degrade_trace') if isinstance(body, dict) else None,
        'relation_paths': (((body or {}).get('bundle') or {}).get('relation_paths') or [])[:8] if isinstance(body, dict) else [],
        'chunks': summarize_chunks(body if isinstance(body, dict) else {}, max_chunks=12),
    }
    if status != 200:
        out['raw'] = text[:2000]
    result['channel_probes'][label] = out
    log(f'execute-plan {label} -> {status}; chunks={len(out["chunks"])}; coverage={abbreviate(out["coverage"], 300)}')

def main():
    if not PDF.exists():
        raise RuntimeError(f'Missing PDF: {PDF}')
    result['pdf_size'] = PDF.stat().st_size
    log(f'Start E2E {RUN_NAME}; PDF={PDF} size={result["pdf_size"]}')

    status, body, _, _ = json_req('GET', '/health', timeout=10)
    result['health'] = {'status': status, 'body': body}
    log(f'GET /health -> {status} {abbreviate(body)}')
    if status != 200:
        raise RuntimeError('/health failed')

    status, body, _, _ = json_req('GET', '/ready', timeout=10)
    result['ready'] = {'status': status, 'body': body}
    log(f'GET /ready -> {status} {abbreviate(body)}')
    if status != 200:
        raise RuntimeError('/ready failed')

    email = f'{RUN_NAME}@e2e.test'
    result['email'] = email
    status, body, _, _ = json_req('POST', '/api/auth/register', {
        'email': email,
        'password': PASSWORD,
        'full_name': 'E2E Minsky Agent Mechanisms',
    }, timeout=60)
    log(f'POST /api/auth/register -> {status}')
    if status not in (200, 201):
        raise RuntimeError(f'register failed: {abbreviate(body)}')
    token = (((body or {}).get('data') or {}).get('token') or '').strip()
    result['token_present'] = bool(token)
    if not token:
        raise RuntimeError('register returned no token')

    status, body, _, _ = json_req('POST', '/api/v1/notebooks', {
        'name': RUN_NAME,
        'title': RUN_NAME,
        'description': 'E2E for Minsky Society of Mind and agent mechanisms',
    }, token=token, timeout=60)
    log(f'POST /api/v1/notebooks -> {status} {abbreviate(body)}')
    if status not in (200, 201):
        raise RuntimeError(f'create notebook failed: {abbreviate(body)}')
    notebook_id = ((body or {}).get('notebook') or {}).get('id')
    result['notebook_id'] = notebook_id

    status, body, _, _ = json_req('POST', f'/api/v1/notebooks/{notebook_id}/documents', {
        'filename': PDF.name,
        'file_size': result['pdf_size'],
        'mime_type': 'application/pdf',
    }, token=token, timeout=60)
    log(f'POST /api/v1/notebooks/{notebook_id}/documents -> {status} {abbreviate(body)}')
    if status not in (200, 201):
        raise RuntimeError(f'create document failed: {abbreviate(body)}')
    document_id = (body or {}).get('document_id')
    result['document_id'] = document_id

    status, text, _ = req('PUT', f'{BASE}/dev-upload/{document_id}', data=PDF.read_bytes(), headers={
        'Authorization': f'Bearer {token}',
        'Content-Type': 'application/octet-stream',
    }, timeout=180)
    log(f'PUT /dev-upload/{document_id} -> {status} {abbreviate(text)}')
    if status not in (200, 201, 202):
        raise RuntimeError(f'upload failed: {text[:1000]}')

    start = time.monotonic()
    final_status = 'unknown'
    last_logged = None
    for i in range(240):
        status, body, _, _ = json_req('GET', f'/api/v1/documents/{document_id}/status', token=token, timeout=30)
        doc_status = (body or {}).get('status') or 'unknown'
        elapsed = int(time.monotonic() - start)
        result['status_history'].append({'elapsed_s': elapsed, 'http_status': status, 'status': doc_status, 'body': body})
        if doc_status != last_logged or i % 12 == 0 or doc_status in ('completed', 'failed'):
            log(f'status poll {i:03d} elapsed={elapsed}s -> HTTP {status}, status={doc_status}, body={abbreviate(body)}')
            last_logged = doc_status
        if doc_status in ('completed', 'failed'):
            final_status = doc_status
            break
        time.sleep(5)
    result['final_status'] = final_status
    log(f'Final document status: {final_status}')
    result['db_counts'] = get_db_counts(document_id)
    if final_status != 'completed':
        return 2

    chat_body = {
        'query': QUERY,
        'notebook_id': notebook_id,
        'agent_type': 'rag',
        'doc_scope': [document_id],
        'stream': True,
    }
    headers = {
        'Authorization': f'Bearer {token}',
        'Content-Type': 'application/json',
        'Accept': 'text/event-stream',
        'x-request-id': result['chat_request_id'],
    }
    status, sse_text, _ = req('POST', f'{BASE}/api/v1/chat', data=chat_body, headers=headers, timeout=420)
    log(f'POST /api/v1/chat SSE -> {status}, bytes={len(sse_text)}')
    events = parse_sse(sse_text)
    counts = Counter(e['event'] for e in events)
    answer = ''.join((e['data'].get('content') or '') for e in events if e['event'] == 'token' and isinstance(e['data'], dict))
    done_payload = next((e['data'].get('payload') for e in reversed(events) if e['event'] == 'done' and isinstance(e['data'], dict)), None)
    citations_payload = next((e['data'] for e in events if e['event'] == 'citations' and isinstance(e['data'], dict)), None)
    start_event = next((e['data'] for e in events if e['event'] == 'start' and isinstance(e['data'], dict)), None)
    if start_event:
        result['session_id'] = start_event.get('session_id')
    result['chat'] = {
        'http_status': status,
        'event_counts': dict(counts),
        'activity_events': [e['data'] for e in events if e['event'] == 'activity'],
        'trace_events': [e['data'] for e in events if e['event'] == 'trace'],
        'answer_length': len(answer),
        'answer': answer,
        'citations_payload': citations_payload,
        'done_payload': done_payload,
    }
    if status != 200:
        raise RuntimeError(f'chat failed: {status} {sse_text[:1000]}')
    log(f'SSE event counts: {dict(counts)}; answer_length={len(answer)}')

    # Product event metadata includes the full execute-plan request that ChatResponse omits.
    time.sleep(1)
    events_meta = get_product_event_metadata(result['chat_request_id'])
    result['product_event_metadata'] = events_meta
    execute_plan = None
    for ev in events_meta or []:
        rag_tool = ((ev.get('metadata') or {}).get('rag_tool') or {})
        if rag_tool.get('execute_plan_request'):
            execute_plan = rag_tool.get('execute_plan_request')
    if not execute_plan and done_payload:
        rag_plan = (((done_payload or {}).get('planner_output') or {}).get('rag_plan') or {})
        if rag_plan.get('items'):
            execute_plan = {
                'plan_version': rag_plan.get('plan_version') or 'rag-execute-v1',
                'doc_scope': [document_id],
                'items': [
                    {k: v for k, v in item.items() if k in ('priority', 'query', 'bm25_terms') and v is not None}
                    for item in rag_plan.get('items', [])
                    if item.get('query') or item.get('bm25_terms')
                ],
                'summary_mode': 'related' if any(item.get('summary') == 'related' for item in rag_plan.get('items', [])) else 'none',
            }
    result['execute_plan_request'] = execute_plan
    log(f'Captured execute_plan_request: {abbreviate(execute_plan, 1000)}')
    if not execute_plan:
        raise RuntimeError('could not capture execute_plan_request')

    # Channel-isolated probes. These use the captured agent plan, with budgets isolating each retrieval path.
    execute_plan(token, execute_plan, 'full_captured_plan', {}, final_chunk_budget=12)
    execute_plan(token, execute_plan, 'text_dense_only', {'text_dense': 40, 'bm25': 0, 'multimodal_dense': 0, 'graph': 0}, final_chunk_budget=8)
    execute_plan(token, execute_plan, 'bm25_only', {'text_dense': 0, 'bm25': 40, 'multimodal_dense': 0, 'graph': 0}, final_chunk_budget=8)
    execute_plan(token, execute_plan, 'multimodal_dense_only', {'text_dense': 0, 'bm25': 0, 'multimodal_dense': 40, 'graph': 0}, final_chunk_budget=8)
    execute_plan(token, execute_plan, 'graph_only', {'text_dense': 0, 'bm25': 0, 'multimodal_dense': 0, 'graph': 40}, final_chunk_budget=8)
    return 0

try:
    code = main()
except Exception as exc:
    result['error'] = repr(exc)
    log(f'ERROR: {exc!r}')
    code = 1
finally:
    OUT_JSON.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
    log(f'Wrote JSON report: {OUT_JSON}')

sys.exit(code)
