# Args Schema

The full JSON Schema for `code_interpreter` args, as enforced by the runtime at the call boundary.

```json
{
  "type": "object",
  "properties": {
    "code": {
      "type": "string",
      "description": "Python code to execute in the sandbox."
    }
  },
  "required": ["code"]
}
```

## Field details

### `code` (required, string)

The Python code to execute. Must be self-contained — the sandbox has no persistent state across calls.

**Good**:
- Single-expression computation: `"sum([1, 2, 3, 4, 5])"`
- Multi-line script with imports and logic
- Data transformation with list/dict operations
- Chart generation with matplotlib

**Bad** (runtime error or unexpected behavior):
- `""` — empty code
- Code that imports blocked modules (`os`, `subprocess`, `socket`, `sys`, `ctypes`)
- Code that attempts file system operations outside the sandbox
- Infinite loops (will be killed by timeout)

## Output

See `reference/output-schema.md` for the complete success/error contract, including `result` semantics and `executed` field behavior.
