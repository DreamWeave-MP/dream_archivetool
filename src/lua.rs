use mlua::{Lua, Result as LuaResult, Table};

pub fn create_module(lua: &Lua) -> LuaResult<Table> {
    lua.create_table()
}

pub fn register(lua: &Lua) -> LuaResult<()> {
    let module = create_module(lua)?;
    lua.globals().set("rome_archivetool", module)
}
