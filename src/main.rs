use std::{io, time::Duration};

use clap::Parser;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use my_dev_clock::app::{Action, App, Cli};
use ratatui::DefaultTerminal;

/// How long one frame waits for input.
const FRAME: Duration = Duration::from_millis(100);
/// Cap on events applied between two frames, so input cannot starve drawing.
const MAX_EVENTS_PER_FRAME: usize = 32;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Handled before the terminal takes over, so the message prints normally.
    if cli.init {
        let path = my_dev_clock::config::config_path(cli.config.as_deref())
            .ok_or("não encontrei um local para gravar a configuração")?;
        my_dev_clock::config::write_example(&path)?;
        println!("Configuração de exemplo gravada em {}", path.display());
        return Ok(());
    }

    // Uma linha para barra de status ou prompt: nada de tela alternativa.
    if let Some(format) = cli.print.clone() {
        let app = App::from_cli(cli);
        println!("{}", app.print_line(&format));
        return Ok(());
    }

    if cli.check {
        App::from_cli(cli).check();
        return Ok(());
    }

    let mut app = App::from_cli(cli);

    // `init` sets raw mode, the alternate screen and a panic hook that restores
    // the terminal, so a crash cannot leave the console unusable.
    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;

    let result = run(&mut terminal, &mut app);

    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();

    // Printed after the alternate screen is gone.
    app.on_exit();
    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        app.tick();
        terminal.draw(|frame| app.ui(frame))?;

        // `on_event` descarta o evento de soltar a tecla, que no Windows sempre
        // chega: sem esse filtro cada atalho seria aplicado duas vezes. A fila
        // inteira é drenada por quadro, para uma rajada de teclas não precisar
        // de um quadro por tecla.
        let mut handled = 0;
        while handled < MAX_EVENTS_PER_FRAME && event::poll(FRAME)? {
            if app.on_event(event::read()?) == Action::Quit {
                return Ok(());
            }
            handled += 1;
        }

        if app.is_ended() {
            break;
        }
    }

    Ok(())
}
