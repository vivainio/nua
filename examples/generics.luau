-- Luau function types keep ->; definitions use -> for the return type too.
export type Mapper<T, U> = (T) -> U

local function map<T, U>(items: {T}, transform: Mapper<T, U>) : {U}
  local result: {U} = {}
  for i, item in items do
    result[i] = transform(item)
  end
  return result

end
local doubled = map({1, 2, 3}, function(value: number) : number
  return value * 2
end
)

-- Type packs and multiple return values are preserved.
local function identity<T, Rest...>(first: T, ...: Rest...) : (T, Rest...)
  return first, ...
end
