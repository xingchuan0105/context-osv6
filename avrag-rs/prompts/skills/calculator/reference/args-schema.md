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

The mathematical expression to evaluate. Must use valid calculator syntax.

**Supported operators**:
- `+` addition
- `-` subtraction
- `*` multiplication
- `/` division
- `%` modulo
- `^` exponentiation

**Supported functions** (all take one argument unless noted):
- `sin`, `cos`, `tan` — trigonometric (radians)
- `sqrt` — square root
- `abs` — absolute value
- `exp` — e^x
- `ln` — natural logarithm
- `log2`, `log10` — base-2 and base-10 logarithms
- `floor`, `ceil`, `round` — rounding
- `pow(a, b)` — a raised to power b (two arguments)
- `min(a, b, ...)` — minimum of all arguments (variadic)
- `max(a, b, ...)` — maximum of all arguments (variadic)

**Constants**:
- `pi` ≈ 3.14159...
- `e` ≈ 2.71828...

**Good**:
- `"1 + 2 * 3"` → 7
- `"sin(30 * pi / 180)"` → 0.5
- `"sqrt(16) + pow(2, 3)"` → 12
- `"min(3, 1, 2)"` → 1

**Bad** (runtime error):
- `""` — empty expression
- `"1 +"` — incomplete syntax
- `"log(100)"` — no single-arg `log`; use `log2` or `log10`

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
