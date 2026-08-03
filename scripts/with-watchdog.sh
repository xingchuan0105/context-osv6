#!/usr/bin/env bash
# with-watchdog.sh — 在"沉默看门狗"下运行长命令。
#
# 用法:
#   scripts/with-watchdog.sh <logfile> <max_silent_secs> -- <cmd...>
#
# 语义:
#   - cmd 的 stdout/stderr 追加进 <logfile>；"进展"定义为 logfile 字节数增长。
#   - 连续 <max_silent_secs> 无新输出 → 判定挂死：TERM（3s 后 KILL）整个进程组，
#     把日志尾部 40 行甩到 stderr，退出码 124（与 timeout(1) 一致）。
#   - 否则原样透传 cmd 退出码。
#   - 需要总时长帽时外套 timeout(1)：
#       timeout 7200 scripts/with-watchdog.sh /tmp/run.log 900 -- cargo test ...
#
# <max_silent_secs> 的取值纪律：按"最长合法静默阶段 ×2"取（它是挂死探测器，
# 不是慢速警察）。例如真实 LLM 单题可能数分钟无输出，全量评测取 900。
set -uo pipefail

usage() {
    echo "usage: $0 <logfile> <max_silent_secs> -- <cmd...>" >&2
    exit 2
}

[[ $# -ge 4 ]] || usage
LOG="$1"
SILENT="$2"
shift 2
[[ "${1:-}" == "--" ]] || usage
shift
[[ $# -gt 0 ]] || usage
if ! [[ "$SILENT" =~ ^[0-9]+$ ]] || (( SILENT < 5 )); then
    echo "with-watchdog: max_silent_secs must be an integer >= 5" >&2
    exit 2
fi

mkdir -p "$(dirname "$LOG")"
POLL=$(( SILENT / 10 ))
(( POLL < 2 )) && POLL=2
(( POLL > 30 )) && POLL=30

{
    echo "[WATCHDOG] begin $(date -Is) silent_cap=${SILENT}s poll=${POLL}s"
    echo "[WATCHDOG] cmd: $*"
} >> "$LOG"

#  monitor mode：后台作业自成进程组（pgid == $!），这样才能一锅端掉
# cmd 及其子孙（cargo → 测试二进制 → worker 子进程），而不误杀本脚本所在组。
set -m
"$@" >> "$LOG" 2>&1 &
CMD_PID=$!

last_size=-1
silent=0
while kill -0 "$CMD_PID" 2>/dev/null; do
    sleep "$POLL"
    size=$(stat -c %s "$LOG" 2>/dev/null || echo 0)
    if [[ "$size" == "$last_size" ]]; then
        silent=$(( silent + POLL ))
        if (( silent >= SILENT )); then
            echo "[WATCHDOG] FAIL $(date -Is): no output for ${SILENT}s — killing process group $CMD_PID" >> "$LOG"
            kill -TERM -"$CMD_PID" 2>/dev/null
            for _ in 1 2 3; do
                kill -0 "$CMD_PID" 2>/dev/null || break
                sleep 1
            done
            kill -KILL -"$CMD_PID" 2>/dev/null
            wait "$CMD_PID" 2>/dev/null
            echo "[WATCHDOG] killed after ${SILENT}s silence; last 40 lines of $LOG:" >&2
            tail -40 "$LOG" >&2
            exit 124
        fi
    else
        silent=0
        last_size=$size
    fi
done

wait "$CMD_PID"
rc=$?
echo "[WATCHDOG] end $(date -Is) exit=$rc" >> "$LOG"
exit "$rc"
