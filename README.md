# nua

A small Rust CLI for reading and writing Lua/Luau with indentation, `let`, `def`, and array literals. Conversion works in both directions.

```python
def double(value: number) -> number:
  return value * 2

let numbers: {number} = [1, 2, 3]
for i, value in numbers:
  numbers[i] = double(value)
```

Emits Luau:

```lua
local function double(value: number) : number
  return value * 2
end

local numbers: {number} = {1, 2, 3}
for i, value in numbers do
  numbers[i] = double(value)
end
```

## Use

Build with a current stable Rust toolchain (edition 2024):

```sh
cargo build --release
./target/release/nua to-luau examples/basics.nua -o /tmp/basics.luau
./target/release/nua from-luau /tmp/basics.luau -o /tmp/basics.nua

# Read existing Lua using nua syntax:
cat existing.lua | ./target/release/nua from-lua
```

Or install from this checkout with `cargo install --path .`.

```
nua <to-luau|from-luau> [INPUT|-] [-o OUTPUT]
```

Input defaults to stdin; output defaults to stdout. `-` explicitly selects stdin/stdout. `to-lua` and `from-lua` are aliases. The output dialect is Luau: the aliases do not erase types or lower Luau features to stock Lua.

## Syntax

| nua | Lua/Luau |
| --- | --- |
| `let x: number = 1` | `local x: number = 1` |
| `def f(x: T) -> U:` | `local function f(x: T): U … end` |
| `global def f():` | `function f() … end` |
| `def Object:method(x):` | `function Object:method(x) … end` |
| `let f = def(x):` | `local f = function(x) … end` |
| `if condition:` | `if condition then … end` |
| `elif condition:` / `else:` | `elseif condition then` / `else` |
| `for i, x in items:` | `for i, x in items do … end` |
| `while condition:` | `while condition do … end` |
| `repeat:` / `until condition` | `repeat … until condition` |
| `do:` | `do … end` |
| `pass` | Empty body (emits a comment) |
| `[1, 2, 3]` | `{1, 2, 3}` |

Use spaces for indentation. Indentation width can vary between blocks, but sibling statements must align. Bodies must be indented; use `pass` for an empty body. Bare `def` declarations are local. Qualified definitions (`def Object.method` or `def Object:method`) preserve Lua's assignment semantics.

`let` bindings are mutable. Assignment and name resolution retain Lua semantics, including assignment to globals. There is no new declaration checker.

Luau type syntax stays intact: `{T}`, `{[string]: T}`, optional types, unions, intersections, generics, function types, type packs, `typeof`, and `::` assertions. Only a function definition's return annotation changes from `: T` to `-> T`. Function types already use `->`.

Array literals create ordinary Lua tables, with one-based indexing. `items[1]` stays indexing. Records and dictionaries keep braces: `{name = "Ada"}` and `{[key] = value}`. The reverse converter renders tables containing only positional fields as arrays, including empty tables. Mixed tables keep braces. Table-call shorthand such as `print {1, 2}` also keeps braces.

Lua long strings remain supported. `[[...]]` is reserved for a long string, so separate nested list openers with a space: `[ [1, 2], [3, 4] ]`. The reverse converter inserts this space automatically.

Comments (`--`, `--[[...]]`), strings, interpolation, explicit `return`, `and`/`or`/`not`, `~=`, `..`, retain their existing syntax. Inline conditional expressions also use `elif`, keeping `then` and `else`: `if x then 1 elif y then 2 else 3`. Escape Lua identifiers that collide with nua's new words using `@let`, `@def`, `@global`, `@pass`, or `@elif`; the reverse converter does this automatically.

Python highlighting is a useful approximation for `def`, indentation, and shared keywords. Lua comments, `let`, Luau types, and strings need a dedicated nua grammar for accurate highlighting.

## Paired transpilation examples

The first three `.luau` files are the checked-in output of their `.nua` partners. The last pair demonstrates reading existing Lua:

| Example | nua source | Generated Luau |
| --- | --- | --- |
| Typed records, arrays, branching, iteration | [basics.nua](examples/basics.nua) | [basics.luau](examples/basics.luau) |
| Generic map, callbacks, type packs | [generics.nua](examples/generics.nua) | [generics.luau](examples/generics.luau) |
| Methods, globals, empty bodies, loops | [methods.nua](examples/methods.nua) | [methods.luau](examples/methods.luau) |
| Read existing Lua (reverse direction) | [reading.nua](examples/reading.nua) | [reading.lua](examples/reading.lua) |

```sh
nua to-luau examples/generics.nua
nua from-luau examples/generics.luau
```

## Status and development

This is an experimental first implementation targeting Lua 5.1 and Luau. Lua 5.2–5.4 extensions are not enabled. Conversion normalizes layout; it does not reproduce the original formatting byte for byte.

The reverse direction uses the `full_moon` Luau parser. Every reverse conversion is transpiled back and its syntax tree compared with the original before output is returned. Unsupported constructs fail explicitly rather than returning a changed tree. This is a syntax check, not Luau type checking or execution. Advanced constructs such as type functions and functions embedded in block conditions are not supported yet. New array syntax inside backtick interpolation expressions is not transformed.

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Tests cover both CLI directions, file output, invalid input, the paired examples, and syntax-tree round trips for blocks, callbacks, multiline headers, comments, long strings, tables, arrays, generics, and type packs.
