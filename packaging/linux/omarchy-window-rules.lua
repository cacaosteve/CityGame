-- Add this to the personal window rules in ~/.config/hypr/hyprland.lua.
-- The main CityGame window and its SDL tools palette share one SDL app ID.
o.window("^com\\.citygame\\.CityGame$", { float = true, workspace = "2" })
