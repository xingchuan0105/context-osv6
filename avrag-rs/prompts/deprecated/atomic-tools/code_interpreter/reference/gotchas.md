# Gotchas

## `executed: true` does NOT mean success

The field `executed` only indicates the sandbox framework did not crash. **It does NOT mean the Python code ran without errors.**

Exceptions are caught by the sandbox wrapper and printed to `stderr`. To detect logic errors, **always check `stderr`** for tracebacks or error messages.

| `executed` | `stderr` | Meaning |
|------------|----------|---------|
| `true` | empty | Code ran without Python exceptions. |
| `true` | non-empty | Code ran but raised an exception (see stderr). |
| `false` | — | Sandbox itself crashed (rare). |

## `result` may be `null` — this is not an error

If the last statement is an assignment (`x = 42`), `result` is `null`. If the last statement is an expression (`42`), `result` is `"42"` (string).

**Workaround**: If you need the value of a variable, end the script with that variable name as the last line, or use `print()`.

## Do not dump large objects to stdout

Dumping a huge DataFrame or long list may hit output size limits and truncate useful content. Prefer:
- Summarizing with `len()`, `sum()`, `statistics.mean()`
- Printing only the first N items: `print(data[:10])`
- Using formatted output: `print(f"Count: {len(data)}")`

## No persistent state across calls

Each invocation runs in a fresh sandbox. Variables defined in one call are not available in the next. If you need state, re-declare it or pass it through the conversation context.

## This is NOT a general-purpose shell

The sandbox is for Python computation only. It is **not** a substitute for:
- File system operations (reading/writing files outside `/tmp`)
- Network access (HTTP requests, API calls)
- Process spawning or system commands
- Web scraping or browser automation

Attempting any of these will raise `ImportError` or `PermissionError`.

## Blocked modules

`os`, `subprocess`, `socket`, `sys`, `ctypes` are blocked. Any import of these modules raises `ImportError`. Do not attempt network I/O, file system access, or process spawning.

## `sys.exit()` is blocked

The `sys` module is in the deny-list. Calling `sys.exit(1)` raises `ImportError`, not `SystemExit`.

## Chart generation

Chart generation via matplotlib is supported if the sandbox has it installed. Use `import matplotlib` with a non-interactive backend:

```python
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
```

Save charts to `/tmp/chart.png` and print the path so the caller can fetch the file.
