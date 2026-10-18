-- Qualified definitions keep Lua's existing assignment semantics.
local Counter = {}

function Counter:increment(amount: number)
  self.value += amount

-- A bare def is local; global def explicitly declares a global function.
end
function announce(message: string)
  print(message)

-- Explicit empty bodies.
end
local function placeholder()

end
local count = 0
repeat
  count += 1
until count >= 3

while count > 0 do
  count -= 1
end
