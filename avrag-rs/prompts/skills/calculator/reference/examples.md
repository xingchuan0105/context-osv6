# Examples

## Basic arithmetic

```json
{ "expression": "1 + 2 * 3" }
```
Result: `{"result": 7.0, "expression": "1 + 2 * 3"}`

## Parentheses and precedence

```json
{ "expression": "(1 + 2) * 3" }
```
Result: `{"result": 9.0, "expression": "(1 + 2) * 3"}`

## Trigonometry

```json
{ "expression": "sin(30 * pi / 180)" }
```
Result: `{"result": 0.5, "expression": "sin(30 * pi / 180)"}`

## Powers and roots

```json
{ "expression": "sqrt(16) + pow(2, 3)" }
```
Result: `{"result": 12.0, "expression": "sqrt(16) + pow(2, 3)"}`

## Logarithms

```json
{ "expression": "ln(e) + log10(100)" }
```
Result: `{"result": 3.0, "expression": "ln(e) + log10(100)"}`

## Min / Max

```json
{ "expression": "min(3, 1, 2) + max(10, 5, 8)" }
```
Result: `{"result": 11.0, "expression": "min(3, 1, 2) + max(10, 5, 8)"}`

## Rounding

```json
{ "expression": "floor(3.7) + ceil(3.2) + round(3.5)" }
```
Result: `{"result": 11.0, "expression": "floor(3.7) + ceil(3.2) + round(3.5)"}`

## Complex formula

```json
{ "expression": "(1583 * 47 + sqrt(1024) - pow(2, 8)) / 100" }
```
Result: `{"result": 743.13, "expression": "(1583 * 47 + sqrt(1024) - pow(2, 8)) / 100"}`

## Error: empty expression

```json
{ "expression": "" }
```
Result: `{"error": "missing expression"}` (status: Error)

## Error: division by zero

```json
{ "expression": "10 / 0" }
```
Result: `{"error": "evalexpr error: DivisionByZero"}` (status: Error)
