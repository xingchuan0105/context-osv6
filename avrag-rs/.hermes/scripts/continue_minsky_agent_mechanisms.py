#!/usr/bin/env python3
# Continue an already-created Minsky E2E document: login -> wait status -> chat -> capture plan -> channel probes.
import json, os, re, subprocess, sys, time, urllib.request, urllib.error
from collections import Counter
from pathlib import Path

ROOT = Path('/home/chuan/context-osv6/avrag-rs')
BASE = os.environ.get('AVRAG_BASE_URL', 'http://127.0.0.1:8080').rstrip('/')
RUN_NAME = os.environ.get('RUN_NAME', f'e2e-minsky-agent-mech-continue-{int(time.time())}')
OUT = ROOT / '.hermes' / 'runs' / f'{RUN_NAME}.json'
OUT.parent.mkdir(parents=True, exist_ok=True)
EMAIL = os.environ['E2E_EMAIL']
PASSWORD = os.environ['E2E_PASSWORD']
NOTEBOOK_ID = os.environ['E2E_NOTEBOOK_ID']
DOCUMENT_ID = os.environ['E2E_DOCUMENT_ID']
ORG_ID = os.environ.get('E2E_ORG_ID')
REQUEST_ID = f'{RUN_NAME}-chat'
QUERY = os.environ.get('E2E_QUERY') or (
    '请基于 minsky86.pdf 中 Minsky 的 Society of Mind 论文，用中文解释：'
    'Society of Mind、agents、K-lines、frames 之间是什么关系？'
    '这种由许多简单 agent 协作形成智能的机制，与现代软件 agent 的规划和工具/检索机制有什么可类比之处？'
)
result = {'run_name': RUN_NAME, 'email': EMAIL, 'notebook_id': NOTEBOOK_ID, 'document_id': DOCUMENT_ID, 'org_id': ORG_ID, 'query': QUERY, 'request_id': REQUEST_ID, 'timeline': [], 'status_history': [], 'chat': None, 'execute_plan_request': None, 'product_event_metadata': None, 'channel_probes': {}, 'db_counts': None, 'error': None}

def log(s):
    line=f'[{time.strftime("%H:%M:%S")}] {s}'
    result['timeline'].append(line); print(line, flush=True)

def req(method, url, data=None, headers=None, timeout=30):
    headers=dict(headers or {}); body=None
    if data is not None:
        if isinstance(data,(bytes,bytearray)): body=bytes(data)
        else:
            body=json.dumps(data).encode(); headers.setdefault('Content-Type','application/json')
    r=urllib.request.Request(url,data=body,headers=headers,method=method)
    try:
        with urllib.request.urlopen(r,timeout=timeout) as resp:
            return resp.status, resp.read().decode('utf-8','replace'), dict(resp.headers)
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode('utf-8','replace'), dict(e.headers)

def jreq(method, path, data=None, token=None, timeout=30, extra_headers=None):
    h=dict(extra_headers or {})
    if token: h['Authorization']=f'Bearer {token}'
    st, txt, hdr=req(method, BASE+path, data, h, timeout)
    try: obj=json.loads(txt) if txt else None
    except Exception: obj={'_raw':txt[:2000]}
    return st,obj,txt,hdr

def parse_sse(txt):
    evs=[]; name='message'; parts=[]
    for raw in txt.splitlines():
        if raw.startswith('event:'): name=raw.split(':',1)[1].strip()
        elif raw.startswith('data:'): parts.append(raw.split(':',1)[1].lstrip())
        elif raw=='':
            if parts:
                data='\n'.join(parts)
                try: data=json.loads(data)
                except Exception: data={'_raw':data}
                evs.append({'event':name,'data':data})
            name='message'; parts=[]
    if parts:
        data='\n'.join(parts)
        try: data=json.loads(data)
        except Exception: data={'_raw':data}
        evs.append({'event':name,'data':data})
    return evs

