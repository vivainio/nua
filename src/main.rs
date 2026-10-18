use std::{
    env, fs,
    io::{self, Read, Write},
    process::ExitCode,
};

const HELP: &str = "nua — bidirectional indentation syntax for Lua/Luau\n\nUsage: nua <to-luau|to-lua|from-luau|from-lua|strip-types> [INPUT|-] [-o OUTPUT]\n\nInput defaults to stdin; output defaults to stdout.\nto-lua and strip-types emit Lua 5.1; from-lua is an alias for from-luau.\n\nExamples:\n  nua to-luau game.nua -o game.luau\n  nua to-lua game.nua -o game.lua\n  nua strip-types game.luau -o game.lua\n  nua from-luau game.luau -o game.nua\n  cat game.lua | nua from-lua\n";

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or(HELP)?;
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        print!("{HELP}");
        return Ok(());
    }
    if matches!(command.as_str(), "--version" | "-V") {
        println!("nua {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if !matches!(
        command.as_str(),
        "to-luau" | "to-lua" | "from-luau" | "from-lua" | "strip-types"
    ) {
        return Err(format!("unknown command: {command}\n\n{HELP}"));
    }
    let mut input = None;
    let mut output = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" | "--output" => {
                if output.is_some() {
                    return Err("output specified twice".into());
                }
                output = Some(args.next().ok_or("missing output path")?);
            }
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(());
            }
            _ if arg.starts_with('-') && arg != "-" => {
                return Err(format!("unknown option: {arg}"));
            }
            _ => {
                if input.replace(arg).is_some() {
                    return Err("only one input is supported".into());
                }
            }
        }
    }
    let source = match input.as_deref() {
        None | Some("-") => {
            let mut s = String::new();
            io::stdin()
                .read_to_string(&mut s)
                .map_err(|e| e.to_string())?;
            s
        }
        Some(path) => fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?,
    };
    let converted = match command.as_str() {
        "to-lua" => nua::to_lua(&source),
        "strip-types" => nua::strip_types(&source),
        "from-lua" | "from-luau" => nua::from_luau(&source),
        _ => nua::to_luau(&source),
    }?;
    match output.as_deref() {
        None | Some("-") => io::stdout()
            .write_all(converted.as_bytes())
            .map_err(|e| e.to_string())?,
        Some(path) => fs::write(path, converted).map_err(|e| format!("{path}: {e}"))?,
    }
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("nua: {e}");
            ExitCode::FAILURE
        }
    }
}
