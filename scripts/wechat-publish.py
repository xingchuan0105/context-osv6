#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
WeChat Official Account Markdown Publisher & Formatter (微信公众号排版与发布工具)

Usage:
  python3 scripts/wechat-publish.py --input docs/articles/foo.md --export docs/articles/foo-wechat.html
  python3 scripts/wechat-publish.py --input docs/articles/foo.md --cover path.jpg --title "..." --push
"""

import os
import re
import sys
import json
import uuid
import mimetypes
import argparse
import urllib.request
import urllib.error

SCRIPTS_DIR = os.path.dirname(os.path.abspath(__file__))
if SCRIPTS_DIR not in sys.path:
    sys.path.insert(0, SCRIPTS_DIR)

from article_html import (  # noqa: E402
    collect_images,
    md_to_wechat_html,
    resolve_path,
)


def load_env_credentials():
    env_path = os.path.join(os.path.dirname(__file__), "..", "avrag-rs", ".env")
    creds = {"app_id": os.getenv("WECHAT_APP_ID", ""), "app_secret": os.getenv("WECHAT_APP_SECRET", "")}
    if os.path.exists(env_path):
        with open(env_path, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line.startswith("WECHAT_APP_ID="):
                    creds["app_id"] = line.split("=", 1)[1].strip().strip('"').strip("'")
                elif line.startswith("WECHAT_APP_SECRET="):
                    creds["app_secret"] = line.split("=", 1)[1].strip().strip('"').strip("'")
    return creds


def get_wechat_access_token(app_id, app_secret):
    url = f"https://api.weixin.qq.com/cgi-bin/token?grant_type=client_credential&appid={app_id}&secret={app_secret}"
    req = urllib.request.Request(url)
    try:
        with urllib.request.urlopen(req) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            if "access_token" in data:
                return data["access_token"]

            errcode = data.get("errcode")
            errmsg = data.get("errmsg", "")
            if errcode == 40164:
                ip_match = re.search(r"invalid ip ([\d\.]+|[0-9a-fA-F:]+)", errmsg)
                bad_ip = ip_match.group(1) if ip_match else "未知"
                print("\n" + "=" * 70, file=sys.stderr)
                print(
                    f"[微信安全拦截 40164] 调用端公网 IP ({bad_ip}) 未在微信公众平台的 IP 白名单中！",
                    file=sys.stderr,
                )
                print("解决步骤：", file=sys.stderr)
                print("1. 登录微信公众平台 (https://mp.weixin.qq.com/)", file=sys.stderr)
                print("2. 进入【设置与开发】->【基本配置】->【IP白名单】->【修改】", file=sys.stderr)
                print(f"3. 添加白名单 IP: {bad_ip}", file=sys.stderr)
                print("4. 保存后重新运行本脚本即可直推草稿箱！", file=sys.stderr)
                print("=" * 70 + "\n", file=sys.stderr)
                sys.exit(2)
            raise RuntimeError(f"WeChat API Error {errcode}: {errmsg}")
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"HTTP Error {e.code}: {e.read().decode('utf-8')}")


def _multipart_file(image_path, field_name="media"):
    resolved_img = resolve_path(image_path)
    if not os.path.exists(resolved_img):
        raise FileNotFoundError(f"Image not found at: {image_path} (resolved: {resolved_img})")

    boundary = f"----WebKitFormBoundary{uuid.uuid4().hex}"
    filename = os.path.basename(resolved_img)
    content_type, _ = mimetypes.guess_type(resolved_img)
    if not content_type:
        content_type = "image/jpeg"

    with open(resolved_img, "rb") as f:
        file_bytes = f.read()

    body = (
        f"--{boundary}\r\n"
        f'Content-Disposition: form-data; name="{field_name}"; filename="{filename}"\r\n'
        f"Content-Type: {content_type}\r\n\r\n"
    ).encode("utf-8") + file_bytes + f"\r\n--{boundary}--\r\n".encode("utf-8")
    headers = {"Content-Type": f"multipart/form-data; boundary={boundary}"}
    return body, headers, resolved_img


def upload_permanent_thumb(token, image_path):
    body, headers, _ = _multipart_file(image_path)
    url = f"https://api.weixin.qq.com/cgi-bin/material/add_material?access_token={token}&type=image"
    req = urllib.request.Request(url, data=body, headers=headers, method="POST")
    with urllib.request.urlopen(req) as resp:
        res = json.loads(resp.read().decode("utf-8"))
        if "media_id" in res:
            return res["media_id"]
        raise RuntimeError(f"Failed to upload permanent material: {res}")


def upload_content_image(token, image_path):
    """Upload an inline article image; returns a mmbiz CDN URL."""
    body, headers, resolved = _multipart_file(image_path)
    url = f"https://api.weixin.qq.com/cgi-bin/media/uploadimg?access_token={token}"
    req = urllib.request.Request(url, data=body, headers=headers, method="POST")
    with urllib.request.urlopen(req) as resp:
        res = json.loads(resp.read().decode("utf-8"))
        if "url" in res:
            print(f"  uploaded {os.path.basename(resolved)}")
            return res["url"]
        raise RuntimeError(f"Failed to upload content image {resolved}: {res}")


def article_payload(title, author, digest, content_html, thumb_media_id, source_url):
    return {
        "title": title,
        "author": author,
        "digest": digest,
        "content": content_html,
        "content_source_url": source_url,
        "thumb_media_id": thumb_media_id,
        "need_open_comment": 0,
        "only_fans_can_comment": 0,
    }


def create_draft(token, title, author, digest, content_html, thumb_media_id, source_url):
    url = f"https://api.weixin.qq.com/cgi-bin/draft/add?access_token={token}"
    payload = {
        "articles": [
            article_payload(title, author, digest, content_html, thumb_media_id, source_url)
        ]
    }

    req = urllib.request.Request(
        url,
        data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        headers={"Content-Type": "application/json; charset=utf-8"},
        method="POST",
    )

    with urllib.request.urlopen(req) as resp:
        res = json.loads(resp.read().decode("utf-8"))
        if "media_id" in res:
            return res["media_id"]
        raise RuntimeError(f"Failed to create draft: {res}")


def update_draft(token, media_id, title, author, digest, content_html, thumb_media_id, source_url):
    url = f"https://api.weixin.qq.com/cgi-bin/draft/update?access_token={token}"
    payload = {
        "media_id": media_id,
        "index": 0,
        "articles": article_payload(title, author, digest, content_html, thumb_media_id, source_url),
    }
    req = urllib.request.Request(
        url,
        data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        headers={"Content-Type": "application/json; charset=utf-8"},
        method="POST",
    )
    with urllib.request.urlopen(req) as resp:
        res = json.loads(resp.read().decode("utf-8"))
        if res.get("errcode", 0) == 0:
            return media_id
        raise RuntimeError(f"Failed to update draft: {res}")


def main():
    parser = argparse.ArgumentParser(description="WeChat Markdown Formatter & Publisher")
    parser.add_argument("--input", required=True, help="Path to input Markdown file")
    parser.add_argument("--export", help="Export to standalone HTML file for direct copy")
    parser.add_argument("--title", default="")
    parser.add_argument("--author", default="邢川")
    parser.add_argument("--digest", default="")
    parser.add_argument("--cover", help="Path to cover image")
    parser.add_argument("--source-url", default="")
    parser.add_argument("--push", action="store_true", help="Push to WeChat Official Account Draft Box")
    parser.add_argument("--update-draft", help="Existing draft media_id to update in place")
    parser.add_argument("--thumb-id", help="Reuse an existing thumb_media_id instead of re-uploading the cover")
    parser.add_argument("--result", help="Write a JSON result file after a successful push")

    args = parser.parse_args()

    input_path = resolve_path(args.input)
    if not os.path.exists(input_path):
        print(f"Error: input file {args.input} does not exist (resolved: {input_path})", file=sys.stderr)
        sys.exit(1)

    with open(input_path, "r", encoding="utf-8") as f:
        md_text = f.read()
    base_dir = os.path.dirname(os.path.abspath(input_path))

    html_result = md_to_wechat_html(md_text, base_dir=base_dir)

    if args.export:
        export_path = resolve_path(args.export)
        os.makedirs(os.path.dirname(os.path.abspath(export_path)) or ".", exist_ok=True)
        with open(export_path, "w", encoding="utf-8") as f:
            f.write(html_result)
        print(f"[OK] 微信富文本内联 HTML 已成功导出至: {args.export}")

    if args.push:
        creds = load_env_credentials()
        if not creds["app_id"] or not creds["app_secret"]:
            print("[ERROR] WECHAT_APP_ID / WECHAT_APP_SECRET 未在 avrag-rs/.env 中配置！", file=sys.stderr)
            sys.exit(1)
        if not args.title:
            print("[ERROR] 推送草稿箱必须提供 --title", file=sys.stderr)
            sys.exit(1)
        if not args.cover:
            print("[ERROR] 推送草稿箱必须提供封面图 (--cover 参数)！", file=sys.stderr)
            sys.exit(1)

        print("[1/4] 正在获取微信公众平台 Access Token...")
        token = get_wechat_access_token(creds["app_id"], creds["app_secret"])
        print("[OK] Token 获取成功")

        print("[2/4] 正在上传正文图片至微信 CDN...")
        url_map = {}
        for alt, src, resolved in collect_images(md_text, base_dir):
            if not os.path.exists(resolved):
                raise FileNotFoundError(f"正文图片不存在: {src} -> {resolved}")
            cdn_url = upload_content_image(token, resolved)
            url_map[os.path.abspath(resolved)] = cdn_url
            url_map[src] = cdn_url
            url_map[resolved] = cdn_url
        html_result = md_to_wechat_html(md_text, base_dir=base_dir, url_map=url_map)
        print(f"[OK] 已上传 {len(collect_images(md_text, base_dir))} 张正文图")

        if args.thumb_id:
            thumb_id = args.thumb_id
            print("[3/4] 复用已有封面 thumb_media_id")
        else:
            print(f"[3/4] 正在上传封面素材: {args.cover}...")
            thumb_id = upload_permanent_thumb(token, args.cover)
            print("[OK] 封面上传成功")

        print("[4/4] 正在写入公众号草稿箱...")
        if args.update_draft:
            draft_id = update_draft(
                token=token,
                media_id=args.update_draft,
                title=args.title,
                author=args.author,
                digest=args.digest,
                content_html=html_result,
                thumb_media_id=thumb_id,
                source_url=args.source_url,
            )
            print("\n" + "=" * 60)
            print(f"已更新公众号草稿。Draft Media ID: {draft_id}")
        else:
            draft_id = create_draft(
                token=token,
                title=args.title,
                author=args.author,
                digest=args.digest,
                content_html=html_result,
                thumb_media_id=thumb_id,
                source_url=args.source_url,
            )
            print("\n" + "=" * 60)
            print(f"成功推送到微信公众号草稿箱。Draft Media ID: {draft_id}")
        print("请在微信公众平台后台 (mp.weixin.qq.com) ->【草稿箱】或手机【订阅号助手】中查看和群发。")
        print("=" * 60 + "\n")

        if args.result:
            result_path = resolve_path(args.result)
            with open(result_path, "w", encoding="utf-8") as f:
                json.dump(
                    {
                        "thumb_id": thumb_id,
                        "draft_id": draft_id,
                        "title": args.title,
                        "author": args.author,
                        "digest": args.digest,
                        "content_source_url": args.source_url,
                        "state": "draft",
                    },
                    f,
                    ensure_ascii=False,
                    indent=2,
                )
                f.write("\n")


if __name__ == "__main__":
    main()