def envv(k):
    p=ROOT/'.env'
    for line in p.read_text(errors='ignore').splitlines():
        if line.startswith(k+'='): return line.split('=',1)[1].strip().strip('"').strip("'")
    return os.environ.get(k)

def psql_json(sql):
    db=envv('DATABASE_URL')
    proc=subprocess.run(['psql',db,'-X','-v','ON_ERROR_STOP=1','-At','-c',sql], cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=90)
    if proc.returncode: raise RuntimeError(proc.stderr.strip())
    txt=proc.stdout.strip()
    return json.loads(txt) if txt else None

def db_counts():
    if not ORG_ID: return None
    doc=DOCUMENT_ID.replace("'", "''"); org=ORG_ID.replace("'", "''")
    return psql_json(f"""
    with _ctx as (select set_config('app.current_org','{org}',false))
    select jsonb_build_object(
      'document',(select row_to_json(d) from (select id, notebook_id, status, chunk_count, file_name, created_at, updated_at from documents where id='{doc}') d),
      'chunks_by_type',(select coalesce(jsonb_object_agg(chunk_type, ct),'{{}}'::jsonb) from (select chunk_type,count(*) ct from chunks where document_id='{doc}' group by chunk_type) s),
      'multimodal_chunks',(select count(*) from document_multimodal_chunks where document_id='{doc}'),
      'parse_runs',(select jsonb_agg(row_to_json(r)) from (select run_id,status,duration_ms,created_at,updated_at from document_parse_runs where document_id='{doc}' order by created_at desc limit 3) r)
    )::text
    from _ctx
    """.split('\n',1)[1])

def product_events():
    rid=REQUEST_ID.replace("'", "''")
    return psql_json(f"""
    select coalesce(jsonb_agg(jsonb_build_object('event_name',event_name,'event_time',event_time,'session_id',session_id,'notebook_id',notebook_id,'result',result,'metadata',metadata) order by event_time),'[]'::jsonb)::text
    from product_events where request_id='{rid}'
    """)

def chunks_summary(body):
    bundle=(body or {}).get('bundle') or {}; out=[]
    for kind,key in [('regular','chunks'),('graph_supported','graph_supported_chunks')]:
        for ch in (bundle.get(key) or [])[:12]:
            out.append({'kind':kind,'chunk_id':ch.get('chunk_id'),'doc_id':ch.get('doc_id'),'chunk_type':ch.get('chunk_type'),'page':ch.get('page'),'score':ch.get('score'),'retrieval_channel':ch.get('retrieval_channel'),'parser_backend':ch.get('parser_backend'),'caption':ch.get('caption'),'text_preview':re.sub(r'\s+',' ',(ch.get('text') or '').strip())[:700]})
    return out

def execute_plan(token, plan, label, cb, final=8):
    probe=json.loads(json.dumps(plan)); probe['trace']={'request_id':REQUEST_ID,'origin':f'e2e_probe:{label}'}; probe['budget']={'total_candidate_budget':40,'final_chunk_budget':final}; probe['channel_budget']=cb
    st, body, txt, _=jreq('POST','/api/v1/rag/execute-plan',probe,token=token,timeout=240)
    result['channel_probes'][label]={'http_status':st,'request':probe,'coverage':(body or {}).get('coverage') if isinstance(body,dict) else None,'backend_trace':(body or {}).get('backend_trace') if isinstance(body,dict) else None,'degrade_trace':(body or {}).get('degrade_trace') if isinstance(body,dict) else None,'relation_paths':(((body or {}).get('bundle') or {}).get('relation_paths') or [])[:10] if isinstance(body,dict) else [],'chunks':chunks_summary(body if isinstance(body,dict) else {})}
    log(f'probe {label}: status={st}, chunks={len(result["channel_probes"][label]["chunks"])}')

