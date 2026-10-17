//! Verificação de que o app realmente desenha: os testes rodam o mesmo caminho
//! de renderização do terminal real, sobre um backend de teste, e conferem o
//! conteúdo do quadro.

use std::{path::PathBuf, time::Duration};

use clap::Parser;
use my_dev_clock::app::{App, Cli};
use ratatui::{Terminal, backend::TestBackend};

/// Config inexistente, para os testes não dependerem do arquivo da máquina.
const SEM_CONFIG: &str = "config-que-nao-existe.toml";

fn app(extra: &[&str], config: &str) -> App {
    let mut argv = vec![
        "my-dev-clock".to_string(),
        "--config".to_string(),
        config.to_string(),
    ];
    argv.extend(extra.iter().map(|arg| (*arg).to_string()));
    let cli = Cli::try_parse_from(argv).expect("linha de comando válida");
    App::from_cli(cli)
}

fn buffer_text(buffer: &ratatui::buffer::Buffer, width: u16, height: u16) -> String {
    let mut text = String::new();
    for y in 0..height {
        for x in 0..width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

fn draw(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal de teste");
    terminal
        .draw(|frame| app.ui(frame))
        .expect("desenho do quadro");
    buffer_text(terminal.backend().buffer(), width, height)
}

fn blocks(text: &str) -> usize {
    text.chars().filter(|c| *c == '█').count()
}

fn write_config(name: &str, contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("my-dev-clock-test-{name}.toml"));
    std::fs::write(&path, contents).expect("grava a config do teste");
    path
}

#[test]
fn the_clock_draws_block_digits() {
    let mut app = app(&[], SEM_CONFIG);
    let frame = draw(&mut app, 80, 24);

    assert!(blocks(&frame) > 40, "poucos blocos desenhados:\n{frame}");
}

#[test]
fn a_larger_scale_draws_more_blocks() {
    let mut small = app(&["--size", "1"], SEM_CONFIG);
    let mut large = app(&["--size", "4"], SEM_CONFIG);

    let small_blocks = blocks(&draw(&mut small, 120, 40));
    let large_blocks = blocks(&draw(&mut large, 120, 40));

    assert!(
        large_blocks > small_blocks * 4,
        "escala 4 ({large_blocks}) deveria ser bem maior que escala 1 ({small_blocks})"
    );
}

#[test]
fn typing_a_scale_grows_the_digits_live() {
    use crossterm::event::KeyCode;

    let mut app = app(&[], SEM_CONFIG);
    let before = blocks(&draw(&mut app, 120, 40));

    app.on_key(KeyCode::Char('s'));
    app.on_key(KeyCode::Char('5'));
    let after = blocks(&draw(&mut app, 120, 40));

    assert!(
        after > before,
        "digitar a escala deveria aumentar os dígitos ({before} -> {after})"
    );
}

#[test]
fn the_header_shows_a_localized_date() {
    let mut app = app(&[], SEM_CONFIG);
    let frame = draw(&mut app, 120, 30);
    let year = chrono::Local::now().format("%Y").to_string();

    assert!(frame.contains(&year), "faltou o ano no cabeçalho:\n{frame}");
    assert!(
        frame.contains("feira") || frame.contains("sábado") || frame.contains("domingo"),
        "faltou o dia da semana:\n{frame}"
    );
}

#[test]
fn hiding_the_date_removes_the_header_line() {
    let mut app = app(&["--no-date"], SEM_CONFIG);
    let frame = draw(&mut app, 120, 30);
    let year = chrono::Local::now().format("%Y").to_string();

    assert!(
        !frame.contains(&year),
        "a data não deveria aparecer:\n{frame}"
    );
}

#[test]
fn every_theme_renders_digits() {
    for theme in ["default", "matrix", "neon", "system", "fire", "ice"] {
        let mut app = app(&["--theme", theme], SEM_CONFIG);
        let frame = draw(&mut app, 100, 30);
        assert!(blocks(&frame) > 40, "o tema {theme} não desenhou:\n{frame}");
    }
}

#[test]
fn an_unknown_theme_still_renders() {
    let mut app = app(&["--theme", "tema-inexistente"], SEM_CONFIG);
    let frame = draw(&mut app, 100, 30);
    assert!(
        blocks(&frame) > 40,
        "tema desconhecido não desenhou:\n{frame}"
    );
}

#[test]
fn every_mode_renders_digits() {
    for extra in [
        vec!["clock"],
        vec!["stopwatch"],
        vec!["timer", "-d", "10m"],
        vec!["countdown", "-t", "23:59"],
    ] {
        let mut app = app(&extra, SEM_CONFIG);
        let frame = draw(&mut app, 100, 30);
        assert!(
            blocks(&frame) > 10,
            "o modo {extra:?} não desenhou:\n{frame}"
        );
    }
}

#[test]
fn the_countdown_title_appears_as_the_header() {
    let mut app = app(&["countdown", "-t", "23:59", "-T", "Jantar"], SEM_CONFIG);
    let frame = draw(&mut app, 100, 30);

    assert!(frame.contains("Jantar"), "faltou o título:\n{frame}");
}

#[test]
fn a_command_widget_renders_its_output() {
    let config = write_config(
        "command-widget",
        r#"
        [clock]
        [[clock.widgets]]
        kind = "command"
        title = "Teste"
        command = "echo widget-ok"
        refresh_secs = 3600
        timeout_secs = 20
        "#,
    );

    let mut app = app(&[], config.to_str().expect("caminho utf-8"));
    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("terminal de teste");

    // O widget só é agendado depois de ficar visível no primeiro desenho, e a
    // saída volta pelo canal do worker, então alternamos desenho e tick.
    let mut frame = String::new();
    for _ in 0..60 {
        terminal.draw(|f| app.ui(f)).expect("desenho do quadro");
        app.tick();
        frame = buffer_text(terminal.backend().buffer(), 100, 24);
        if frame.contains("widget-ok") {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(
        frame.contains("widget-ok"),
        "o widget não mostrou a saída do comando:\n{frame}"
    );
    assert!(
        frame.contains("Teste"),
        "faltou o título do widget:\n{frame}"
    );
}

#[test]
fn z_hides_and_shows_the_widgets() {
    use crossterm::event::KeyCode;

    let config = write_config(
        "hide-widget",
        r#"
        [clock]
        [[clock.widgets]]
        kind = "command"
        title = "Painel"
        command = "echo pronto"
        refresh_secs = 3600
        "#,
    );

    let mut app = app(&[], config.to_str().expect("caminho utf-8"));
    let shown = draw(&mut app, 100, 24);
    assert!(shown.contains("Painel"), "faltou o widget:\n{shown}");

    app.on_key(KeyCode::Char('z'));
    let hidden = draw(&mut app, 100, 24);
    assert!(
        !hidden.contains("Painel"),
        "o widget deveria sumir:\n{hidden}"
    );
    assert!(
        blocks(&hidden) > 40,
        "o relógio deveria ocupar a tela toda:\n{hidden}"
    );

    app.on_key(KeyCode::Char('z'));
    let back = draw(&mut app, 100, 24);
    assert!(back.contains("Painel"), "o widget deveria voltar:\n{back}");
}

#[test]
fn a_tiny_terminal_does_not_panic() {
    for (width, height) in [(1, 1), (4, 2), (20, 5), (10, 40)] {
        let mut app = app(&[], SEM_CONFIG);
        let frame = draw(&mut app, width, height);
        assert!(!frame.is_empty());
    }
}

#[test]
fn the_scale_prompt_is_drawn_at_the_bottom() {
    use crossterm::event::KeyCode;

    let mut app = app(&[], SEM_CONFIG);
    app.on_key(KeyCode::Char('s'));
    app.on_key(KeyCode::Char('7'));
    let frame = draw(&mut app, 100, 24);

    assert!(
        frame.contains("Tamanho da fonte"),
        "faltou o prompt:\n{frame}"
    );
    assert!(frame.contains(": 7_"), "faltou o valor digitado:\n{frame}");
}

#[test]
fn every_font_draws_the_digits() {
    for font in ["bricks", "slim", "slant", "flip"] {
        let mut app = app(&["--font", font], SEM_CONFIG);
        let frame = draw(&mut app, 120, 36);
        assert!(blocks(&frame) > 30, "a fonte {font} não desenhou:\n{frame}");
    }
}

#[test]
fn the_flip_face_paints_filled_cards() {
    let mut app = app(&["--font", "flip"], SEM_CONFIG);
    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).expect("terminal de teste");
    terminal.draw(|f| app.ui(f)).expect("desenho do quadro");
    let buffer = terminal.backend().buffer();

    let painted = (0..36)
        .flat_map(|y| (0..120).map(move |x| (x, y)))
        .filter(|(x, y)| buffer[(*x, *y)].bg != ratatui::style::Color::Reset)
        .count();
    assert!(
        painted > 250,
        "os cartões deveriam cobrir boa parte da tela, pintaram {painted} células"
    );
}

#[test]
fn the_flip_animation_changes_a_digit_when_the_second_turns() {
    let mut app = app(&["--font", "flip"], SEM_CONFIG);
    let first = draw(&mut app, 120, 36);

    // Espera virar o segundo para o último dígito mudar e o flip acontecer.
    let started = std::time::Instant::now();
    let mut flipping = String::new();
    while started.elapsed() < std::time::Duration::from_secs(2) {
        flipping = draw(&mut app, 120, 36);
        if flipping != first && flipping.contains('█') {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    assert_ne!(first, flipping, "nada mudou em dois segundos");
}

#[test]
fn every_animation_paints_a_background_behind_the_clock() {
    let plain = {
        let mut app = app(&[], SEM_CONFIG);
        draw(&mut app, 120, 36)
    };
    let plain_blocks = blocks(&plain);

    for kind in ["matrix", "plasma", "waves", "stars"] {
        let mut app = app(&["--animation", kind], SEM_CONFIG);
        let frame = draw(&mut app, 120, 36);

        assert_ne!(frame, plain, "a animação {kind} não apareceu no quadro");
        assert_eq!(
            blocks(&frame),
            plain_blocks,
            "a animação {kind} mexeu nos dígitos:\n{frame}"
        );
    }
}

#[test]
fn the_animation_moves_between_frames() {
    let mut app = app(&["--animation", "plasma"], SEM_CONFIG);
    let first = draw(&mut app, 120, 36);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let later = draw(&mut app, 120, 36);

    assert_ne!(first, later, "a animação ficou parada");
}

#[test]
fn fill_gives_the_clock_more_room_than_sharing_the_screen() {
    let config = write_config(
        "fill",
        r#"
        [clock]
        [[clock.widgets]]
        kind = "command"
        title = "Painel"
        command = "echo ok"
        refresh_secs = 3600
        "#,
    );
    let path = config.to_str().expect("caminho utf-8");

    let mut shared = app(&[], path);
    let shared_blocks = blocks(&draw(&mut shared, 300, 44));

    let mut full = app(&["--fill"], path);
    let full_blocks = blocks(&draw(&mut full, 300, 44));

    assert!(
        full_blocks > shared_blocks,
        "tela cheia deveria aumentar o relógio ({full_blocks} vs {shared_blocks})"
    );
}

#[test]
fn the_look_cycle_keys_change_the_picture() {
    use crossterm::event::KeyCode;

    let mut app = app(&[], SEM_CONFIG);
    let plain = draw(&mut app, 120, 36);

    app.on_key(KeyCode::Char('F'));
    let after_font = draw(&mut app, 120, 36);
    assert_ne!(plain, after_font, "F deveria trocar a fonte");

    app.on_key(KeyCode::Char('A'));
    let after_animation = draw(&mut app, 120, 36);
    assert_ne!(
        after_font, after_animation,
        "A deveria ligar a animação de fundo"
    );
}

#[test]
fn unknown_look_names_are_refused_on_the_command_line() {
    assert!(Cli::try_parse_from(["my-dev-clock", "--font", "comic"]).is_err());
    assert!(Cli::try_parse_from(["my-dev-clock", "--animation", "fogos"]).is_err());
    assert!(Cli::try_parse_from(["my-dev-clock", "--font", "slim"]).is_ok());
    assert!(Cli::try_parse_from(["my-dev-clock", "--animation", "stars"]).is_ok());
}

#[test]
fn a_shortcut_shows_a_confirmation_line() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let mut app = app(&[], SEM_CONFIG);
    let plain = draw(&mut app, 120, 36);

    app.on_event(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Char('F'),
        KeyModifiers::NONE,
        KeyEventKind::Press,
    )));
    let changed = draw(&mut app, 120, 36);

    assert_ne!(plain, changed);
    assert!(
        changed.contains("fonte: "),
        "faltou a confirmação do atalho:\n{changed}"
    );
}

#[test]
fn a_key_release_does_not_change_the_picture() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let mut app = app(&[], SEM_CONFIG);
    let press = |app: &mut App| {
        app.on_event(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('F'),
            KeyModifiers::NONE,
            KeyEventKind::Press,
        )));
    };
    let release = |app: &mut App| {
        app.on_event(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('F'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )));
    };

    press(&mut app);
    let after_press = draw(&mut app, 120, 36);

    release(&mut app);
    let after_release = draw(&mut app, 120, 36);

    assert_eq!(
        after_press, after_release,
        "soltar a tecla não deveria mexer no quadro"
    );
}

