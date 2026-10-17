//! Testes que saem para o mundo real: rede e o CLI `gh`. Ficam marcados como
//! `#[ignore]` porque dependem de internet, de autenticação e de um servidor
//! externo responder.
//!
//! Rodar com:
//!   cargo test --test live_widgets -- --ignored --nocapture

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use clap::Parser;
use my_dev_clock::app::{App, Cli};
use ratatui::{Terminal, backend::TestBackend};

fn write_config(name: &str, contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("my-dev-clock-live-{name}.toml"));
    std::fs::write(&path, contents).expect("grava a config do teste");
    path
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

/// Desenha e roda os workers do app até `done` aceitar o quadro, ou até o tempo
/// acabar. Devolve o último quadro desenhado.
fn run_until(
    config: &Path,
    width: u16,
    height: u16,
    deadline: Duration,
    done: impl Fn(&str) -> bool,
) -> String {
    let cli = Cli::try_parse_from([
        "my-dev-clock",
        "--config",
        config.to_str().expect("caminho utf-8"),
    ])
    .expect("linha de comando válida");
    let mut app = App::from_cli(cli);

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal de teste");

    let started = std::time::Instant::now();
    let mut frame = String::new();
    while started.elapsed() < deadline {
        terminal.draw(|f| app.ui(f)).expect("desenho do quadro");
        app.tick();
        frame = buffer_text(terminal.backend().buffer(), width, height);
        if done(&frame) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    frame
}

#[test]
#[ignore = "usa a rede (Open-Meteo)"]
fn the_weather_widget_shows_a_real_forecast() {
    let config = write_config(
        "weather",
        r#"
        [clock]
        [[clock.widgets]]
        kind = "weather"
        title = "Clima"
        refresh_secs = 3600
        timeout_secs = 25

        [clock.widgets.weather]
        city = "Curitiba"
        days = 3
        "#,
    );

    let frame = run_until(&config, 110, 26, Duration::from_secs(70), |frame| {
        frame.contains("°C") || frame.contains("[erro]")
    });

    println!("{frame}");

    // A Open-Meteo é um serviço de fora: quando ela não responde, o widget
    // precisa dizer isso, e a suíte não pode acusar o app por causa da rede.
    if frame.contains("[erro]") {
        println!("a rede não respondeu agora, o widget avisou: {frame}");
        return;
    }

    assert!(frame.contains("°C"), "sem temperatura no quadro:\n{frame}");
    assert!(frame.contains("hoje"), "sem linha de hoje:\n{frame}");
}

#[test]
#[ignore = "mede CPU e RAM da máquina"]
fn the_cpu_and_memory_widgets_report_real_numbers() {
    let config = write_config(
        "sysinfo",
        r#"
        [clock]
        [[clock.widgets]]
        kind = "cpu"
        title = "CPU"
        refresh_secs = 3600

        [[clock.widgets]]
        kind = "mem"
        title = "RAM"
        refresh_secs = 3600
        "#,
    );

    let frame = run_until(
        &config,
        110,
        26,
        Duration::from_secs(60),
        // A CPU faz duas leituras com um intervalo mínimo entre elas, então o
        // critério precisa exigir a saída dela, e não só a da RAM.
        |frame| frame.contains("núcleos") && frame.contains("GB"),
    );

    println!("{frame}");
    assert!(frame.contains('%'), "sem porcentagem:\n{frame}");
    assert!(frame.contains("GB"), "sem memória em GB:\n{frame}");
    assert!(
        frame.contains("núcleos"),
        "sem contagem de núcleos:\n{frame}"
    );
    assert!(!frame.contains("[erro]"), "erro no provedor:\n{frame}");
}

#[test]
#[ignore = "usa a configuração instalada na máquina"]
fn the_installed_config_renders_clock_and_widgets() {
    let Some(dir) = dirs::config_dir() else {
        return;
    };
    let path = dir.join("my-dev-clock").join("config.toml");
    if !path.exists() {
        println!("sem configuração em {}", path.display());
        return;
    }
    println!("configuração: {}", path.display());

    // Janela larga, como a de um terminal maximizado.
    let wide = run_until(&path, 200, 45, Duration::from_secs(60), |frame| {
        frame.contains("CPU") && frame.contains("GB") && frame.contains("Pendências")
    });
    println!("=== quadro 200x45 ===\n{wide}");

    // Janela estreita: menos colunas de widgets e o relógio maior.
    let narrow = run_until(&path, 90, 30, Duration::from_secs(30), |frame| {
        frame.contains("CPU")
    });
    println!("=== quadro 90x30 ===\n{narrow}");

    assert!(wide.contains("CPU"), "faltou o widget de CPU:");
    assert!(wide.contains("RAM"), "faltou o widget de RAM:");
    assert!(narrow.contains('█'), "o relógio não desenhou");
}

#[test]
#[ignore = "usa git, gh e a rede"]
fn the_workflow_widgets_answer_with_real_data() {
    let config = write_config(
        "fluxo",
        r#"
        [clock]
        max_columns = 6

        [[clock.widgets]]
        kind = "git"
        title = "Git"
        refresh_secs = 3600

        [[clock.widgets]]
        kind = "disk"
        title = "Disco"
        refresh_secs = 3600

        [[clock.widgets]]
        kind = "net"
        title = "Rede"
        refresh_secs = 3600

        [[clock.widgets]]
        kind = "gh"
        title = "Pendências"
        refresh_secs = 3600

        [[clock.widgets]]
        kind = "actions"
        title = "CI"
        refresh_secs = 3600

        [clock.widgets.gh]
        limit = 2
        repo = "cli/cli"
        "#,
    );

    let frame = run_until(&config, 200, 30, Duration::from_secs(90), |frame| {
        frame.contains("revisão") && frame.contains("recebidos") && frame.contains('%')
    });

    println!("{frame}");
    for expected in ["Git", "Disco", "Rede", "Pendências", "CI"] {
        assert!(
            frame.contains(expected),
            "faltou o widget {expected}:\n{frame}"
        );
    }
    assert!(
        !frame.contains("[erro]"),
        "algum widget devolveu erro:\n{frame}"
    );
    assert!(
        frame.contains('%'),
        "o disco não trouxe porcentagem:\n{frame}"
    );
    assert!(
        frame.contains("recebidos"),
        "a rede não trouxe totais:\n{frame}"
    );
}

#[test]
#[ignore = "usa o CLI gh autenticado"]
fn the_github_widget_groups_what_is_pending() {
    let config = write_config(
        "gh-grupos",
        r#"
        [clock]
        [[clock.widgets]]
        kind = "gh"
        title = "Pendências"
        refresh_secs = 3600
        timeout_secs = 45

        [clock.widgets.gh]
        limit = 3
        "#,
    );

    let started = std::time::Instant::now();
    let frame = run_until(&config, 190, 26, Duration::from_secs(60), |frame| {
        frame.contains("meus PRs") || frame.contains("[erro]")
    });
    let elapsed = started.elapsed();

    println!("{frame}");
    println!("tempo até o quadro: {elapsed:?}");

    assert!(!frame.contains("[erro]"), "{frame}");
    // A linha de contagens e o cabeçalho da categoria com o item embaixo.
    assert!(
        frame.contains("revisões") || frame.contains("revisão"),
        "{frame}"
    );
    assert!(frame.contains("meus PRs"), "{frame}");
}

#[test]
#[ignore = "usa git e a configuração instalada na máquina"]
fn the_commits_widget_lists_real_commits_across_projects() {
    // A pasta varrida sai da configuração instalada nesta máquina, então o teste
    // roda contra o que a pessoa realmente configurou.
    let (config, Some(path)) = my_dev_clock::config::load(None) else {
        return;
    };
    let configured = config.clock.widgets.iter().any(|widget| {
        matches!(widget.kind.as_str(), "commits" | "log") && !widget.commits.roots.is_empty()
    });
    if !configured {
        println!("a configuração desta máquina não tem widget de commits com roots");
        return;
    }
    println!("configuração: {}", path.display());

    let frame = run_until(&path, 150, 30, Duration::from_secs(90), |frame| {
        frame.contains("nenhum commit") || frame.contains(" · ") || frame.contains("[erro]")
    });

    println!("{frame}");
    assert!(!frame.contains("[erro]"), "{frame}");
    assert!(
        frame.contains(" · ") || frame.contains("nenhum commit"),
        "o widget não mostrou commit nem a mensagem de vazio:\n{frame}"
    );
}
