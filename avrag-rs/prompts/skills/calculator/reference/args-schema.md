# Args Schema

The full JSON Schema for `calculator` args, as enforced by the runtime at the call boundary.

```json
{
  "type": "object",
  "properties": {
    "expression": {
      "type": "string",
      "description": "Mathematical expression to evaluate."
    }
  },
  "required": ["expression"]
}
```

## Field details

### `expression` (required, string)

A single mathematical expression. The parser does **not** accept scientific notation, implicit multiplication, variable assignment, or multiple statements.

**Supported operators**:
- `+` addition
- `-` subtraction
- `*` multiplication
- `/` division
- `%` modulo
- `^` exponentiation (same as `pow(a, b)`)

**Supported functions** (all take one argument unless noted):
- `sin`, `cos`, `tan` — trigonometric (radians)
- `sqrt` — square root
- `abs` — absolute value
- `exp` — e^x
- `ln` — natural logarithm
- `log2`, `log10` — base-2 and base-10 logarithms
- `floor`, `ceil`, `round` — rounding
- `pow(a, b)` — a raised to power b (exactly 2 arguments)
- `min(a, b, ...)` — minimum of all arguments (variadic, 2+)
- `max(a, b, ...)` — maximum of all arguments (variadic, 2+)

**Constants**:
- `pi` ≈ 3.14159...
- `e` ≈ 2.71828...

**Good**:
- `"1 + 2 * 3"` → 7
- `"sin(30 * pi / 180)"` → 0.5
- `"sqrt(16) + pow(2, 3)"` → 12
- `"min(3, 1, 2)"` → 1

## Common invalid expressions

These are **runtime errors**:

| Expression | Why it fails | Correct form |
|------------|--------------|--------------|
| `"1e3"` | Scientific notation is **not supported** | `"1000"` |
| `"2.5e-4"` | Scientific notation is **not supported** | `"0.00025"` |
| `"2(3+4)"` | Implicit multiplication is **not supported** | `"2 * (3 + 4)"` |
| `"x = 5; x + 1"` | Variable assignment and multiple statements are **not supported** | Use `code_interpreter` |
| `"log(100)"` | No single-argument `log` function. Only `ln`, `log2`, `log10` are supported. | `"log10(100)"` |
| `""` | Empty expression | Provide a valid expression |
| `"1 +"` | Incomplete syntax | Complete the expression |

## Output schema

```json
{
  "type": "object",
  "properties": {
    "result": { "type": "number", "description": "The computed numeric result." },
    "expression": { "type": "string", "description": "The original expression." }
  }
}
```
