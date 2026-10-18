# Ideas from other compile-to-Lua languages

These are design candidates, not implemented syntax. Reviewed against the linked primary references.

| Language | Relevant ideas | Possible fit for nua |
| --- | --- | --- |
| [MoonScript](https://moonscript.org/reference/) | Default arguments, destructuring, comprehensions | Small, explicit conveniences with readable Lua expansions |
| [YueScript](https://yuescript.org/doc/) | [Nil-coalescing](https://yuescript.org/doc/language-basics/operator.html), [destructuring](https://yuescript.org/doc/assignment/destructuring-assignment/), [bound-method stubs](https://yuescript.org/doc/functions/function-stubs/) | Address nil/false mistakes and callback boilerplate |
| [Fennel](https://github.com/bakpakin/Fennel/blob/main/reference.md) | Sequential/dictionary table notation, destructuring, compile-time macros; a Lua reader called antifennel | Keep Lua interop and bidirectional reading central; defer macros |
| [Teal](https://teal-language.org/) | Typed Lua compiled to ordinary Lua | Keep erasure distinct from type checking; reuse Luau's type system rather than inventing another |

## Proposed nua spellings

```text
-- Defaults apply only to nil, preserving false and zero.
def connect(host: string, port: number = 8080):
  ...

-- Distinguish table destructuring from Lua's existing multiple returns.
let {name = userName, score = userScore} = user
let [first, second] = position
let value, err = load()

-- Nil-coalescing; false remains false, and fallback is evaluated lazily.
let enabled = settings.enabled ?? true

-- A comprehension with explicit Lua iteration semantics.
let names = [user.name for _, user in ipairs(users) if user.active]
```

Field shorthand is excluded. Record construction uses explicit `key = value` fields, such as `{name = name, score = score}`. Any future record destructuring must also name keys explicitly. Positional brace literals retain their existing Lua meaning.

Defaults and destructuring look like the best next additions. Nil-coalescing needs careful single evaluation and short-circuiting. Comprehensions need a decision about nil results, sequence length, and multiple returns. A bound-method operator would also need to specify whether the object and method are captured when the closure is created or looked up when called.

Keep explicit returns and parenthesized calls. MoonScript's reference documents whitespace-sensitive ambiguities in omitted-parenthesis calls; nua gains little from introducing them. Class systems and macros would substantially expand the language and the conversion problem.

## Reading and debugging

Sugared constructs can compile to ordinary blocks and temporary variables. Reverse conversion should show that expanded form unless reconstruction is unambiguous; semantic reading matters more than recovering author intent.

[Fennel's `correlate` option](https://github.com/bakpakin/Fennel/blob/main/api.md) attempts to align generated Lua line numbers with its input. Source mappings or a similar diagnostic mode would be valuable as nua starts generating more code.
