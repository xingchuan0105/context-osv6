#!/usr/bin/env python3
"""Parse PDF, chunk it, generate embeddings, and upload to Milvus."""

import fitz
import json
import os
import sys
import uuid
import requests
import time

# Config
PDF_PATH = "/mnt/e/OneDrive/桌面/Taleb_Antifragile__2012.pdf"
MILVUS_URI = "http://172.27.208.1:19530"
EMBEDDING_API_KEY = "sk-292b16fbc7b34cb9b82ec3293aa3717e"
EMBEDDING_BASE_URL = "https://dashscope.aliyuncs.com/compatible-mode/v1"
EMBEDDING_MODEL = "text-embedding-v4"
COLLECTION = "avrag_rag_text_chunks"
ORG_ID = "00000000-0000-0000-0000-000000000001"
WORKSPACE_ID = "ws-default"
DOC_ID = "00000000-0000-0000-0000-000000000001"
CHUNK_SIZE = 800
CHUNK_OVERLAP = 100
MAX_CHUNKS = 200  # limit to avoid too many API calls


def parse_pdf(path):
    doc = fitz.open(path)
    pages = []
    for page_num in range(len(doc)):
        text = doc[page_num].get_text()
        if text.strip():
            pages.append((page_num + 1, text))
    doc.close()
    return pages


def chunk_text(pages):
    chunks = []
    for page_num, text in pages:
        start = 0
        while start < len(text):
            end = min(start + CHUNK_SIZE, len(text))
            chunk_text = text[start:end].strip()
            if chunk_text:
                chunks.append({
                    "page": page_num,
                    "text": chunk_text,
                })
            start += CHUNK_SIZE - CHUNK_OVERLAP
    return chunks[:MAX_CHUNKS]


def get_embeddings(texts):
    url = f"{EMBEDDING_BASE_URL}/embeddings"
    headers = {
        "Authorization": f"Bearer {EMBEDDING_API_KEY}",
        "Content-Type": "application/json",
    }
    all_embeddings = []
    batch_size = 10
    for i in range(0, len(texts), batch_size):
        batch = texts[i:i+batch_size]
        payload = {
            "model": EMBEDDING_MODEL,
            "input": batch,
            "dimensions": 1024,
            "encoding_format": "float",
        }
        resp = requests.post(url, headers=headers, json=payload, timeout=60)
        resp.raise_for_status()
        data = resp.json()
        for item in data["data"]:
            all_embeddings.append(item["embedding"])
        time.sleep(0.5)
    return all_embeddings


def upload_to_milvus(chunks, embeddings):
    from pymilvus import MilvusClient
    client = MilvusClient(uri=MILVUS_URI)

    rows = []
    for i, (chunk, emb) in enumerate(zip(chunks, embeddings)):
        rows.append({
            "id": str(uuid.uuid4()),
            "org_id": ORG_ID,
            "workspace_id": WORKSPACE_ID,
            "doc_id": DOC_ID,
            "chunk_id": str(uuid.uuid4()),
            "parse_run_id": "e2e-upload-001",
            "doc_version": 1,
            "page": chunk["page"],
            "text": chunk["text"][:4096],  # truncate if too long
            "text_dense": emb,
            "chunk_type": "text",
            "parser_backend": "pdf",
            "source_locator": json.dumps({"page": chunk["page"]}),
        })

    # Insert in batches
    batch_size = 50
    for i in range(0, len(rows), batch_size):
        batch = rows[i:i+batch_size]
        client.insert(collection_name=COLLECTION, data=batch)
        print(f"Inserted batch {i//batch_size + 1}/{(len(rows)-1)//batch_size + 1} ({len(batch)} rows)")

    print(f"Total inserted: {len(rows)}")


def main():
    print(f"Parsing PDF: {PDF_PATH}")
    pages = parse_pdf(PDF_PATH)
    print(f"Total pages with text: {len(pages)}")

    print("Chunking...")
    chunks = chunk_text(pages)
    print(f"Total chunks: {len(chunks)}")

    if not chunks:
        print("No chunks generated!")
        sys.exit(1)

    print("Generating embeddings...")
    texts = [c["text"] for c in chunks]
    embeddings = get_embeddings(texts)
    print(f"Generated {len(embeddings)} embeddings")

    print("Uploading to Milvus...")
    upload_to_milvus(chunks, embeddings)
    print("Done!")


if __name__ == "__main__":
    main()
