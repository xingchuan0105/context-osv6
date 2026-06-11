# Gotchas

## Expression preprocessing

The parser receives the `expression` string as-is. Preprocess before calling if needed:

- **Remove thousand separators**: `"1,000,000 + 2"` → error. Strip commas first: `"1000000 + 2"`.
- **Replace `^` with `pow` when ambiguous**: The parser treats `^` as exponentiation (same as `pow(a, b)`). Both `2^3` and `pow(2, 3)` are valid and equivalent.

## Error mapping reference

| Input condition | Error code | Error message pattern | Caller action |
|-----------------|------------|----------------------|---------------|
| Empty or missing `expression` | `MISSING_EXPRESSION` | `"missing expression"` | Verify expression is non-empty before calling |
| Syntax error (incomplete, unexpected token) | `SYNTAX_ERROR` | `"evalexpr error: ..."` | Inspect expression for typos or unsupported syntax |
| Division by zero | `DIVISION_BY_ZERO` | `"evalexpr error: DivisionByZero"` | Guard denominators that may evaluate to zero |
| Modulo by zero | `DIVISION_BY_ZERO` | `"evalexpr error: DivisionByZero"` | Same as division by zero |
| Wrong function arity | `WRONG_ARGUMENT_COUNT` | `"evalexpr error: WrongArgumentCount"` | Check function signatures in args-schema |
| Unknown identifier / function name | `UNKNOWN_IDENTIFIER` | `"evalexpr error: UnknownIdentifier"` | Use only supported functions and constants |
| Floating-point overflow | `OVERFLOW` | `"evalexpr error: ..."` | Reduce expression magnitude or use `code_interpreter` |

## Scientific notation is NOT supported

The parser does not accept scientific notation. Rewrite as decimals:
- `1e3` → `1000`
- `2.5e-4` → `0.00025`

## Division by zero produces an error

`10 / 0` returns an error, not `Infinity` or `NaN`.

## Function arity matters

- `pow(2, 3)` — exactly 2 arguments
- `min(3, 1, 2)` — 2 or more arguments
- `sin(1, 2)` — wrong arity, runtime error

## No implicit multiplication

`2(3 + 4)` is invalid. Use explicit `*`: `2 * (3 + 4)`.

## No variable assignment

The calculator is pure expression evaluation. You cannot define variables:
- `x = 5; x + 3` → error
Use `code_interpreter` for multi-step computation with variables.

## Floating-point precision

Results are computed as `f64`. Comparisons with exact integers may show tiny rounding differences (e.g., `0.1 + 0.2` → `0.30000000000000004`). Round if presenting to the user.
