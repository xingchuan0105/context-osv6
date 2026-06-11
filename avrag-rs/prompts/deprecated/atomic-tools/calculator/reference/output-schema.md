# Output Schema

## Success response

```json
{
  "status": "success",
  "result": 7.0,
  "expression": "1 + 2 * 3"
}
```

### Field details

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | Always `"success"` on valid computation. |
| `result` | number | The computed numeric result (`f64`). |
| `expression` | string | The original expression string, echoed back for verification. |

## Error response

```json
{
  "status": "error",
  "error": {
    "code": "MISSING_EXPRESSION | SYNTAX_ERROR | DIVISION_BY_ZERO | WRONG_ARGUMENT_COUNT | UNKNOWN_IDENTIFIER | OVERFLOW",
    "message": "Human-readable description of what went wrong.",
    "expression": "the expression that failed"
  }
}
```

### Error codes

| Code | When it happens | Caller action |
|------|-----------------|---------------|
| `MISSING_EXPRESSION` | Empty or missing `expression` field. | Verify expression is non-empty before calling. |
| `SYNTAX_ERROR` | Incomplete expression, unexpected token, unsupported notation (e.g., `1e3`, `2(3+4)`). | Inspect expression for typos or unsupported syntax; see `gotchas.md`. |
| `DIVISION_BY_ZERO` | Division or modulo by zero. | Guard denominators that may evaluate to zero. |
| `WRONG_ARGUMENT_COUNT` | Function called with incorrect number of arguments. | Check function signatures in `args-schema.md`. |
| `UNKNOWN_IDENTIFIER` | Unknown function name or constant (e.g., bare `log`, `foo(1)`). | Use only supported functions and constants listed in `args-schema.md`. |
| `OVERFLOW` | Result exceeds `f64` range. | Reduce expression magnitude or use `code_interpreter`. |

## Note on error propagation

The caller MUST check `status` before reading `result`. When `status` is `"error"`, `result` is absent and the `error` object is present. Do not fall back to guessing the result from the error message.