#[test]
fn the_help_overlay_shows_the_key_map() {
    use crossterm::event::KeyCode;

    let mut app = app(&[], SEM_CONFIG);
    let plain = draw(&mut app, 120, 36);

    app.on_key(KeyCode::Char('?'));
    let help = draw(&mut app, 120, 36);

    assert_ne!(plain, help, "a ajuda deveria cobrir a tela");
    assert!(help.contains("teclas"), "faltou o título:\n{help}");
    assert!(help.contains("marca uma volta"), "faltou a linha:\n{help}");
    assert!(help.contains("próximo tema"), "faltou a linha:\n{help}");

    app.on_key(KeyCode::Char('?'));
    assert_eq!(
        draw(&mut app, 120, 36),
        plain,
        "fechar volta ao quadro anterior"
    );
}

/// Desenha e roda os workers até `done` aceitar o quadro, o que é o que um
/// widget de comando precisa para aparecer.
fn draw_until(app: &mut App, width: u16, height: u16, done: impl Fn(&str) -> bool) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal de teste");

    let mut frame = String::new();
    for _ in 0..80 {
        terminal.draw(|f| app.ui(f)).expect("desenho do quadro");
        app.tick();
        frame = buffer_text(terminal.backend().buffer(), width, height);
        if done(&frame) {
            break;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    frame
}

fn four_widgets_config(name: &str) -> PathBuf {
    write_config(
        name,
        r#"
        [clock]
        max_columns = 4

        [[clock.widgets]]
        kind = "command"
        title = "Um"
        command = "echo primeiro"
        refresh_secs = 3600

        [[clock.widgets]]
        kind = "command"
        title = "Dois"
        command = "echo segundo"
        refresh_secs = 3600

        [[clock.widgets]]
        kind = "command"
        title = "Tres"
        command = "echo terceiro"
        refresh_secs = 3600

        [[clock.widgets]]
        kind = "command"
        title = "Quatro"
        command = "echo quarto"
        refresh_secs = 3600
        "#,
    )
}

#[test]
fn splitting_the_terminal_keeps_every_widget_on_screen() {
    let config = four_widgets_config("estreito");
    let path = config.to_str().expect("caminho utf-8");
    let mut app = app(&[], path);

    // Numa janela larga cabem as quatro colunas.
    let wide = draw_until(&mut app, 170, 40, |frame| frame.contains("quarto"));
    for titulo in ["Um", "Dois", "Tres", "Quatro"] {
        assert!(
            wide.contains(titulo),
            "faltou {titulo} na janela larga:\n{wide}"
        );
    }

    // A janela encolhe, como num terminal dividido: nada pode sumir sozinho.
    let narrow = draw_until(&mut app, 70, 40, |frame| frame.contains("primeiro"));
    for titulo in ["Um", "Dois", "Tres", "Quatro"] {
        assert!(
            narrow.contains(titulo),
            "o widget {titulo} sumiu ao encolher:\n{narrow}"
        );
    }
}

#[test]
fn a_very_narrow_pane_still_shows_the_time_instead_of_a_cut_digit() {
    let mut app = app(&[], SEM_CONFIG);
    let frame = draw(&mut app, 24, 8);

    // Uma grade de blocos precisa de 62 colunas para "HH:MM:SS"; num painel
    // estreito, a hora precisa aparecer como texto em vez de cortada.
    let digits: String = frame.chars().filter(|c| c.is_ascii_digit()).collect();
    assert!(
        digits.len() >= 4,
        "a hora deveria aparecer inteira, apareceu {digits:?}:\n{frame}"
    );
}

#[test]
fn the_help_still_shows_something_in_a_short_pane() {
    use crossterm::event::KeyCode;

    let mut app = app(&[], SEM_CONFIG);
    let plain = draw(&mut app, 40, 6);

    app.on_key(KeyCode::Char('?'));
    let help = draw(&mut app, 40, 6);

    assert_ne!(plain, help, "a ajuda não apareceu num painel curto");
    assert!(
        help.contains("sai") || help.contains("ajuda"),
        "a ajuda ficou vazia:\n{help}"
    );
}

#[test]
#[ignore = "imprime amostras para a documentação"]
fn print_split_samples() {
    let config = four_widgets_config("amostra-split");
    let path = config.to_str().expect("caminho utf-8");

    for (width, height, label) in [
        (170, 34, "janela larga: quatro colunas"),
        (70, 34, "terminal dividido: duas colunas, duas linhas"),
        (40, 22, "painel estreito"),
    ] {
        let mut app = app(&[], path);
        let frame = draw_until(&mut app, width, height, |frame| frame.contains("quarto"));
        println!("=== {label} ({width}x{height}) ===");
        println!("{frame}");
    }
}

#[test]
#[ignore = "imprime amostras para a documentação"]
fn print_samples_for_the_readme() {
    for font in ["bricks", "slim", "slant", "flip"] {
        let mut app = app(&["--font", font, "--no-seconds", "--no-date"], SEM_CONFIG);
        println!("=== fonte {font} ===");
        println!("{}", draw(&mut app, 92, 14));
    }

    for kind in ["matrix", "plasma", "waves", "stars"] {
        let mut app = app(
            &["--animation", kind, "--no-seconds", "--no-date"],
            SEM_CONFIG,
        );
        println!("=== animação {kind} ===");
        println!("{}", draw(&mut app, 92, 14));
    }
}
