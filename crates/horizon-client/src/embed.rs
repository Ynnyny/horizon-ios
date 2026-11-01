//! Running the game inside a host process (the iOS launcher): the same setup
//! as `main`, but the game's result is returned instead of ending the process.

use std::path::PathBuf;

use crate::{crash, game, logging};

/// Runs the game with `args` and returns its result. The caller owns the
/// process, so a failed run stays a value instead of `process::exit`.
pub fn run(args: Vec<String>) -> crate::Result<()> {
    horizon_threads::configure_allocator();
    crate::telemetry::mark_start();
    // --home before the log opens in it.
    let home = args
        .iter()
        .skip_while(|a| *a != "--home")
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(logging::game_home);
    logging::set_game_home(home.clone());
    logging::init(&home, "Render thread");
    log::info!("Horizon {}", crash::version());
    #[cfg(not(target_os = "ios"))]
    crash::install(home.clone(), "client");
    game::set_args(args);
    match game::run() {
        Ok(()) => {
            logging::flush();
            Ok(())
        }
        Err(error) => {
            let usage = error.is::<game::UsageError>();
            log::error!("Horizon: {error}");
            logging::flush();
            #[cfg(not(target_os = "ios"))]
            if !usage {
                crash::report_error(&home, "client", &*error);
            }
            #[cfg(target_os = "ios")]
            let _ = usage;
            Err(error)
        }
    }
}
