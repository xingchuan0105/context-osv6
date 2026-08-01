Ingestion 使用 MCP workspace 工具与 HTTP PUT 传文件字节。流程：create_upload → PUT upload_url → complete_upload → 轮询 document_status 至 completed。
