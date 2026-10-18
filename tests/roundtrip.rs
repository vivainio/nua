use full_moon::node::Node;
fn roundtrip(lua: &str) {
    let nua = nua::from_luau(lua).unwrap();
    let back = nua::to_luau(&nua).unwrap_or_else(|e| panic!("{e}\nNUA:\n{nua}"));
    let a = full_moon::parse(lua).unwrap();
    let b = full_moon::parse(&back).unwrap();
    assert!(a.nodes().similar(b.nodes()), "NUA:\n{nua}\nBACK:\n{back}");
}
#[test]
fn lua_control_flow() {
    roundtrip(
        "local function f(x) if x then return 1 elseif x == nil then return 2 else return 3 end end\nfor i=1,10 do f(i) end\nfor k,v in pairs(t) do print(k,v) end\nwhile running do tick() end\nrepeat tick() until done\ndo print('scope') end",
    );
}
#[test]
fn luau_types() {
    roundtrip(
        "export type Result<T> = {value: T?, error: string?}\ntype Callback<T, U> = (T) -> U\nlocal function map<T, U>(items: {T}, f: (T) -> U): {U}\nlocal result: {U} = {}\nfor i, v in items do result[i] = f(v) end\nreturn result\nend\nlocal label = if enabled then 'On' else 'Off'\nlocal user = value :: Result<string>",
    );
}
#[test]
fn functions_and_callbacks() {
    roundtrip(
        "function global(a) return a end\nfunction Player:move(x: number): () self.x += x end\nlocal f = function(x: number): number return x*2 end\nbutton:Connect(function() print('hi') end)",
    );
}
#[test]
fn empty_blocks() {
    roundtrip(
        "local function f() end\nif true then else end\nrepeat until true\nwhile false do end",
    );
}
#[test]
fn strings_comments_and_multiline() {
    roundtrip(
        "-- function then end\nlocal function f() -- header\n-- inside\nlocal s = [=[\nend\nif x then\n]=]\nlocal t = {\n'function',\n'end'\n}\nreturn `hello {s}`\nend -- tail\n",
    );
}
#[test]
fn chosen_syntax() {
    let source =
        "def greet(user: User) -> string:\n  let name = user.name\n  return `Hello, {name}`\n";
    let result = nua::to_luau(source).unwrap();
    assert!(result.starts_with("local function greet(user: User) : string"));
    assert!(result.contains("local name"));
}
#[test]
fn rejects_invalid_input() {
    for input in [
        "def f():\nreturn 1",
        "  let x = 1",
        "if true:\n\tpass",
        "let x =",
        "repeat:\n  pass",
    ] {
        assert!(nua::to_luau(input).is_err(), "{input}");
    }
    assert!(nua::from_luau("function broken(").is_err());
}
#[test]
fn function_return_types_and_packs() {
    roundtrip(
        "local function f<T, U...>(x: T, ...: U...): (T, U...) return x, ... end\nlocal function maker(): (number) -> string return function(x) return tostring(x) end end",
    );
}
#[test]
fn reserved_identifiers() {
    roundtrip(
        "local def, let, pass, global = 1,2,3,4\nlet = def + pass\nlocal function def(let) return let end\nlocal t = {def = def, let = let}\nprint(t.def)",
    );
}
#[test]
fn multiline_headers() {
    roundtrip(
        "local function f(\n x: number,\n y: string\n): {\n value: string\n}\n return {value = y}\nend\nif (\n enabled and ready\n) then\n f(1, 'hi')\nend",
    );
}
#[test]
fn nested_callback_and_tables() {
    roundtrip(
        "local t = {run = function(x) if x then print(x) end end}\nfoo(function() bar(function() print('nested') end) end, 42)\nlocal a = {[1] = 'x', nested = {1,2,3}}",
    );
}
#[test]
fn compact_statements() {
    roundtrip(
        "local x = 1 local y = 2 if x then y = 3 end print(y) local function f() local a = 1 return a end",
    );
}
#[test]
fn callback_followed_by_multiline_table() {
    let source = "foo(def():\n  print('hi')\n)\nlet t = {\n  1,\n  2,\n}\n";
    assert!(nua::to_luau(source).is_ok());
}
#[test]
fn semicolons() {
    roundtrip("local x = 1; print(x); if x then print(x); end; local function f() return 1; end;");
}
#[test]
fn array_literals() {
    let source = "let xs: {number} = [1, 2, 3]\nlet first = xs[1]\nlet nested = [ [1], [2] ]\nlet empty = []\nlet dict = {[first] = xs}\nlet s = ('abc')[1]\n";
    let lua = nua::to_luau(source).unwrap();
    assert!(lua.contains("local xs: {number} = {1, 2, 3}"));
    assert!(lua.contains("xs[1]"));
    assert!(lua.contains("{[first] = xs}"));
    assert!(lua.contains("('abc')[1]"));
}
#[test]
fn arrays_reverse() {
    roundtrip(
        "local xs: {number} = {1, 2, 3}\nlocal x = xs[1]\nlocal nested = {{1},{2}}\nlocal d: {[string]: {number}} = {x = {1,2}}\nprint {1,2}\nlocal s = [[literal]]",
    );
    let nua = nua::from_luau("local xs = {1, 2}").unwrap();
    assert!(nua.contains("[1, 2]"));
}

#[test]
fn elif_branches_in_both_directions() {
    let source = "if x == 1:\n  print('one')\nelif x == 2:\n  print('two')\nelif (\n  x == 3\n):\n  print('three')\nelse:\n  print('other')\n";
    let lua = nua::to_luau(source).unwrap();
    assert!(lua.contains("elseif x == 2 then"));
    let back = nua::from_luau(&lua).unwrap();
    assert!(back.contains("elif x == 2:"));
    assert!(!back.contains("elseif"));
    roundtrip(&lua);
}

#[test]
fn elif_expression_and_identifier() {
    let source = "let label = if x then 1 elif y then 2 else 3\n";
    let lua = nua::to_luau(source).unwrap();
    assert!(lua.contains("elseif y then 2"));
    assert!(nua::from_luau(&lua).unwrap().contains("elif y then 2"));
    roundtrip("local elif = 1\nprint(elif)\nlocal t = {elif = elif}\nprint(t.elif)");
    roundtrip("local label = if x then 1 elseif y then 2 else 3");
}
