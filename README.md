# my-dev-clock

**Português** · [English](README_en.md)

Um relógio de terminal para Windows, com timer, cronômetro, contagem regressiva e
uma fileira de painéis que transformam o terminal num painel de status.

Escrito em Rust, nativo no Windows: não depende de `sh`, de script bash nem de
utilitário de sistema do Linux.

**Sumário**

- [Instalação](#instalação)
- [Primeiros passos](#primeiros-passos)
- [Teclas](#teclas)
- [Modos](#modos)
- [Aparência](#aparência)
- [Terminal dividido](#terminal-dividido)
- [Widgets](#widgets)
- [Receitas prontas](#receitas-prontas)
- [Configuração](#configuração)
- [Linha de comando](#linha-de-comando)
- [Diagnóstico e uso em scripts](#diagnóstico-e-uso-em-scripts)
- [Problemas comuns](#problemas-comuns)
- [Como funciona](#como-funciona)
- [Desenvolvimento](#desenvolvimento)

## Instalação

Baixe o `my-dev-clock.exe` na [página de releases](../../releases/latest) e
coloque numa pasta qualquer. É um executável único, sem instalador e sem nada
mais para instalar:

```powershell
.\my-dev-clock.exe
```

Na primeira execução o Windows mostra o aviso "O Windows protegeu o seu PC",
porque o binário não tem assinatura digital. Clique em **Mais informações** e
depois em **Executar assim mesmo**; o aviso não volta.

Para chamar o programa de qualquer pasta, ponha o `.exe` numa pasta que já esteja
no `PATH`.

### Compilar do código

Precisa de Rust estável e do toolset C++ do Visual Studio, que fornece o linker
do Windows.

```powershell
cargo build --release
.\target\release\my-dev-clock.exe
```

Se o `cargo` reclamar que não achou o linker, use o script que entra no ambiente
do Visual Studio antes de chamar o cargo:

```powershell
.\build.cmd build --release
```

Ele repassa qualquer comando do cargo, então `.\build.cmd test` e
`.\build.cmd clippy --all-targets -- -D warnings` também funcionam.

## Primeiros passos

Sem arquivo de configuração o app abre só o relógio, ocupando a tela toda. Os
widgets vêm da configuração, então o primeiro passo é criar uma:

```powershell
my-dev-clock --init
```

Isso grava um `config.toml` comentado em `%APPDATA%\my-dev-clock\`. Abra o arquivo,
descomente alguns widgets e rode de novo. Um começo pequeno:

```toml
[clock]

[[clock.widgets]]
kind = "cpu"
title = "CPU"

[[clock.widgets]]
kind = "mem"
title = "RAM"

[[clock.widgets]]
kind = "git"
title = "Git"
refresh_secs = 15
```

Com o app aberto, quatro teclas já mudam bastante coisa:

| Tecla | O que faz |
| --- | --- |
| `?` | Mostra a lista completa de teclas, dentro do próprio app |
| `Shift+T` | Próximo tema |
| `Shift+F` | Próximo estilo de dígitos |
| `Shift+A` | Próxima animação de fundo |

Cada um desses mostra por um instante, no rodapé, o que acabou de mudar. Se uma
tecla não tiver efeito visível, ela diz isso — `Shift+L`, por exemplo, avisa que o
relógio já ocupa tudo quando não há widgets na tela.

Salvar o `config.toml` **recarrega a configuração na hora**, sem reiniciar o app.
Dá para ajustar uma cor ou trocar um widget com o relógio na frente.

## Teclas

| Tecla | O que faz | Onde |
| --- | --- | --- |
| `q` | Sai | sempre |
| `espaço` | Pausa e retoma | timer e cronômetro |
| `c` / `w` / `t` | Vai para relógio / cronômetro / timer | sempre |
| `Shift+T` | Próximo tema | sempre |
| `Shift+F` | Próximo estilo de dígitos | sempre |
| `Shift+A` | Próxima animação de fundo | sempre |
| `Shift+L` | Tela cheia sim/não | modo relógio |
| `z` | Esconde ou mostra todos os widgets | modo relógio |
| `s` | Abre o campo para digitar o tamanho dos dígitos | sempre |
| `?` | Mostra a lista de teclas (e `Esc` fecha) | sempre |
| `l` | Marca uma volta | cronômetro |
| `n` | Pula para o próximo intervalo | timer |
| `+` ou `=` / `-` | Soma ou tira um minuto do intervalo atual | timer |
| `r` | Reinicia desde o primeiro intervalo | timer |
| `Home` / `End` | Vai para o topo ou o fim do widget em foco | modo relógio |
| `PageUp` / `PageDown` | Rola o widget em foco uma página | modo relógio |
| roda do mouse | Rola o widget sob o cursor | modo relógio |

No campo de tamanho, digite um número de 1 a 12 e confirme com `Enter`; o relógio
redimensiona enquanto você digita, e `Esc` cancela. Os quatro atalhos de aparência
continuam funcionando com o campo aberto.

## Modos

| Modo | O que mostra |
| --- | --- |
| `clock` | Hora atual, com data em português e fuso opcional. É o padrão. |
| `timer` | Tempo restante, com durações em sequência, repetição e comando ao terminar. |
| `stopwatch` | Tempo decorrido, com pausa e voltas. Ao sair, imprime o tempo total e as voltas. |
| `countdown` | Tempo restante até um horário ou uma data. |

```powershell
my-dev-clock clock --timezone America/Sao_Paulo
my-dev-clock timer -d 25m 5m -T Foco Pausa
my-dev-clock stopwatch --no-millis
my-dev-clock countdown -t 2026-01-01 -T 'Ano novo'
```

**Timer.** Aceita várias durações e corre em sequência. As durações vêm, nesta
ordem: `--duration`, a chave `durations` da configuração, e por fim 5 minutos.
Durante a corrida: `n` pula o intervalo, `+` e `-` somam ou tiram um minuto, `r`
volta ao começo mantendo o estado, e `espaço` pausa. Quando o tempo acaba, a tela
pisca e o comando de `execute` roda, se houver.

**Cronômetro.** `l` marca uma volta, e o rodapé mostra a volta em andamento
enquanto ela corre. Ao sair, cada volta aparece na tela, e o arquivo de `laps_csv`
recebe tudo em CSV:

```toml
[stopwatch]
title = "Benchmark"
laps_csv = "voltas.csv"
```

```
volta,volta_s,total_s
1,12.480,12.480
2,11.901,24.381
```

**Contagem regressiva.** `time` aceita `2026-01-01`, `20:00`,
`2026-12-25 20:00:00` ou um horário RFC3339. `continue_on_zero` continua contando
depois do alvo e `reverse` conta para cima em vez de para baixo.

### Aviso quando o tempo acaba

`bell = true` no `[timer]` ou no `[countdown]` toca a campainha do terminal, além
do flash na tela. Para algo mais forte, `execute` roda um comando nesse momento —
duas receitas verificadas nesta máquina:

```toml
[timer]
# Um bipe curto, sem depender de arquivo nenhum.
execute = ["powershell", "-NoProfile", "-Command", "[console]::beep(880,300)"]

# Um som de verdade, do próprio Windows.
# execute = ["powershell", "-NoProfile", "-Command", "(New-Object Media.SoundPlayer 'C:\\Windows\\Media\\Alarm01.wav').PlaySync()"]
```

## Aparência

### Estilos de dígitos

Quatro estilos, trocados com `Shift+F` ou por `--font`. As amostras abaixo são
quadros do próprio renderizador.

**bricks** — blocos maciços, o estilo original.

```
          ████████      ████████████                ████████████  ████████████
          ████████      ████████████                ████████████  ████████████
              ████              ████      ████      ████    ████          ████
              ████              ████      ████      ████    ████          ████
              ████      ████████████                ████    ████  ████████████
              ████      ████████████                ████    ████  ████████████
              ████              ████      ████      ████    ████          ████
              ████              ████      ████      ████    ████          ████
          ████████████  ████████████                ████████████  ████████████
          ████████████  ████████████                ████████████  ████████████
```

**slim** — traço fino, mais alto e mais leve.

```
                      ██    ██████                ████    ██████
                      ██    ██████                ████    ██████
                    ████          ██    ████    ██    ██        ██
                    ████          ██    ████    ██    ██        ██
                      ██          ██    ████    ██    ██        ██
                      ██          ██    ████    ██    ██        ██
                      ██      ████              ██    ██    ████
                      ██      ████              ██    ██    ████
                      ██          ██    ████    ██    ██        ██
                      ██          ██    ████    ██    ██        ██
                      ██          ██    ████    ██    ██        ██
                      ██          ██    ████    ██    ██        ██
                    ██████  ██████                ████    ██████
                    ██████  ██████                ████    ██████
```

**slant** — os mesmos blocos, inclinados como numa itálica.

```
    ████████          ████████████                        ████████████      ████████████
    ████████          ████████████                        ████████████      ████████████
      ████                  ████          ████          ████    ████              ████
      ████                  ████          ████          ████    ████              ████
    ████          ████████████                        ████    ████      ████████████
    ████          ████████████                        ████    ████      ████████████
    ████                  ████          ████          ████    ████              ████
    ████                  ████          ████          ████    ████              ████
████████████      ████████████                        ████████████      ████████████
████████████      ████████████                        ████████████      ████████████
```

**flip** — cada dígito num cartão preenchido com a cor do relógio, o dígito
desenhado escuro por cima e uma linha de dobradiça cortando o meio. O quadro
mostra só as formas: no terminal aparecem os cartões coloridos com a dobradiça.

```
  ████████          ████████████                        ████████████      ████████████
  ████████          ████████████                        ████████████      ████████████
      ████                  ████          ████          ████    ████              ████
      ████                  ████          ████          ████    ████              ████
      ████          ████████████                        ████    ████      ████████████
      ████          ████████████                        ████    ████      ████████████
      ████                  ████          ████          ████    ████              ████
      ████                  ████          ████          ████    ████              ████
  ████████████      ████████████                        ████████████      ████████████
  ████████████      ████████████                        ████████████      ████████████
```

### Animação de flip

A face `flip` liga sozinha a virada dos dígitos: quando um dígito muda, a metade
de baixo cai primeiro e a de cima acompanha, como num painel de aeroporto. Para
usar em qualquer face, ligue na mão.

```toml
[clock]
font = "flip"
flip = true      # liga mesmo sem a face flip
flip_ms = 180    # quanto tempo a virada leva; 0 desliga
```

### Animações de fundo

Desenhadas atrás dos dígitos, sempre com o estilo escurecido para não atrapalhar
a leitura. Trocadas com `Shift+A` ou por `--animation`.

**matrix** — colunas de caracteres caindo, com a cabeça mais clara que a cauda.

```
 ﾂ  ｶ       ﾓ ﾒ ﾑ     ﾓ  ﾋ ｵﾓﾂｹ    ｻ ｰ  ﾎﾑﾓ  ｹ   ﾓ    ﾏｹﾑ ﾅｶ    ﾓ  ｷﾜﾅﾅ ｹﾆ   ﾎﾎｷﾜ ﾘ ｶｷ ﾅ ｻ
 ﾘ ｼｵ       ﾅ ﾋﾏﾅ  ｻｹﾐ     ﾊ ｰﾊ ﾜ  ｰ    ｰ ﾂ ｶ    ｳ    ｼｻﾐｶｳｶ   ﾐﾜ  ｱｼﾑ  ﾑ     ﾅﾋ   ﾜｻｶ ｹ ﾊ
 ﾆ ﾋﾐ     ████████ ﾅｰﾏﾒ ████████████             ﾐ  ████████████ﾅ ████████████    ｷ  ﾋ ｰ ﾋ
 ｼ ｵ  ﾐ   ████████  ﾂﾐﾐ ████████████      ﾐ ｰ       ████████████ｼ ████████████    ﾒﾓ ｹ
 ﾅ ﾂ  ﾑ  ﾏ  ﾎ ████ ﾎｷｰｻ ｶ   ﾂ   ████  ｱ   ████    ﾅ ████ｷﾑ  ████ｶ  ｷｴ ｳｶ  ████    ﾘﾎ ｱ
   ﾑ  ﾋ    ｶｷ ████ ﾏｷﾏﾐ ﾓ   ﾂ   ████      ████ ﾓ  ﾓｷ████ﾑﾘ  ████ｶ ﾅﾜﾓﾊ    ████     ｱ ﾓ
         ﾅ ﾒﾊ ████ ﾜｰﾆﾅ ████████████ｰ ﾜ   ﾓ  ﾊ ｼ  ﾅ ████ ﾆ  ████  ████████████    ﾎｰ
      ﾐ ﾎﾒ ﾂｼ ████ ﾃﾐ ｷ ████████████ﾜ ﾎ     ﾂﾊ     ﾐ████ ｻ  ████  ████████████    ﾎｶ
     ﾘﾑ ｱｹﾋｷｴ ████ ﾒﾐ   ﾘ      ﾆ████  ﾘ   ████ ﾐ  ｻﾜ████ ﾆ  ████  ﾏ       ████    ﾎﾏ
     ﾂｼ  ｰｱｴ  ████ ﾂﾒ   ﾑ      ｻ████ｶ ﾎ   ████ ﾎ  ﾘﾎ████    ████  ｹ    ﾆ  ████     ﾐ
  ﾂ  ｹﾃ  ｹ████████████  ████████████ｷ ｳ       ﾐﾂ  ﾒﾓ████████████  ████████████   ｹ
  ﾎ  ﾏ   ﾊ████████████  ████████████  ｳ       ﾎｹﾐ ﾐｻ████████████  ████████████   ｻ
ﾐ ﾐ     ﾋﾊﾓ      ﾅﾏ              ｷﾎ ﾐ ﾂ       ﾎ    ﾒ ｵ        ﾏ           ｼ ｴ    ﾒ    ｰ
  ﾏ  ｷ  ﾆ    ｻ    ﾘ               ﾘ ﾊ         ｱ ｶ  ﾃ ﾋ       ﾒ            ｼｴﾘ         ｶ
```

**plasma** — campos senoidais somados, mapeados numa rampa de luminosidade.

```
+++*****#####################******+++++=======------------------------===========++++++++
+*****#######%%%%%%%%%%#######******+++++======----------------------==========+++++++++++
****######████████%%%%%%████████████++++++======----████████████--████████████+++++++++***
**#####%%%████████%%%%%%████████████*+++++======----████████████--████████████++++********
#####%%%%%%%%%████%%%%%%%%%#####████*+++++████===---████----████========++████+***********
###%%%%%%%%@@@████@@%%%%%%%%####████*+++++████===---████----████======++++████*******#####
##%%%%%%%@@@@@████@@@%%%████████████*+++++=======---████----████==████████████****########
#%%%%%%%@@@@@@████@@@%%%████████████*+++++======----████----████==████████████**##########
%%%%%%%@@@@@@@████@@@%%%%%%#####████+++++=████==----████----████====+++++*████*#########%%
%%%%%%%@@@@@@@████@@%%%%%%#####*████++++==████=-----████----████===+++++**████#######%%%%%
%%%%%%%@@@████████████%%████████████+++======-------████████████==████████████######%%%%%%
%%%%%%%%%@████████████%%████████████++======--------████████████==████████████#####%%%%%%%
%%%%%%%%%%%%%%%%%%%%%%#####*****+++++=====-------------------======++++*****#######%%%%%%%
##%%%%%%%%%%%%%%%%%######*****+++++=====---------------------======+++++****#######%%%%%%%
```

**waves** — anéis concêntricos se espalhando do centro.

```
%@@@%#*+=:.    .:-=*#%%@@@%%#*+=-:..                 ..:-=+*#%%@@@%%#*=-:.    .:=+*#%@@@%#
@@@@%#+=-:.   .:-=*#%@@@@%#*+=-:..     ....:::....     ..:-=+*#%@@@@%#*=-:.   .:-=+#%@@@@%
@@@%#*+-:.████████#%@@@@████████████..:--=======--:.████████████%@████████████ .:-+*#%@@@%
@@@%#+=-..████████#%@@@%████████████:-=+**#####**+=-████████████#%████████████ ..-=+#%@@@%
@@%#*+-:.   .:████%@@@%#*+=:.   ████=+*#%%████@%%#*+████    ████*#%@@@%#*=████  .:-+*#%@@@
@@%#*=-:.   .:████%@@@%#*=-:.   ████*#%%@@████@@@%%#████.   ████*#%@@@%#*+████  .:-=*#%@@@
@@%#*=-:.   .:████%@@@%#████████████#%@@@@%###%@@@@%████..  ████+#████████████  .:-=*#%@@@
@@%#*=-:.   .:████@@@%%*████████████#%@@@%#*+*#%@@@%████:.  ████+*████████████  .:-=*#%@@@
@@%#*=-:.   .:████%@@@%#+=:.   .████#%@@@@████%@@@@%████..  ████+#%@@@%%*+████  .:-=*#%@@@
@@%#*=-:.   .:████%@@@%#*=-:.   ████*#%%@@████@@@%%#████.   ████*#%@@@%#*+████  .:-=*#%@@@
@@%#*+-:. ████████████%#████████████=+*#%%@@@@@%%#*+████████████*#████████████  .:-+*#%@@@
@@@%#+=-..████████████@%████████████:-=+**#####**+=-████████████#%████████████ ..-=+#%@@@%
@@@%#*+-:.   .:-=+#%@@@@%#*+-:..    ..:--=======--:..    ..:-+*#%@@@@%#+=-:.   .:-+*#%@@@%
@@@@%#+=-:.   .:-=*#%@@@@%#*+=-:..     ....:::....     ..:-=+*#%@@@@%#*=-:.   .:-=+#%@@@@%
```

**stars** — um campo esparso de estrelas piscando no lugar.

```
-   -                                       -  - =
            -                      =                                                    -
          ████████      ████████████                ████████████  ████████████    =
          ████████      ████████████                ████████████  ████████████        -
    -         ████              ████      ████ =    ████    ████  -     - ████      -
              ████              ████      ████      ████    ████          ████
              ████      ████████████                ████    ████  ████████████
              ████      ████████████        - -     ████    ████  ████████████
              ████              ████      ████      ████    ████          ████
              ████              ████      ████      ████    ████          ████
          ████████████  ████████████                ████████████  ████████████
  =       ████████████  ████████████                ████████████  ████████████
-                                                   -
     -                                   =                    -
```

O tempo avança em passos inteiros de quadro (`animation_fps`, 12 por padrão), o
que dá o movimento picotado de terminal e limita o custo de desenho. A mesma
semente e o mesmo instante sempre desenham o mesmo quadro, e é isso que permite
testar a animação em vez de só olhar para ela.

### Tela cheia

Com widgets na tela, o relógio fica na metade de cima. Com `fill`, ele toma tudo
o que a linha de widgets não precisa.

```toml
[clock]
fill = true
```

Alterna em execução com `Shift+L`. Ele só muda algo quando há widgets na tela;
sem widgets (nenhum configurado, ou escondidos com `z`) o relógio já ocupa tudo,
e a confirmação diz isso.

### Temas

| Tema | Cara |
| --- | --- |
| `default` | Usa a cor pedida em `--color`, ou verde. |
| `matrix` | Verde fósforo. |
| `neon` | Magenta e ciano, alto contraste. |
| `system` | As cores do próprio terminal, sem impor paleta. |
| `fire` | Laranja e vermelho. |
| `ice` | Azuis claros. |

A lista do ciclo vem de `[clock] themes`, e `--theme` escolhe com qual o app
abre. O tema também define a cor da animação de fundo (sempre escurecida) e as
cores dos widgets.

Uma paleta própria entra como um tema chamado `custom`:

```toml
[clock]
themes = ["custom", "matrix", "neon"]

[theme.custom]
clock = "#00ff88"       # dígitos
header = "#cccccc"      # linha de cima e confirmações
widget = "#88aaff"      # corpo dos widgets
title = "#ffffff"       # título dos widgets
marker = "#ffcc00"      # bolinha na frente do título
accent = "#ff00aa"      # barras e destaques
animation = "#004422"   # animação de fundo, sempre escurecida
```

Cada cor aceita nome (`green`, `lightyellow`, …) ou `#RRGGBB`, e o que faltar vem
do tema padrão.

## Terminal dividido

Dividir a janela, ou redimensioná-la, não pode esconder nada. Quando a largura
muda, o app se rearranja sozinho:

- Menos largura significa **menos colunas e mais linhas**. Com quatro widgets
  numa janela larga são quatro colunas; num terminal pela metade viram duas
  colunas e duas linhas, e os quatro continuam na tela.
- Cada linha de widgets tem a altura do widget mais alto dela, então o espaço não
  fica esburacado.
- Se nem assim tudo couber, aparece uma faixa `+N painel(ns) sem espaço` no
  rodapé dos widgets, em vídeo invertido, em vez de sumir com eles em silêncio.
- Num painel estreito demais para os dígitos de bloco — menos de 62 colunas para
  `HH:MM:SS` — a hora passa a ser texto comum, centralizado, e continua legível
  em vez de sair cortada pela metade.
- Quando o tamanho muda, a área é limpa antes do próximo quadro, então não sobra
  pedaço do layout anterior.

Só os widgets que estão na tela rodam. Um painel que ficou sem espaço não consome
processo e volta a rodar quando o lugar aparece.

## Widgets

Declarados em `[[clock.widgets]]`, aparecem embaixo do relógio em 2, 4 ou 6
colunas, conforme a largura (limitado por `max_columns`).

| `kind` | O que mostra | O que precisa |
| --- | --- | --- |
| `cpu` | Uso do processador, média dos núcleos, com barra. | nada |
| `mem` | Memória RAM usada e total. | nada |
| `disk` | Espaço do disco, com barra. | nada (aceita `mount`) |
| `net` | Tráfego desde o boot, mais a taxa desde a leitura anterior. | nada (aceita `interface`) |
| `git` | Branch, sujeira, à frente e atrás do remoto, stash. | estar num repositório, ou `path` |
| `commits` | Os seus últimos commits, em todos os projetos varridos. | `roots` ou `paths` |
| `http` | Status e um valor tirado do JSON de um endereço. | `url` |
| `weather` | Previsão de agora e dos próximos dias. | `city` ou `lat`/`lon` |
| `gh` | O que está pendente para você no GitHub. | `gh auth login` |
| `actions` | As últimas execuções de CI. | `gh auth login` |
| `command` | Qualquer comando que imprima algo e termine. | `command` |

Opções de todo widget: `title` (título acima do conteúdo; `""` esconde a linha do
título), `refresh_secs` (de quanto em quanto tempo roda, padrão 900) e
`timeout_secs` (limite de tempo, padrão 30).

### Sistema: cpu, mem, disk, net

Nativos, sem script e sem configuração. O disco escolhido é o que tem menos
espaço livre, a menos que você aponte um:

```toml
[[clock.widgets]]
kind = "disk"
title = "Disco C:"
refresh_secs = 60
[clock.widgets.disk]
mount = "C:"

[[clock.widgets]]
kind = "net"
title = "Rede"
refresh_secs = 30
[clock.widgets.net]
interface = "Ethernet"
```

Sem `interface`, a rede soma todas as interfaces menos a loopback.

### Repositório: git e commits

`git` olha o repositório do diretório atual (ou o de `path`) e mostra tudo numa
chamada só:

```toml
[[clock.widgets]]
kind = "git"
title = "Git"
refresh_secs = 15
[clock.widgets.git]
path = "C:/dev/projeto"
```

```
main * ↑2 ↓1 stash 3
4 arquivos pendentes
```

`commits` olha para fora do repositório aberto: varre as pastas configuradas,
encontra os repositórios e mostra os seus commits mais recentes em todos eles.

```toml
[[clock.widgets]]
kind = "commits"
title = "Meus commits"
refresh_secs = 300
[clock.widgets.commits]
roots = ["C:/dev"]                 # pastas onde procurar repositórios
paths = ["C:/avulso/repo"]         # ou repositórios avulsos
limit = 5
depth = 2
author = "eu@exemplo.com"
```

```
loja-virtual · 23 h · Merge pull request #37 from feature/carrinho
api-pagamentos · 2 d · feat: notificacao de pagamento aprovado
```

Cada linha é `projeto · quando · assunto`. O projeto é o nome da pasta do
repositório, e o "quando" usa a data de autor do commit. `roots` são pastas
varridas até `depth` níveis; `paths` são repositórios avulsos. O `author` aceita
nome ou email e, sem ele, o widget pergunta ao git o `user.email` configurado; a
comparação ignora maiúsculas, porque o nome guardado no commit raramente bate com
o da configuração. A varredura das pastas é guardada por cinco minutos.

### GitHub: gh e actions

`gh` agrupa o que está pendente por categoria, com uma linha de contagem no topo,
e cada item traz o que ajuda a decidir se vale abrir: o projeto, há quanto tempo
está parado, quantos comentários tem e o estado dele.

```toml
[[clock.widgets]]
kind = "gh"
title = "Pendências"
refresh_secs = 600
[clock.widgets.gh]
limit = 3
```

```
2 revisões · 1 meu PR · 0 issues
meus PRs
#1 Ajustar o calculo do frete para o Nor… · minha-org/loja-virtual · 1 mês · 3 comentários
```

- **revisões**: PRs esperando a sua revisão. Mostram quem escreveu, porque não é
  você.
- **meus PRs**: os seus, abertos. Trazem `rascunho`, `aprovado`,
  `mudanças pedidas` ou `aguardando revisão` — dá para ver o que está travado por
  quem.
- **issues**: as atribuídas a você.

Uma categoria sem itens não ganha cabeçalho. Tudo vem de **uma** chamada GraphQL.

`actions` mostra as últimas execuções de CI, marcadas com `✓`, `✗`, `…` (em
andamento) ou `×` (cancelada):

```toml
[[clock.widgets]]
kind = "actions"
title = "CI"
refresh_secs = 120
[clock.widgets.gh]
repo = "owner/projeto"    # sem isto, o repositório do diretório atual
```

Ambos precisam do CLI `gh` autenticado (`gh auth login`).

### Endereços: http

Para vigiar um dev server, com o valor tirado do JSON:

```toml
[[clock.widgets]]
kind = "http"
title = "API"
refresh_secs = 30
timeout_secs = 5
[clock.widgets.http]
url = "http://localhost:3000/health"
path = ".status"          # caminho com pontos, aceita índice: .items.0.name
expect = "ok"             # opcional: o widget diz se o valor bate
```

```
HTTP 200 · 12 ms
ok
esperado: ok
```

Uma resposta `4xx` ou `5xx` **não** é tratada como erro do widget: ela é a
informação que um health check existe para dar.

### Clima

```toml
[[clock.widgets]]
kind = "weather"
title = "Clima"
refresh_secs = 900
[clock.widgets.weather]
city = "Curitiba"   # ou lat / lon
units = "metric"    # metric (°C, km/h) ou imperial (°F, mph)
days = 2            # linhas de previsão, de 1 a 7
```

```
22.4°C · parcialmente nublado
sensação 21.8° · umidade 78% · vento 11 km/h
hoje    17.1° / 25.3°
amanhã  16.4° / 26.0°
```

Usa a [Open-Meteo](https://open-meteo.com), que não pede chave de API.

### Qualquer comando

```toml
[[clock.widgets]]
kind = "command"
title = "Bateria"
command = "powercfg /batteryreport /output -"
refresh_secs = 300
timeout_secs = 20
```

Uma string só passa pelo shell (`cmd /C`); uma lista executa o programa direto,
sem shell, que é o caminho certo quando o comando tem aspas:

```toml
command = ["powershell", "-NoProfile", "-Command", "$v = Get-Volume -DriveLetter C; '{0:N1} GB livres' -f ($v.SizeRemaining/1GB)"]
```

Todo comando recebe o tema atual na variável `MY_DEV_CLOCK_THEME`. Comando que passa
do tempo limite é encerrado **junto com tudo que ele iniciou**, via Job Object do
Windows, então fechar o app não deixa processo pendurado.

## Receitas prontas

**Pomodoro**, com aviso sonoro e repetição:

```toml
[default]
mode = "timer"
color = "lightred"

[timer]
durations = ["25m", "5m"]
titles = ["Foco", "Pausa"]
bell = true
repeat = true

[[clock.widgets]]
kind = "gh"
title = "Pendências"
```

**Painel de dev**, para deixar num monitor de lado:

```toml
[clock]
font = "flip"
max_columns = 5

[[clock.widgets]]
kind = "git"
title = "Repo"
refresh_secs = 15

[[clock.widgets]]
kind = "commits"
title = "Meus commits"
refresh_secs = 300
[clock.widgets.commits]
roots = ["~/dev"]   # troque pela pasta onde ficam os seus repositórios

[[clock.widgets]]
kind = "gh"
title = "Pendências"
refresh_secs = 600

[[clock.widgets]]
kind = "actions"
title = "CI"
refresh_secs = 120

[[clock.widgets]]
kind = "cpu"
title = "CPU"
refresh_secs = 3
```

**Monitor de servidor**, com o health check na frente:

```toml
[clock]
max_columns = 4

[[clock.widgets]]
kind = "http"
title = "API"
refresh_secs = 30
[clock.widgets.http]
url = "http://localhost:3000/health"   # aponte para o seu servidor
path = ".status"
expect = "ok"

[[clock.widgets]]
kind = "cpu"
title = "CPU"
refresh_secs = 3

[[clock.widgets]]
kind = "mem"
title = "RAM"
refresh_secs = 5

[[clock.widgets]]
kind = "disk"
title = "Disco"
refresh_secs = 60

[[clock.widgets]]
kind = "net"
title = "Rede"
refresh_secs = 10
```

**Relógio de parede**, só o relógio, grande e com fundo animado:

```toml
[default]
mode = "clock"
size = 6
show_seconds = false

[clock]
fill = true
animation = "matrix"
animation_fps = 10
```

Esse último combine com `--theme matrix` na linha de comando, ou escreva
`themes = ["matrix"]`.

## Configuração

O app lê o primeiro arquivo que existir, nesta ordem:

1. o caminho passado em `--config <arquivo>`
2. `%XDG_CONFIG_HOME%\my-dev-clock\config.toml`, quando a variável aponta para um caminho absoluto
3. `%APPDATA%\my-dev-clock\config.toml`
4. `%USERPROFILE%\.config\my-dev-clock\config.toml`

A precedência das opções de exibição é: **flag da linha de comando**, depois a
seção do modo, depois `[default]`, e por fim o padrão embutido daquele modo.

Padrões embutidos: o relógio mostra data e segundos; timer e cronômetro mostram
décimos; a contagem regressiva mostra segundos inteiros.

### `[default]`

| Chave | Tipo | Padrão | Efeito |
| --- | --- | --- | --- |
| `mode` | texto | `"clock"` | Modo inicial: `clock`, `timer`, `stopwatch`, `countdown`. Nome desconhecido cai no relógio. |
| `color` | texto | `"green"` | Cor base dos dígitos: nome (`green`, `cyan`, `lightred`, …) ou `"#RRGGBB"`. |
| `size` | número | ausente | Escala dos dígitos, 1 a 12. Ausente significa ajustar à janela. |
| `title` | texto | ausente | Título acima dos dígitos. |
| `show_date` | booleano | por modo | Mostra a linha da data no relógio. |
| `show_seconds` | booleano | por modo | Mostra os segundos; desligado, todo modo fica `H:MM`. |
| `show_millis` | booleano | por modo | Mostra os décimos de segundo. |
| `start_paused` | booleano | `false` | Começa pausado (timer e cronômetro). |

### `[clock]`

| Chave | Tipo | Padrão | Efeito |
| --- | --- | --- | --- |
| `font` | texto | `"bricks"` | Estilo dos dígitos: `bricks`, `slim`, `slant`, `flip`. |
| `flip` | booleano | segue a fonte | Liga a virada dos dígitos. Ausente: ligada na face `flip`, desligada nas outras. |
| `flip_ms` | número | `180` | Duração da virada, em milissegundos. `0` desliga. |
| `animation` | texto | `"none"` | Animação de fundo: `none`, `matrix`, `plasma`, `waves`, `stars`. |
| `animation_fps` | número | `12` | Quadros por segundo da animação, de 1 a 60. |
| `fill` | booleano | `false` | O relógio ocupa a área toda em vez de só a metade de cima. |
| `format` | texto | derivado | Formato da hora no padrão do chrono, por exemplo `"%H:%M:%S"`. `"%I:%M %p"` dá 12 horas com AM/PM. Um formato inválido é recusado com aviso. |
| `date_format` | texto | forma longa | Formato da linha de data no padrão do chrono, por exemplo `"%d/%m/%Y"`. |
| `world` | lista | vazia | Fusos extras mostrados ao lado da data, como nomes IANA: `["America/New_York", "Europe/Lisbon"]`. |
| `timezone` | texto | ausente | Fuso IANA, por exemplo `"America/Sao_Paulo"`. |
| `themes` | lista | a lista embutida | Nomes que o `Shift+T` percorre. |
| `max_columns` | número | sem limite | Teto de colunas de widgets (o terminal também decide). |
| `widgets` | lista de tabelas | vazio | Os painéis. |

O `[clock]` aceita ainda as chaves de exibição (`title`, `show_date`,
`show_seconds`, `show_millis`, `start_paused`).

### `[timer]`

| Chave | Tipo | Padrão | Efeito |
| --- | --- | --- | --- |
| `durations` | lista | vazio → 5m | Durações em sequência, ex.: `["25m", "5m"]`. |
| `titles` | lista | vazio | Um título por duração; ganha do `title`. |
| `repeat` | booleano | `false` | Reinicia quando termina. |
| `auto_quit` | booleano | `false` | Encerra o app quando o tempo acaba. |
| `execute` | texto ou lista | vazio | Comando a executar quando o tempo acaba. |
| `bell` | booleano | `false` | Toca a campainha do terminal quando o tempo acaba. |

### `[stopwatch]` e `[countdown]`

`[stopwatch]` aceita as chaves de exibição mais `laps_csv` (caminho de um arquivo
onde as voltas são gravadas em CSV ao sair). `[countdown]` aceita as chaves de
exibição mais `time` (texto: `2026-01-01`, `20:00`, `2026-12-25 20:00:00` ou
RFC3339), `continue_on_zero`, `reverse` e `bell`.

### `[theme.custom]`

Uma paleta própria, usada quando o tema ativo se chama `custom`. As chaves são
`clock`, `header`, `widget`, `title`, `marker`, `accent` e `animation`.

### `[[clock.widgets]]`

| Chave | Tipo | Padrão | Efeito |
| --- | --- | --- | --- |
| `kind` | texto | obrigatório | Tipo do painel, da tabela de [Widgets](#widgets). |
| `title` | texto | nome do comando | Título do painel; `""` esconde a linha do título. |
| `command` | texto ou lista | — | Só para `kind = "command"`. |
| `refresh_secs` | número | `900` | Intervalo entre execuções. |
| `timeout_secs` | número | `30` | Limite de tempo do comando. |

Sub-tabelas: `[clock.widgets.git]` com `path`; `[clock.widgets.commits]` com
`roots`, `paths`, `author`, `limit` e `depth`; `[clock.widgets.gh]` com `limit` e
`repo`; `[clock.widgets.http]` com `url`, `path` e `expect`;
`[clock.widgets.weather]` com `city`, `lat`, `lon`, `units` e `days`;
`[clock.widgets.disk]` com `mount`; `[clock.widgets.net]` com `interface`.

## Linha de comando

Opções de exibição, que valem em qualquer modo e antes ou depois do nome do modo:

| Opção | Efeito |
| --- | --- |
| `-T, --title <TEXTO>...` | Título acima dos dígitos. No timer, um por duração. |
| `--seconds` / `-S, --no-seconds` | Mostra ou esconde os segundos. |
| `-m, --millis` / `-M, --no-millis` | Mostra ou esconde os décimos. |
| `--date` / `-D, --no-date` | Mostra ou esconde a linha da data. |
| `-P, --paused` | Começa pausado. |
| `-c, --color <COR>` | Cor base, nome ou `#RRGGBB`. |
| `-s, --size <1-12>` | Escala dos dígitos. |
| `--theme <NOME>` | Tema inicial. |
| `--font <NOME>` | Estilo dos dígitos. |
| `--animation <NOME>` | Animação de fundo. |
| `--fill` / `--no-fill` | Relógio na tela toda ou na metade de cima. |
| `--print [FORMATO]` | Imprime uma linha usando os marcadores e sai. |
| `--config <ARQUIVO>` | Arquivo de configuração específico. |
| `--init` | Grava a configuração de exemplo no local padrão e sai. |
| `--check` | Confere a configuração e roda cada widget uma vez. |

Opções de cada modo:

| Modo | Opções |
| --- | --- |
| `clock` | `--timezone <FUSO>` |
| `timer` | `-d, --duration <DURAÇÃO>...`, `-r, --repeat`, `-Q, --auto-quit`, `-e, --execute <COMANDO>...` |
| `stopwatch` | — |
| `countdown` | `-t, --time <ALVO>`, `-C, --continue`, `--reverse` |

## Diagnóstico e uso em scripts

**`--check`** mostra de onde veio a configuração, qual é a aparência resolvida e
roda cada widget uma vez, imprimindo o que ele devolveu. É o primeiro lugar para
olhar quando um painel não aparece ou aparece vazio:

```powershell
my-dev-clock --check
```

```
configuração: C:\Users\exemplo\AppData\Roaming\my-dev-clock\config.toml
tema inicial: default
fonte: bricks · animação: none (12 fps) · tela cheia: não · flip: não
formato: (padrão) · data: (padrão) · fusos extras: nenhum
tema custom: não · campainha: não · voltas em CSV: (não)

[1/7] cpu · CPU · 0.5s
██░░░░░░░░░░
21.6%
8 núcleos
```

**`--print`** imprime uma linha e sai, sem abrir a tela alternativa. É o modo para
tmux, prompt ou qualquer barra de status:

```powershell
my-dev-clock --print "{time} · {git} · cpu {cpu} · ram {mem} · {host}"
```

```
14:09:15 · main * · cpu 15% · ram 68% · DESKTOP-8KQ2M4
```

| Marcador | Vira |
| --- | --- |
| `{time}` | A hora, com o formato de `[clock] format` |
| `{date}` | A linha de data, com os fusos extras |
| `{git}` | Branch e estado do diretório atual, como `main *` |
| `{cpu}` | Uso do processador, em porcentagem |
| `{mem}` | Uso da memória, em porcentagem |
| `{host}` | Nome da máquina |

Um marcador desconhecido fica como está, para um erro de digitação aparecer em vez
de virar uma linha vazia.

## Problemas comuns

**Os atalhos parecem não funcionar.** Aperte `?` para ver a lista e repare na
linha de confirmação no rodapé: todo atalho de aparência diz o que mudou. Se ela
não aparecer, o atalho não chegou — e a causa mais comum é o campo de tamanho
estar aberto (ele aparece na última linha). `Esc` fecha.

**Um widget não aparece.** Ele pode estar sem espaço: veja a faixa
`+N painel(ns) sem espaço`. Ou o `kind` está errado — o `--check` mostra o erro
exato de cada um. Widgets que não cabem não rodam, então não há custo escondido.

**O app não compila: `linker link.exe not found`.** O `cargo` não achou o toolset
C++. Use `.\build.cmd`, que entra no ambiente do Visual Studio antes de chamar o
cargo. Se nem isso resolver, instale a carga "Desenvolvimento para desktop com
C++" no Visual Studio.

**`failed to remove ... my-dev-clock.exe`.** O binário está em uso: feche o app e
compile de novo.

**A face `flip` parece igual à `bricks`.** Os cartões dela são cor de fundo. Num
console que não pinta fundo (o `conhost` antigo), ela fica igual à `bricks` — a
linha de confirmação dirá `fonte: flip` enquanto o desenho não muda. No Windows
Terminal aparece como esperado.

**O clima não carrega.** Ele depende da Open-Meteo, um serviço de fora. Quando a
rede falha, o widget diz o que aconteceu — e a mensagem diferencia "falta a
cidade na configuração" de "a consulta não respondeu".

**Um toast nativo do Windows não funciona.** A projeção WinRT não está disponível
nesta instalação: tanto o PowerShell 7 quanto o 5.1 respondem `Unable to find
type [Windows.UI.Notifications.ToastNotificationManager]`. Para um aviso sonoro
use as receitas de [`execute`](#aviso-quando-o-tempo-acaba); para um toast de
verdade, instale um módulo que faça isso (BurntToast, por exemplo) e chame-o pelo
`execute`.

**Os testes ao vivo não rodam.** Eles são marcados com `#[ignore]` porque usam
rede, git e o CLI `gh`:

```powershell
.\build.cmd test -- --ignored
```

## Como funciona

- Cada widget roda numa thread própria e devolve o texto por um canal; o laço de
  desenho nunca espera por um comando.
- As animações usam uma rampa de luminosidade e corrigem a proporção da célula (a
  célula do terminal é cerca de duas vezes mais alta que larga). Sem essa
  correção, todo desenho redondo sai esticado na vertical.
- O tempo da animação é injetado e a aleatoriedade vem de uma semente fixa, então
  um quadro é reproduzível — é isso que permite testar animação.
- Comandos de widget rodam dentro de um Job Object do Windows com `kill-on-close`:
  o processo e tudo que ele iniciou morrem junto.
- O widget do GitHub faz uma única chamada GraphQL com as três buscas (pedidos de
  revisão, PRs seus, issues atribuídas). Antes eram três chamadas REST em
  sequência, e a diferença aparece no relógio: cerca de 1,2 s contra 4,4 s.
- Salvar o `config.toml` é notado pelo carimbo de tempo do arquivo. A aparência é
  reaplicada na hora; um timer ou cronômetro em andamento continua contando.
- Um formato de data ou hora inválido no arquivo é recusado com aviso, em vez de
  derrubar o app ao desenhar.

## Desenvolvimento

```powershell
.\build.cmd test                                  # testes
.\build.cmd clippy --all-targets -- -D warnings    # lint, sem avisos
.\build.cmd fmt --all --check                      # formatação
.\build.cmd test -- --ignored                      # testes que usam rede, git e gh
```

O CI roda o mesmo ciclo em `windows-latest`, e uma tag `v*` publica na release o
executável, um pacote com o exemplo de configuração e a soma de verificação; a tag
precisa combinar com a versão do `Cargo.toml`, ou o workflow falha. Os testes de
renderização desenham quadros de verdade
num backend de teste e conferem o conteúdo, o que é o que permite afirmar que uma
fonte ou uma animação funciona sem depender de alguém olhar.

Licença MIT, em [LICENSE](LICENSE).
