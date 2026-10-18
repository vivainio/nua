fn formats(source: &str, expected: &str) {
    let result = nua::from_luau(source).unwrap();
    assert_eq!(result, expected);
    let lua = nua::to_luau(&result).unwrap();
    assert_eq!(
        nua::from_luau(&lua).unwrap(),
        result,
        "formatting must stabilize after a round trip"
    );
}

#[test]
fn records_arrays_types_and_blank_lines() {
    formats(
        "local options = {\n    name = 'demo',\n    nested = {\n        enabled = true,\n        values = {\n            1,\n            2,\n        },\n    },\n}\n\ntype Options = {\n    name: string,\n    nested: {\n        enabled: boolean,\n        values: {number},\n    },\n}\n\nlocal empty = {}\n",
        "let options = {\n  name = 'demo',\n  nested = {\n    enabled = true,\n    values = [\n      1,\n      2,\n    ],\n  },\n}\n\ntype Options = {\n  name: string,\n  nested: {\n    enabled: boolean,\n    values: {number},\n  },\n}\n\nlet empty = {}\n",
    );
}

#[test]
fn callback_fields_close_with_their_comma() {
    formats(
        "local worker = configure('worker', {\n  timeout = 5,\n  handler = function(event)\n    return { got = event }\n  end,\n})\n",
        "let worker = configure('worker', {\n  timeout = 5,\n  handler = (def(event):\n    return { got = event }\n  ),\n})\n",
    );
}

#[test]
fn blocks_inside_data_literals_keep_both_kinds_of_indentation() {
    formats(
        "return {\n  entry = function()\n    local hovered = current()\n    if hovered then\n      emit('enter', {})\n    else\n      emit('preview', {})\n    end\n  end,\n}\n",
        "return {\n  entry = (def():\n    let hovered = current()\n    if hovered:\n      emit('enter', {})\n    else:\n      emit('preview', {})\n  ),\n}\n",
    );
}

#[test]
fn blank_lines_and_header_comments() {
    formats(
        "-- header\n\nlocal function f() -- explanation\n  -- body\n  return {}\nend\n\n-- next section\n\nfunction Object:method()\n  f()\nend\n",
        "-- header\n\ndef f(): -- explanation\n  -- body\n  return {}\n\n-- next section\n\ndef Object:method():\n  f()\n",
    );
}

#[test]
fn removed_block_ends_do_not_leave_semicolon_lines() {
    formats(
        "if ready then work() end;\nlocal function f() return 1 end;\n",
        "if ready:\n  work()\ndef f():\n  return 1\n",
    );
}

#[test]
fn tab_indentation_normalizes_but_long_string_contents_do_not() {
    formats(
        "local t = {\n\tx = 1,\n\ty = [[\n\tunchanged\n]],\n}\n",
        "let t = {\n  x = 1,\n  y = [[\n\tunchanged\n]],\n}\n",
    );
}

#[test]
fn inline_data_stays_inline_and_indexing_stays_indexing() {
    formats(
        "local t = {name = name, nested = {1, 2}, empty = {}}\nprint(t.nested[1])\n",
        "let t = {name = name, nested = [1, 2], empty = {}}\nprint(t.nested[1])\n",
    );
}

#[test]
fn commas_in_callback_bodies_are_not_argument_separators() {
    formats(
        "bench('sum', function(n) local total = 0 for i=1,n do total = total + i end return total, n end)\n",
        "bench(\n  'sum',\n  def(n):\n    let total = 0\n    for i=1,n:\n      total = total + i\n    return total, n\n)\n",
    );
}

#[test]
fn empty_bodies_do_not_accumulate_synthetic_comments() {
    formats(
        "local function f() end\n\nif yes then else end\n",
        "def f():\n  pass\n\nif yes:\n  pass\nelse:\n  pass\n",
    );
}

#[test]
fn nested_section_comments_and_header_spacing_stay_stable() {
    formats(
        "local function f()\n    for i=1,3 do\n        work()\n    end\n    -- next section\n    if yes then    -- explanation\n        work()\n    end\nend\n",
        "def f():\n  for i=1,3:\n    work()\n  -- next section\n  if yes:    -- explanation\n    work()\n",
    );
}

#[test]
fn blank_lines_before_callback_closers_survive_end_paren_joining() {
    formats(
        "foo((function()\n  for i=1,3 do\n    work()\n  end\n\nend), 'name')\n",
        "foo((def():\n  for i=1,3:\n    work()\n\n), 'name')\n",
    );
}
