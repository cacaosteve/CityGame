//! Desktop entry. Mobile hosts link `libcitygame` and call `SDL_main` instead.

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if citygame::desktop_cli::maybe_run_cli(args)? {
        return Ok(());
    }
    if let Err(error) = citygame::run() {
        eprintln!("CityGame failed: {error:#}");
        return Err(error);
    }
    Ok(())
}
