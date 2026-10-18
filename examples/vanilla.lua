-- This example can target either Luau or vanilla Lua 5.1.


local function map(items, transform)
  local result = {}
  for i, item in ipairs(items) do
    result[i] = transform(item)
  end
  return result

end
local doubled = map({1, 2, 3}, function(value)
  return value * 2
end)

for _, value in ipairs(doubled) do
  print(value)
end
