use anyhow::Result;
use hibiki::application::config_service::ConfigService;
use hibiki::infrastructure::settings_repository::SettingsRepository;
use hibiki::{app, compositor, input, tray};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const HELP: &str = "\
Hibiki - keystroke and bubble visualizer for Wayland

USAGE:
    hibiki [OPTIONS]

OPTIONS:
    -n, --no-ui             Skip the launcher and start in the configured mode
    -k, --keystroke-toggle  Switch to keystroke mode
    -b, --bubble-toggle     Switch to bubble mode
        --keystroke         Alias for --keystroke-toggle
        --bubble            Alias for --bubble-toggle
    -t, --toggle            Toggle between keystroke and bubble mode
    -p, --pause-toggle      Pause or resume key capture
        --settings          Open the settings window
        --launcher          Show the launcher window
        --quit              Quit the running instance
    -h, --help              Print this help and exit
    -V, --version           Print the version and exit

All control options are forwarded to an already-running instance, so they can
be bound to compositor shortcuts, e.g. on Hyprland:

    bind = SUPER, K, exec, hibiki --keystroke-toggle
    bind = SUPER, B, exec, hibiki --bubble-toggle
    bind = SUPER, P, exec, hibiki --pause-toggle
";

fn main() -> Result<()> {
    init_logging();

    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return Ok(());
    }

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("hibiki {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let no_ui = args.iter().any(|a| a == "--no-ui" || a == "-n");

    info!("Starting Hibiki v{}", env!("CARGO_PKG_VERSION"));
    if no_ui {
        info!("--no-ui flag set: skipping Dashboard window");
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");
    let _guard = runtime.enter();

    let compositor = compositor::detect();
    info!("Detected compositor: {}", compositor);

    let settings_repo = SettingsRepository::new()?;
    let config_service = ConfigService::new(settings_repo)?;
    let config = config_service.get_config();

    info!("Configuration loaded: {:?}", config.position);

    let app = app::App::new(config_service, no_ui);

    if app.is_remote() {
        info!("Another instance is running; forwarding command line");
        let exit_code = app.run();
        std::process::exit(exit_code);
    }

    if config.auto_detect_layout {
        let layout_manager = input::LayoutManager::new();
        if let Err(e) = layout_manager.init() {
            warn!("Failed to initialize layout detection: {}", e);
        } else if let Some(layout) = layout_manager.current_layout_name() {
            info!("Detected keyboard layout: {}", layout);
        }
    }

    match tray::start_tray() {
        Ok((tray_rx, tray_handle)) => {
            info!("System tray started successfully");
            let exit_code = app.run_with_tray(tray_rx, tray_handle);
            std::process::exit(exit_code);
        }
        Err(e) => {
            warn!("Failed to start system tray: {}, running without tray", e);
            let exit_code = app.run();
            std::process::exit(exit_code);
        }
    }
}

fn init_logging() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,hibiki=debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();
}
