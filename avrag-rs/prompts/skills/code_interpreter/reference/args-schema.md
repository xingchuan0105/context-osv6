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

## Output schema

```json
{
  "type": "object",
  "properties": {
    "stdout": { "type": "string", "description": "Printed output." },
    "stderr": { "type": "string", "description": "Error output / exceptions." },
    "result": { "type": "string", "description": "Last expression value (if any)." },
    "success": { "type": "boolean", "description": "Always true in current sandbox." },
    "exit_code": { "type": "integer", "description": "Process exit code." },
    "killed": { "type": "boolean", "description": "Whether terminated for resource limits." }
  }
}
```

## Important: `result` vs `stdout`

- The `result` field is only populated when the **last statement is an expression** (not an assignment).
- Use `print()` to guarantee output appears in `stdout`.
