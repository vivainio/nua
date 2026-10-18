-- Existing Lua can be read through the same CLI.
local function sum(values)
    local total = 0
    for _, value in ipairs(values) do
        total = total + value
    end
    return total
end

local values = {10, 20, 30}
if sum(values) > 50 then
    print("over fifty")
else
    print("fifty or less")
end
