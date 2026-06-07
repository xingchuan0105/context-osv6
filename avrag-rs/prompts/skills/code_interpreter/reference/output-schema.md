# Output Schema

## Success response

```json
{
  "stdout": "3\n",
  "stderr": "",
  "result": "3",
  "executed": true,
  "exit_code": 0,
  "killed": false
}
```

### Field details

| Field | Type | Description |
|-------|------|-------------|
| `stdout` | string | Printed output from the code. Empty string if nothing was printed. |
| `stderr` | string | Error messages or exception tracebacks. Empty if no exception. |
| `result` | string \| null | Value of the last expression, serialized as a string. **`null` if the last statement is an assignment or not an expression.** |
| `executed` | boolean | `true` if the sandbox framework ran the code without crashing. **Does NOT mean the code logic succeeded** — check `stderr` for Python exceptions. |
| `exit_code` | integer | Process exit code. `0` for normal termination. Non-zero (e.g., `137`) if killed by resource limits. |
| `killed` | boolean | `true` if the sandbox terminated the process for exceeding CPU time or memory limits. |

## Error response

When the sandbox itself cannot run (e.g., missing `code` field):

```json
{
  "status": "error",
  "error": {
    "code": "MISSING_CODE",
    "message": "missing code"
  }
}
```

## Interpreting the result

| Last statement | `result` | How to get the value |
|----------------|----------|---------------------|
| Expression (`42`) | `"42"` | Read `result` directly. |
| Assignment (`x = 42`) | `null` | End the script with `x` as the last line, or use `print(x)`. |
| `print()` call | `null` | Read `stdout`. |
| Exception raised | `null` | Read `stderr` for traceback. |

## Interpreting execution status

| `executed` | `stderr` | `killed` | Meaning |
|------------|----------|----------|---------|
| `true` | empty | `false` | Code ran without exceptions. |
| `true` | non-empty | `false` | Code ran but raised a Python exception (see `stderr`). |
| `true` | — | `true` | Code exceeded resource limits and was terminated. Check `exit_code` (typically 137). |
| `false` | — | — | Sandbox framework crashed (rare). |
