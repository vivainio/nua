-- This example can target either Luau or vanilla Lua 5.1.
export type Mapper<T, U> = (T) -> U

local function map<T, U>(items: {T}, transform: Mapper<T, U>) : {U}
  local result: {U} = {}
  for i, item in ipairs(items) do
    result[i] = transform(item)
  end
  return result

end
local doubled: {number} = map({1, 2, 3}, function(value: number) : number
  return value * 2
end
)

for _, value in ipairs(doubled) do
  print(value)
end
