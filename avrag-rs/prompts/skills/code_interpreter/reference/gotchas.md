# Gotchas

## The sandbox always returns `success: true`

Exceptions are caught by the sandbox wrapper and printed to `stderr`. Do not rely on `success` to detect logic errors — **always check `stderr`** for tracebacks or error messages.

## `sys.exit()` is blocked

The `sys` module is in the deny-list. Calling `sys.exit(1)` raises `ImportError`, not `SystemExit`.

## The `_result` field is only set for expressions

If the last statement is an assignment (`x = 42`), `result` will be `null`. If the last statement is an expression (`42`), `result` will be `"42"`.

**Workaround**: If you need the value of a variable, end the script with that variable name as the last line, or use `print()`.

## Large outputs may be truncated

Dumping a huge DataFrame or long list to stdout may hit output size limits. Prefer:
- Summarizing with `len()`, `sum()`, `statistics.mean()`
- Printing only the first N items: `print(data[:10])`
- Using formatted output: `print(f"Count: {len(data)}")`

## No persistent state across calls

Each `code_interpreter` invocation runs in a fresh sandbox. Variables defined in one call are not available in the next. If you need state, re-declare it or pass it through the conversation context.

## Blocked modules

`os`, `subprocess`, `socket`, `sys`, `ctypes` are blocked. Any import of these modules raises `ImportError`. Do not attempt network I/O, file system access, or process spawning.

## Matplotlib availability

Chart generation via matplotlib is supported if the sandbox has it installed. Use `import matplotlib` with a non-interactive backend:
```python
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
```