def main():
    log(f'continue doc={DOCUMENT_ID}')
    st, body, _, _=jreq('POST','/api/auth/login',{'email':EMAIL,'password':PASSWORD},timeout=60)
    log(f'login -> {st}')
    if st!=200: raise RuntimeError(str(body)[:500])
    token=((body or {}).get('data') or {}).get('token')
    for i in range(180):
        st, b, _, _=jreq('GET',f'/api/v1/documents/{DOCUMENT_ID}/status',token=token,timeout=30)
        ds=(b or {}).get('status') or 'unknown'; result['status_history'].append({'i':i,'status':ds,'http_status':st,'body':b})
        if i%12==0 or ds in ('completed','failed'): log(f'status {i}: {st} {ds} {str(b)[:300]}')
        if ds in ('completed','failed'): break
        time.sleep(5)
    result['db_counts']=db_counts()
    if result['status_history'][-1]['status']!='completed': return 2
    chat={'query':QUERY,'notebook_id':NOTEBOOK_ID,'agent_type':'rag','doc_scope':[DOCUMENT_ID],'stream':True}
    st, sse, _=req('POST',BASE+'/api/v1/chat',chat,{'Authorization':f'Bearer {token}','Content-Type':'application/json','Accept':'text/event-stream','x-request-id':REQUEST_ID},timeout=480)
    evs=parse_sse(sse); cnt=Counter(e['event'] for e in evs); ans=''.join(e['data'].get('content') or '' for e in evs if e['event']=='token' and isinstance(e['data'],dict)); done=next((e['data'].get('payload') for e in reversed(evs) if e['event']=='done' and isinstance(e['data'],dict)),None)
    result['chat']={'http_status':st,'event_counts':dict(cnt),'activity_events':[e['data'] for e in evs if e['event']=='activity'],'trace_events':[e['data'] for e in evs if e['event']=='trace'],'answer_length':len(ans),'answer':ans,'done_payload':done,'citations_payload':next((e['data'] for e in evs if e['event']=='citations' and isinstance(e['data'],dict)),None)}
    log(f'chat -> {st}, events={dict(cnt)}, answer_len={len(ans)}')
    if st!=200: raise RuntimeError(sse[:1000])
    time.sleep(1)
    evmeta=product_events(); result['product_event_metadata']=evmeta
    plan=None
    for ev in evmeta or []:
        rt=((ev.get('metadata') or {}).get('rag_tool') or {})
        if rt.get('execute_plan_request'): plan=rt['execute_plan_request']
    if not plan and done:
        rp=((done.get('planner_output') or {}).get('rag_plan') or {})
        plan={'plan_version':rp.get('plan_version') or 'rag-execute-v1','doc_scope':[DOCUMENT_ID],'items':[{k:v for k,v in it.items() if k in ('priority','query','bm25_terms') and v is not None} for it in rp.get('items',[]) if it.get('query') or it.get('bm25_terms')],'summary_mode':'related' if any(it.get('summary')=='related' for it in rp.get('items',[])) else 'none'}
    result['execute_plan_request']=plan; log('plan='+json.dumps(plan,ensure_ascii=False)[:1200])
    if not plan: raise RuntimeError('no plan')
    execute_plan(token,plan,'full_captured_plan',{},12)
    execute_plan(token,plan,'text_dense_only',{'text_dense':40,'bm25':0,'multimodal_dense':0,'graph':0})
    execute_plan(token,plan,'bm25_only',{'text_dense':0,'bm25':40,'multimodal_dense':0,'graph':0})
    execute_plan(token,plan,'multimodal_dense_only',{'text_dense':0,'bm25':0,'multimodal_dense':40,'graph':0})
    execute_plan(token,plan,'graph_only',{'text_dense':0,'bm25':0,'multimodal_dense':0,'graph':40})
    return 0
try:
    code=main()
except Exception as e:
    result['error']=repr(e); log(f'ERROR {e!r}'); code=1
finally:
    OUT.write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8'); log(f'wrote {OUT}')
sys.exit(code)
