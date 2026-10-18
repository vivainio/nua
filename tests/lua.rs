use full_moon::{LuaVersion, node::Node};

fn equivalent(actual: &str, expected: &str) {
    let parse = |s: &str| {
        full_moon::parse_fallible(s, LuaVersion::lua51())
            .into_result()
            .unwrap()
    };
    assert!(
        parse(actual).nodes().similar(parse(expected).nodes()),
        "ACTUAL:\n{actual}\nEXPECTED:\n{expected}"
    );
}

#[test]
fn strips_annotations_aliases_generics_and_assertions() {
    let luau = "export type Result<T> = {value: T?, error: string?};\ntype Callback = (number) -> number;\nlocal function identity<T, Rest...>(value: T, ...: Rest...): (T, Rest...)\nreturn value, ...\nend\nlocal values: {number} = {1, 2}\nlocal first = (values[1] :: number)\nlocal function apply(f: Callback, value: number): number return f(value) end\n";
    let expected = "local function identity(value, ...) return value, ... end\nlocal values = {1, 2}\nlocal first = (values[1])\nlocal function apply(f, value) return f(value) end";
    equivalent(&nua::strip_types(luau).unwrap(), expected);
}

#[test]
fn strips_method_and_loop_types_without_changing_colons() {
    let source = "function Object:move(amount: number): () self.x = self.x + amount end\nfor i: number = 1, 3 do print(i) end\nfor k: string, v: number in pairs(values) do print(k, v) end";
    equivalent(
        &nua::strip_types(source).unwrap(),
        "function Object:move(amount) self.x = self.x + amount end\nfor i = 1,3 do print(i) end\nfor k,v in pairs(values) do print(k,v) end",
    );
}

#[test]
fn preserves_literals_comments_and_line_numbers() {
    let source = "-- header\nexport type Name = string -- alias comment\nlocal text: string = 'type T = number :: X' -- value comment\nlocal block: string = [=[\n{x: number}\n]=]\nlocal x: -- annotation comment\nnumber = 1\n";
    let output = nua::strip_types(source).unwrap();
    for text in [
        "-- header",
        "-- alias comment",
        "-- value comment",
        "-- annotation comment",
        "'type T = number :: X'",
        "[=[\n{x: number}\n]=]",
    ] {
        assert!(output.contains(text), "missing {text}:\n{output}");
    }
    assert_eq!(source.lines().count(), output.lines().count());
    equivalent(
        &output,
        "local text = 'type T = number :: X'\nlocal block = [=[\n{x: number}\n]=]\nlocal x = 1",
    );
}

#[test]
fn to_lua_transpiles_nua_and_to_luau_preserves_types() {
    let source = "type Numbers = {number}\ndef double(x: number) -> number:\n  return x * 2\nlet numbers: Numbers = [1, 2, 3]\nfor i, value in ipairs(numbers):\n  numbers[i] = double(value)\n";
    equivalent(
        &nua::to_lua(source).unwrap(),
        "local function double(x) return x*2 end\nlocal numbers = {1,2,3}\nfor i,value in ipairs(numbers) do numbers[i] = double(value) end",
    );
    let luau = nua::to_luau(source).unwrap();
    assert!(luau.contains("type Numbers"));
    assert!(luau.contains("x: number"));
}

#[test]
fn rejects_runtime_extensions_with_actionable_errors() {
    for (source, message) in [
        ("local x = 1; x += 2", "compound assignment"),
        ("while true do continue end", "continue"),
        ("local x = if yes then 1 else 2", "inline conditional"),
        ("local s = `hello {name}`", "backtick strings"),
        ("for k,v in items do print(v) end", "generalized iteration"),
        ("local x = 0b1010", "numeric literal"),
        ("local x = 1_000", "numeric literal"),
        (r#"local x = "\u{41}""#, "string escape"),
        (r#"local x = "\x41""#, "string escape"),
        (r#"local x = "\z  hi""#, "string escape"),
    ] {
        let error = nua::strip_types(source).unwrap_err();
        assert!(error.contains(message), "{source}: {error}");
        assert!(error.contains("line 1"), "{error}");
    }
}

#[test]
fn accepts_plain_lua_and_explicit_iterators() {
    let source = "local t = {1,2}\nfor k,v in next,t do print(k,v) end\nfor x in iterator() do print(x) end\nlocal s = [[\u{41}]]\nlocal escaped = '\\\\x41'\n";
    assert_eq!(nua::strip_types(source).unwrap(), source);
}

#[test]
fn compile_time_bodies_are_erased_before_runtime_checks() {
    let source = "type T = typeof(if enabled then 1 else 2)\ntype function f(t) local x = `hi` return t end\nlocal x: T = 1";
    equivalent(&nua::strip_types(source).unwrap(), "local x = 1");
}
