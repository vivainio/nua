-- Typed declarations, arrays, records, and indentation.
type User = {
  name: string,
  score: number,
}

local function greet(user: User) : string
  return `Hello, {user.name}!`

end
local users: {User} = {
  {name = "Ada", score = 120},
  {name = "Lin", score = 40},
}

for _, user in users do
  if user.score >= 100 then
    print(greet(user))
  elseif user.score > 0 then
    print("Keep going")
  else
    continue

  end
end
local first = users[1]
local label = if first.score >= 100 then "Expert" else "Beginner"
