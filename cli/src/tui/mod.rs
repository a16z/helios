mod app;
mod ui;

pub use app::App;
pub use ui::draw;

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use tokio::sync::mpsc;
use tokio::time::interval;

use helios_common::network_spec::NetworkSpec;
use helios_core::client::HeliosClient;

/// Run the TUI application
pub async fn run<N: NetworkSpec>(client: HeliosClient<N>) -> Result<(), io::Error> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(client);

    // Create channel for UI updates
    let (tx, mut rx) = mpsc::channel(100);

    // Spawn update task
    let update_tx = tx.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;
            let _ = update_tx.send(()).await;
        }
    });

    // Run the app
    let res = run_app(&mut terminal, &mut app, &mut rx).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("{:?}", err);
    }

    Ok(())
}

async fn run_app<B: Backend, N: NetworkSpec>(
    terminal: &mut Terminal<B>,
    app: &mut App<N>,
    rx: &mut mpsc::Receiver<()>,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;

        tokio::select! {
            _ = rx.recv() => {
                app.update().await;
            }
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                if event::poll(Duration::from_millis(0))? {
                    if let Event::Key(key) = event::read()? {
                        if let KeyCode::Char('q') = key.code {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}
